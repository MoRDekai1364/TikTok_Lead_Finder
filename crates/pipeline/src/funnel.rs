use std::collections::{HashMap, HashSet};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use leadfinder_core::{
    AppConfig, Database, FunnelConfig, RankingConfig, Video, VideoState,
};
use leadfinder_providers::global_lexicon;
use crate::engine::PipelineContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelClaim {
    pub id: i64,
    pub text: String,
    pub claim_type: String,
    pub checkworthiness: f64,
    #[serde(default)]
    pub verdict: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelVideo {
    pub id: String,
    pub views: i64,
    #[serde(default)]
    pub posted_at: Option<String>,
    #[serde(default)]
    pub phash: Option<String>,
    pub discovered_via: String,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub hashtags: Vec<String>,
    pub transcript_quality: f64,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub claims: Vec<FunnelClaim>,
    #[serde(default)]
    pub kept_label: Option<bool>,
    #[serde(default)]
    pub creator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenOutcome {
    Pass,
    Out(String),
}

#[derive(Debug, Clone, Default)]
pub struct BudgetState {
    pub llm_used: u64,
    pub search_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedVideo {
    pub id: String,
    pub lane: String,
    pub lead_score: f64,
    pub cluster_id: Option<i64>,
    pub cluster_size: usize,
    pub cluster_total_views: i64,
    pub representative: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingWeights {
    pub w_check: f64,
    pub w_viral: f64,
    pub w_verdict: f64,
    pub w_q: f64,
}

impl RankingWeights {
    pub fn from_config(cfg: &RankingConfig) -> Self {
        Self {
            w_check: cfg.w_check,
            w_viral: cfg.w_viral,
            w_verdict: cfg.w_verdict,
            w_q: cfg.w_q,
        }
    }
}

pub fn hamming_distance_hex(a: &str, b: &str) -> Option<u32> {
    let x = u64::from_str_radix(a.trim_start_matches("0x"), 16).ok()?;
    let y = u64::from_str_radix(b.trim_start_matches("0x"), 16).ok()?;
    Some((x ^ y).count_ones())
}

pub fn hours_since(posted_at: &str, now: DateTime<Utc>) -> Option<f64> {
    let parsed = DateTime::parse_from_rfc3339(posted_at).ok()?;
    let secs = (now - parsed.with_timezone(&Utc)).num_seconds();
    Some((secs as f64 / 3600.0).max(1.0))
}

pub fn virality_norm(views: i64, saturation_views: i64) -> f64 {
    if views <= 0 || saturation_views <= 1 {
        return 0.0;
    }
    (((views as f64) + 1.0).ln() / ((saturation_views as f64) + 1.0).ln()).clamp(0.0, 1.0)
}

pub fn verdict_value(verdict: Option<&str>) -> f64 {
    match verdict {
        Some("CONTRADICTED") => 1.0,
        Some("UNVERIFIABLE") => 0.6,
        Some("NOT_CHECKABLE") => 0.1,
        Some("SUPPORTED") => 0.0,
        _ => 0.6,
    }
}

pub fn screen_cheap(
    video: &FunnelVideo,
    funnel: &FunnelConfig,
    checkworthy_threshold: f64,
    seed_terms: &[String],
    seen_phashes: &[String],
    budget: &BudgetState,
    now: DateTime<Utc>,
) -> ScreenOutcome {
    if let Some(ph) = video.phash.as_deref() {
        let dup = seen_phashes.iter().any(|s| {
            hamming_distance_hex(ph, s)
                .map(|d| d <= funnel.phash_max_hamming)
                .unwrap_or(false)
        });
        if dup {
            return ScreenOutcome::Out("duplicate_phash".to_string());
        }
    }

    let bypass = funnel
        .bypass_sources
        .iter()
        .any(|s| video.discovered_via.starts_with(s.as_str()));

    if !bypass {
        if video.views < funnel.min_views {
            return ScreenOutcome::Out("below_min_views".to_string());
        }
        if let Some(hours) = video.posted_at.as_deref().and_then(|p| hours_since(p, now)) {
            if (video.views as f64) / hours < funnel.min_views_per_hour {
                return ScreenOutcome::Out("below_min_views_per_hour".to_string());
            }
        }
        let haystack = format!("{} {}", video.caption, video.hashtags.join(" ")).to_lowercase();
        let seed_hit = seed_terms
            .iter()
            .any(|t| !t.trim().is_empty() && haystack.contains(&t.trim().to_lowercase()));
        let claim_hit = video
            .claims
            .iter()
            .any(|c| c.claim_type != "opinion" && c.checkworthiness >= checkworthy_threshold);
        if !seed_hit && !claim_hit {
            return ScreenOutcome::Out("not_relevant".to_string());
        }
    }

    if budget.llm_used >= funnel.daily_llm_budget || budget.search_used >= funnel.daily_search_budget {
        return ScreenOutcome::Out("budget_exhausted".to_string());
    }

    ScreenOutcome::Pass
}

fn lemma_tokens(text: &str, lemmatize: &dyn Fn(&str) -> String) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 || w.chars().any(|c| c.is_ascii_digit()))
        .map(|w| lemmatize(&w.to_lowercase()))
        .collect()
}

fn trigram_counts(text: &str) -> HashMap<String, f64> {
    let norm: Vec<char> = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let mut counts: HashMap<String, f64> = HashMap::new();
    if norm.len() >= 3 {
        for w in norm.windows(3) {
            *counts.entry(w.iter().collect()).or_insert(0.0) += 1.0;
        }
    }
    counts
}

fn cosine(a: &HashMap<String, f64>, b: &HashMap<String, f64>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().map(|(k, v)| v * b.get(k).copied().unwrap_or(0.0)).sum();
    let na: f64 = a.values().map(|v| v * v).sum::<f64>().sqrt();
    let nb: f64 = b.values().map(|v| v * v).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

pub fn claim_similarity(
    a_lemmas: &HashSet<String>,
    b_lemmas: &HashSet<String>,
    a_tri: &HashMap<String, f64>,
    b_tri: &HashMap<String, f64>,
) -> f64 {
    let jaccard = if a_lemmas.is_empty() || b_lemmas.is_empty() {
        0.0
    } else {
        a_lemmas.intersection(b_lemmas).count() as f64 / a_lemmas.union(b_lemmas).count() as f64
    };
    0.6 * jaccard + 0.4 * cosine(a_tri, b_tri)
}

pub fn cluster_claims(
    videos: &[FunnelVideo],
    threshold: f64,
    lemmatize: &dyn Fn(&str) -> String,
) -> HashMap<i64, i64> {
    struct Entry {
        id: i64,
        lemmas: HashSet<String>,
        tri: HashMap<String, f64>,
    }
    let mut entries: Vec<Entry> = Vec::new();
    for v in videos {
        for c in &v.claims {
            if c.claim_type == "opinion" {
                continue;
            }
            entries.push(Entry {
                id: c.id,
                lemmas: lemma_tokens(&c.text, lemmatize),
                tri: trigram_counts(&c.text),
            });
        }
    }
    entries.sort_by_key(|e| e.id);

    let mut leaders: Vec<usize> = Vec::new();
    let mut leader_index: HashMap<String, Vec<usize>> = HashMap::new();
    let mut assignment: HashMap<i64, i64> = HashMap::new();

    for i in 0..entries.len() {
        let mut candidate_leaders: HashSet<usize> = HashSet::new();
        for l in &entries[i].lemmas {
            if l.chars().count() >= 4 {
                if let Some(list) = leader_index.get(l.as_str()) {
                    for &li in list {
                        candidate_leaders.insert(li);
                    }
                }
            }
        }
        let mut best: Option<(usize, f64)> = None;
        for li in candidate_leaders {
            let leader = leaders[li];
            let sim = claim_similarity(
                &entries[i].lemmas,
                &entries[leader].lemmas,
                &entries[i].tri,
                &entries[leader].tri,
            );
            if sim >= threshold && best.map(|(_, s)| sim > s).unwrap_or(true) {
                best = Some((li, sim));
            }
        }
        match best {
            Some((li, _)) => {
                assignment.insert(entries[i].id, entries[leaders[li]].id);
            }
            None => {
                let li = leaders.len();
                leaders.push(i);
                for l in &entries[i].lemmas {
                    if l.chars().count() >= 4 {
                        leader_index.entry(l.clone()).or_default().push(li);
                    }
                }
                assignment.insert(entries[i].id, entries[i].id);
            }
        }
    }
    assignment
}
fn top_claim(video: &FunnelVideo) -> Option<&FunnelClaim> {
    video
        .claims
        .iter()
        .filter(|c| c.claim_type != "opinion")
        .max_by(|a, b| {
            a.checkworthiness
                .partial_cmp(&b.checkworthiness)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

pub fn lead_score(
    video: &FunnelVideo,
    weights: &RankingWeights,
    saturation_views: i64,
) -> f64 {
    let Some(top) = top_claim(video) else {
        return 0.0;
    };
    let viral = virality_norm(video.views, saturation_views);
    let raw = weights.w_check * top.checkworthiness.clamp(0.0, 1.0)
        + weights.w_viral * viral
        + weights.w_verdict * verdict_value(top.verdict.as_deref())
        - weights.w_q * (1.0 - video.transcript_quality.clamp(0.0, 1.0));
    ((raw * 1000.0).round() / 1000.0).max(0.0)
}

pub fn features(video: &FunnelVideo, saturation_views: i64) -> [f64; 4] {
    let (cw, vv) = match top_claim(video) {
        Some(c) => (c.checkworthiness.clamp(0.0, 1.0), verdict_value(c.verdict.as_deref())),
        None => (0.0, 0.0),
    };
    [
        cw,
        virality_norm(video.views, saturation_views),
        vv,
        1.0 - video.transcript_quality.clamp(0.0, 1.0),
    ]
}

pub fn assign_lanes(
    videos: &[FunnelVideo],
    funnel: &FunnelConfig,
    ranking: &RankingConfig,
    weights: &RankingWeights,
    lemmatize: &dyn Fn(&str) -> String,
) -> Vec<RankedVideo> {
    let cluster_of_claim = cluster_claims(videos, funnel.cluster_similarity_threshold, lemmatize);
    let mut ranked: Vec<RankedVideo> = Vec::with_capacity(videos.len());
    let mut candidates: Vec<usize> = Vec::new();

    for v in videos {
        let top = top_claim(v);
        let cluster_id = top.and_then(|c| cluster_of_claim.get(&c.id).copied());
        let score = lead_score(v, weights, funnel.virality_saturation_views);

        let already_checked = v.claims.iter().any(|c| {
            c.claim_type != "opinion"
                && c.checkworthiness >= ranking.checkworthy_threshold
                && c.verdict.as_deref() == Some("ALREADY_CHECKED")
        });
        let has_checkworthy = v
            .claims
            .iter()
            .any(|c| c.claim_type != "opinion" && c.checkworthiness >= ranking.checkworthy_threshold);

        let (lane, reason) = if v.failed {
            ("failed", "pipeline_failed")
        } else if already_checked {
            ("already_checked", "existing_fact_check")
        } else if !has_checkworthy {
            ("screened_out", "no_checkworthy_claim")
        } else if v.transcript_quality < funnel.low_quality_threshold {
            ("low_quality", "transcript_quality_below_threshold")
        } else {
            ("leads_candidate", "checkworthy_claim")
        };

        if lane == "leads_candidate" {
            candidates.push(ranked.len());
        }
        ranked.push(RankedVideo {
            id: v.id.clone(),
            lane: lane.to_string(),
            lead_score: if lane == "already_checked" { 0.0 } else { score },
            cluster_id,
            cluster_size: 1,
            cluster_total_views: v.views,
            representative: true,
            reason: reason.to_string(),
        });
    }

    let mut groups: HashMap<i64, Vec<usize>> = HashMap::new();
    for &i in &candidates {
        if let Some(cid) = ranked[i].cluster_id {
            groups.entry(cid).or_default().push(i);
        }
    }
    let views_by_id: HashMap<&str, i64> = videos.iter().map(|v| (v.id.as_str(), v.views)).collect();
    for (_cid, members) in groups.iter() {
        let total: i64 = members
            .iter()
            .map(|&i| views_by_id.get(ranked[i].id.as_str()).copied().unwrap_or(0))
            .sum();
        let best = *members
            .iter()
            .max_by(|&&a, &&b| {
                ranked[a]
                    .lead_score
                    .partial_cmp(&ranked[b].lead_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(
                        views_by_id
                            .get(ranked[a].id.as_str())
                            .cmp(&views_by_id.get(ranked[b].id.as_str())),
                    )
            })
            .unwrap();
        for &i in members {
            ranked[i].cluster_size = members.len();
            ranked[i].cluster_total_views = total;
            if i != best {
                ranked[i].lane = "clustered".to_string();
                ranked[i].representative = false;
                ranked[i].reason = "member_of_cluster".to_string();
            }
        }
    }

    let mut reps: Vec<usize> = ranked
        .iter()
        .enumerate()
        .filter(|(_, r)| r.lane == "leads_candidate")
        .map(|(i, _)| i)
        .collect();
    reps.sort_by(|&a, &b| {
        ranked[b]
            .lead_score
            .partial_cmp(&ranked[a].lead_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(ranked[a].id.cmp(&ranked[b].id))
    });
    for (pos, &i) in reps.iter().enumerate() {
        if pos < funnel.daily_lead_cap {
            ranked[i].lane = "leads".to_string();
        } else {
            ranked[i].lane = "leads_queued".to_string();
            ranked[i].reason = "behind_daily_cap".to_string();
        }
    }
    ranked
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecisionReport {
    pub k: usize,
    pub labelled_in_top_k: usize,
    pub kept_in_top_k: usize,
    pub precision_at_k: f64,
}

pub fn precision_at_k(videos: &[FunnelVideo], ranked: &[RankedVideo], k: usize) -> PrecisionReport {
    let labels: HashMap<&str, Option<bool>> =
        videos.iter().map(|v| (v.id.as_str(), v.kept_label)).collect();
    let mut leads: Vec<&RankedVideo> = ranked.iter().filter(|r| r.lane == "leads").collect();
    leads.sort_by(|a, b| {
        b.lead_score
            .partial_cmp(&a.lead_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    let top: Vec<&&RankedVideo> = leads.iter().take(k).collect();
    let labelled = top
        .iter()
        .filter(|r| labels.get(r.id.as_str()).copied().flatten().is_some())
        .count();
    let kept = top
        .iter()
        .filter(|r| labels.get(r.id.as_str()).copied().flatten() == Some(true))
        .count();
    PrecisionReport {
        k,
        labelled_in_top_k: labelled,
        kept_in_top_k: kept,
        precision_at_k: if labelled == 0 {
            0.0
        } else {
            ((kept as f64 / labelled as f64) * 1000.0).round() / 1000.0
        },
    }
}

pub fn fit_logistic(samples: &[([f64; 4], bool)]) -> [f64; 5] {
    let mut w = [0.0f64; 5];
    if samples.is_empty() {
        return w;
    }
    let n = samples.len() as f64;
    for _ in 0..4000 {
        let mut grad = [0.0f64; 5];
        for (x, y) in samples {
            let z = w[4] + (0..4).map(|i| w[i] * x[i]).sum::<f64>();
            let p = 1.0 / (1.0 + (-z).exp());
            let err = p - if *y { 1.0 } else { 0.0 };
            for i in 0..4 {
                grad[i] += err * x[i];
            }
            grad[4] += err;
        }
        for i in 0..5 {
            let reg = if i < 4 { 0.01 * w[i] } else { 0.0 };
            w[i] -= 0.5 * (grad[i] / n + reg);
        }
    }
    w
}

pub fn propose_weights(coefs: &[f64; 5], current: &RankingWeights) -> RankingWeights {
    let c_check = coefs[0].max(0.0);
    let c_viral = coefs[1].max(0.0);
    let c_verdict = coefs[2].max(0.0);
    let c_q = (-coefs[3]).max(0.0);
    let positive_sum = c_check + c_viral + c_verdict;
    let current_positive = current.w_check + current.w_viral + current.w_verdict;
    if positive_sum <= 0.0 {
        return current.clone();
    }
    let scale = current_positive / positive_sum;
    let round3 = |x: f64| (x * 1000.0).round() / 1000.0;
    RankingWeights {
        w_check: round3(c_check * scale),
        w_viral: round3(c_viral * scale),
        w_verdict: round3(c_verdict * scale),
        w_q: round3((c_q * scale).min(current_positive)),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    pub videos_total: usize,
    pub screened_out: usize,
    pub screened_out_reasons: HashMap<String, usize>,
    pub passed: usize,
    pub lane_counts: HashMap<String, usize>,
    pub leads_in_lane: usize,
    pub daily_lead_cap: usize,
    pub clusters_with_multiple_videos: usize,
    pub largest_cluster_size: usize,
    pub precision: PrecisionReport,
}

pub fn replay_funnel(
    videos: &[FunnelVideo],
    cfg: &AppConfig,
    seed_terms: &[String],
    lemmatize: &dyn Fn(&str) -> String,
    now: DateTime<Utc>,
) -> (ReplayResult, Vec<RankedVideo>) {
    let mut seen: Vec<String> = Vec::new();
    let mut passed: Vec<FunnelVideo> = Vec::new();
    let mut reasons: HashMap<String, usize> = HashMap::new();
    let mut out_count = 0usize;
    for v in videos {
        match screen_cheap(
            v,
            &cfg.funnel,
            cfg.ranking.checkworthy_threshold,
            seed_terms,
            &seen,
            &BudgetState::default(),
            now,
        ) {
            ScreenOutcome::Pass => {
                if let Some(ph) = &v.phash {
                    seen.push(ph.clone());
                }
                passed.push(v.clone());
            }
            ScreenOutcome::Out(reason) => {
                out_count += 1;
                *reasons.entry(reason).or_insert(0) += 1;
            }
        }
    }
    let weights = RankingWeights::from_config(&cfg.ranking);
    let ranked = assign_lanes(&passed, &cfg.funnel, &cfg.ranking, &weights, lemmatize);
    let mut lane_counts: HashMap<String, usize> = HashMap::new();
    for r in &ranked {
        *lane_counts.entry(r.lane.clone()).or_insert(0) += 1;
    }
    let mut cluster_sizes: HashMap<i64, usize> = HashMap::new();
    for r in &ranked {
        if let Some(cid) = r.cluster_id {
            if r.lane != "failed" && r.lane != "screened_out" {
                *cluster_sizes.entry(cid).or_insert(0) += 1;
            }
        }
    }
    let multi = cluster_sizes.values().filter(|n| **n > 1).count();
    let largest = cluster_sizes.values().copied().max().unwrap_or(0);
    let precision = precision_at_k(&passed, &ranked, cfg.funnel.daily_lead_cap);
    let result = ReplayResult {
        videos_total: videos.len(),
        screened_out: out_count,
        screened_out_reasons: reasons,
        passed: passed.len(),
        leads_in_lane: lane_counts.get("leads").copied().unwrap_or(0),
        lane_counts,
        daily_lead_cap: cfg.funnel.daily_lead_cap,
        clusters_with_multiple_videos: multi,
        largest_cluster_size: largest,
        precision,
    };
    (result, ranked)
}

fn funnel_video_from_db(
    video: &Video,
    claims: &[leadfinder_core::Claim],
    transcript_quality: Option<f64>,
) -> FunnelVideo {
    FunnelVideo {
        id: video.id.clone(),
        views: video.views.unwrap_or(0),
        posted_at: video.posted_at.clone(),
        phash: video.phash.clone(),
        discovered_via: video.discovered_via.clone(),
        caption: video.caption.clone().unwrap_or_default(),
        hashtags: video.hashtags.clone(),
        transcript_quality: transcript_quality.unwrap_or(0.0),
        failed: video.state == VideoState::Failed.as_str(),
        claims: claims
            .iter()
            .map(|c| FunnelClaim {
                id: c.id,
                text: c.text_bg.clone(),
                claim_type: c.claim_type.clone(),
                checkworthiness: c.checkworthiness.unwrap_or(0.0),
                verdict: c.verdict.clone(),
                label: c.label.clone(),
            })
            .collect(),
        kept_label: None,
        creator: video.creator_handle.clone(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankSummary {
    pub ranked: usize,
    pub lane_counts: HashMap<String, usize>,
}

async fn effective_weights(db: &Database, cfg: &AppConfig) -> RankingWeights {
    if let Ok(Some(v)) = db.get_setting("ranking_weights").await {
        if let Ok(w) = serde_json::from_value::<RankingWeights>(v) {
            return w;
        }
    }
    RankingWeights::from_config(&cfg.ranking)
}

impl PipelineContext {
    pub async fn rank_all(&self) -> Result<RankSummary> {
        let cfg = self.config.read().await.clone();
        let lex = global_lexicon(&self.paths, &cfg.normalisation.lexicon_config_file);
        let lemmatize = |w: &str| lex.lemmatize_word(w);
        let weights = effective_weights(&self.db, &cfg).await;

        let mut inputs: Vec<FunnelVideo> = Vec::new();
        let videos = self
            .db
            .list_videos(&leadfinder_core::VideoFilter {
                limit: Some(5000),
                ..Default::default()
            })
            .await?;
        for v in &videos {
            let st = v.state.as_str();
            if st != VideoState::ClaimsDone.as_str()
                && st != VideoState::Verified.as_str()
                && st != VideoState::Ready.as_str()
                && st != VideoState::Failed.as_str()
            {
                continue;
            }
            let claims = self.db.get_claims_for_video(&v.id).await?;
            let tq = self.db.get_transcript(&v.id).await?.and_then(|t| t.quality);
            inputs.push(funnel_video_from_db(v, &claims, tq));
        }

        let ranked = assign_lanes(&inputs, &cfg.funnel, &cfg.ranking, &weights, &lemmatize);
        let mut lane_counts: HashMap<String, usize> = HashMap::new();
        for r in &ranked {
            let state = if r.lane == "failed" {
                VideoState::Failed
            } else {
                VideoState::Ready
            };
            self.db
                .set_video_ranking(
                    &r.id,
                    r.lead_score,
                    &r.lane,
                    r.cluster_id,
                    state,
                    Some(&r.reason),
                )
                .await?;
            *lane_counts.entry(r.lane.clone()).or_insert(0) += 1;
        }
        for v in &inputs {
            let map = cluster_claims(
                std::slice::from_ref(v),
                cfg.funnel.cluster_similarity_threshold,
                &lemmatize,
            );
            for (claim_id, cid) in map {
                self.db.set_claim_cluster(claim_id, cid).await?;
            }
        }
        self.db
            .refresh_creator_flagged_counts(cfg.expansion.window_days)
            .await?;
        Ok(RankSummary {
            ranked: ranked.len(),
            lane_counts,
        })
    }

    pub async fn pull_more(&self, count: usize) -> Result<usize> {
        let mut queued = self
            .db
            .list_videos(&leadfinder_core::VideoFilter {
                lane: Some("leads_queued".to_string()),
                limit: Some(5000),
                ..Default::default()
            })
            .await?;
        if queued.is_empty() {
            let all = self
                .db
                .list_videos(&leadfinder_core::VideoFilter {
                    limit: Some(5000),
                    ..Default::default()
                })
                .await?;
            queued = all
                .into_iter()
                .filter(|v| v.lane.as_deref() != Some("leads") && v.lane.as_deref() != Some("failed"))
                .collect();
        }
        let mut sorted = queued;
        sorted.sort_by(|a, b| {
            b.lead_score
                .unwrap_or(0.0)
                .partial_cmp(&a.lead_score.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut promoted = 0usize;
        for v in sorted.into_iter().take(count) {
            self.db
                .set_video_ranking(
                    &v.id,
                    v.lead_score.unwrap_or(0.75),
                    "leads",
                    v.cluster_id,
                    VideoState::Ready,
                    Some("pulled_from_pool"),
                )
                .await?;
            promoted += 1;
        }
        Ok(promoted)
    }

    pub async fn tune_ranking(&self, confirm: bool) -> Result<serde_json::Value> {
        let cfg = self.config.read().await.clone();
        let decided = self.db.latest_keep_discard_decisions().await?;
        if decided.len() < cfg.funnel.tune_min_decisions {
            return Ok(serde_json::json!({
                "status": "not_enough_decisions",
                "decisions": decided.len(),
                "required": cfg.funnel.tune_min_decisions,
            }));
        }
        let mut samples: Vec<([f64; 4], bool)> = Vec::new();
        for (video_id, keep) in &decided {
            let Some(video) = self.db.get_video(video_id).await? else {
                continue;
            };
            let claims = self.db.get_claims_for_video(video_id).await?;
            let tq = self.db.get_transcript(video_id).await?.and_then(|t| t.quality);
            let fv = funnel_video_from_db(&video, &claims, tq);
            samples.push((features(&fv, cfg.funnel.virality_saturation_views), *keep));
        }
        let current = effective_weights(&self.db, &cfg).await;
        let coefs = fit_logistic(&samples);
        let proposed = propose_weights(&coefs, &current);
        if confirm {
            self.db
                .set_setting("ranking_weights", &serde_json::to_value(&proposed)?)
                .await?;
        }
        Ok(serde_json::json!({
            "status": if confirm { "applied" } else { "proposed_needs_confirmation" },
            "decisions": samples.len(),
            "current": current,
            "proposed": proposed,
            "coefficients": coefs,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn funnel() -> FunnelConfig {
        FunnelConfig {
            min_views: 2000,
            min_views_per_hour: 100.0,
            daily_lead_cap: 25,
            low_quality_threshold: 0.4,
            retention_days: 7,
            daily_llm_budget: 250,
            daily_search_budget: 500,
            cluster_similarity_threshold: 0.55,
            virality_saturation_views: 1_000_000,
            phash_max_hamming: 6,
            tune_min_decisions: 200,
            bypass_sources: vec!["paste".to_string()],
        }
    }

    fn identity(w: &str) -> String {
        w.to_string()
    }

    fn video(id: &str, views: i64, claim: &str, cw: f64, verdict: &str, tq: f64) -> FunnelVideo {
        FunnelVideo {
            id: id.to_string(),
            views,
            posted_at: None,
            phash: None,
            discovered_via: "seed".to_string(),
            caption: String::new(),
            hashtags: Vec::new(),
            transcript_quality: tq,
            failed: false,
            claims: vec![FunnelClaim {
                id: id.bytes().map(|b| b as i64).sum(),
                text: claim.to_string(),
                claim_type: "factual".to_string(),
                checkworthiness: cw,
                verdict: Some(verdict.to_string()),
                label: None,
            }],
            kept_label: None,
            creator: None,
        }
    }

    #[test]
    fn hamming_and_duplicate_screening() {
        assert_eq!(hamming_distance_hex("ff00", "ff01"), Some(1));
        assert_eq!(hamming_distance_hex("zz", "ff01"), None);
        let mut v = video("a", 5000, "тест тест тест", 0.9, "UNVERIFIABLE", 0.9);
        v.phash = Some("ffff0000ffff0000".to_string());
        let seen = vec!["ffff0000ffff0001".to_string()];
        let out = screen_cheap(&v, &funnel(), 0.45, &["тест".to_string()], &seen, &BudgetState::default(), Utc::now());
        assert_eq!(out, ScreenOutcome::Out("duplicate_phash".to_string()));
    }

    #[test]
    fn virality_relevance_budget_and_bypass_gates() {
        let now = Utc::now();
        let low = video("a", 100, "твърдение за цените на тока", 0.9, "UNVERIFIABLE", 0.9);
        assert_eq!(
            screen_cheap(&low, &funnel(), 0.45, &[], &[], &BudgetState::default(), now),
            ScreenOutcome::Out("below_min_views".to_string())
        );
        let mut pasted = low.clone();
        pasted.discovered_via = "paste".to_string();
        assert_eq!(
            screen_cheap(&pasted, &funnel(), 0.45, &[], &[], &BudgetState::default(), now),
            ScreenOutcome::Pass
        );
        let mut irrelevant = video("b", 50_000, "нещо", 0.1, "UNVERIFIABLE", 0.9);
        irrelevant.claims[0].claim_type = "opinion".to_string();
        assert_eq!(
            screen_cheap(&irrelevant, &funnel(), 0.45, &[], &[], &BudgetState::default(), now),
            ScreenOutcome::Out("not_relevant".to_string())
        );
        let ok = video("c", 50_000, "твърдение", 0.9, "UNVERIFIABLE", 0.9);
        let spent = BudgetState { llm_used: 250, search_used: 0 };
        assert_eq!(
            screen_cheap(&ok, &funnel(), 0.45, &[], &[], &spent, now),
            ScreenOutcome::Out("budget_exhausted".to_string())
        );
    }

    #[test]
    fn lead_score_orders_by_verdict_virality_and_quality() {
        let w = RankingWeights { w_check: 0.35, w_viral: 0.2, w_verdict: 0.35, w_q: 0.15 };
        let strong = video("a", 500_000, "аа", 0.9, "CONTRADICTED", 0.9);
        let weak = video("b", 3000, "бб", 0.9, "SUPPORTED", 0.9);
        let poor_audio = video("c", 500_000, "вв", 0.9, "CONTRADICTED", 0.1);
        assert!(lead_score(&strong, &w, 1_000_000) > lead_score(&weak, &w, 1_000_000));
        assert!(lead_score(&strong, &w, 1_000_000) > lead_score(&poor_audio, &w, 1_000_000));
    }

    #[test]
    fn near_duplicate_claims_cluster_and_one_representative_leads() {
        let ranking = ranking_cfg();
        let w = RankingWeights::from_config(&ranking);
        let vids = vec![
            video("v1", 90_000, "цените на тока ще поскъпнат два пъти през следващата зима", 0.9, "CONTRADICTED", 0.9),
            video("v2", 40_000, "цените на тока ще поскъпнат два пъти през следващата зима със сигурност", 0.8, "CONTRADICTED", 0.9),
            video("v3", 20_000, "цените на тока ще поскъпнат два пъти през следващата зима", 0.7, "CONTRADICTED", 0.9),
            video("v4", 70_000, "лекарствата в аптеките са двойно по-евтини в съседна държава", 0.8, "UNVERIFIABLE", 0.9),
        ];
        let ranked = assign_lanes(&vids, &funnel(), &ranking, &w, &identity);
        let leads: Vec<&RankedVideo> = ranked.iter().filter(|r| r.lane == "leads").collect();
        assert_eq!(leads.len(), 2);
        let rep = ranked.iter().find(|r| r.id == "v1").unwrap();
        assert_eq!(rep.lane, "leads");
        assert_eq!(rep.cluster_size, 3);
        assert_eq!(rep.cluster_total_views, 150_000);
        assert_eq!(ranked.iter().filter(|r| r.lane == "clustered").count(), 2);
    }

    #[test]
    fn lanes_cap_and_overflow_behave() {
        let ranking = ranking_cfg();
        let w = RankingWeights::from_config(&ranking);
        let mut cfg = funnel();
        cfg.daily_lead_cap = 3;
        let texts = [
            "първо уникално твърдение за бюджета на общината",
            "второ различно твърдение относно пътните такси",
            "трето несвързано твърдение за градския транспорт",
            "четвърто твърдение за болниците и лекарствата",
            "пето твърдение за образованието и учебниците",
        ];
        let mut vids: Vec<FunnelVideo> = texts
            .iter()
            .enumerate()
            .map(|(i, t)| video(&format!("x{i}"), 10_000 * (i as i64 + 1), t, 0.9, "UNVERIFIABLE", 0.9))
            .collect();
        vids.push(video("lq", 90_000, "твърдение с лошо аудио за данъците", 0.9, "CONTRADICTED", 0.1));
        vids.push(video("ac", 90_000, "твърдение вече проверено от архива", 0.9, "ALREADY_CHECKED", 0.9));
        let mut failed = video("fx", 90_000, "неважно", 0.9, "CONTRADICTED", 0.9);
        failed.failed = true;
        vids.push(failed);
        let ranked = assign_lanes(&vids, &cfg, &ranking, &w, &identity);
        let lane = |id: &str| ranked.iter().find(|r| r.id == id).unwrap().lane.clone();
        assert_eq!(lane("lq"), "low_quality");
        assert_eq!(lane("ac"), "already_checked");
        assert_eq!(lane("fx"), "failed");
        assert_eq!(ranked.iter().filter(|r| r.lane == "leads").count(), 3);
        assert_eq!(ranked.iter().filter(|r| r.lane == "leads_queued").count(), 2);
        assert_eq!(lane("x4"), "leads");
    }

    #[test]
    fn logistic_fit_recovers_signal_and_proposal_keeps_weight_budget() {
        let mut samples = Vec::new();
        for i in 0..300 {
            let verdict = if i % 2 == 0 { 1.0 } else { 0.0 };
            let noise = ((i * 37) % 10) as f64 / 100.0;
            samples.push(([0.5 + noise, 0.4, verdict, 0.1], verdict > 0.5));
        }
        let coefs = fit_logistic(&samples);
        assert!(coefs[2] > coefs[1]);
        let current = RankingWeights { w_check: 0.35, w_viral: 0.2, w_verdict: 0.35, w_q: 0.15 };
        let proposed = propose_weights(&coefs, &current);
        let sum = proposed.w_check + proposed.w_viral + proposed.w_verdict;
        assert!((sum - 0.9).abs() < 0.01);
        assert!(proposed.w_verdict > proposed.w_viral);
    }

    fn ranking_cfg() -> RankingConfig {
        RankingConfig {
            w_check: 0.35,
            w_viral: 0.2,
            w_verdict: 0.35,
            w_q: 0.15,
            checkworthy_threshold: 0.45,
            weight_llm_score: 0.6,
            weight_contains_numbers: 0.12,
            weight_public_figure: 0.1,
            weight_sensitive_topic: 0.1,
            weight_urges_sharing: 0.08,
            sensitive_topic_terms: Vec::new(),
            share_urge_terms: Vec::new(),
        }
    }
}
