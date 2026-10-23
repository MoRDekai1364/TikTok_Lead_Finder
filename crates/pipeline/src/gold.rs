use std::collections::HashSet;
use std::path::Path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use leadfinder_core::{AttributionAct, Claim, Evidence, LlmJsonRequest};
use leadfinder_providers::{LlmChainRunner, RankedPassage};
use crate::engine::PipelineContext;
use crate::verify::{
    build_verify_user_prompt, enforce_verification_rules, verification_output_json_schema,
    RawVerificationLlmOutput,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldSetFixture {
    #[serde(default)]
    pub synthetic: bool,
    #[serde(default)]
    pub notice: String,
    pub total_claims: usize,
    pub claims: Vec<GoldSetClaimItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldSetClaimItem {
    pub id: i64,
    pub claim_text: String,
    pub speaker: Option<String>,
    pub claim_type: String,
    #[serde(default)]
    pub attribution: Vec<AttributionAct>,
    pub expected_verdict: String,
    pub expected_label: String,
    pub is_trap: bool,
    pub trap_category: Option<String>,
    pub date_window: Option<String>,
    pub passages: Vec<RankedPassageFixture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedPassageFixture {
    pub passage_id: String,
    pub source_url: String,
    pub source_name: Option<String>,
    pub source_kind: String,
    pub published_at: Option<String>,
    pub text: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldSetSingleEval {
    pub id: i64,
    pub claim_text: String,
    pub expected_verdict: String,
    pub actual_verdict: String,
    pub expected_label: String,
    pub actual_label: String,
    pub is_trap: bool,
    pub trap_category: Option<String>,
    pub trap_passed: bool,
    pub llm_called: bool,
    pub first_try_valid: Option<bool>,
    pub forced_reason: Option<String>,
    pub rules_compliant: bool,
    pub evidence_kinds: Vec<String>,
    pub reasoning_bg: String,
    pub evidence_strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldSetEvalResult {
    pub synthetic_fixture: bool,
    pub total_claims: usize,
    pub traps_total: usize,
    pub traps_supported_count: usize,
    pub traps_pass_count: usize,
    pub traps_pass_rate: f64,
    pub verdict_agreement_count: usize,
    pub verdict_agreement_rate: f64,
    pub contradicted_total: usize,
    pub contradicted_recalled: usize,
    pub contradicted_recall: f64,
    pub forcing_rules_compliant_count: usize,
    pub forcing_rules_compliance_rate: f64,
    pub llm_calls: usize,
    pub first_try_valid_count: usize,
    pub first_try_validity_rate: f64,
    pub disinformation_label_count: usize,
    pub results: Vec<GoldSetSingleEval>,
}

pub fn rules_hold(verdict: &str, label: &str, evidence: &[Evidence], min_news: usize) -> bool {
    if label.eq_ignore_ascii_case("disinformation") {
        return false;
    }
    match verdict {
        "SUPPORTED" | "CONTRADICTED" => {
            let strong = evidence
                .iter()
                .any(|e| e.source_kind == "primary" || e.source_kind == "factcheck");
            let outlets: HashSet<String> = evidence
                .iter()
                .filter(|e| e.source_kind == "news")
                .filter_map(|e| {
                    url::Url::parse(&e.source_url)
                        .ok()
                        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
                })
                .collect();
            strong || outlets.len() >= min_news
        }
        "ALREADY_CHECKED" => evidence.iter().any(|e| e.source_kind == "factcheck"),
        "UNVERIFIABLE" | "NOT_CHECKABLE" => true,
        _ => false,
    }
}

pub async fn run_gold_set_eval(
    ctx: &PipelineContext,
    fixture_path: &Path,
) -> Result<GoldSetEvalResult> {
    let content = std::fs::read_to_string(fixture_path)?;
    let fixture: GoldSetFixture = serde_json::from_str(content.trim_start_matches('\u{feff}'))?;

    let cfg = ctx.config.read().await.clone();
    let min_news = cfg.verification.min_independent_news_outlets;
    let runner = LlmChainRunner::from_runtime(&ctx.paths, &cfg, &ctx.db).await;
    let prompt_path = ctx.paths.prompts_dir.join("verify_v1.txt");
    let system_prompt = std::fs::read_to_string(&prompt_path).unwrap_or_else(|_| {
        "You are a retrieval-grounded Bulgarian fact-checking assistant. Return strictly valid JSON."
            .to_string()
    });

    let mut rows: Vec<GoldSetSingleEval> = Vec::new();
    let mut llm_calls = 0usize;
    let mut first_try_valid = 0usize;

    for item in &fixture.claims {
        let claim = Claim {
            id: item.id,
            video_id: format!("gold_{}", item.id),
            text_bg: item.claim_text.clone(),
            claim_type: item.claim_type.clone(),
            speaker: item.speaker.clone(),
            attribution: item.attribution.clone(),
            start_ms: Some(0),
            end_ms: Some(5000),
            checkworthiness: Some(0.8),
            verdict: None,
            label: None,
            reasoning_bg: None,
            evidence_strength: None,
            verified_at: None,
            cluster_id: None,
        };

        let passages: Vec<RankedPassage> = item
            .passages
            .iter()
            .map(|p| RankedPassage {
                passage_id: p.passage_id.clone(),
                source_url: p.source_url.clone(),
                source_name: p.source_name.clone(),
                source_kind: p.source_kind.clone(),
                published_at: p.published_at.clone(),
                text: p.text.clone(),
                score: p.score,
            })
            .collect();

        let needs_llm = claim.claim_type != "opinion" && !passages.is_empty();

        let (raw, first_try) = if needs_llm {
            let req = LlmJsonRequest {
                system_prompt: system_prompt.clone(),
                user_prompt: build_verify_user_prompt(
                    &claim,
                    &passages,
                    item.date_window.as_deref(),
                ),
                json_schema: Some(verification_output_json_schema()),
                temperature: 0.0,
                max_tokens: 1536,
            };
            llm_calls += 1;
            match runner.complete_validated(&req, &ctx.db).await {
                Ok(v) => {
                    if v.first_try_valid {
                        first_try_valid += 1;
                    }
                    let parsed: RawVerificationLlmOutput =
                        serde_json::from_value(v.response.parsed_json)?;
                    (parsed, Some(v.first_try_valid))
                }
                Err(_) => (
                    RawVerificationLlmOutput {
                        claim_id: claim.id,
                        verdict: "UNVERIFIABLE".to_string(),
                        label: "unknown".to_string(),
                        evidence: Vec::new(),
                        reasoning_bg: "Невалиден изход от модела след повторен опит.".to_string(),
                        evidence_strength: 0.0,
                    },
                    Some(false),
                ),
            }
        } else {
            (
                RawVerificationLlmOutput {
                    claim_id: claim.id,
                    verdict: "UNVERIFIABLE".to_string(),
                    label: "unknown".to_string(),
                    evidence: Vec::new(),
                    reasoning_bg: String::new(),
                    evidence_strength: 0.0,
                },
                None,
            )
        };

        let (verdict, label, evidence, reasoning, strength, forced) =
            enforce_verification_rules(&claim, &raw, &passages, min_news);
        let compliant = rules_hold(&verdict, &label, &evidence, min_news);
        let agree = verdict == item.expected_verdict;
        let trap_passed = if item.is_trap {
            verdict != "SUPPORTED" && agree
        } else {
            true
        };

        rows.push(GoldSetSingleEval {
            id: item.id,
            claim_text: item.claim_text.clone(),
            expected_verdict: item.expected_verdict.clone(),
            actual_verdict: verdict,
            expected_label: item.expected_label.clone(),
            actual_label: label,
            is_trap: item.is_trap,
            trap_category: item.trap_category.clone(),
            trap_passed,
            llm_called: needs_llm,
            first_try_valid: first_try,
            forced_reason: forced,
            rules_compliant: compliant,
            evidence_kinds: evidence.iter().map(|e| e.source_kind.clone()).collect(),
            reasoning_bg: reasoning,
            evidence_strength: strength,
        });
    }

    let n = rows.len();
    let ratio = |a: usize, b: usize| -> f64 {
        if b == 0 {
            1.0
        } else {
            ((a as f64 / b as f64) * 1000.0).round() / 1000.0
        }
    };
    let traps_total = rows.iter().filter(|r| r.is_trap).count();
    let traps_pass = rows.iter().filter(|r| r.is_trap && r.trap_passed).count();
    let traps_supported = rows
        .iter()
        .filter(|r| r.is_trap && r.actual_verdict == "SUPPORTED")
        .count();
    let agree = rows
        .iter()
        .filter(|r| r.actual_verdict == r.expected_verdict)
        .count();
    let contradicted_total = rows
        .iter()
        .filter(|r| r.expected_verdict == "CONTRADICTED")
        .count();
    let contradicted_recalled = rows
        .iter()
        .filter(|r| r.expected_verdict == "CONTRADICTED" && r.actual_verdict == "CONTRADICTED")
        .count();
    let compliant = rows.iter().filter(|r| r.rules_compliant).count();
    let disinfo = rows
        .iter()
        .filter(|r| r.actual_label.eq_ignore_ascii_case("disinformation"))
        .count();

    Ok(GoldSetEvalResult {
        synthetic_fixture: fixture.synthetic,
        total_claims: n,
        traps_total,
        traps_supported_count: traps_supported,
        traps_pass_count: traps_pass,
        traps_pass_rate: ratio(traps_pass, traps_total),
        verdict_agreement_count: agree,
        verdict_agreement_rate: ratio(agree, n),
        contradicted_total,
        contradicted_recalled,
        contradicted_recall: ratio(contradicted_recalled, contradicted_total),
        forcing_rules_compliant_count: compliant,
        forcing_rules_compliance_rate: ratio(compliant, n),
        llm_calls,
        first_try_valid_count: first_try_valid,
        first_try_validity_rate: ratio(first_try_valid, llm_calls),
        disinformation_label_count: disinfo,
        results: rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::RawEvidenceRef;

    fn claim(kind: &str) -> Claim {
        Claim {
            id: 1,
            video_id: "v".to_string(),
            text_bg: "тестово твърдение".to_string(),
            claim_type: kind.to_string(),
            speaker: None,
            attribution: Vec::new(),
            start_ms: Some(0),
            end_ms: Some(1),
            checkworthiness: Some(0.9),
            verdict: None,
            label: None,
            reasoning_bg: None,
            evidence_strength: None,
            verified_at: None,
            cluster_id: None,
        }
    }

    fn passage(id: &str, kind: &str, host: &str) -> RankedPassage {
        RankedPassage {
            passage_id: id.to_string(),
            source_url: format!("https://{host}/a"),
            source_name: Some(host.to_string()),
            source_kind: kind.to_string(),
            published_at: None,
            text: "пасаж".to_string(),
            score: 1.0,
        }
    }

    fn raw(verdict: &str, ids: &[&str]) -> RawVerificationLlmOutput {
        RawVerificationLlmOutput {
            claim_id: 1,
            verdict: verdict.to_string(),
            label: "false_claim".to_string(),
            evidence: ids
                .iter()
                .map(|i| RawEvidenceRef {
                    passage_id: i.to_string(),
                    stance: "contradicts".to_string(),
                })
                .collect(),
            reasoning_bg: "обяснение".to_string(),
            evidence_strength: 0.9,
        }
    }

    #[test]
    fn single_news_outlet_cannot_support_or_contradict() {
        let ps = vec![passage("p1", "news", "a.example.invalid")];
        let (v, _, _, _, _, f) = enforce_verification_rules(&claim("factual"), &raw("CONTRADICTED", &["p1"]), &ps, 2);
        assert_eq!(v, "UNVERIFIABLE");
        assert!(f.is_some());
    }

    #[test]
    fn two_independent_outlets_or_one_primary_allow_verdict() {
        let two = vec![
            passage("p1", "news", "a.example.invalid"),
            passage("p2", "news", "b.example.invalid"),
        ];
        let (v, ..) = enforce_verification_rules(&claim("factual"), &raw("CONTRADICTED", &["p1", "p2"]), &two, 2);
        assert_eq!(v, "CONTRADICTED");
        let same_host = vec![
            passage("p1", "news", "a.example.invalid"),
            passage("p2", "news", "a.example.invalid"),
        ];
        let (v, ..) = enforce_verification_rules(&claim("factual"), &raw("CONTRADICTED", &["p1", "p2"]), &same_host, 2);
        assert_eq!(v, "UNVERIFIABLE");
        let primary = vec![passage("p1", "primary", "c.example.invalid")];
        let (v, ..) = enforce_verification_rules(&claim("factual"), &raw("SUPPORTED", &["p1"]), &primary, 2);
        assert_eq!(v, "SUPPORTED");
    }

    #[test]
    fn unknown_passage_ids_and_empty_retrieval_force_unverifiable() {
        let ps = vec![passage("p1", "primary", "c.example.invalid")];
        let (v, ..) = enforce_verification_rules(&claim("factual"), &raw("CONTRADICTED", &["p9"]), &ps, 2);
        assert_eq!(v, "UNVERIFIABLE");
        let (v, _, ev, ..) = enforce_verification_rules(&claim("factual"), &raw("CONTRADICTED", &["p1"]), &[], 2);
        assert_eq!(v, "UNVERIFIABLE");
        assert!(ev.is_empty());
    }

    #[test]
    fn already_checked_requires_factcheck_link_and_opinion_is_not_checkable() {
        let primary = vec![passage("p1", "primary", "c.example.invalid")];
        let (v, ..) = enforce_verification_rules(&claim("factual"), &raw("ALREADY_CHECKED", &["p1"]), &primary, 2);
        assert_eq!(v, "UNVERIFIABLE");
        let fc = vec![passage("p1", "factcheck", "d.example.invalid")];
        let (v, _, ev, ..) = enforce_verification_rules(&claim("factual"), &raw("ALREADY_CHECKED", &["p1"]), &fc, 2);
        assert_eq!(v, "ALREADY_CHECKED");
        assert_eq!(ev[0].source_url, "https://d.example.invalid/a");
        let (v, ..) = enforce_verification_rules(&claim("opinion"), &raw("SUPPORTED", &["p1"]), &fc, 2);
        assert_eq!(v, "NOT_CHECKABLE");
    }

    #[test]
    fn disinformation_label_is_never_emitted() {
        let fc = vec![passage("p1", "factcheck", "d.example.invalid")];
        let mut r = raw("CONTRADICTED", &["p1"]);
        r.label = "disinformation".to_string();
        let (_, label, ..) = enforce_verification_rules(&claim("factual"), &r, &fc, 2);
        assert_ne!(label, "disinformation");
        assert!(!rules_hold("CONTRADICTED", "disinformation", &[], 2));
    }
}
