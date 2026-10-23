pub mod bootstrap;
pub mod discovery;
pub mod lexicon;
pub mod media;
pub mod ocr;
pub mod transcriber;
pub mod ytdlp;

pub use bootstrap::*;
pub use discovery::*;
pub use lexicon::*;
pub use media::*;
pub use ocr::*;
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
}
