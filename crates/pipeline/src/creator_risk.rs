use std::collections::HashSet;
use std::time::Duration;
use anyhow::Result;
use chrono::Utc;
use regex::Regex;
use reqwest::Client;
use leadfinder_core::{
    CreatorLocalStats, CreatorRiskAssessment, CreatorWebSource, VideoFilter,
};
use crate::engine::PipelineContext;

pub async fn assess_creator_risk(
    ctx: &PipelineContext,
    handle_or_id: &str,
    _display_name: Option<&str>,
    force_web: bool,
) -> Result<CreatorRiskAssessment> {
    let clean_handle = handle_or_id.trim().trim_start_matches('@').to_string();
    let id = clean_handle.to_lowercase();

    let existing_creator = ctx.db.get_creator(&id).await.ok().flatten();
    if !force_web {
        if let Some(ref c) = existing_creator {
            if let Some(ref json_str) = c.risk_assessment_json {
                if let Ok(ra) = serde_json::from_str::<CreatorRiskAssessment>(json_str) {
                    return Ok(ra);
                }
            }
        }
    }

    let filter = VideoFilter {
        creator_id: Some(id.clone()),
        limit: Some(200),
        ..Default::default()
    };
    let local_videos = ctx.db.list_videos(&filter).await.unwrap_or_default();
    let total_videos = local_videos.len() as i64;
    let mut flagged_videos = 0i64;
    let mut contradicted_claims = 0i64;
    let mut supported_claims = 0i64;

    for v in &local_videos {
        let claims = ctx.db.get_claims_for_video(&v.id).await.unwrap_or_default();
        let mut vid_flagged = false;
        for c in claims {
            let verd = c.verdict.as_deref().unwrap_or("").to_uppercase();
            if verd == "CONTRADICTED" {
                contradicted_claims += 1;
                vid_flagged = true;
            } else if verd == "SUPPORTED" {
                supported_claims += 1;
            }
        }
        if vid_flagged {
            flagged_videos += 1;
        }
    }

    if let Some(ref c) = existing_creator {
        flagged_videos = flagged_videos.max(c.flagged_count);
    }

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(12))
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut web_sources: Vec<CreatorWebSource> = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();

    let encoded_handle: String = url::form_urlencoded::byte_serialize(clean_handle.as_bytes()).collect();
    let factcheck_search_url = format!("https://factcheck.bg/?s={}", encoded_handle);
    if let Ok(resp) = client.get(&factcheck_search_url).send().await {
        if let Ok(html) = resp.text().await {
            let re_article = Regex::new(r#"(?is)<h[23][^>]*class="[^"]*entry-title[^"]*"[^>]*>\s*<a[^>]+href="([^"]+)"[^>]*>([^<]+)</a>"#).unwrap();
            for cap in re_article.captures_iter(&html).take(6) {
                let link = cap[1].trim().to_string();
                let title = htmlescape_clean(&cap[2]);
                if !seen_urls.contains(&link) && !title.is_empty() {
                    seen_urls.insert(link.clone());
                    web_sources.push(CreatorWebSource {
                        title,
                        url: link,
                        snippet: "Factcheck.bg проверка на фактите".to_string(),
                        domain: "factcheck.bg".to_string(),
                    });
                }
            }
        }
    }

    let bnt_search_url = format!("https://bntnews.bg/search?q={}", encoded_handle);
    if let Ok(resp) = client.get(&bnt_search_url).send().await {
        if let Ok(html) = resp.text().await {
            let re_bnt = Regex::new(r#"(?is)<a[^>]+href="([^"]*bntnews\.bg/[^"]+)"[^>]*title="([^"]+)"#).unwrap();
            for cap in re_bnt.captures_iter(&html).take(4) {
                let link = cap[1].trim().to_string();
                let title = htmlescape_clean(&cap[2]);
                if !seen_urls.contains(&link) && !title.is_empty() {
                    seen_urls.insert(link.clone());
                    web_sources.push(CreatorWebSource {
                        title,
                        url: link,
                        snippet: "БНТ репортаж / проверка".to_string(),
                        domain: "bntnews.bg".to_string(),
                    });
                }
            }
        }
    }

    let py_script = [
        std::path::PathBuf::from("scripts/google_ai_search.py"),
        std::path::PathBuf::from("tools/google_ai_search.py"),
    ]
    .into_iter()
    .find(|p| p.is_file());

    if let Some(script) = py_script {
        let q = format!("{} дезинформация фалшиви новини проверка", clean_handle);
        let mut cmd = tokio::process::Command::new("python");
        cmd.args([&script.to_string_lossy(), "--json", "--mode", "research", &q]);
        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }
        if let Ok(Ok(out)) = tokio::time::timeout(Duration::from_secs(15), cmd.output()).await {
            if out.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if let Some(citations) = val.get("citations").and_then(|v| v.as_array()) {
                        for c in citations {
                            let link = c.get("url").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                            let title = c.get("title").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                            if !link.is_empty() && !seen_urls.contains(&link) {
                                seen_urls.insert(link.clone());
                                let domain = reqwest::Url::parse(&link)
                                    .ok()
                                    .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
                                    .unwrap_or_else(|| "web".to_string());
                                web_sources.push(CreatorWebSource {
                                    title: if title.is_empty() { link.clone() } else { title },
                                    url: link,
                                    snippet: "Google AI онлайн източник".to_string(),
                                    domain,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    let mut factcheck_count = 0i64;
    let mut risk_keyword_count = 0i64;

    for s in &web_sources {
        let d = s.domain.to_lowercase();
        let t = s.title.to_lowercase();
        if d.contains("factcheck.bg") || d.contains("afp.com") || d.contains("bnr.bg") || d.contains("bntnews.bg") {
            factcheck_count += 1;
        }

        if t.contains("дезинформация")
            || t.contains("фалшив")
            || t.contains("опроверга")
            || t.contains("не е вярно")
            || t.contains("манипулация")
            || t.contains("измама")
            || t.contains("руска пропаганда")
            || t.contains("скандал")
            || t.contains("свален")
            || t.contains("fake news")
            || t.contains("misleading")
        {
            risk_keyword_count += 1;
        }
    }

    let db_sample_score = match total_videos {
        0 => 0.05,
        1 => 0.18,
        2 => 0.32,
        3..=4 => 0.52,
        5..=9 => 0.72,
        _ => 0.88,
    };

    let web_sample_score = if web_sources.is_empty() {
        0.05
    } else {
        (0.20 + (web_sources.len() as f64 * 0.07) + (factcheck_count as f64 * 0.20)).clamp(0.15, 0.95)
    };

    let raw_confidence = (db_sample_score * 0.40 + web_sample_score * 0.60).clamp(0.10, 0.95);
    let confidence = (raw_confidence * 100.0).round() / 100.0;

    let confidence_level = if confidence >= 0.70 {
        "high".to_string()
    } else if confidence >= 0.35 {
        "moderate".to_string()
    } else {
        "low".to_string()
    };

    let (risk_level, risk_score, rationale) = if contradicted_claims >= 2 || (contradicted_claims >= 1 && (factcheck_count >= 1 || risk_keyword_count >= 2)) {
        let score = (0.75 + (contradicted_claims as f64 * 0.08) + (factcheck_count as f64 * 0.05)).clamp(0.75, 0.98);
        let rat = format!(
            "Открити {} опровергани твърдения в локалната база и {} свързани публикации/фактчек проверки в мрежата. Висока степен на риск от разпространение на подвеждащи данни.",
            contradicted_claims,
            web_sources.len()
        );
        ("high".to_string(), score, rat)
    } else if contradicted_claims == 1 || flagged_videos > 0 || risk_keyword_count >= 2 {
        let score = 0.55;
        let rat = format!(
            "Регистрирано 1 спорно/опровергано твърдение или сигнализирано съдържание сред общо {} видеа и {} уеб източника.",
            total_videos,
            web_sources.len()
        );
        ("moderate".to_string(), score, rat)
    } else if total_videos >= 3 && contradicted_claims == 0 && supported_claims >= 1 {
        let score = 0.15;
        let rat = format!(
            "Установена положителна история от {} видеа с потвърдени твърдения в базата данни и липса на фактчек опровержения.",
            total_videos
        );
        ("low".to_string(), score, rat)
    } else {
        let score = 0.30;
        let rat = format!(
            "Недостатъчно данни: само {} видео в базата и {} външни уеб източника. Липсва статистика за определяне на категоричен рисков профил.",
            total_videos,
            web_sources.len()
        );
        ("unverified".to_string(), score, rat)
    };

    let confidence_reason = if confidence_level == "low" {
        format!(
            "Нисък доверителен интервал ({}%): извадката е ограничена до {} видео и {} открити източника в мрежата.",
            (confidence * 100.0) as i64,
            total_videos,
            web_sources.len()
        )
    } else if confidence_level == "moderate" {
        format!(
            "Умерен доверителен интервал ({}%): налични данни от {} видео и {} открити източника в мрежата.",
            (confidence * 100.0) as i64,
            total_videos,
            web_sources.len()
        )
    } else {
        format!(
            "Висока увереност ({}%): потвърдено от {} видео в базата и {} проверени независими публикации в мрежата.",
            (confidence * 100.0) as i64,
            total_videos,
            web_sources.len()
        )
    };

    let assessment = CreatorRiskAssessment {
        risk_level,
        risk_score: (risk_score * 100.0).round() / 100.0,
        confidence,
        confidence_level,
        confidence_reason,
        rationale,
        web_sources,
        local_stats: CreatorLocalStats {
            total_videos,
            flagged_videos,
            contradicted_claims,
            supported_claims,
        },
        last_calculated_at: Utc::now().to_rfc3339(),
    };

    if let Ok(json_str) = serde_json::to_string(&assessment) {
        let _ = ctx.db.update_creator_risk_assessment(&id, &json_str).await;
    }

    Ok(assessment)
}

fn htmlescape_clean(raw: &str) -> String {
    let t = raw
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#8222;", "„")
        .replace("&#8220;", "“")
        .replace("&#8211;", "–")
        .replace("&#8217;", "'");
    t.trim().to_string()
}
