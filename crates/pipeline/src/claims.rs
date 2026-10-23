use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use leadfinder_core::{
    AppConfig, AttributionAct, Claim, ClaimInput, ClaimsOutput, CoreEvent, LlmJsonRequest,
    RankingConfig, Transcript, Video, VideoState,
};
use leadfinder_providers::{
    global_lexicon, LlmChainRunner, PeopleResolver, ResolvedPersonRecord, VideoTextSources,
};
use crate::engine::PipelineContext;

const DEFAULT_SYSTEM_INSTRUCTION: &str = "Extract verifiable claims and named real people from the provided video transcript, OCR text, caption, and hashtags. Return strictly valid JSON with claims and people arrays.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimsStageReport {
    pub video_id: String,
    pub provider: String,
    pub model: String,
    pub first_try_valid: bool,
    pub repair_used: bool,
    pub extracted_claims_count: usize,
    pub checkworthy_claims_count: usize,
    pub opinion_claims_count: usize,
    pub attribution_claims_count: usize,
    pub resolved_people_count: usize,
    pub public_figures_count: usize,
    pub claims: Vec<Claim>,
    pub people: Vec<ResolvedPersonRecord>,
}

pub fn claims_output_json_schema() -> Value {
    json!({
        "type": "object",
        "required": ["claims", "people"],
        "additionalProperties": false,
        "properties": {
            "claims": {
                "type": "array",
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "required": ["text_bg", "claim_type", "speaker", "attribution", "start_ms", "end_ms", "checkworthiness"],
                    "additionalProperties": false,
                    "properties": {
                        "text_bg": { "type": "string", "minLength": 3 },
                        "claim_type": {
                            "type": "string",
                            "enum": ["factual", "attribution", "statistical", "health", "quote", "prediction", "opinion"]
                        },
                        "speaker": { "type": ["string", "null"] },
                        "attribution": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["actor", "act", "target", "about"],
                                "additionalProperties": false,
                                "properties": {
                                    "actor": { "type": "string", "minLength": 1 },
                                    "act": {
                                        "type": "string",
                                        "enum": ["said", "agreed", "proposed", "voted", "denied", "other"]
                                    },
                                    "target": { "type": ["string", "null"] },
                                    "about": { "type": "string", "minLength": 1 }
                                }
                            }
                        },
                        "start_ms": { "type": "integer", "minimum": 0 },
                        "end_ms": { "type": "integer", "minimum": 0 },
                        "checkworthiness": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                    }
                }
            },
            "people": {
                "type": "array",
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "required": ["name", "variants", "role_hint"],
                    "additionalProperties": false,
                    "properties": {
                        "name": { "type": "string", "minLength": 2 },
                        "variants": { "type": "array", "items": { "type": "string" } },
                        "role_hint": { "type": ["string", "null"] }
                    }
                }
            }
        }
    })
}

pub fn compute_combined_checkworthiness(
    llm_score: f64,
    claim_text: &str,
    claim_type: &str,
    full_video_text: &str,
    has_public_figure_in_claim: bool,
    ranking: &RankingConfig,
) -> f64 {
    if claim_type == "opinion" {
        return ((llm_score.clamp(0.0, 1.0) * 0.35).min(ranking.checkworthy_threshold * 0.75)
            * 1000.0)
            .round()
            / 1000.0;
    }

    let claim_low = claim_text.to_lowercase();
    let video_low = full_video_text.to_lowercase();

    let contains_numbers = if claim_text.chars().any(|c| c.is_ascii_digit()) {
        1.0
    } else {
        0.0
    };

    let public_fig_feat = if has_public_figure_in_claim { 1.0 } else { 0.0 };

    let sensitive_feat = if claim_type == "health"
        || ranking
            .sensitive_topic_terms
            .iter()
            .any(|t| !t.is_empty() && claim_low.contains(&t.to_lowercase()))
    {
        1.0
    } else {
        0.0
    };

    let share_urge_feat = if ranking.share_urge_terms.iter().any(|t| {
        !t.is_empty()
            && (video_low.contains(&t.to_lowercase()) || claim_low.contains(&t.to_lowercase()))
    }) {
        1.0
    } else {
        0.0
    };

    let raw = ranking.weight_llm_score * llm_score.clamp(0.0, 1.0)
        + ranking.weight_contains_numbers * contains_numbers
        + ranking.weight_public_figure * public_fig_feat
        + ranking.weight_sensitive_topic * sensitive_feat
        + ranking.weight_urges_sharing * share_urge_feat;

    ((raw.clamp(0.0, 1.0)) * 1000.0).round() / 1000.0
}

pub fn split_and_normalize_attribution(
    input: &mut ClaimInput,
    speaker_fallback: Option<&str>,
    attribution_verbs: &[(String, String)],
) {
    let speaker_label = input
        .speaker
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .or(speaker_fallback)
        .unwrap_or("speaker");

    if input.attribution.is_empty() {
        if let Some(inferred) = detect_attribution_in_sentence(&input.text_bg, attribution_verbs) {
            input.attribution.push(inferred);
            if matches!(
                input.claim_type.as_str(),
                "factual" | "statistical" | "prediction"
            ) {
                input.claim_type = leadfinder_core::ClaimType::Attribution;
            }
        }
    }

    if !input.attribution.is_empty()
        && !matches!(input.claim_type.as_str(), "attribution" | "quote" | "opinion")
    {
        input.claim_type = leadfinder_core::ClaimType::Attribution;
    }

    for act_item in &mut input.attribution {
        if act_item.act == "said" || act_item.act == "other" {
            if let Some(inferred) = detect_attribution_in_sentence(&input.text_bg, attribution_verbs) {
                if inferred.act != "said" && inferred.act != "other" {
                    act_item.act = inferred.act;
                }
            }
        }
        if !matches!(
            act_item.act.as_str(),
            "said" | "agreed" | "proposed" | "voted" | "denied" | "other"
        ) {
            act_item.act = "other".to_string();
        }
        let target_clause = match act_item.target.as_deref() {
            Some(t) if !t.trim().is_empty() => format!(" -> {t}"),
            _ => String::new(),
        };
        let effective_speaker = if input.speaker.as_deref() == Some(act_item.actor.as_str()) {
            speaker_fallback.unwrap_or(speaker_label)
        } else {
            speaker_label
        };
        act_item.asserted_part = Some(format!(
            "[transcript {}:{}..{}ms] {}",
            effective_speaker, input.start_ms, input.end_ms, input.text_bg
        ));
        act_item.attributed_part = Some(format!(
            "{} ({}{}): {}",
            act_item.actor.trim(),
            act_item.act,
            target_clause,
            act_item.about.trim()
        ));
    }
}

pub fn detect_attribution_in_sentence(
    text: &str,
    attribution_verbs: &[(String, String)],
) -> Option<AttributionAct> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let low = trimmed.to_lowercase();

    for (phrase, act_code) in attribution_verbs {
        let p_low = phrase.trim().to_lowercase();
        if p_low.is_empty() {
            continue;
        }
        if let Some(byte_pos) = find_whole_phrase(&low, &p_low) {
            let before = trimmed.get(..byte_pos).unwrap_or("").trim();
            let after_start = byte_pos + p_low.len();
            let after = trimmed
                .get(after_start..)
                .unwrap_or("")
                .trim()
                .trim_start_matches(|c: char| matches!(c, ',' | ':' | '-' | '—' | '"' | '„' | '“'))
                .trim();

            let actor = extract_trailing_subject(before);
            if actor.chars().count() >= 2 && after.chars().count() >= 4 {
                return Some(AttributionAct {
                    actor,
                    act: act_code.clone(),
                    target: None,
                    about: after.trim_end_matches('.').to_string(),
                    asserted_part: None,
                    attributed_part: None,
                });
            }
        }
    }
    None
}

fn find_whole_phrase(haystack_low: &str, phrase_low: &str) -> Option<usize> {
    let mut offset = 0usize;
    while let Some(rel) = haystack_low[offset..].find(phrase_low) {
        let pos = offset + rel;
        let left_ok = pos == 0
            || haystack_low[..pos]
                .chars()
                .next_back()
                .map(|c| !c.is_alphabetic())
                .unwrap_or(true);
        let end = pos + phrase_low.len();
        let right_ok = end >= haystack_low.len()
            || haystack_low[end..]
                .chars()
                .next()
                .map(|c| !c.is_alphabetic())
                .unwrap_or(true);
        if left_ok && right_ok {
            return Some(pos);
        }
        offset = pos
            + phrase_low
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
    }
    None
}

fn extract_trailing_subject(before: &str) -> String {
    let clause = before
        .rsplit(|c: char| matches!(c, '.' | '!' | '?' | ';' | ':'))
        .next()
        .unwrap_or(before)
        .trim();
    let words: Vec<&str> = clause.split_whitespace().collect();
    let take_n = words.len().min(5);
    words[words.len().saturating_sub(take_n)..]
        .join(" ")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string()
}

pub fn build_claims_user_prompt(
    video: &Video,
    transcript: Option<&Transcript>,
    ocr_lines: &[(i64, String)],
    linguistic_notes: &str,
) -> String {
    let mut sections = Vec::new();

    sections.push(format!(
        "VIDEO_METADATA:\n- id: {}\n- creator: @{} ({})\n- posted_at: {}\n- caption: {}\n- hashtags: {}",
        video.id,
        video.creator_handle.as_deref().unwrap_or("unknown"),
        video.creator_display_name.as_deref().unwrap_or(""),
        video.posted_at.as_deref().unwrap_or("unknown"),
        video.caption.as_deref().unwrap_or(""),
        video.hashtags.join(", ")
    ));

    if let Some(tr) = transcript {
        if let Some(ref edited) = tr.edited_text {
            if !edited.trim().is_empty() {
                let end_ms = (video.duration_s.unwrap_or(30.0) * 1000.0) as i64;
                sections.push(format!(
                    "EDITED_TRANSCRIPT [0-{end_ms} ms]:\n{}",
                    edited.trim()
                ));
            }
        } else if !tr.segments.is_empty() {
            let mut seg_lines = Vec::with_capacity(tr.segments.len());
            for seg in &tr.segments {
                seg_lines.push(format!(
                    "[{}-{} ms] {}",
                    seg.start_ms, seg.end_ms, seg.text
                ));
            }
            sections.push(format!("TRANSCRIPT_SEGMENTS:\n{}", seg_lines.join("\n")));
        } else if !tr.text.trim().is_empty() {
            let end_ms = (video.duration_s.unwrap_or(30.0) * 1000.0) as i64;
            sections.push(format!("TRANSCRIPT [0-{end_ms} ms]:\n{}", tr.text.trim()));
        }
    }

    if !ocr_lines.is_empty() {
        let mut olines = Vec::with_capacity(ocr_lines.len());
        for (ms, txt) in ocr_lines.iter().take(25) {
            olines.push(format!("[OCR {ms} ms] {txt}"));
        }
        sections.push(format!("OCR_LINES:\n{}", olines.join("\n")));
    }

    if !linguistic_notes.trim().is_empty() {
        sections.push(format!("LEXICON_NOTES:\n{}", linguistic_notes.trim()));
    }

    sections.join("\n\n")
}

impl PipelineContext {
    pub async fn extract_claims_for_video(&self, video_id: &str) -> Result<ClaimsStageReport> {
        let video = self
            .db
            .get_video(video_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("video not found: {video_id}"))?;
        let transcript = self.db.get_transcript(video_id).await?;
        let ocr_rows = self.db.get_ocr_texts(video_id).await?;

        let cfg: AppConfig = self.config.read().await.clone();
        let lex = global_lexicon(&self.paths, &cfg.normalisation.lexicon_config_file);

        let effective_transcript_text = transcript
            .as_ref()
            .and_then(|t| t.edited_text.clone().filter(|s| !s.trim().is_empty()))
            .or_else(|| transcript.as_ref().map(|t| t.text.clone()))
            .unwrap_or_default();

        let ocr_joined = ocr_rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let full_text_blob = format!(
            "{}\n{}\n{}\n{}",
            effective_transcript_text,
            ocr_joined,
            video.caption.as_deref().unwrap_or(""),
            video.hashtags.join(" ")
        );

        let ling_ctx = lex.analyze_for_llm(&full_text_blob);
        let ocr_pairs: Vec<(i64, String)> = ocr_rows
            .iter()
            .map(|r| (r.frame_ms, r.text.clone()))
            .collect();

        let prompt_path = self.paths.prompts_dir.join("claims_v1.txt");
        let system_prompt = std::fs::read_to_string(&prompt_path)
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SYSTEM_INSTRUCTION.to_string());
        let user_prompt = build_claims_user_prompt(
            &video,
            transcript.as_ref(),
            &ocr_pairs,
            &ling_ctx.prompt_notes,
        );

        let runner = LlmChainRunner::from_runtime(&self.paths, &cfg, &self.db).await;
        let llm_req = LlmJsonRequest {
            system_prompt,
            user_prompt,
            json_schema: Some(claims_output_json_schema()),
            temperature: 0.0,
            max_tokens: 2560,
        };

        let validated = match runner.complete_validated(&llm_req, &self.db).await {
            Ok(v) => v,
            Err(err) => {
                self.db
                    .set_video_state(&video.id, VideoState::Failed, Some(&err.to_string()))
                    .await?;
                return Err(anyhow::anyhow!(err));
            }
        };

        let mut parsed_output: ClaimsOutput =
            serde_json::from_value(validated.response.parsed_json.clone()).map_err(|e| {
                anyhow::anyhow!("failed deserializing ClaimsOutput after validation: {e}")
            })?;

        let sources = VideoTextSources {
            transcript: effective_transcript_text.clone(),
            ocr: ocr_joined.clone(),
            caption: video.caption.clone().unwrap_or_default(),
            hashtags: video.hashtags.clone(),
            creator_handle: video.creator_handle.clone(),
            creator_display_name: video.creator_display_name.clone(),
        };

        let people_resolver =
            PeopleResolver::new(cfg.normalisation.clone(), lex.clone(), self.guard.clone());
        let resolved_people = people_resolver
            .extract_and_resolve_people(&parsed_output.people, &sources)
            .await?;

        self.db.clear_people_for_video(&video.id).await?;
        for person in &resolved_people {
            self.db
                .upsert_person_for_video(
                    &video.id,
                    &person.canonical_name,
                    &person.variants,
                    person.role.as_deref(),
                    person.public_figure,
                    person.wikidata_id.as_deref(),
                    person.summary.as_deref(),
                    person.mentions,
                    &person.found_in,
                )
                .await?;
        }

        let attr_verbs = lex.attribution_verbs();
        let max_video_ms = ((video.duration_s.unwrap_or(60.0) * 1000.0) as i64).max(1000);
        let mut claims_to_save = Vec::with_capacity(parsed_output.claims.len());

        for item in &mut parsed_output.claims {
            if item.text_bg.trim().is_empty() {
                continue;
            }
            if item.end_ms < item.start_ms {
                item.end_ms = item.start_ms;
            }
            if item.start_ms > max_video_ms {
                item.start_ms = 0;
            }

            split_and_normalize_attribution(
                item,
                video.creator_handle.as_deref(),
                &attr_verbs,
            );

            let claim_low = item.text_bg.to_lowercase();
            let mentions_pub_fig = resolved_people.iter().any(|p| {
                p.public_figure
                    && p.variants
                        .iter()
                        .any(|v| v.chars().count() >= 4 && claim_low.contains(&v.to_lowercase()))
            });

            let ctype_str = item.claim_type.as_str().to_string();
            let final_checkworthiness = compute_combined_checkworthiness(
                item.checkworthiness,
                &item.text_bg,
                &ctype_str,
                &full_text_blob,
                mentions_pub_fig,
                &cfg.ranking,
            );

            claims_to_save.push(Claim {
                id: 0,
                video_id: video.id.clone(),
                text_bg: item.text_bg.trim().to_string(),
                claim_type: ctype_str,
                speaker: item.speaker.clone(),
                attribution: item.attribution.clone(),
                start_ms: Some(item.start_ms),
                end_ms: Some(item.end_ms),
                checkworthiness: Some(final_checkworthiness),
                verdict: None,
                label: None,
                reasoning_bg: None,
                evidence_strength: None,
                verified_at: None,
                cluster_id: None,
            });
        }

        let saved_claims = self
            .db
            .replace_claims_for_video(&video.id, &claims_to_save)
            .await?;

        let checkworthy_count = saved_claims
            .iter()
            .filter(|c| {
                c.claim_type != "opinion"
                    && c.checkworthiness.unwrap_or(0.0) >= cfg.ranking.checkworthy_threshold
            })
            .count();
        let opinion_count = saved_claims
            .iter()
            .filter(|c| c.claim_type == "opinion")
            .count();
        let attribution_count = saved_claims
            .iter()
            .filter(|c| !c.attribution.is_empty() || c.claim_type == "attribution")
            .count();
        let public_fig_count = resolved_people.iter().filter(|p| p.public_figure).count();

        self.db
            .set_video_state(&video.id, VideoState::ClaimsDone, None)
            .await?;
        self.emit(CoreEvent::VideoUpdated {
            video_id: video.id.clone(),
            state: VideoState::ClaimsDone.as_str().to_string(),
            lane: video.lane.clone(),
        });

        Ok(ClaimsStageReport {
            video_id: video.id,
            provider: validated.response.provider,
            model: validated.response.model,
            first_try_valid: validated.first_try_valid,
            repair_used: validated.repair_used,
            extracted_claims_count: saved_claims.len(),
            checkworthy_claims_count: checkworthy_count,
            opinion_claims_count: opinion_count,
            attribution_claims_count: attribution_count,
            resolved_people_count: resolved_people.len(),
            public_figures_count: public_fig_count,
            claims: saved_claims,
            people: resolved_people,
        })
    }
}
