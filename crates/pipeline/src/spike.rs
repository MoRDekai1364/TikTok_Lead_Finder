use std::collections::HashSet;
use std::time::{Duration, Instant};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use leadfinder_core::{
    DiscoveryQuery, PolitenessGuard, PolitenessPolicy, ProviderErrorKind, RuntimePaths, Seed,
    VideoSource,
};
use leadfinder_providers::{
    has_bulgarian_or_cyrillic_signal, DirectHttpDiscoverySource, HeadlessBrowserDiscoverySource,
    YtDlpClient, YtDlpDiscoverySource,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproachMetrics {
    pub approach_name: String,
    pub total_attempts: usize,
    pub successful_attempts: usize,
    pub blocked_attempts: usize,
    pub success_rate_pct: f64,
    pub block_rate_pct: f64,
    pub p50_latency_ms: u64,
    pub p95_latency_ms: u64,
    pub unique_videos: usize,
    pub unique_bulgarian_videos: usize,
    pub sample_urls: Vec<String>,
    pub sample_errors: Vec<String>,
    pub meets_go_criteria: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverySpikeReport {
    pub seeds_tested: Vec<String>,
    pub runs_per_seed: usize,
    pub approaches: Vec<ApproachMetrics>,
    pub overall_go: bool,
    pub winning_approach: Option<String>,
    pub recommendation: String,
}

pub async fn run_discovery_spike(
    paths: &RuntimePaths,
    seeds: &[Seed],
    runs_per_seed: usize,
    max_per_seed: usize,
) -> Result<DiscoverySpikeReport> {
    let mut hashtags: Vec<Seed> = seeds.iter().filter(|s| s.kind == "hashtag").take(5).cloned().collect();
    let mut keywords: Vec<Seed> = seeds.iter().filter(|s| s.kind == "keyword").take(5).cloned().collect();
    hashtags.append(&mut keywords);
    if hashtags.len() < 10 {
        for s in seeds {
            if hashtags.len() >= 10 {
                break;
            }
            if !hashtags.iter().any(|x| x.id == s.id) {
                hashtags.push(s.clone());
            }
        }
    }
    let selected_seeds = hashtags;
    let seed_labels: Vec<String> = selected_seeds
        .iter()
        .map(|s| format!("{}:{}", s.kind, s.value))
        .collect();

    let make_guard = |sub: &str| {
        let policy = PolitenessPolicy {
            requests_per_minute: 30,
            breaker_failures: 100,
            breaker_cooldown: Duration::from_secs(60),
            cache_ttl: Duration::from_secs(0),
        };
        PolitenessGuard::new(policy, paths.cache_dir.join(sub))
    };

    let ytdlp_guard = make_guard("spike_ytdlp");
    let ytdlp_client = YtDlpClient::new(paths, Duration::from_secs(25), ytdlp_guard);
    let approach_ytdlp = YtDlpDiscoverySource::new(ytdlp_client);

    let approach_headless = HeadlessBrowserDiscoverySource::new(
        make_guard("spike_headless"),
        Duration::from_secs(20),
    );
    let approach_http = DirectHttpDiscoverySource::new(make_guard("spike_http"));

    let mut approaches_metrics = Vec::new();
    for source in [
        &approach_ytdlp as &dyn VideoSource,
        &approach_headless as &dyn VideoSource,
        &approach_http as &dyn VideoSource,
    ] {
        let m = evaluate_source(source, &selected_seeds, runs_per_seed, max_per_seed).await;
        approaches_metrics.push(m);
    }

    let winner = approaches_metrics
        .iter()
        .filter(|m| m.meets_go_criteria)
        .max_by_key(|m| m.unique_bulgarian_videos)
        .map(|m| m.approach_name.clone());

    let overall_go = winner.is_some();
    let recommendation = if let Some(ref w) = winner {
        format!(
            "GO for `{w}`: returned >= 30 unique Bulgarian videos across 10 seeds with block rate < 20%. Keep circuit breaker and 12 req/min politeness guard active, with paste-a-link and partner import always enabled."
        )
    } else {
        "NO-GO for automated public-page scraping: no unauthenticated approach achieved >= 30 Bulgarian videos across 10 seeds with < 20% block rate under hard safety boundaries (no login, no CAPTCHA solving, no fingerprint spoofing). Proceed with Paste-a-Link inbox and Partner-Export CSV/text import as primary discovery sources.".to_string()
    };

    Ok(DiscoverySpikeReport {
        seeds_tested: seed_labels,
        runs_per_seed,
        approaches: approaches_metrics,
        overall_go,
        winning_approach: winner,
        recommendation,
    })
}

async fn evaluate_source(
    source: &dyn VideoSource,
    seeds: &[Seed],
    runs_per_seed: usize,
    max_per_seed: usize,
) -> ApproachMetrics {
    let mut latencies_ms = Vec::new();
    let mut total_attempts = 0usize;
    let mut successful_attempts = 0usize;
    let mut blocked_attempts = 0usize;
    let mut unique_ids = HashSet::new();
    let mut unique_bg_ids = HashSet::new();
    let mut sample_urls = Vec::new();
    let mut sample_errors = Vec::new();

    for _run in 0..runs_per_seed {
        for seed in seeds {
            total_attempts += 1;
            let query = match seed.kind.as_str() {
                "hashtag" => DiscoveryQuery::Hashtag {
                    tag: seed.value.clone(),
                    max_results: max_per_seed,
                },
                "creator" => DiscoveryQuery::Creator {
                    handle: seed.value.clone(),
                    max_results: max_per_seed,
                    expansion_depth: 0,
                },
                _ => DiscoveryQuery::Keyword {
                    keyword: seed.value.clone(),
                    max_results: max_per_seed,
                },
            };

            let start = Instant::now();
            let res = source.discover(&query).await;
            let elapsed_ms = start.elapsed().as_millis() as u64;
            latencies_ms.push(elapsed_ms);

            match res {
                Ok(items) if !items.is_empty() => {
                    successful_attempts += 1;
                    for item in items {
                        if sample_urls.len() < 25 && !sample_urls.contains(&item.url) {
                            sample_urls.push(item.url.clone());
                        }
                        if has_bulgarian_or_cyrillic_signal(&item) {
                            unique_bg_ids.insert(item.id.clone());
                        }
                        unique_ids.insert(item.id);
                    }
                }
                Ok(_) => {
                    blocked_attempts += 1;
                    if sample_errors.len() < 5 {
                        sample_errors.push(format!("{}: empty unauthenticated response (JS/challenge gated)", seed.value));
                    }
                }
                Err(err) => {
                    if matches!(
                        err.kind,
                        ProviderErrorKind::BlockedOrCaptcha
                            | ProviderErrorKind::RateLimited
                            | ProviderErrorKind::CircuitOpen
                            | ProviderErrorKind::NotFoundOrPrivate
                    ) {
                        blocked_attempts += 1;
                    }
                    if sample_errors.len() < 5 {
                        let short: String = err.message.chars().take(140).collect();
                        sample_errors.push(format!("{}: [{:?}] {}", seed.value, err.kind, short));
                    }
                }
            }
        }
    }

    latencies_ms.sort_unstable();
    let p50 = percentile(&latencies_ms, 0.50);
    let p95 = percentile(&latencies_ms, 0.95);
    let success_rate_pct = if total_attempts > 0 {
        (successful_attempts as f64 / total_attempts as f64) * 100.0
    } else {
        0.0
    };
    let block_rate_pct = if total_attempts > 0 {
        (blocked_attempts as f64 / total_attempts as f64) * 100.0
    } else {
        100.0
    };
    let unique_bulgarian_videos = unique_bg_ids.len();
    let meets_go_criteria = unique_bulgarian_videos >= 30 && block_rate_pct < 20.0;

    ApproachMetrics {
        approach_name: source.name().to_string(),
        total_attempts,
        successful_attempts,
        blocked_attempts,
        success_rate_pct,
        block_rate_pct,
        p50_latency_ms: p50,
        p95_latency_ms: p95,
        unique_videos: unique_ids.len(),
        unique_bulgarian_videos,
        sample_urls,
        sample_errors,
        meets_go_criteria,
    }
}

fn percentile(sorted: &[u64], q: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub fn render_spike_markdown(report: &DiscoverySpikeReport) -> String {
    let mut md = String::new();
    md.push_str("# Discovery Spike Report (Phase 1)\n\n");
    md.push_str(&format!(
        "- **Date**: {}\n- **Seeds tested**: {} (`{}`)\n- **Runs per seed**: {}\n- **Overall Decision**: **{}**\n\n",
        chrono::Utc::now().format("%Y-%m-%d"),
        report.seeds_tested.len(),
        report.seeds_tested.join("`, `"),
        report.runs_per_seed,
        if report.overall_go { "GO" } else { "NO-GO" }
    ));

    md.push_str("## Measured Results Summary\n\n");
    md.push_str("| Approach | Attempts | Success Rate | Block / Gate Rate | p50 Latency (ms) | p95 Latency (ms) | Unique Videos | Unique BG Videos | Pass Criteria (>=30 BG & <20% Block) |\n");
    md.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for m in &report.approaches {
        md.push_str(&format!(
            "| `{}` | {} | {:.1}% | {:.1}% | {} | {} | {} | {} | {} |\n",
            m.approach_name,
            m.total_attempts,
            m.success_rate_pct,
            m.block_rate_pct,
            m.p50_latency_ms,
            m.p95_latency_ms,
            m.unique_videos,
            m.unique_bulgarian_videos,
            if m.meets_go_criteria { "PASS (GO)" } else { "FAIL (NO-GO)" }
        ));
    }

    md.push_str("\n## Recommendation\n\n");
    md.push_str(&report.recommendation);
    md.push_str("\n\n## Sample Observations & Error Taxonomy\n\n");
    for m in &report.approaches {
        md.push_str(&format!("### `{}`\n", m.approach_name));
        if m.sample_errors.is_empty() {
            md.push_str("- No errors recorded.\n");
        } else {
            for e in &m.sample_errors {
                md.push_str(&format!("- `{}`\n", e.replace('`', "'")));
            }
        }
        if !m.sample_urls.is_empty() {
            md.push_str(&format!("- Sample URLs extracted: {}\n", m.sample_urls.len()));
        }
        md.push('\n');
    }
    md
}
