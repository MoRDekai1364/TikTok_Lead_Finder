use std::collections::HashSet;
use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use leadfinder_core::{
    ArchiveEndpointConfig, HttpTransportMode, PolitenessGuard, PrimarySourceEntry, ProviderError,
    ProviderErrorKind, Search, SearchHit, SearchQuery,
};
use crate::archive::{extract_xml_tag, strip_html_tags, RobotsTxtChecker};

pub struct WebAndPrimarySearch {
    searxng_url: String,
    primary_sources: Vec<PrimarySourceEntry>,
    archive_endpoints: Vec<ArchiveEndpointConfig>,
    news_domains: Vec<String>,
    guard: PolitenessGuard,
    robots: RobotsTxtChecker,
    client: Client,
}

impl WebAndPrimarySearch {
    pub fn new(
        searxng_url: impl Into<String>,
        primary_sources: Vec<PrimarySourceEntry>,
        archive_endpoints: Vec<ArchiveEndpointConfig>,
        news_domains: Vec<String>,
        guard: PolitenessGuard,
    ) -> Self {
        let client = Client::builder()
            .user_agent("TikTokLeadFinder/0.1 (local fact-checking research assistant)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| Client::new());
        let robots = RobotsTxtChecker::new(guard.clone(), client.clone());
        Self {
            searxng_url: searxng_url.into().trim_end_matches('/').to_string(),
            primary_sources,
            archive_endpoints,
            news_domains,
            guard,
            robots,
            client,
        }
    }

    pub fn classify_url(&self, url: &str) -> (String, Option<String>) {
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
            .unwrap_or_default();
        let clean_host = host.strip_prefix("www.").unwrap_or(&host);

        for arch in &self.archive_endpoints {
            if let Ok(au) = reqwest::Url::parse(&arch.base_url) {
                if let Some(ah) = au.host_str() {
                    let ah_clean = ah.to_lowercase();
                    let ah_clean = ah_clean.strip_prefix("www.").unwrap_or(&ah_clean);
                    if domain_matches(clean_host, ah_clean) {
                        return ("factcheck".to_string(), Some(arch.name.clone()));
                    }
                }
            }
        }

        for ps in &self.primary_sources {
            let pd = ps.domain.to_lowercase();
            let pd = pd.strip_prefix("www.").unwrap_or(&pd);
            if domain_matches(clean_host, pd) {
                return ("primary".to_string(), Some(ps.name.clone()));
            }
        }

        for nd in &self.news_domains {
            let nd_clean = nd.to_lowercase();
            let nd_clean = nd_clean.strip_prefix("www.").unwrap_or(&nd_clean);
            if domain_matches(clean_host, nd_clean) {
                return ("news".to_string(), Some(nd_clean.to_string()));
            }
        }

        let fallback_name = if clean_host.is_empty() {
            None
        } else {
            Some(clean_host.to_string())
        };
        ("other".to_string(), fallback_name)
    }

    async fn fetch_cached(&self, url: &str, check_robots: bool) -> Option<String> {
        if let Some(hit) = self.guard.get_cached(url) {
            if hit.status >= 200 && hit.status < 300 && !hit.body.is_empty() {
                return Some(hit.body);
            }
            return None;
        }
        if self.guard.mode() == HttpTransportMode::ReplayFixturesOnly {
            return None;
        }
        if check_robots && !self.robots.is_allowed(url).await {
            return None;
        }
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_default();
        if self.guard.acquire_slot(&host, "web_search").await.is_err() {
            return None;
        }

        match self.client.get(url).send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                let ctype = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                if resp.status().is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    self.guard.record_success(&host).await;
                    self.guard.put_cached(url, code, body.clone(), ctype);
                    Some(body)
                } else {
                    self.guard
                        .record_failure(&host, &format!("HTTP {code}"))
                        .await;
                    None
                }
            }
            Err(e) => {
                self.guard.record_failure(&host, &e.to_string()).await;
                None
            }
        }
    }

    async fn execute_single_search(&self, q: &str) -> Vec<(String, String, String, Option<String>)> {
        if !self.searxng_url.is_empty() {
            let sx_url = format!(
                "{}/search?q={}&format=json",
                self.searxng_url,
                urlencoding::encode(q)
            );
            if let Some(body) = self.fetch_cached(&sx_url, false).await {
                let hits = parse_searxng_json(&body);
                if !hits.is_empty() {
                    return hits;
                }
            }
        }

        let bing_rss_url = format!(
            "https://www.bing.com/search?format=rss&q={}",
            urlencoding::encode(q)
        );
        if let Some(rss) = self.fetch_cached(&bing_rss_url, false).await {
            let hits = parse_bing_rss(&rss);
            if !hits.is_empty() {
                return hits;
            }
        }

        let ddg_url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(q)
        );
        if let Some(html) = self.fetch_cached(&ddg_url, false).await {
            return parse_ddg_html(&html);
        }

        Vec::new()
    }
}

#[async_trait]
impl Search for WebAndPrimarySearch {
    fn name(&self) -> &str {
        "web_and_primary_search"
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, ProviderError> {
        let mut raw_queries: Vec<String> = Vec::new();
        if query.restrict_domains.is_empty() {
            raw_queries.push(query.query.clone());
        } else {
            for chunk in query.restrict_domains.chunks(4) {
                let site_expr = chunk
                    .iter()
                    .map(|d| format!("site:{d}"))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                raw_queries.push(format!("({}) {}", site_expr, query.query));
            }
        }

        let mut out = Vec::new();
        let mut seen = HashSet::new();

        for rq in raw_queries.into_iter().take(4) {
            let found = self.execute_single_search(&rq).await;
            for (url, title, snippet, pub_date) in found {
                if is_social_video_self_url(&url) || is_untrustworthy_or_junk_url(&url) {
                    continue;
                }
                if !query.restrict_domains.is_empty() {
                    let host = reqwest::Url::parse(&url)
                        .ok()
                        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
                        .unwrap_or_default();
                    let clean_h = host.strip_prefix("www.").unwrap_or(&host);
                    let allowed = query
                        .restrict_domains
                        .iter()
                        .any(|d| domain_matches(clean_h, d.strip_prefix("www.").unwrap_or(d)));
                    if !allowed {
                        continue;
                    }
                }
                if !seen.insert(url.clone()) {
                    continue;
                }
                let (source_kind, source_name) = self.classify_url(&url);
                out.push(SearchHit {
                    url,
                    title,
                    snippet,
                    source_name,
                    source_kind,
                    published_at: pub_date,
                });
                if out.len() >= query.max_results.max(1) {
                    break;
                }
            }
            if out.len() >= query.max_results.max(1) {
                break;
            }
        }

        let _ = ProviderErrorKind::Internal;
        Ok(out)
    }
}

pub fn domain_matches(host: &str, domain: &str) -> bool {
    let h = host.trim().to_lowercase();
    let d = domain.trim().to_lowercase();
    if h.is_empty() || d.is_empty() {
        return false;
    }
    h == d || h.ends_with(&format!(".{d}"))
}

pub fn is_social_video_self_url(url: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else {
        return true;
    };
    let h = u.host_str().unwrap_or("").to_lowercase();
    domain_matches(&h, "tiktok.com")
}

pub fn is_untrustworthy_or_junk_url(url: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else {
        return true;
    };
    let host = u.host_str().unwrap_or("").to_lowercase();
    let path = u.path().to_lowercase();

    if host.is_empty() {
        return true;
    }

    if is_social_video_self_url(url) {
        return true;
    }

    if host.ends_with(".google")
        || host.contains("google.")
        || host.ends_with("google.com")
        || host.ends_with("google.bg")
        || host.contains("familylink")
        || host.contains("families.google")
    {
        return true;
    }

    if host.contains("bing.com")
        || host.contains("duckduckgo.com")
        || host.contains("yahoo.com")
        || host.contains("yandex.")
        || host.contains("mail.ru")
        || host.contains("searx")
        || host.contains("baidu.com")
    {
        return true;
    }

    if host.contains("abv.bg")
        && (host.starts_with("passport.")
            || host.starts_with("help.")
            || host.starts_with("mail.")
            || host.starts_with("home.")
            || host.starts_with("trustsuite."))
    {
        return true;
    }

    if host.contains("play.google")
        || host.contains("apps.apple.com")
        || host.contains("microsoft.com")
    {
        return true;
    }

    if host.contains("casino")
        || host.contains("betano")
        || host.contains("palmsbet")
        || host.contains("inbet")
        || host.contains("efbet")
        || host.contains("winbet")
        || host.contains("sesame.bg")
        || host.contains("alphawin")
    {
        return true;
    }

    if host.contains("porn")
        || host.contains("xxx")
        || host.contains("xhamster")
        || host.contains("kompanionki")
        || host.contains("badoo")
        || host.contains("tinder")
        || host.contains("eskort")
        || host.contains("escort")
        || host.contains("elmaz.com")
        || host.contains("macki.info")
        || host.contains("ometv")
    {
        return true;
    }

    if host.contains("alo.bg")
        || host.contains("olx.bg")
        || host.contains("bazar.bg")
        || host.contains("ozone.bg")
        || host.contains("playhop.com")
    {
        return true;
    }

    if host.contains("bolshoyvopros.ru")
        || host.contains("otvet.mail.ru")
        || path.contains("/login")
        || path.contains("/signin")
        || path.contains("/signup")
        || path.contains("/auth")
    {
        return true;
    }

    false
}

fn parse_searxng_json(raw: &str) -> Vec<(String, String, String, Option<String>)> {
    let Ok(val) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let Some(arr) = val.get("results").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in arr {
        let Some(url) = item.get("url").and_then(|v| v.as_str()) else {
            continue;
        };
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let snippet = item.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let pub_date = item
            .get("publishedDate")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        out.push((url.to_string(), title, snippet, pub_date));
    }
    out
}

fn parse_bing_rss(xml: &str) -> Vec<(String, String, String, Option<String>)> {
    let mut out = Vec::new();
    for chunk in xml.split("<item>").skip(1) {
        let item_xml = chunk.split("</item>").next().unwrap_or(chunk);
        let Some(link) = extract_xml_tag(item_xml, "link") else {
            continue;
        };
        let link = link.trim().to_string();
        if !link.starts_with("http") {
            continue;
        }
        let title = strip_html_tags(&extract_xml_tag(item_xml, "title").unwrap_or_default());
        let desc = strip_html_tags(&extract_xml_tag(item_xml, "description").unwrap_or_default());
        let pub_date = extract_xml_tag(item_xml, "pubDate");
        out.push((link, title, desc, pub_date));
    }
    out
}

fn parse_ddg_html(html: &str) -> Vec<(String, String, String, Option<String>)> {
    let doc = Html::parse_document(html);
    let Ok(res_sel) = Selector::parse(".result") else {
        return Vec::new();
    };
    let Ok(a_sel) = Selector::parse("a.result__a") else {
        return Vec::new();
    };
    let Ok(snip_sel) = Selector::parse(".result__snippet") else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for block in doc.select(&res_sel) {
        let Some(a_el) = block.select(&a_sel).next() else {
            continue;
        };
        let Some(raw_href) = a_el.value().attr("href") else {
            continue;
        };
        let resolved_url = unwrap_ddg_redirect(raw_href);
        if !resolved_url.starts_with("http") {
            continue;
        }
        let title = a_el
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let snippet = block
            .select(&snip_sel)
            .next()
            .map(|s| {
                s.text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        out.push((resolved_url, title, snippet, None));
    }
    out
}

fn unwrap_ddg_redirect(href: &str) -> String {
    if let Some(idx) = href.find("uddg=") {
        let rest = &href[idx + 5..];
        let encoded = rest.split('&').next().unwrap_or(rest);
        if let Ok(decoded) = urlencoding::decode(encoded) {
            return decoded.into_owned();
        }
    }
    href.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_matching_handles_subdomains_and_rejects_lookalikes() {
        assert!(domain_matches("news.example.bg", "example.bg"));
        assert!(domain_matches("example.bg", "example.bg"));
        assert!(!domain_matches("badexample.bg", "example.bg"));
        assert!(!domain_matches("", "example.bg"));
    }

    #[test]
    fn tiktok_urls_are_rejected_as_self_evidence() {
        assert!(is_social_video_self_url("https://www.tiktok.com/@a/video/1"));
        assert!(is_social_video_self_url("https://vm.tiktok.com/abc"));
        assert!(!is_social_video_self_url("https://news.example.bg/a"));
        assert!(is_social_video_self_url("not a url"));
    }

    #[test]
    fn search_result_parsers_extract_urls_titles_and_dates() {
        let sx = r#"{"results":[{"url":"https://a.example.bg/1","title":"Т","content":"С","publishedDate":"2026-01-02"}]}"#;
        let hits = parse_searxng_json(sx);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].3.as_deref(), Some("2026-01-02"));

        let rss = "<rss><channel><item><title>Новина</title><link>https://b.example.bg/2</link><description>Описание</description><pubDate>Mon, 05 Jan 2026</pubDate></item></channel></rss>";
        let hits = parse_bing_rss(rss);
        assert_eq!(hits[0].0, "https://b.example.bg/2");
        assert_eq!(hits[0].1, "Новина");

        let ddg = r#"<div class="result"><a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fc.example.bg%2F3&rut=x">Заглавие</a><a class="result__snippet">Откъс</a></div>"#;
        let hits = parse_ddg_html(ddg);
        assert_eq!(hits[0].0, "https://c.example.bg/3");
        assert_eq!(hits[0].2, "Откъс");
    }

    #[test]
    fn junk_and_untrustworthy_urls_are_rejected() {
        assert!(is_untrustworthy_or_junk_url("https://familylink.google.com/"));
        assert!(is_untrustworthy_or_junk_url("https://families.google.com/intl/bg/faq"));
        assert!(is_untrustworthy_or_junk_url("https://www.bing.com/camera"));
        assert!(is_untrustworthy_or_junk_url("https://rt.pornhub.com/"));
        assert!(is_untrustworthy_or_junk_url("https://www.betano.bg/live"));
        assert!(is_untrustworthy_or_junk_url("https://passport.abv.bg/app/login"));
        assert!(!is_untrustworthy_or_junk_url("https://www.bta.bg/bg/news/123"));
        assert!(!is_untrustworthy_or_junk_url("https://www.mvr.bg/press/news"));
    }
}
