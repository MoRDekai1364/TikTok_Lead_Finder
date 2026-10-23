use anyhow::{anyhow, Result};
use leadfinder_core::{models::VideoDetail, VideoFilter};
use serde::{Deserialize, Serialize};

use crate::engine::PipelineContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Csv,
    Json,
}

impl ExportFormat {
    pub fn parse_str(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            "markdown" | "md" => Ok(ExportFormat::Markdown),
            "csv" => Ok(ExportFormat::Csv),
            "json" => Ok(ExportFormat::Json),
            other => Err(anyhow!("unsupported export format: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedLeadRecord {
    pub video_id: String,
    pub tiktok_url: String,
    pub creator_handle: String,
    pub creator_display_name: Option<String>,
    pub views: i64,
    pub likes: i64,
    pub shares: i64,
    pub duration_seconds: Option<f64>,
    pub lead_score: f64,
    pub transcript_text: String,
    pub top_claim_text: Option<String>,
    pub top_claim_verdict: Option<String>,
    pub top_claim_reasoning: Option<String>,
    pub evidence_urls: Vec<String>,
    pub wikidata_entities: Vec<String>,
    pub detail: VideoDetail,
}

impl PipelineContext {
    pub async fn export_leads(&self, lane: Option<&str>, format: ExportFormat) -> Result<String> {
        let lane_target = lane.unwrap_or("leads");
        let filter = VideoFilter {
            lane: Some(lane_target.to_string()),
            state: None,
            verdict: None,
            label: None,
            creator_id: None,
            search_query: None,
            min_score: None,
            min_views: None,
            limit: Some(2000),
            offset: Some(0),
        };

        let videos = self.db.list_videos(&filter).await?;
        let mut records: Vec<ExportedLeadRecord> = Vec::new();

        for v in videos {
            if let Some(detail) = self.db.get_video_detail(&v.id).await? {
                let handle = detail
                    .creator
                    .as_ref()
                    .map(|c| c.handle.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                let display = detail.creator.as_ref().and_then(|c| c.display_name.clone());
                let tiktok_url = format!("https://www.tiktok.com/@{}/video/{}", handle, v.id);

                let transcript_text = detail
                    .transcript
                    .as_ref()
                    .and_then(|t| t.edited_text.clone().or_else(|| Some(t.text.clone())))
                    .unwrap_or_default();

                let top_claim = detail.claims.first();
                let top_claim_text = top_claim.map(|c| c.claim.text_bg.clone());
                let top_claim_verdict = top_claim.and_then(|c| c.claim.verdict.clone());
                let top_claim_reasoning = top_claim.and_then(|c| c.claim.reasoning_bg.clone());

                let mut evidence_urls = Vec::new();
                for c in &detail.claims {
                    for ev in &c.evidence {
                        if !ev.source_url.is_empty() && !evidence_urls.contains(&ev.source_url) {
                            evidence_urls.push(ev.source_url.clone());
                        }
                    }
                }

                let mut wikidata_entities = Vec::new();
                for p in &detail.people {
                    if let Some(qid) = &p.wikidata_id {
                        let entry = format!("{}: {}", p.canonical_name, qid);
                        if !wikidata_entities.contains(&entry) {
                            wikidata_entities.push(entry);
                        }
                    }
                }

                records.push(ExportedLeadRecord {
                    video_id: v.id.clone(),
                    tiktok_url,
                    creator_handle: handle,
                    creator_display_name: display,
                    views: v.views.unwrap_or(0),
                    likes: v.likes.unwrap_or(0),
                    shares: v.shares.unwrap_or(0),
                    duration_seconds: v.duration_s,
                    lead_score: v.lead_score.unwrap_or(0.0),
                    transcript_text,
                    top_claim_text,
                    top_claim_verdict,
                    top_claim_reasoning,
                    evidence_urls,
                    wikidata_entities,
                    detail,
                });
            }
        }

        match format {
            ExportFormat::Markdown => Ok(Self::format_leads_markdown(lane_target, &records)),
            ExportFormat::Csv => Ok(Self::format_leads_csv(&records)),
            ExportFormat::Json => Ok(serde_json::to_string_pretty(&records)?),
        }
    }

    fn format_leads_markdown(lane: &str, records: &[ExportedLeadRecord]) -> String {
        let mut out = String::new();
        out.push_str(&format!("# TikTok Lead Finder — Kept Leads ({})\n\n", lane));
        out.push_str(&format!("Exported: {} leads\n\n", records.len()));

        out.push_str("| # | Video ID | Creator | Views | Verdict | Lead Score | Top Claim |\n");
        out.push_str("|---|---|---|---|---|---|---|\n");
        for (i, r) in records.iter().enumerate() {
            let claim_preview = r
                .top_claim_text
                .as_deref()
                .unwrap_or("-")
                .replace('|', "\\|")
                .chars()
                .take(60)
                .collect::<String>();
            let verdict = r.top_claim_verdict.as_deref().unwrap_or("NEW");
            out.push_str(&format!(
                "| {} | [{}]({}) | @{} | {} | **{}** | {:.2} | {} |\n",
                i + 1,
                r.video_id,
                r.tiktok_url,
                r.creator_handle,
                r.views,
                verdict,
                r.lead_score,
                claim_preview
            ));
        }
        out.push_str("\n---\n\n");

        for (i, r) in records.iter().enumerate() {
            out.push_str(&format!(
                "## {}. @{} — Video {}\n\n",
                i + 1,
                r.creator_handle,
                r.video_id
            ));
            out.push_str(&format!("- **URL**: {}\n", r.tiktok_url));
            out.push_str(&format!(
                "- **Metrics**: {} views · {} likes · {} shares\n",
                r.views, r.likes, r.shares
            ));
            out.push_str(&format!("- **Lead Score**: {:.3}\n", r.lead_score));
            if let Some(dur) = r.duration_seconds {
                out.push_str(&format!("- **Duration**: {:.1}s\n", dur));
            }

            if !r.wikidata_entities.is_empty() {
                out.push_str("- **Identified Figures**: ");
                out.push_str(&r.wikidata_entities.join(", "));
                out.push('\n');
            }

            out.push_str("\n### Transcript\n\n");
            if r.transcript_text.is_empty() {
                out.push_str("_No transcript available._\n\n");
            } else {
                out.push_str(&format!("> {}\n\n", r.transcript_text.replace('\n', "\n> ")));
            }

            out.push_str("### Claims & Evidence\n\n");
            if r.detail.claims.is_empty() {
                out.push_str("_No claims flagged._\n\n");
            } else {
                for (ci, cwith) in r.detail.claims.iter().enumerate() {
                    let v = cwith.claim.verdict.as_deref().unwrap_or("NEW");
                    out.push_str(&format!(
                        "#### Claim {}.{} [{}]\n\n",
                        i + 1,
                        ci + 1,
                        v
                    ));
                    out.push_str(&format!("**Text**: {}\n\n", cwith.claim.text_bg));
                    if let Some(spk) = &cwith.claim.speaker {
                        out.push_str(&format!("**Speaker**: {}\n\n", spk));
                    }
                    if let Some(reasoning) = &cwith.claim.reasoning_bg {
                        out.push_str(&format!("**Reasoning**: {}\n\n", reasoning));
                    }
                    if !cwith.evidence.is_empty() {
                        out.push_str("**Evidence Passages**:\n\n");
                        for ev in &cwith.evidence {
                            let stance = ev.stance.as_deref().unwrap_or("unknown");
                            let title = ev.source_name.as_deref().unwrap_or("Source");
                            let url_str = &ev.source_url;
                            let publisher = &ev.source_kind;
                            out.push_str(&format!(
                                "- [{}] [{}]({}) ({})\n",
                                stance.to_uppercase(),
                                title,
                                url_str,
                                publisher
                            ));
                            out.push_str(&format!("  > {}\n", ev.passage.trim()));
                        }
                        out.push('\n');
                    }
                }
            }
            out.push_str("---\n\n");
        }

        out
    }

    fn format_leads_csv(records: &[ExportedLeadRecord]) -> String {
        let mut out = String::new();
        out.push_str("video_id,tiktok_url,creator_handle,views,likes,shares,lead_score,transcript_snippet,top_claim_text,top_claim_verdict,top_claim_reasoning,evidence_urls,wikidata_entities\n");
        for r in records {
            let row = [
                Self::csv_escape(&r.video_id),
                Self::csv_escape(&r.tiktok_url),
                Self::csv_escape(&r.creator_handle),
                r.views.to_string(),
                r.likes.to_string(),
                r.shares.to_string(),
                format!("{:.3}", r.lead_score),
                Self::csv_escape(&r.transcript_text.chars().take(180).collect::<String>()),
                Self::csv_escape(r.top_claim_text.as_deref().unwrap_or("")),
                Self::csv_escape(r.top_claim_verdict.as_deref().unwrap_or("")),
                Self::csv_escape(r.top_claim_reasoning.as_deref().unwrap_or("")),
                Self::csv_escape(&r.evidence_urls.join(" | ")),
                Self::csv_escape(&r.wikidata_entities.join(" | ")),
            ];
            out.push_str(&row.join(","));
            out.push('\n');
        }
        out
    }

    fn csv_escape(s: &str) -> String {
        let escaped = s.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use leadfinder_core::{RuntimePaths, VideoRef, VideoState};
    use tempfile::tempdir;

    #[tokio::test]
    async fn export_markdown_csv_json_produces_valid_formats() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let repo_cfg = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("config")
            .join("default.toml");
        std::fs::copy(&repo_cfg, config_dir.join("default.toml")).unwrap();

        let paths = RuntimePaths {
            static_root: temp.path().to_path_buf(),
            mutable_root: temp.path().to_path_buf(),
            config_dir,
            data_dir: temp.path().join("data"),
            db_path: temp.path().join("data").join("test.db"),
            log_dir: temp.path().join("logs"),
            cache_dir: temp.path().join("cache"),
            prompts_dir: temp.path().join("prompts"),
            assets_dir: temp.path().join("assets"),
            tools_dir: temp.path().join("tools"),
            bundled_tools_dir: temp.path().join("tools"),
            models_dir: temp.path().join("models"),
        };
        let ctx = PipelineContext::init_with_paths(paths).await.unwrap();

        let vref = VideoRef {
            id: "7300000000000000001".to_string(),
            url: "https://www.tiktok.com/@sample_user/video/7300000000000000001".to_string(),
            creator_id: Some("user_1".to_string()),
            creator_handle: Some("sample_user".to_string()),
            creator_display_name: Some("Sample Creator".to_string()),
            creator_follower_count: Some(5000),
            creator_region: Some("BG".to_string()),
            caption: Some("Примерно съдържание".to_string()),
            hashtags: vec!["българия".to_string()],
            posted_at: Some("2026-10-23T10:00:00Z".to_string()),
            views: Some(12500),
            likes: Some(890),
            shares: Some(120),
            comments: Some(45),
            duration_s: Some(45.0),
            sound_title: None,
            subtitle_urls: vec![],
            discovered_via: "paste".to_string(),
        };
        ctx.db.upsert_video_discovered(&vref, 0).await.unwrap();
        ctx.db
            .set_video_ranking(
                &vref.id,
                0.85,
                "leads",
                None,
                VideoState::Verified,
                Some("high priority lead"),
            )
            .await
            .unwrap();

        let md = ctx.export_leads(Some("leads"), ExportFormat::Markdown).await.unwrap();
        assert!(md.contains("# TikTok Lead Finder — Kept Leads (leads)"));
        assert!(md.contains("sample_user"));

        let csv = ctx.export_leads(Some("leads"), ExportFormat::Csv).await.unwrap();
        assert!(csv.contains("video_id,tiktok_url,creator_handle"));
        assert!(csv.contains("7300000000000000001"));

        let json = ctx.export_leads(Some("leads"), ExportFormat::Json).await.unwrap();
        assert!(json.contains("7300000000000000001"));
        assert!(json.contains("sample_user"));
    }
}
