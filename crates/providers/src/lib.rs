pub mod discovery;
pub mod ytdlp;

pub use discovery::*;
pub use ytdlp::*;

pub fn crate_ready() -> &'static str {
    leadfinder_core::crate_ready()
}

#[cfg(test)]
mod tests {
    use super::*;
    use leadfinder_core::ProviderErrorKind;
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

        let vref = YtDlpClient::parse_metadata_json(&raw, "https://www.tiktok.com/@fakt_bg/video/7400112233445566778", "paste").unwrap();
        assert_eq!(vref.id, "7400112233445566778");
        assert_eq!(vref.creator_handle.as_deref(), Some("fakt_bg"));
        assert_eq!(vref.views, Some(48200));
        assert!(vref.hashtags.contains(&"българия".to_string()));
        assert!(vref.hashtags.contains(&"еврозона".to_string()));
        assert_eq!(vref.subtitle_urls.len(), 1);
    }
}
