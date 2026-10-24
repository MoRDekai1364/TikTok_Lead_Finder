use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::process::Command;
use leadfinder_core::{
    DiscoveryQuery, PolitenessGuard, ProviderError, ProviderErrorKind, VideoRef, VideoSource,
};
use crate::ytdlp::{
    extract_hashtags, extract_tiktok_urls_from_text, extract_video_id_from_url, YtDlpClient,
};

#[derive(Clone)]
pub struct PasteLinkSource {
    ytdlp: YtDlpClient,
    http: Client,
    guard: PolitenessGuard,
}

impl PasteLinkSource {
    pub fn new(ytdlp: YtDlpClient, guard: PolitenessGuard) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::limited(8))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { ytdlp, http, guard }
    }

    pub async fn resolve_short_link(&self, raw_url: &str) -> String {
        if extract_video_id_from_url(raw_url).is_some() {
            return raw_url.to_string();
        }
        let host = PolitenessGuard::extract_host(raw_url);
        if self.guard.acquire_slot(&host, "paste_resolver").await.is_err() {
            return raw_url.to_string();
        }
        match self.http.get(raw_url).send().await {
            Ok(resp) => {
                let final_url = resp.url().to_string();
                self.guard.record_success(&host).await;
                final_url
            }
            Err(_) => raw_url.to_string(),
        }
    }

    async fn fetch_oembed_fallback(&self, resolved_url: &str, discovered_via: &str) -> Option<VideoRef> {
        let encoded = urlencoding::encode(resolved_url);
        let oembed_url = format!("https://www.tiktok.com/oembed?url={encoded}");
        if let Some(cached) = self.guard.get_cached(&oembed_url) {
            if let Ok(val) = serde_json::from_str::<Value>(&cached.body) {
                return Self::parse_oembed(&val, resolved_url, discovered_via);
            }
        }

        let host = PolitenessGuard::extract_host(&oembed_url);
        self.guard.acquire_slot(&host, "tiktok_oembed").await.ok()?;
        let resp = self.http.get(&oembed_url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body = resp.text().await.ok()?;
        self.guard
            .put_cached(&oembed_url, 200, body.clone(), Some("application/json".to_string()));
        let val: Value = serde_json::from_str(&body).ok()?;
        Self::parse_oembed(&val, resolved_url, discovered_via)
    }

    fn parse_oembed(val: &Value, resolved_url: &str, discovered_via: &str) -> Option<VideoRef> {
        let id = val
            .get("embed_product_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| extract_video_id_from_url(resolved_url))?;

        let handle = val
            .get("author_unique_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim_start_matches('@').to_string())
            .or_else(|| {
                val.get("author_url").and_then(|v| v.as_str()).and_then(|u| {
                    u.split('@').nth(1).map(|s| s.trim_matches('/').to_string())
                })
            });

        let display_name = val
            .get("author_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let caption = val
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let hashtags = caption.as_deref().map(extract_hashtags).unwrap_or_default();

        Some(VideoRef {
            id: id.clone(),
            url: if let Some(h) = &handle {
                format!("https://www.tiktok.com/@{h}/video/{id}")
            } else {
                resolved_url.to_string()
            },
            creator_id: handle.as_ref().map(|h| h.to_lowercase()),
            creator_handle: handle,
            creator_display_name: display_name,
            creator_follower_count: None,
            creator_region: None,
            caption,
            hashtags,
            posted_at: None,
            views: None,
            likes: None,
            shares: None,
            comments: None,
            duration_s: None,
            sound_title: None,
            subtitle_urls: Vec::new(),
            discovered_via: discovered_via.to_string(),
        })
    }

    pub async fn resolve_single_url(&self, raw_url: &str, discovered_via: &str) -> VideoRef {
        let resolved = self.resolve_short_link(raw_url).await;
        if let Ok(vref) = self.ytdlp.fetch_metadata(&resolved, discovered_via).await {
            return vref;
        }
        if let Some(oembed_ref) = self.fetch_oembed_fallback(&resolved, discovered_via).await {
            return oembed_ref;
        }
        Self::minimal_ref(resolved, discovered_via)
    }

    pub async fn resolve_short_link_fast(&self, raw_url: &str) -> String {
        if extract_video_id_from_url(raw_url).is_some() {
            return raw_url.to_string();
        }
        match self.http.get(raw_url).timeout(Duration::from_millis(1500)).send().await {
            Ok(resp) => resp.url().to_string(),
            Err(_) => raw_url.to_string(),
        }
    }

    async fn fetch_oembed_direct(&self, resolved_url: &str, discovered_via: &str) -> Option<VideoRef> {
        let encoded = urlencoding::encode(resolved_url);
        let oembed_url = format!("https://www.tiktok.com/oembed?url={encoded}");
        if let Some(cached) = self.guard.get_cached(&oembed_url) {
            if let Ok(val) = serde_json::from_str::<Value>(&cached.body) {
                return Self::parse_oembed(&val, resolved_url, discovered_via);
            }
        }
        let resp = self.http.get(&oembed_url).timeout(Duration::from_millis(1500)).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body = resp.text().await.ok()?;
        self.guard.put_cached(&oembed_url, 200, body.clone(), Some("application/json".to_string()));
        let val: Value = serde_json::from_str(&body).ok()?;
        Self::parse_oembed(&val, resolved_url, discovered_via)
    }

    pub async fn resolve_single_url_fast(&self, raw_url: &str, discovered_via: &str) -> VideoRef {
        let resolved = self.resolve_short_link_fast(raw_url).await;
        if let Some(oembed_ref) = self.fetch_oembed_direct(&resolved, discovered_via).await {
            return oembed_ref;
        }
        Self::minimal_ref(resolved, discovered_via)
    }

    pub async fn enrich_metadata(&self, url: &str, discovered_via: &str) -> Option<VideoRef> {
        self.ytdlp.fetch_metadata(url, discovered_via).await.ok()
    }

    fn minimal_ref(resolved: String, discovered_via: &str) -> VideoRef {
        let fallback_id = extract_video_id_from_url(&resolved).unwrap_or_else(|| {
            let mut h = Sha256::new();
            h.update(resolved.as_bytes());
            hex::encode(&h.finalize()[..8])
        });
        let handle = Regex::new(r"tiktok\.com/@([^/?#]+)")
            .unwrap()
            .captures(&resolved)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

        VideoRef {
            id: fallback_id,
            url: resolved,
            creator_id: handle.as_ref().map(|h| h.to_lowercase()),
            creator_handle: handle,
            creator_display_name: None,
            creator_follower_count: None,
            creator_region: None,
            caption: None,
            hashtags: Vec::new(),
            posted_at: None,
            views: None,
            likes: None,
            shares: None,
            comments: None,
            duration_s: None,
            sound_title: None,
            subtitle_urls: Vec::new(),
            discovered_via: discovered_via.to_string(),
        }
    }

    pub fn parse_partner_export_urls(content: &str) -> Vec<String> {
        let clean = content.trim_start_matches('\u{feff}');
        let mut collected = extract_tiktok_urls_from_text(clean);

        let delimiter = if clean.lines().next().map(|l| l.contains('\t')).unwrap_or(false) {
            b'\t'
        } else if clean.lines().next().map(|l| l.contains(';')).unwrap_or(false) {
            b';'
        } else {
            b','
        };

        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .has_headers(true)
            .from_reader(clean.as_bytes());

        if let Ok(headers) = rdr.headers().cloned() {
            let target_cols: Vec<usize> = headers
                .iter()
                .enumerate()
                .filter_map(|(idx, h)| {
                    let key = h.trim().to_lowercase();
                    if matches!(
                        key.as_str(),
                        "url" | "video_url" | "link" | "tiktok_url" | "id" | "video_id" | "aweme_id"
                    ) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();

            for record in rdr.records().flatten() {
                for &col in &target_cols {
                    if let Some(cell) = record.get(col) {
                        for u in extract_tiktok_urls_from_text(cell) {
                            if !collected.contains(&u) {
                                collected.push(u);
                            }
                        }
                    }
                }
            }
        }

        collected
    }
}

#[async_trait]
impl VideoSource for PasteLinkSource {
    fn name(&self) -> &str {
        "paste_link"
    }

    async fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<VideoRef>, ProviderError> {
        let (urls, via) = match query {
            DiscoveryQuery::PasteText { raw_text } => {
                (extract_tiktok_urls_from_text(raw_text), "paste".to_string())
            }
            DiscoveryQuery::PartnerImport { file_path } => {
                let content = std::fs::read_to_string(file_path).map_err(|e| {
                    ProviderError::new(
                        ProviderErrorKind::Internal,
                        "partner_import",
                        format!("failed reading {}: {e}", file_path.display()),
                        false,
                    )
                })?;
                (Self::parse_partner_export_urls(&content), "partner_import".to_string())
            }
            DiscoveryQuery::PartnerImportContent { content, source_label } => (
                Self::parse_partner_export_urls(content),
                format!("partner_import:{source_label}"),
            ),
            _ => {
                return Err(ProviderError::new(
                    ProviderErrorKind::Unsupported,
                    "paste_link",
                    "PasteLinkSource handles PasteText and PartnerImport queries",
                    false,
                ))
            }
        };

        let out = futures::future::join_all(
            urls.iter().map(|url| self.resolve_single_url_fast(url, &via)),
        )
        .await;
        Ok(out)
    }
}

#[derive(Clone)]
pub struct YtDlpDiscoverySource {
    ytdlp: YtDlpClient,
}

impl YtDlpDiscoverySource {
    pub fn new(ytdlp: YtDlpClient) -> Self {
        Self { ytdlp }
    }
}

#[async_trait]
impl VideoSource for YtDlpDiscoverySource {
    fn name(&self) -> &str {
        "ytdlp_public"
    }

    async fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<VideoRef>, ProviderError> {
        match query {
            DiscoveryQuery::Hashtag { tag, max_results } => {
                let clean = tag.trim().trim_start_matches('#');
                let encoded = urlencoding::encode(clean);
                let url = format!("https://www.tiktok.com/tag/{encoded}");
                self.ytdlp
                    .extract_playlist(&url, *max_results, &format!("seed:hashtag:{clean}"))
                    .await
            }
            DiscoveryQuery::Creator {
                handle,
                max_results,
                expansion_depth,
            } => {
                let clean = handle.trim().trim_start_matches('@');
                let url = format!("https://www.tiktok.com/@{clean}");
                let via = if *expansion_depth > 0 {
                    format!("expansion:{expansion_depth}:@{clean}")
                } else {
                    format!("watchlist:@{clean}")
                };
                self.ytdlp.extract_playlist(&url, *max_results, &via).await
            }
            DiscoveryQuery::Keyword { keyword, max_results } => {
                let tag_form: String = keyword
                    .split_whitespace()
                    .next()
                    .unwrap_or(keyword)
                    .trim_start_matches('#')
                    .to_string();
                let encoded = urlencoding::encode(&tag_form);
                let url = format!("https://www.tiktok.com/tag/{encoded}");
                self.ytdlp
                    .extract_playlist(&url, *max_results, &format!("seed:keyword:{keyword}"))
                    .await
            }
            _ => Err(ProviderError::new(
                ProviderErrorKind::Unsupported,
                "ytdlp_public",
                "unsupported query kind",
                false,
            )),
        }
    }
}

#[derive(Clone)]
pub struct DirectHttpDiscoverySource {
    http: Client,
    guard: PolitenessGuard,
}

impl DirectHttpDiscoverySource {
    pub fn new(guard: PolitenessGuard) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { http, guard }
    }

    pub fn parse_public_html(html: &str, discovered_via: &str, max_results: usize) -> Result<Vec<VideoRef>, ProviderError> {
        let lower = html.to_lowercase();
        if lower.contains("tiktok-verify-page")
            || lower.contains("captcha_verify")
            || lower.contains("secsdk-captcha")
            || lower.contains("verify to continue")
        {
            return Err(ProviderError::new(
                ProviderErrorKind::BlockedOrCaptcha,
                "direct_http",
                "public page returned challenge/verification gate",
                true,
            ));
        }

        let mut refs = Vec::new();
        let script_re = Regex::new(
            r#"<script[^>]*id="__UNIVERSAL_DATA_FOR_REHYDRATION__"[^>]*>([\s\S]*?)</script>"#,
        )
        .unwrap();

        if let Some(cap) = script_re.captures(html) {
            if let Some(json_str) = cap.get(1).map(|m| m.as_str()) {
                if let Ok(val) = serde_json::from_str::<Value>(json_str) {
                    collect_items_from_hydration_json(&val, discovered_via, max_results, &mut refs);
                }
            }
        }

        let doc = scraper::Html::parse_document(html);
        if let Ok(a_sel) = scraper::Selector::parse("a[href*='/video/']") {
            let img_sel = scraper::Selector::parse("img[alt]").ok();
            let vid_re = Regex::new(r"tiktok\.com/@([^/?#\s]+)/video/(\d{17,21})").unwrap();

            for a_el in doc.select(&a_sel) {
                if refs.len() >= max_results {
                    break;
                }
                let Some(href) = a_el.value().attr("href") else {
                    continue;
                };
                let Some(cap) = vid_re.captures(href) else {
                    continue;
                };
                let handle = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let id = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                if id.is_empty() || refs.iter().any(|r| r.id == id) {
                    continue;
                }

                let mut text_parts = Vec::new();
                if let Some(title) = a_el.value().attr("title") {
                    if !title.trim().is_empty() {
                        text_parts.push(title.trim().to_string());
                    }
                }
                if let Some(ref isel) = img_sel {
                    for img in a_el.select(isel) {
                        if let Some(alt) = img.value().attr("alt") {
                            if !alt.trim().is_empty() {
                                text_parts.push(alt.trim().to_string());
                            }
                        }
                    }
                }
                let inner_txt: String = a_el.text().collect::<Vec<_>>().join(" ");
                if !inner_txt.trim().is_empty() {
                    text_parts.push(inner_txt.trim().to_string());
                }

                let combined = text_parts.join(" ").trim().to_string();
                let caption = if combined.is_empty() {
                    None
                } else {
                    Some(combined.clone())
                };
                let mut hashtags = extract_hashtags(&combined);
                if let Some(seed_tag) = discovered_via.strip_prefix("seed:hashtag:") {
                    let clean_tag = seed_tag.to_lowercase();
                    if !hashtags.contains(&clean_tag) {
                        hashtags.push(clean_tag);
                    }
                }

                refs.push(VideoRef {
                    id: id.clone(),
                    url: format!("https://www.tiktok.com/@{handle}/video/{id}"),
                    creator_id: Some(handle.to_lowercase()),
                    creator_handle: Some(handle),
                    creator_display_name: None,
                    creator_follower_count: None,
                    creator_region: None,
                    caption,
                    hashtags,
                    posted_at: None,
                    views: None,
                    likes: None,
                    shares: None,
                    comments: None,
                    duration_s: None,
                    sound_title: None,
                    subtitle_urls: Vec::new(),
                    discovered_via: discovered_via.to_string(),
                });
            }
        }

        let link_re = Regex::new(r#"https://www\.tiktok\.com/@([^/"'?\s]+)/video/(\d{17,21})"#).unwrap();
        for cap in link_re.captures_iter(html) {
            if refs.len() >= max_results {
                break;
            }
            let handle = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let id = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
            if !id.is_empty() && !refs.iter().any(|r| r.id == id) {
                refs.push(VideoRef {
                    id: id.clone(),
                    url: format!("https://www.tiktok.com/@{handle}/video/{id}"),
                    creator_id: Some(handle.to_lowercase()),
                    creator_handle: Some(handle),
                    creator_display_name: None,
                    creator_follower_count: None,
                    creator_region: None,
                    caption: None,
                    hashtags: Vec::new(),
                    posted_at: None,
                    views: None,
                    likes: None,
                    shares: None,
                    comments: None,
                    duration_s: None,
                    sound_title: None,
                    subtitle_urls: Vec::new(),
                    discovered_via: discovered_via.to_string(),
                });
            }
        }

        Ok(refs)
    }

    async fn fetch_public_page(&self, url: &str, via: &str, max_results: usize) -> Result<Vec<VideoRef>, ProviderError> {
        let host = PolitenessGuard::extract_host(url);
        self.guard.acquire_slot(&host, "direct_http").await?;

        let resp = match self
            .http
            .get(url)
            .header("Accept-Language", "bg-BG,bg;q=0.9,en;q=0.7")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                self.guard.record_failure(&host, &e.to_string()).await;
                return Err(ProviderError::new(
                    ProviderErrorKind::NetworkError,
                    "direct_http",
                    e.to_string(),
                    true,
                ));
            }
        };

        let status = resp.status();
        if status.as_u16() == 429 {
            self.guard.record_failure(&host, "HTTP 429").await;
            return Err(ProviderError::new(
                ProviderErrorKind::RateLimited,
                "direct_http",
                format!("HTTP 429 on {url}"),
                true,
            ));
        }
        if status.is_client_error() || status.is_server_error() {
            self.guard
                .record_failure(&host, &format!("HTTP {}", status.as_u16()))
                .await;
            return Err(ProviderError::new(
                ProviderErrorKind::BlockedOrCaptcha,
                "direct_http",
                format!("HTTP {} on {url}", status.as_u16()),
                true,
            ));
        }

        let body = resp.text().await.map_err(|e| {
            ProviderError::new(ProviderErrorKind::NetworkError, "direct_http", e.to_string(), true)
        })?;

        match Self::parse_public_html(&body, via, max_results) {
            Ok(items) => {
                self.guard.record_success(&host).await;
                Ok(items)
            }
            Err(err) => {
                if err.kind == ProviderErrorKind::BlockedOrCaptcha {
                    self.guard.record_failure(&host, &err.message).await;
                }
                Err(err)
            }
        }
    }
}

#[async_trait]
impl VideoSource for DirectHttpDiscoverySource {
    fn name(&self) -> &str {
        "direct_http_public"
    }

    async fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<VideoRef>, ProviderError> {
        match query {
            DiscoveryQuery::Hashtag { tag, max_results } => {
                let clean = tag.trim().trim_start_matches('#');
                let encoded = urlencoding::encode(clean);
                let url = format!("https://www.tiktok.com/tag/{encoded}");
                self.fetch_public_page(&url, &format!("seed:hashtag:{clean}"), *max_results)
                    .await
            }
            DiscoveryQuery::Keyword { keyword, max_results } => {
                let encoded = urlencoding::encode(keyword.trim());
                let url = format!("https://www.tiktok.com/search?q={encoded}");
                self.fetch_public_page(&url, &format!("seed:keyword:{keyword}"), *max_results)
                    .await
            }
            DiscoveryQuery::Creator {
                handle,
                max_results,
                expansion_depth,
            } => {
                let clean = handle.trim().trim_start_matches('@');
                let url = format!("https://www.tiktok.com/@{clean}");
                let via = if *expansion_depth > 0 {
                    format!("expansion:{expansion_depth}:@{clean}")
                } else {
                    format!("watchlist:@{clean}")
                };
                self.fetch_public_page(&url, &via, *max_results).await
            }
            _ => Err(ProviderError::new(
                ProviderErrorKind::Unsupported,
                "direct_http_public",
                "unsupported query kind",
                false,
            )),
        }
    }
}

#[derive(Clone)]
pub struct HeadlessBrowserDiscoverySource {
    browser_path: Option<PathBuf>,
    guard: PolitenessGuard,
    timeout: Duration,
}

impl HeadlessBrowserDiscoverySource {
    pub fn new(guard: PolitenessGuard, timeout: Duration) -> Self {
        Self {
            browser_path: Self::detect_browser(),
            guard,
            timeout,
        }
    }

    pub fn detect_browser() -> Option<PathBuf> {
        let candidates = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        candidates.iter().map(PathBuf::from).find(|p| p.is_file())
    }

    pub fn is_available(&self) -> bool {
        self.browser_path.is_some()
    }

    async fn dump_public_dom(&self, url: &str, via: &str, max_results: usize) -> Result<Vec<VideoRef>, ProviderError> {
        let browser = self.browser_path.as_deref().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::BinaryMissing,
                "headless_browser",
                "neither msedge.exe nor chrome.exe found on system",
                false,
            )
        })?;

        let host = PolitenessGuard::extract_host(url);
        self.guard.acquire_slot(&host, "headless_browser").await?;

        let temp_profile = std::env::temp_dir().join(format!("leadfinder_headless_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_profile);
        let user_data_arg = format!("--user-data-dir={}", temp_profile.display());

        let mut cmd = Command::new(browser);
        cmd.args([
            "--headless=new",
            "--disable-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            "--virtual-time-budget=6000",
            "--dump-dom",
            &user_data_arg,
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

        let res = tokio::time::timeout(self.timeout, cmd.output()).await;
        let _ = std::fs::remove_dir_all(&temp_profile);

        let output = match res {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                self.guard.record_failure(&host, &e.to_string()).await;
                return Err(ProviderError::new(
                    ProviderErrorKind::Internal,
                    "headless_browser",
                    e.to_string(),
                    true,
                ));
            }
            Err(_) => {
                self.guard.record_failure(&host, "headless browser timeout").await;
                return Err(ProviderError::new(
                    ProviderErrorKind::Timeout,
                    "headless_browser",
                    format!("headless browser timed out on {url}"),
                    true,
                ));
            }
        };

        let html = String::from_utf8_lossy(&output.stdout).to_string();
        match DirectHttpDiscoverySource::parse_public_html(&html, via, max_results) {
            Ok(items) => {
                self.guard.record_success(&host).await;
                Ok(items)
            }
            Err(err) => {
                if err.kind == ProviderErrorKind::BlockedOrCaptcha {
                    self.guard.record_failure(&host, &err.message).await;
                }
                Err(err)
            }
        }
    }
}

#[async_trait]
impl VideoSource for HeadlessBrowserDiscoverySource {
    fn name(&self) -> &str {
        "headless_browser_public"
    }

    async fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<VideoRef>, ProviderError> {
        match query {
            DiscoveryQuery::Hashtag { tag, max_results } => {
                let clean = tag.trim().trim_start_matches('#');
                let encoded = urlencoding::encode(clean);
                let url = format!("https://www.tiktok.com/tag/{encoded}");
                self.dump_public_dom(&url, &format!("seed:hashtag:{clean}"), *max_results)
                    .await
            }
            DiscoveryQuery::Keyword { keyword, max_results } => {
                let encoded = urlencoding::encode(keyword.trim());
                let url = format!("https://www.tiktok.com/search?q={encoded}");
                self.dump_public_dom(&url, &format!("seed:keyword:{keyword}"), *max_results)
                    .await
            }
            DiscoveryQuery::Creator {
                handle,
                max_results,
                expansion_depth,
            } => {
                let clean = handle.trim().trim_start_matches('@');
                let url = format!("https://www.tiktok.com/@{clean}");
                let via = if *expansion_depth > 0 {
                    format!("expansion:{expansion_depth}:@{clean}")
                } else {
                    format!("watchlist:@{clean}")
                };
                self.dump_public_dom(&url, &via, *max_results).await
            }
            _ => Err(ProviderError::new(
                ProviderErrorKind::Unsupported,
                "headless_browser_public",
                "unsupported query kind",
                false,
            )),
        }
    }
}

fn collect_items_from_hydration_json(
    val: &Value,
    discovered_via: &str,
    max_results: usize,
    out: &mut Vec<VideoRef>,
) {
    if out.len() >= max_results {
        return;
    }
    match val {
        Value::Object(map) => {
            if let (Some(id_val), Some(desc_val)) = (map.get("id"), map.get("desc")) {
                if let (Some(id), Some(desc)) = (id_val.as_str(), desc_val.as_str()) {
                    if id.len() >= 15 && id.chars().all(|c| c.is_ascii_digit()) && !out.iter().any(|r| r.id == id) {
                        let author_handle = map
                            .get("author")
                            .and_then(|a| {
                                a.get("uniqueId")
                                    .and_then(|u| u.as_str())
                                    .or_else(|| a.as_str())
                            })
                            .map(|s| s.to_string());
                        let author_nick = map
                            .get("author")
                            .and_then(|a| a.get("nickname"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());
                        let views = map
                            .get("stats")
                            .and_then(|s| s.get("playCount"))
                            .and_then(|v| v.as_i64());
                        let likes = map
                            .get("stats")
                            .and_then(|s| s.get("diggCount"))
                            .and_then(|v| v.as_i64());
                        let shares = map
                            .get("stats")
                            .and_then(|s| s.get("shareCount"))
                            .and_then(|v| v.as_i64());
                        let comments = map
                            .get("stats")
                            .and_then(|s| s.get("commentCount"))
                            .and_then(|v| v.as_i64());
                        let duration_s = map
                            .get("video")
                            .and_then(|v| v.get("duration"))
                            .and_then(|v| v.as_f64());

                        let url = if let Some(h) = &author_handle {
                            format!("https://www.tiktok.com/@{h}/video/{id}")
                        } else {
                            format!("https://www.tiktok.com/@tiktok/video/{id}")
                        };

                        out.push(VideoRef {
                            id: id.to_string(),
                            url,
                            creator_id: author_handle.as_ref().map(|h| h.to_lowercase()),
                            creator_handle: author_handle,
                            creator_display_name: author_nick,
                            creator_follower_count: None,
                            creator_region: None,
                            caption: if desc.trim().is_empty() {
                                None
                            } else {
                                Some(desc.trim().to_string())
                            },
                            hashtags: extract_hashtags(desc),
                            posted_at: None,
                            views,
                            likes,
                            shares,
                            comments,
                            duration_s,
                            sound_title: None,
                            subtitle_urls: Vec::new(),
                            discovered_via: discovered_via.to_string(),
                        });
                    }
                }
            }
            for child in map.values() {
                collect_items_from_hydration_json(child, discovered_via, max_results, out);
                if out.len() >= max_results {
                    break;
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_items_from_hydration_json(item, discovered_via, max_results, out);
                if out.len() >= max_results {
                    break;
                }
            }
        }
        _ => {}
    }
}

pub fn has_bulgarian_or_cyrillic_signal(vref: &VideoRef) -> bool {
    let check_text = |s: &str| s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    if let Some(cap) = &vref.caption {
        if check_text(cap) {
            return true;
        }
    }
    if vref
        .hashtags
        .iter()
        .any(|h| check_text(h) || h == "bulgaria" || h == "bg")
    {
        return true;
    }
    if let Some(handle) = &vref.creator_handle {
        let lower = handle.to_lowercase();
        if lower.contains("bg") || lower.contains("bulgar") {
            return true;
        }
    }
    if vref.creator_region.as_deref() == Some("BG") {
        return true;
    }
    false
}
