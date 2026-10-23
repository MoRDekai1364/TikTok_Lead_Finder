use std::collections::HashSet;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use leadfinder_core::{
    AppConfig, Claim, CoreEvent, Evidence, FactCheckArchive, LlmJsonRequest, Search, SearchHit,
    SearchQuery, Stage, VideoState,
};
use leadfinder_providers::{
    global_lexicon, ConfiguredFactCheckArchive, LlmChainRunner, PassageExtractorAndRanker,
    RankedPassage, WebAndPrimarySearch,
};
use crate::engine::PipelineContext;

const DEFAULT_VERIFY_SYSTEM_PROMPT: &str = "You are a retrieval-grounded Bulgarian fact-checking assistant. Compare the claim strictly to the provided passages. Return strictly valid JSON.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationStageReport {
    pub video_id: String,
    pub claim_id: i64,
    pub claim_text: String,
    pub verdict: String,
    pub label: String,
    pub reasoning_bg: String,
    pub evidence_strength: f64,
    pub passages_count: usize,
    pub stored_evidence_count: usize,
    pub forced_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawVerificationLlmOutput {
    pub claim_id: i64,
    pub verdict: String,
    pub label: String,
    pub evidence: Vec<RawEvidenceRef>,
    pub reasoning_bg: String,
    pub evidence_strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEvidenceRef {
    pub passage_id: String,
    pub stance: String,
}

pub fn verification_output_json_schema() -> Value {
    json!({
        "type": "object",
        "required": ["claim_id", "verdict", "label", "evidence", "reasoning_bg", "evidence_strength"],
        "additionalProperties": false,
        "properties": {
            "claim_id": { "type": "integer" },
            "verdict": {
                "type": "string",
                "enum": ["SUPPORTED", "CONTRADICTED", "UNVERIFIABLE", "NOT_CHECKABLE", "ALREADY_CHECKED"]
            },
            "label": {
                "type": "string",
                "enum": ["false_claim", "misleading", "missing_context", "satire_or_opinion", "unknown"]
            },
            "evidence": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["passage_id", "stance"],
                    "additionalProperties": false,
                    "properties": {
                        "passage_id": { "type": "string", "minLength": 2 },
                        "stance": { "type": "string", "enum": ["supports", "contradicts", "neutral"] }
                    }
                }
            },
            "reasoning_bg": { "type": "string", "minLength": 5 },
            "evidence_strength": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
        }
    })
}

pub fn sanitize_label(label: &str) -> String {
    let low = label.trim().to_lowercase();
    match low.as_str() {
        "false_claim" => "false_claim".to_string(),
        "misleading" => "misleading".to_string(),
        "missing_context" => "missing_context".to_string(),
        "satire_or_opinion" => "satire_or_opinion".to_string(),
        "disinformation" | "fake_news" => "false_claim".to_string(),
        _ => "unknown".to_string(),
    }
}

pub fn enforce_verification_rules(
    claim: &Claim,
    raw_output: &RawVerificationLlmOutput,
    passages: &[RankedPassage],
    min_independent_news_outlets: usize,
) -> (String, String, Vec<Evidence>, String, f64, Option<String>) {
    if claim.claim_type == "opinion" {
        return (
            "NOT_CHECKABLE".to_string(),
            "satire_or_opinion".to_string(),
            Vec::new(),
            "Твърдението представлява субективно мнение или оценка и не подлежи на фактическа проверка.".to_string(),
            0.0,
            Some("opinion_not_checkable".to_string()),
        );
    }

    if passages.is_empty() {
        return (
            "UNVERIFIABLE".to_string(),
            "unknown".to_string(),
            Vec::new(),
            "Няма открити релевантни институционални пасажи или проверки на факти за това твърдение.".to_string(),
            0.0,
            Some("no_passages_retrieved".to_string()),
        );
    }

    let valid_ids: HashSet<&str> = passages.iter().map(|p| p.passage_id.as_str()).collect();
    let mut stored_evidence: Vec<Evidence> = Vec::new();
    let mut seen_pids = HashSet::new();

    for ev in &raw_output.evidence {
        if !valid_ids.contains(ev.passage_id.as_str()) {
            continue;
        }
        if !seen_pids.insert(ev.passage_id.clone()) {
            continue;
        }
        if let Some(p) = passages.iter().find(|x| x.passage_id == ev.passage_id) {
            let stance = match ev.stance.to_lowercase().as_str() {
                "supports" => "supports".to_string(),
                "contradicts" => "contradicts".to_string(),
                _ => "neutral".to_string(),
            };
            stored_evidence.push(Evidence {
                id: 0,
                claim_id: claim.id,
                passage_id: p.passage_id.clone(),
                source_url: p.source_url.clone(),
                source_name: p.source_name.clone(),
                source_kind: p.source_kind.clone(),
                published_at: p.published_at.clone(),
                passage: p.text.clone(),
                stance: Some(stance),
                retrieved_at: chrono::Utc::now().to_rfc3339(),
            });
        }
    }

    let mut verdict = raw_output.verdict.to_uppercase();
    let mut label = sanitize_label(&raw_output.label);
    let mut reasoning = raw_output.reasoning_bg.trim().to_string();
    let mut strength = raw_output.evidence_strength.clamp(0.0, 1.0);
    let mut forced_reason = None;

    if verdict == "ALREADY_CHECKED" {
        let has_factcheck_evidence = stored_evidence.iter().any(|e| e.source_kind == "factcheck");
        if !has_factcheck_evidence {
            verdict = "UNVERIFIABLE".to_string();
            label = "unknown".to_string();
            strength = 0.0;
            forced_reason = Some("already_checked_missing_factcheck_link".to_string());
        }
    } else if verdict == "SUPPORTED" || verdict == "CONTRADICTED" {
        let has_primary_or_fc = stored_evidence
            .iter()
            .any(|e| e.source_kind == "primary" || e.source_kind == "factcheck");

        let news_domains: HashSet<String> = stored_evidence
            .iter()
            .filter(|e| e.source_kind == "news")
            .filter_map(|e| url::Url::parse(&e.source_url).ok()?.host_str().map(|h| h.to_lowercase()))
            .collect();

        let sufficient = has_primary_or_fc || news_domains.len() >= min_independent_news_outlets;
        if !sufficient {
            verdict = "UNVERIFIABLE".to_string();
            label = "unknown".to_string();
            strength = (strength * 0.4).min(0.35);
            forced_reason = Some("insufficient_authoritative_sources_forced_unverifiable".to_string());
            reasoning = format!("{reasoning} (Липсва минимално необходимият брой независими потвърждения съгласно правилата.)");
        }
    }

    if stored_evidence.is_empty() && verdict != "NOT_CHECKABLE" {
        verdict = "UNVERIFIABLE".to_string();
        label = "unknown".to_string();
        strength = 0.0;
        forced_reason = Some("no_valid_passage_ids_forced_unverifiable".to_string());
    }

    (verdict, label, stored_evidence, reasoning, strength, forced_reason)
}

pub fn build_verify_user_prompt(
    claim: &Claim,
    passages: &[RankedPassage],
    date_window: Option<&str>,
) -> String {
    let mut sections = Vec::new();
    sections.push(format!(
        "CLAIM_TO_VERIFY:\n- id: {}\n- text_bg: {}\n- claim_type: {}\n- speaker: {}\n- date_window: {}",
        claim.id,
        claim.text_bg,
        claim.claim_type,
        claim.speaker.as_deref().unwrap_or("unknown"),
        date_window.unwrap_or("unknown")
    ));

    if !claim.attribution.is_empty() {
        let mut att_lines = Vec::new();
        for a in &claim.attribution {
            att_lines.push(format!(
                "- actor: {} | act: {} | target: {:?} | about: {}\n  asserted_part: {}\n  attributed_part: {}",
                a.actor,
                a.act,
                a.target,
                a.about,
                a.asserted_part.as_deref().unwrap_or(""),
                a.attributed_part.as_deref().unwrap_or("")
            ));
        }
        sections.push(format!("ATTRIBUTION_DETAILS:\n{}", att_lines.join("\n")));
    }

    let mut pass_lines = Vec::new();
    for p in passages {
        pass_lines.push(format!(
            "[{}] ({}) {} [pub: {}]:\n{}",
            p.passage_id,
            p.source_kind,
            p.source_name.as_deref().unwrap_or(&p.source_url),
            p.published_at.as_deref().unwrap_or("unknown"),
            p.text
        ));
    }
    sections.push(format!("RETRIEVED_PASSAGES:\n{}", pass_lines.join("\n\n")));

    sections.join("\n\n")
}

impl PipelineContext {
    pub async fn verify_claim_with_retrieval(
        &self,
        video_id: &str,
        claim_id: i64,
    ) -> Result<VerificationStageReport> {
        let claim = self
            .db
            .get_claims_for_video(video_id)
            .await?
            .into_iter()
            .find(|c| c.id == claim_id)
            .ok_or_else(|| anyhow::anyhow!("claim not found: {claim_id}"))?;
        let video = self
            .db
            .get_video(video_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("video not found: {video_id}"))?;

        let cfg: AppConfig = self.config.read().await.clone();
        let lex = global_lexicon(&self.paths, &cfg.normalisation.lexicon_config_file);

        if claim.claim_type == "opinion" {
            let dummy = RawVerificationLlmOutput {
                claim_id: claim.id,
                verdict: "NOT_CHECKABLE".to_string(),
                label: "satire_or_opinion".to_string(),
                evidence: Vec::new(),
                reasoning_bg: "Субективно мнение или оценка.".to_string(),
                evidence_strength: 0.0,
            };
            let (verdict, label, evs, reasoning, strength, forced) = enforce_verification_rules(
                &claim,
                &dummy,
                &[],
                cfg.verification.min_independent_news_outlets,
            );
            self.db
                .update_claim_verification(claim.id, &verdict, &label, &reasoning, strength, &evs)
                .await?;
            self.check_and_mark_video_verified(video_id).await?;
            return Ok(VerificationStageReport {
                video_id: video_id.to_string(),
                claim_id: claim.id,
                claim_text: claim.text_bg,
                verdict,
                label,
                reasoning_bg: reasoning,
                evidence_strength: strength,
                passages_count: 0,
                stored_evidence_count: evs.len(),
                forced_reason: forced,
            });
        }

        let archive_ep = cfg.factcheck_archives.clone();
        let archive = ConfiguredFactCheckArchive::new(
            archive_ep.clone(),
            lex.clone(),
            self.guard.clone(),
            cfg.discovery.region.clone(),
        );

        let primary_entries =
            leadfinder_core::load_primary_sources(&self.paths.config_dir).unwrap_or_default();
        let search_engine = WebAndPrimarySearch::new(
            &cfg.providers.searxng_url,
            primary_entries.clone(),
            archive_ep.clone(),
            cfg.verification.news_domains.clone(),
            self.guard.clone(),
        );

        let ranker = PassageExtractorAndRanker::new(self.guard.clone(), lex.clone());
        let mut candidate_hits: Vec<(SearchHit, String)> = Vec::new();

        if let Ok(fc_matches) = archive.find_fact_checks(&claim.text_bg).await {
            for m in fc_matches {
                let hit = SearchHit {
                    url: m.url.clone(),
                    title: m.title.clone(),
                    snippet: m.summary.clone(),
                    source_name: Some(m.archive_name.clone()),
                    source_kind: "factcheck".to_string(),
                    published_at: m.published_at.clone(),
                };
                let body = ranker.fetch_html_cached(&m.url).await.unwrap_or_default();
                let main_txt = ranker.extract_main_text(&body);
                candidate_hits.push((hit, main_txt));
            }
        }

        let mut primary_domains: Vec<String> = Vec::new();
        for p in &primary_entries {
            primary_domains.push(p.domain.clone());
        }

        let claim_q = if !claim.attribution.is_empty() {
            let first = &claim.attribution[0];
            format!("{} {}", first.actor, first.about)
        } else {
            claim.text_bg.clone()
        };

        let primary_query = SearchQuery {
            query: claim_q.clone(),
            restrict_domains: primary_domains,
            date_from: None,
            date_to: None,
            max_results: 6,
        };
        if let Ok(p_hits) = search_engine.search(&primary_query).await {
            for hit in p_hits {
                let body = ranker.fetch_html_cached(&hit.url).await.unwrap_or_default();
                let main_txt = ranker.extract_main_text(&body);
                candidate_hits.push((hit, main_txt));
            }
        }

        let web_query = SearchQuery {
            query: claim_q,
            restrict_domains: Vec::new(),
            date_from: None,
            date_to: None,
            max_results: 6,
        };
        if let Ok(w_hits) = search_engine.search(&web_query).await {
            for hit in w_hits {
                let body = ranker.fetch_html_cached(&hit.url).await.unwrap_or_default();
                let main_txt = ranker.extract_main_text(&body);
                candidate_hits.push((hit, main_txt));
            }
        }

        let top_passages = ranker.rank_passages(
            &claim.text_bg,
            &candidate_hits,
            cfg.verification.passages_per_claim,
        );

        let prompt_path = self.paths.prompts_dir.join("verify_v1.txt");
        let system_prompt = std::fs::read_to_string(&prompt_path).unwrap_or_else(|_| {
            DEFAULT_VERIFY_SYSTEM_PROMPT.to_string()
        });

        let (verdict, label, stored_evidence, reasoning, strength, forced_reason) =
            if top_passages.is_empty() {
                let dummy = RawVerificationLlmOutput {
                    claim_id: claim.id,
                    verdict: "UNVERIFIABLE".to_string(),
                    label: "unknown".to_string(),
                    evidence: Vec::new(),
                    reasoning_bg: "Няма намерени пасажи".to_string(),
                    evidence_strength: 0.0,
                };
                enforce_verification_rules(
                    &claim,
                    &dummy,
                    &[],
                    cfg.verification.min_independent_news_outlets,
                )
            } else {
                let user_prompt = build_verify_user_prompt(
                    &claim,
                    &top_passages,
                    video.posted_at.as_deref(),
                );
                let runner = LlmChainRunner::from_runtime(&self.paths, &cfg, &self.db).await;
                let llm_req = LlmJsonRequest {
                    system_prompt,
                    user_prompt,
                    json_schema: Some(verification_output_json_schema()),
                    temperature: 0.0,
                    max_tokens: 1536,
                };
                let validated = runner.complete_validated(&llm_req, &self.db).await?;
                let raw_parsed: RawVerificationLlmOutput =
                    serde_json::from_value(validated.response.parsed_json)?;
                enforce_verification_rules(
                    &claim,
                    &raw_parsed,
                    &top_passages,
                    cfg.verification.min_independent_news_outlets,
                )
            };

        self.db
            .update_claim_verification(
                claim.id,
                &verdict,
                &label,
                &reasoning,
                strength,
                &stored_evidence,
            )
            .await?;
        self.check_and_mark_video_verified(video_id).await?;

        Ok(VerificationStageReport {
            video_id: video_id.to_string(),
            claim_id: claim.id,
            claim_text: claim.text_bg,
            verdict,
            label,
            reasoning_bg: reasoning,
            evidence_strength: strength,
            passages_count: top_passages.len(),
            stored_evidence_count: stored_evidence.len(),
            forced_reason,
        })
    }

    pub async fn run_claims_job(&self, video_id: &str) -> Result<()> {
        self.extract_claims_for_video(video_id).await?;
        self.db.enqueue_job(Some(video_id), Stage::Verify).await?;
        Ok(())
    }

    pub async fn run_verify_job(&self, video_id: &str) -> Result<()> {
        let claims = self.db.get_claims_for_video(video_id).await?;
        if claims.is_empty() {
            self.db
                .set_video_state(video_id, VideoState::Verified, None)
                .await?;
            return Ok(());
        }
        for claim in claims {
            if claim.verdict.is_none() {
                self.verify_claim_with_retrieval(video_id, claim.id).await?;
            }
        }
        let _ = self.rank_all().await;
        Ok(())
    }

    async fn check_and_mark_video_verified(&self, video_id: &str) -> Result<()> {
        let claims = self.db.get_claims_for_video(video_id).await?;
        let all_verified = !claims.is_empty() && claims.iter().all(|c| c.verdict.is_some());
        if all_verified {
            self.db
                .set_video_state(video_id, VideoState::Verified, None)
                .await?;
            let v = self.db.get_video(video_id).await?.unwrap();
            self.emit(CoreEvent::VideoUpdated {
                video_id: video_id.to_string(),
                state: VideoState::Verified.as_str().to_string(),
                lane: v.lane,
            });
        }
        Ok(())
    }
}
