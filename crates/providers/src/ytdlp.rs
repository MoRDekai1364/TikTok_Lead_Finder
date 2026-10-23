use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use chrono::{TimeZone, Utc};
use regex::Regex;
use serde_json::Value;
use tokio::process::Command;
use leadfinder_core::{
    PolitenessGuard, ProviderError, ProviderErrorKind, RuntimePaths, SubtitleTrackRef, VideoRef,
};

#[derive(Clone)]
pub struct YtDlpClient {
    binary_path: Option<PathBuf>,
    timeout: Duration,
    guard: PolitenessGuard,
}

impl YtDlpClient {
    pub fn new(paths: &RuntimePaths, timeout: Duration, guard: PolitenessGuard) -> Self {
        Self {
            binary_path: paths.resolve_binary("yt-dlp"),
            timeout,
            guard,
        }
    }

    pub fn with_binary(binary_path: Option<PathBuf>, timeout: Duration, guard: PolitenessGuard) -> Self {
        Self {
            binary_path,
            timeout,
            guard,
        }
    }

    pub fn is_available(&self) -> bool {
        self.binary_path.is_some()
    }

    pub fn binary_path(&self) -> Result<&Path, ProviderError> {
        self.binary_path.as_deref().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::BinaryMissing,
                "yt-dlp",
                "yt-dlp executable was not found in tools directory or PATH",
                false,
            )
        })
    }

    pub fn classify_stderr(stderr: &str) -> (ProviderErrorKind, bool) {
        let lower = stderr.to_lowercase();
        if lower.contains("429") || lower.contains("too many requests") || lower.contains("rate limit") {
            (ProviderErrorKind::RateLimited, true)
        } else if lower.contains("captcha")
            || lower.contains("verify")
            || lower.contains("http error 403")
            || lower.contains("forbidden")
            || lower.contains("blocked")
            || lower.contains("login required")
            || lower.contains("sign in")
        {
            (ProviderErrorKind::BlockedOrCaptcha, true)
        } else if lower.contains("404")
            || lower.contains("unavailable")
            || lower.contains("private")
            || lower.contains("removed")
            || lower.contains("status code 10204")
            || lower.contains("unable to extract")
        {
            (ProviderErrorKind::NotFoundOrPrivate, false)
        } else if lower.contains("not available in your country") || lower.contains("geo") {
            (ProviderErrorKind::GeoRestricted, false)
        } else if lower.contains("timed out")
            || lower.contains("connection")
            || lower.contains("getaddrinfo")
            || lower.contains("network")
            || lower.contains("eof")
        {
            (ProviderErrorKind::NetworkError, true)
        } else {
            (ProviderErrorKind::Internal, true)
        }
    }

    pub async fn fetch_metadata(&self, url: &str, discovered_via: &str) -> Result<VideoRef, ProviderError> {
        let cache_key = format!("ytdlp_meta::{url}");
        if let Some(cached) = self.guard.get_cached(&cache_key) {
            if let Ok(val) = serde_json::from_str::<Value>(&cached.body) {
                return Self::parse_metadata_json(&val, url, discovered_via);
            }
        }

        let bin = self.binary_path()?;
        let host = PolitenessGuard::extract_host(url);
        self.guard.acquire_slot(&host, "yt-dlp").await?;

        let mut cmd = Command::new(bin);
        cmd.args([
            "--no-warnings",
            "--no-playlist",
            "--skip-download",
            "--dump-single-json",
            "--socket-timeout",
            "20",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let output_res = tokio::time::timeout(self.timeout, cmd.output()).await;
        let output = match output_res {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                self.guard.record_failure(&host, &e.to_string()).await;
                return Err(ProviderError::new(
                    ProviderErrorKind::Internal,
                    "yt-dlp",
                    format!("failed spawning yt-dlp: {e}"),
                    true,
                ));
            }
            Err(_) => {
                self.guard.record_failure(&host, "yt-dlp metadata timeout").await;
                return Err(ProviderError::new(
                    ProviderErrorKind::Timeout,
                    "yt-dlp",
                    format!("yt-dlp metadata timed out after {}s for {url}", self.timeout.as_secs()),
                    true,
                ));
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let (kind, retryable) = Self::classify_stderr(&stderr);
            if matches!(
                kind,
                ProviderErrorKind::BlockedOrCaptcha
                    | ProviderErrorKind::RateLimited
                    | ProviderErrorKind::NetworkError
            ) {
                self.guard.record_failure(&host, &stderr).await;
            }
            return Err(ProviderError::new(kind, "yt-dlp", stderr, retryable));
        }

        self.guard.record_success(&host).await;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let val: Value = serde_json::from_str(&stdout).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::ParseError,
                "yt-dlp",
                format!("invalid JSON from yt-dlp: {e}"),
                false,
            )
        })?;

        self.guard
            .put_cached(&cache_key, 200, stdout, Some("application/json".to_string()));
        Self::parse_metadata_json(&val, url, discovered_via)
    }

    pub async fn extract_playlist(&self, page_url: &str, max_items: usize, discovered_via: &str) -> Result<Vec<VideoRef>, ProviderError> {
        let bin = self.binary_path()?;
        let host = PolitenessGuard::extract_host(page_url);
        self.guard.acquire_slot(&host, "yt-dlp-playlist").await?;

        let stop_arg = max_items.max(1).to_string();
        let mut cmd = Command::new(bin);
        cmd.args([
            "--no-warnings",
            "--flat-playlist",
            "--playlist-end",
            &stop_arg,
            "-J",
            "--socket-timeout",
            "20",
            page_url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let output = match tokio::time::timeout(self.timeout, cmd.output()).await {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                self.guard.record_failure(&host, &e.to_string()).await;
                return Err(ProviderError::new(
                    ProviderErrorKind::Internal,
                    "yt-dlp-playlist",
                    e.to_string(),
                    true,
                ));
            }
            Err(_) => {
                self.guard.record_failure(&host, "playlist timeout").await;
                return Err(ProviderError::new(
                    ProviderErrorKind::Timeout,
                    "yt-dlp-playlist",
                    format!("playlist extraction timed out for {page_url}"),
                    true,
                ));
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let (kind, retryable) = Self::classify_stderr(&stderr);
            if matches!(kind, ProviderErrorKind::BlockedOrCaptcha | ProviderErrorKind::RateLimited) {
                self.guard.record_failure(&host, &stderr).await;
            }
            return Err(ProviderError::new(kind, "yt-dlp-playlist", stderr, retryable));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let val: Value = serde_json::from_str(&stdout).map_err(|e| {
            ProviderError::new(ProviderErrorKind::ParseError, "yt-dlp-playlist", e.to_string(), false)
        })?;

        let mut results = Vec::new();
        if let Some(entries) = val.get("entries").and_then(|v| v.as_array()) {
            for entry in entries.iter().take(max_items) {
                let fallback_url = entry
                    .get("url")
                    .or_else(|| entry.get("webpage_url"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(page_url);
                if let Ok(vref) = Self::parse_metadata_json(entry, fallback_url, discovered_via) {
                    results.push(vref);
                }
            }
        }
        self.guard.record_success(&host).await;
        Ok(results)
    }

    pub fn parse_metadata_json(val: &Value, fallback_url: &str, discovered_via: &str) -> Result<VideoRef, ProviderError> {
        let id = val
            .get("id")
            .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
            .or_else(|| extract_video_id_from_url(fallback_url))
            .ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::ParseError,
                    "yt-dlp",
                    format!("missing video id in metadata for {fallback_url}"),
                    false,
                )
            })?;

        let creator_handle = val
            .get("uploader")
            .and_then(|v| v.as_str())
            .or_else(|| val.get("creator").and_then(|v| v.as_str()))
            .map(|s| s.trim_start_matches('@').to_string())
            .filter(|s| !s.is_empty());

        let creator_id = val
            .get("uploader_id")
            .and_then(|v| v.as_str())
            .or_else(|| val.get("channel_id").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| creator_handle.as_ref().map(|h| h.to_lowercase()));

        let creator_display_name = val
            .get("channel")
            .and_then(|v| v.as_str())
            .or_else(|| val.get("creator").and_then(|v| v.as_str()))
            .or_else(|| val.get("uploader").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        let canonical_url = val
            .get("webpage_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if let Some(handle) = &creator_handle {
                    format!("https://www.tiktok.com/@{handle}/video/{id}")
                } else {
                    fallback_url.to_string()
                }
            });

        let caption = val
            .get("description")
            .and_then(|v| v.as_str())
            .or_else(|| val.get("title").and_then(|v| v.as_str()))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let mut hashtags = Vec::new();
        if let Some(tags) = val.get("tags").and_then(|v| v.as_array()) {
            for t in tags {
                if let Some(s) = t.as_str() {
                    let clean = s.trim().trim_start_matches('#').to_lowercase();
                    if !clean.is_empty() && !hashtags.contains(&clean) {
                        hashtags.push(clean);
                    }
                }
            }
        }
        if let Some(cap) = &caption {
            for tag in extract_hashtags(cap) {
                if !hashtags.contains(&tag) {
                    hashtags.push(tag);
                }
            }
        }

        let posted_at = val
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
            .map(|dt| dt.to_rfc3339())
            .or_else(|| {
                val.get("upload_date").and_then(|v| v.as_str()).and_then(|d| {
                    if d.len() == 8 {
                        Some(format!("{}-{}-{}T00:00:00Z", &d[0..4], &d[4..6], &d[6..8]))
                    } else {
                        None
                    }
                })
            });

        let views = val.get("view_count").and_then(|v| v.as_i64());
        let likes = val.get("like_count").and_then(|v| v.as_i64());
        let shares = val
            .get("repost_count")
            .and_then(|v| v.as_i64())
            .or_else(|| val.get("share_count").and_then(|v| v.as_i64()));
        let comments = val.get("comment_count").and_then(|v| v.as_i64());
        let duration_s = val
            .get("duration")
            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|n| n as f64)));
        let creator_follower_count = val.get("channel_follower_count").and_then(|v| v.as_i64());
        let sound_title = val
            .get("track")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        let mut subtitle_urls = Vec::new();
        for key in ["subtitles", "automatic_captions"] {
            if let Some(sub_obj) = val.get(key).and_then(|v| v.as_object()) {
                for (lang, tracks) in sub_obj {
                    if let Some(arr) = tracks.as_array() {
                        for track in arr {
                            let ext = track.get("ext").and_then(|v| v.as_str()).unwrap_or("");
                            let turl = track.get("url").and_then(|v| v.as_str()).unwrap_or("");
                            if !turl.is_empty() && matches!(ext, "vtt" | "srt" | "json3" | "ttml") {
                                subtitle_urls.push(SubtitleTrackRef {
                                    language: lang.clone(),
                                    ext: ext.to_string(),
                                    url: turl.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(VideoRef {
            id,
            url: canonical_url,
            creator_id,
            creator_handle,
            creator_display_name,
            creator_follower_count,
            creator_region: None,
            caption,
            hashtags,
            posted_at,
            views,
            likes,
            shares,
            comments,
            duration_s,
            sound_title,
            subtitle_urls,
            discovered_via: discovered_via.to_string(),
        })
    }
}

pub fn extract_hashtags(text: &str) -> Vec<String> {
    let re = Regex::new(r"#([\p{L}\p{N}_]+)").unwrap();
    let mut out = Vec::new();
    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let tag = m.as_str().to_lowercase();
            if !tag.is_empty() && !out.contains(&tag) {
                out.push(tag);
            }
        }
    }
    out
}

pub fn extract_video_id_from_url(url_or_id: &str) -> Option<String> {
    let trimmed = url_or_id.trim();
    if trimmed.len() >= 17 && trimmed.len() <= 21 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }
    let re = Regex::new(r"(?:/video/|/photo/|/v/|item_id=|video_id=)(\d{15,22})").unwrap();
    re.captures(trimmed)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_tiktok_urls_from_text(input: &str) -> Vec<String> {
    let url_re = Regex::new(
        r"https?://(?:www\.|m\.|vm\.|vt\.)?tiktok\.com/[^\s,\x22'<>]+",
    )
    .unwrap();
    let mut urls = Vec::new();
    for m in url_re.find_iter(input) {
        let cleaned = m
            .as_str()
            .trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ')' | ']'))
            .to_string();
        if !urls.contains(&cleaned) {
            urls.push(cleaned);
        }
    }

    for token in input.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = token.trim().trim_matches('"').trim_matches('\'');
        if t.len() >= 17 && t.len() <= 21 && t.chars().all(|c| c.is_ascii_digit()) {
            let synth = format!("https://www.tiktok.com/@tiktok/video/{t}");
            if !urls.contains(&synth) {
                urls.push(synth);
            }
        }
    }

    urls
}
