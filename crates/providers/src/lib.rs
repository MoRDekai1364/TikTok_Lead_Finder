pub mod archive;
pub mod bootstrap;
pub mod discovery;
pub mod lexicon;
pub mod llm;
pub mod media;
pub mod ocr;
pub mod passages;
pub mod people;
pub mod search;
pub mod transcriber;
pub mod ytdlp;

pub use archive::*;
pub use bootstrap::*;
pub use discovery::*;
pub use lexicon::*;
pub use llm::*;
pub use media::*;
pub use ocr::*;
pub use passages::*;
pub use people::*;
pub use search::*;
pub use transcriber::*;
pub use ytdlp::*;

pub fn crate_ready() -> &'static str {
    leadfinder_core::crate_ready()
}

#[cfg(test)]
mod tests {
    use super::*;
    use leadfinder_core::{OcrLine, ProviderErrorKind};
    use serde_json::json;

    #[test]
    fn parses_tiktok_urls_short_links_and_bare_ids() {
        let sample = "Виж това https://www.tiktok.com/@bntnews/video/7345678901234567890 и https://vm.tiktok.com/ZMkAbCdEf/ както и 7399887766554433221";
        let urls = extract_tiktok_urls_from_text(sample);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://www.tiktok.com/@bntnews/video/7345678901234567890");
        assert_eq!(urls[1], "https://vm.tiktok.com/ZMkAbCdEf/");
        assert_eq!(urls[2], "https://www.tiktok.com/@tiktok/video/7399887766554433221");
    }

    #[test]
    fn parses_partner_csv_and_semicolon_exports() {
        let csv_data = "\u{feff}video_id;creator;url\n7311111111111111111;user1;https://www.tiktok.com/@user1/video/7311111111111111111\n7322222222222222222;user2;\n";
        let urls = PasteLinkSource::parse_partner_export_urls(csv_data);
        assert!(urls.iter().any(|u| u.contains("7311111111111111111")));
        assert!(urls.iter().any(|u| u.contains("7322222222222222222")));
    }

    #[test]
    fn classifies_ytdlp_stderr_taxonomy() {
        assert_eq!(
            YtDlpClient::classify_stderr("ERROR: HTTP Error 429: Too Many Requests").0,
            ProviderErrorKind::RateLimited
        );
        assert_eq!(
            YtDlpClient::classify_stderr("ERROR: [TikTok] Verify to continue captcha challenge").0,
            ProviderErrorKind::BlockedOrCaptcha
        );
        assert_eq!(
            YtDlpClient::classify_stderr("ERROR: Video is unavailable or status code 10204").0,
            ProviderErrorKind::NotFoundOrPrivate
        );
    }

    #[test]
    fn parses_ytdlp_metadata_json_with_cyrillic_hashtags_and_captions() {
        let raw = json!({
            "id": "7400112233445566778",
            "uploader": "fakt_bg",
            "uploader_id": "uid_99",
            "channel": "Факти БГ",
            "description": "Извънредно съобщение за еврото! #българия #еврозона #новини",
            "timestamp": 1761000000,
            "view_count": 48200,
            "like_count": 2100,
            "repost_count": 640,
            "comment_count": 310,
            "duration": 41.2,
            "subtitles": {
                "bg-BG": [
                    { "ext": "vtt", "url": "https://example.com/subs_bg.vtt" }
                ]
            }
        });

        let vref = YtDlpClient::parse_metadata_json(
            &raw,
            "https://www.tiktok.com/@fakt_bg/video/7400112233445566778",
            "paste",
        )
        .unwrap();
        assert_eq!(vref.id, "7400112233445566778");
        assert_eq!(vref.creator_handle.as_deref(), Some("fakt_bg"));
        assert_eq!(vref.views, Some(48200));
        assert!(vref.hashtags.contains(&"българия".to_string()));
        assert!(vref.hashtags.contains(&"еврозона".to_string()));
        assert_eq!(vref.subtitle_urls.len(), 1);
    }

    #[test]
    fn parses_vtt_and_computes_bulgarian_wer() {
        let vtt = "WEBVTT\n\n00:00:00.200 --> 00:00:03.500\n<c>Парламентът</c> прие новия бюджет.\n\n00:00:03.500 --> 00:00:06.000\nПарламентът прие новия бюджет.\n\n00:00:06.100 --> 00:00:09.000\nСпоред данните на НСИ.\n";
        let segs = parse_vtt_or_srt(vtt);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "Парламентът прие новия бюджет.");
        assert_eq!(segs[0].end_ms, 6000);

        let exact_wer = compute_wer(
            "Парламентът прие новия бюджет!",
            "парламентът прие новия бюджет",
        );
        assert_eq!(exact_wer, 0.0);

        let partial_wer = compute_wer(
            "Парламентът прие новия държавен бюджет",
            "Парламентът прие стария държавен бюджет",
        );
        assert!((partial_wer - 0.20).abs() < 1e-6);
    }

    #[test]
    fn deduplicates_near_identical_ocr_lines_across_frames() {
        let raw = vec![
            OcrLine {
                frame_ms: 0,
                text: "ИЗВЪНРЕДНО: НОВ ДАНЪК ОТ ЯНУАРИ".to_string(),
                confidence: 0.82,
            },
            OcrLine {
                frame_ms: 3000,
                text: "ИЗВЪНРЕДНО НОВ ДАНЪК ОТ ЯНУАРИ!".to_string(),
                confidence: 0.91,
            },
            OcrLine {
                frame_ms: 6000,
                text: "ГЛЕДАЙТЕ ДО КРАЯ".to_string(),
                confidence: 0.88,
            },
        ];
        let deduped = deduplicate_ocr_lines(raw, 0.85);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].frame_ms, 0);
        assert!((deduped[0].confidence - 0.91).abs() < 1e-6);
        assert_eq!(deduped[1].frame_ms, 6000);
    }

    #[test]
    fn lexicon_normalizes_asr_phonetics_grammar_and_builds_llm_notes() {
        let paths = leadfinder_core::RuntimePaths::discover().unwrap();
        let lex = LexiconNormalizer::load(&paths, "lexicon.bg.toml");
        let stats = lex.stats();
        assert!(stats.valid_wordforms > 100_000);
        assert!(stats.ipa_transcriptions > 40_000);

        let raw_asr = "Знаешь ли че господин Радеф влезна февро зоната и плащаме дересе за най ниските цени и евротосте за чушкъ?";
        let norm = lex.normalize_text(raw_asr);
        assert!(norm.contains("Знаеш"));
        assert!(norm.contains("Радев"));
        assert!(norm.contains("еврозоната"));
        assert!(norm.contains("ДДС"));
        assert!(norm.contains("най-ниските"));
        assert!(norm.contains("еврото сте"));
        assert!(norm.contains("чушка"));

        let llm_sample = "Кво прайш ве баце, щот неска у парламента бил приел нов закон за ДДС и НОИ.";
        let ctx = lex.analyze_for_llm(llm_sample);
        assert!(!ctx.glossary_matches.is_empty());
        assert!(!ctx.acronym_matches.is_empty());
        assert!(!ctx.renarrated_evidential_markers.is_empty());
        assert!(ctx.prompt_notes.contains("ДДС"));
        assert!(lex.phonetic_transcription("указ").is_some());
        assert_eq!(lex.lemmatize_word("указите"), "указ");
    }

    #[tokio::test]
    async fn caption_first_uses_bul_vtt_as_found_and_normalizes() {
        use std::time::Duration;
        use leadfinder_core::{TranscribeInput, Transcriber};
        let paths = leadfinder_core::RuntimePaths::discover().unwrap();
        let whisper = WhisperCppTranscriber::new(&paths, "small", Duration::from_secs(30));
        let cap_first = CaptionFirstTranscriber::new(whisper);

        let dir = tempfile::tempdir().unwrap();
        let vtt_path = dir.path().join("video.bul-BG.vtt");
        std::fs::write(
            &vtt_path,
            "WEBVTT\n\n00:00:00.100 --> 00:00:04.200\nГосподин Радеф говори февро зоната за най ниските цени.\n",
        )
        .unwrap();

        let rec = cap_first
            .transcribe(&TranscribeInput {
                video_id: "test_found_1".to_string(),
                audio_wav_path: None,
                caption_tracks: vec![("bul-BG".to_string(), vtt_path)],
                expected_languages: vec!["bg".to_string(), "en".to_string()],
            })
            .await
            .unwrap();

        assert_eq!(rec.source, "found");
        assert_eq!(rec.language, "bg");
        assert!(rec.text.contains("Радев"));
        assert!(rec.text.contains("еврозоната"));
        assert!(rec.text.contains("най-ниските"));
    }

    struct SequenceMockLlm {
        name: String,
        responses: tokio::sync::Mutex<Vec<Result<serde_json::Value, leadfinder_core::ProviderError>>>,
    }

    #[async_trait::async_trait]
    impl leadfinder_core::Llm for SequenceMockLlm {
        fn name(&self) -> &str {
            &self.name
        }
        fn model_name(&self) -> &str {
            "mock-model"
        }
        async fn complete_json(
            &self,
            _req: &leadfinder_core::LlmJsonRequest,
        ) -> Result<leadfinder_core::LlmJsonResponse, leadfinder_core::ProviderError> {
            let mut guard = self.responses.lock().await;
            let next = guard.remove(0);
            match next {
                Ok(val) => Ok(leadfinder_core::LlmJsonResponse {
                    provider: self.name.clone(),
                    model: "mock-model".to_string(),
                    raw_text: serde_json::to_string(&val).unwrap(),
                    parsed_json: val,
                    tokens_in: 42,
                    tokens_out: 21,
                }),
                Err(e) => Err(e),
            }
        }
    }

    #[tokio::test]
    async fn llm_chain_repairs_once_tracks_usage_and_blocks_disabled_cloud() {
        use std::sync::Arc;
        use std::time::Duration;
        use leadfinder_core::{Database, Llm, LlmJsonRequest};

        let gemini_disabled = GeminiLlm::new(
            "gemini-2.5-flash",
            Some("dummy_key".to_string()),
            false,
            Duration::from_secs(5),
        );
        let gem_err = gemini_disabled
            .complete_json(&LlmJsonRequest {
                system_prompt: "sys".to_string(),
                user_prompt: "usr".to_string(),
                json_schema: None,
                temperature: 0.0,
                max_tokens: 64,
            })
            .await
            .unwrap_err();
        assert_eq!(gem_err.kind, ProviderErrorKind::Unsupported);

        let dir = tempfile::tempdir().unwrap();
        let db = Database::connect(&dir.path().join("llm_test.db")).await.unwrap();

        let schema = json!({
            "type": "object",
            "required": ["claims", "people"],
            "properties": {
                "claims": { "type": "array" },
                "people": { "type": "array" }
            }
        });

        let mock = Arc::new(SequenceMockLlm {
            name: "local".to_string(),
            responses: tokio::sync::Mutex::new(vec![
                Ok(json!({ "wrong_field": 123 })),
                Ok(json!({ "claims": [], "people": [] })),
            ]),
        });
        let chain = LlmChainRunner::from_providers(vec![mock], 250);
        let res = chain
            .complete_validated(
                &LlmJsonRequest {
                    system_prompt: "sys".to_string(),
                    user_prompt: "usr".to_string(),
                    json_schema: Some(schema.clone()),
                    temperature: 0.0,
                    max_tokens: 128,
                },
                &db,
            )
            .await
            .unwrap();
        assert!(!res.first_try_valid);
        assert!(res.repair_used);
        assert_eq!(db.total_requests_today_for_prefix("local").await.unwrap(), 2);

        let broken_mock = Arc::new(SequenceMockLlm {
            name: "local".to_string(),
            responses: tokio::sync::Mutex::new(vec![
                Ok(json!({ "bad": 1 })),
                Ok(json!({ "still_bad": 2 })),
            ]),
        });
        let broken_chain = LlmChainRunner::from_providers(vec![broken_mock], 250);
        let fail_err = broken_chain
            .complete_validated(
                &LlmJsonRequest {
                    system_prompt: "sys".to_string(),
                    user_prompt: "usr".to_string(),
                    json_schema: Some(schema),
                    temperature: 0.0,
                    max_tokens: 128,
                },
                &db,
            )
            .await
            .unwrap_err();
        assert_eq!(fail_err.kind, ProviderErrorKind::InvalidOutput);
    }

    #[tokio::test]
    async fn people_resolver_unifies_cyrillic_latin_articles_and_rejects_unmentioned() {
        use std::collections::HashMap;
        use std::time::Duration;
        use leadfinder_core::{load_app_config, PersonInput, PolitenessGuard, PolitenessPolicy};

        let paths = leadfinder_core::RuntimePaths::discover().unwrap();
        let cfg = load_app_config(&paths.config_dir).unwrap();
        let lex = global_lexicon(&paths, &cfg.normalisation.lexicon_config_file);
        let dir = tempfile::tempdir().unwrap();
        let guard = PolitenessGuard::new(
            PolitenessPolicy {
                requests_per_minute: 60,
                breaker_failures: 5,
                breaker_cooldown: Duration::from_secs(60),
                cache_ttl: Duration::from_secs(3600),
            },
            dir.path().to_path_buf(),
        );

        let mut fixtures = HashMap::new();
        let resolver_base = PeopleResolver::new(cfg.normalisation.clone(), lex, guard);
        let borisov_key = resolver_base.person_phonetic_key("Бойко Борисов");
        fixtures.insert(
            borisov_key,
            WikidataEntityMatch {
                wikidata_id: "Q57623".to_string(),
                label: "Бойко Борисов".to_string(),
                description: Some("български политик".to_string()),
                is_human: true,
                has_public_office_or_party: true,
                sitelink_count: 75,
                public_figure: true,
            },
        );

        let resolver = resolver_base.with_offline_fixtures(fixtures);
        let llm_people = vec![
            PersonInput {
                name: "премиерът Бойко Борисов".to_string(),
                variants: vec!["Бойко Борисов".to_string()],
                role_hint: None,
                entity_type: None,
            },
            PersonInput {
                name: "Boyko Borissov".to_string(),
                variants: vec!["Borissov".to_string()],
                role_hint: None,
                entity_type: None,
            },
            PersonInput {
                name: "г-н Борисов".to_string(),
                variants: vec![],
                role_hint: None,
                entity_type: None,
            },
            PersonInput {
                name: "Иван Петров".to_string(),
                variants: vec![],
                role_hint: None,
                entity_type: None,
            },
            PersonInput {
                name: "Несъществуващ Човек".to_string(),
                variants: vec![],
                role_hint: None,
                entity_type: None,
            },
        ];

        let sources = VideoTextSources {
            transcript: "Днес премиерът Бойко Борисов и Иван Петров коментираха бюджета.".to_string(),
            ocr: "BOYKO BORISSOV LIVE".to_string(),
            caption: "Г-н Борисов говори за данъците".to_string(),
            hashtags: vec!["политика".to_string()],
            creator_handle: Some("news_clip".to_string()),
            creator_display_name: None,
        };

        let resolved = resolver
            .extract_and_resolve_people(&llm_people, &sources)
            .await
            .unwrap();

        assert_eq!(resolved.len(), 2);
        let borisov = resolved
            .iter()
            .find(|p| p.wikidata_id.as_deref() == Some("Q57623"))
            .unwrap();
        assert!(borisov.public_figure);
        assert!(borisov.variants.iter().any(|v| v.contains("Boyko Borissov")));
        assert!(borisov.found_in.contains("transcript"));
        assert!(borisov.found_in.contains("ocr"));

        let private_person = resolved
            .iter()
            .find(|p| p.canonical_name == "Иван Петров")
            .unwrap();
        assert!(!private_person.public_figure);
        assert!(private_person.wikidata_id.is_none());
        assert!(private_person.summary.is_none());
    }

    #[test]
    fn detect_entity_type_identifies_events_and_entities() {
        assert_eq!(detect_entity_type("Парламентарни избори 2026", None), "event");
        assert_eq!(detect_entity_type("Протест пред парламента", None), "event");
        assert_eq!(detect_entity_type("Тежка катастрофа", None), "event");
        assert_eq!(detect_entity_type("Европейска комисия", None), "institution");
        assert_eq!(detect_entity_type("Румен Радев", Some("президент")), "person");
        assert_eq!(detect_entity_type("Република България", None), "country");
        assert_eq!(detect_entity_type("Възраждане", None), "group");
        assert_eq!(detect_entity_type("bTV Новините", None), "channel");
    }
}
