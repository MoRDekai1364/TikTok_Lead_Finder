use std::collections::HashSet;
use std::sync::Arc;
use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use leadfinder_core::{
    resolve_secret, ArchiveEndpointConfig, ArchiveMatch, FactCheckArchive, HttpTransportMode,
    PolitenessGuard, ProviderError, ProviderErrorKind,
};
use crate::lexicon::LexiconNormalizer;

#[derive(Clone)]
pub struct RobotsTxtChecker {
    guard: PolitenessGuard,
    client: Client,
}

impl RobotsTxtChecker {
    pub fn new(guard: PolitenessGuard, client: Client) -> Self {
        Self { guard, client }
    }

    pub async fn is_allowed(&self, target_url: &str) -> bool {
        let Ok(parsed) = reqwest::Url::parse(target_url) else {
            return false;
        };
        let Some(host) = parsed.host_str() else {
            return false;
        };
        let robots_url = format!("{}://{}/robots.txt", parsed.scheme(), host);
        let path_and_query = match parsed.query() {
            Some(q) => format!("{}?{}", parsed.path(), q),
            None => parsed.path().to_string(),
        };

        let body = if let Some(cached) = self.guard.get_cached(&robots_url) {
            cached.body
        } else if self.guard.mode() == HttpTransportMode::ReplayFixturesOnly {
            return true;
        } else {
            if self.guard.acquire_slot(host, "robots_txt").await.is_err() {
                return false;
            }
            match self.client.get(&robots_url).send().await {
                Ok(resp) => {
                    let code = resp.status().as_u16();
                    if resp.status().is_success() {
                        let txt = resp.text().await.unwrap_or_default();
                        self.guard.record_success(host).await;
                        self.guard
                            .put_cached(&robots_url, code, txt.clone(), Some("text/plain".to_string()));
                        txt
                    } else if code == 403 || code == 429 {
                        self.guard
                            .record_failure(host, &format!("robots HTTP {code}"))
                            .await;
                        return false;
                    } else {
                        self.guard
                            .put_cached(&robots_url, code, String::new(), Some("text/plain".to_string()));
                        String::new()
                    }
                }
                Err(_) => return false,
            }
        };

        check_robots_rules(&body, &path_and_query)
    }
}

pub fn check_robots_rules(robots_txt: &str, path_and_query: &str) -> bool {
    if robots_txt.trim().is_empty() {
        return true;
    }
    let mut applies = false;
    let mut saw_rule_in_group = false;
    let mut disallows: Vec<String> = Vec::new();
    let mut allows: Vec<String> = Vec::new();

    for raw_line in robots_txt.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim().to_lowercase();
        let val = v.trim();

        if key == "user-agent" {
            let ua = val.to_lowercase();
            let matching_ua = ua == "*" || ua.contains("tiktokleadfinder");
            if saw_rule_in_group {
                applies = matching_ua;
                saw_rule_in_group = false;
            } else {
                applies = applies || matching_ua;
            }
        } else if key == "disallow" {
            saw_rule_in_group = true;
            if applies && !val.is_empty() {
                disallows.push(val.to_string());
            }
        } else if key == "allow" {
            saw_rule_in_group = true;
            if applies && !val.is_empty() {
                allows.push(val.to_string());
            }
        }
    }

    let mut longest_disallow = 0usize;
    for pat in &disallows {
        if robots_pattern_matches(pat, path_and_query) {
            longest_disallow = longest_disallow.max(pat.len());
        }
    }
    if longest_disallow == 0 {
        return true;
    }

    let mut longest_allow = 0usize;
    for pat in &allows {
        if robots_pattern_matches(pat, path_and_query) {
            longest_allow = longest_allow.max(pat.len());
        }
    }
    longest_allow >= longest_disallow
}

fn robots_pattern_matches(pattern: &str, path: &str) -> bool {
    let end_anchored = pattern.ends_with('$');
    let clean_pat = if end_anchored {
        &pattern[..pattern.len() - 1]
    } else {
        pattern
    };
    let parts: Vec<&str> = clean_pat.split('*').collect();
    if parts.len() == 1 {
        return if end_anchored {
            path == clean_pat
        } else {
            path.starts_with(clean_pat)
        };
    }

    let mut pos = 0usize;
    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if idx == 0 && !clean_pat.starts_with('*') {
            if !path.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if let Some(rel) = path[pos..].find(part) {
            pos += rel + part.len();
        } else {
            return false;
        }
    }
    if end_anchored {
        if let Some(last) = parts.last() {
            if !last.is_empty() && !path.ends_with(last) {
                return false;
            }
        }
    }
    true
}

pub struct ConfiguredFactCheckArchive {
    endpoints: Vec<ArchiveEndpointConfig>,
    lexicon: Arc<LexiconNormalizer>,
    guard: PolitenessGuard,
    robots: RobotsTxtChecker,
    client: Client,
    language_code: String,
}

impl ConfiguredFactCheckArchive {
    pub fn new(
        endpoints: Vec<ArchiveEndpointConfig>,
        lexicon: Arc<LexiconNormalizer>,
        guard: PolitenessGuard,
        language_code: impl Into<String>,
    ) -> Self {
        let client = Client::builder()
            .user_agent("TikTokLeadFinder/0.1 (local fact-checking research assistant)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| Client::new());
        let robots = RobotsTxtChecker::new(guard.clone(), client.clone());
        Self {
            endpoints,
            lexicon,
            guard,
            robots,
            client,
            language_code: language_code.into(),
        }
    }

    async fn fetch_cached(&self, url: &str) -> Option<String> {
        if let Some(hit) = self.guard.get_cached(url) {
            if hit.status >= 200 && hit.status < 300 && !hit.body.is_empty() {
                return Some(hit.body);
            }
            return None;
        }
        if self.guard.mode() == HttpTransportMode::ReplayFixturesOnly {
            return None;
        }
        if !self.robots.is_allowed(url).await {
            return None;
        }
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_default();
        if self.guard.acquire_slot(&host, "factcheck_archive").await.is_err() {
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

    fn build_compact_query(&self, claim_text: &str) -> String {
        let mut terms = Vec::new();
        for tok in claim_text.split_whitespace() {
            let clean = tok.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.chars().count() >= 4 || clean.chars().any(|c| c.is_ascii_digit()) {
                terms.push(clean.to_string());
            }
            if terms.len() >= 6 {
                break;
            }
        }
        if terms.is_empty() {
            claim_text.trim().to_string()
        } else {
            terms.join(" ")
        }
    }

    pub fn similarity(&self, claim_text: &str, candidate_text: &str) -> f64 {
        compute_lemma_trigram_similarity(&self.lexicon, claim_text, candidate_text)
    }
}

#[async_trait]
impl FactCheckArchive for ConfiguredFactCheckArchive {
    fn name(&self) -> &str {
        "configured_factcheck_archives"
    }

    async fn find_fact_checks(&self, claim_text: &str) -> Result<Vec<ArchiveMatch>, ProviderError> {
        let compact_q = self.build_compact_query(claim_text);
        if compact_q.is_empty() {
            return Ok(Vec::new());
        }

        let mut matches: Vec<ArchiveMatch> = Vec::new();
        let mut seen_urls: HashSet<String> = HashSet::new();

        for ep in self.endpoints.iter().filter(|e| e.enabled) {
            let base = ep.base_url.trim_end_matches('/');

            if ep.feed_url.contains("/feed") {
                let wp_url = format!(
                    "{base}/wp-json/wp/v2/posts?per_page=8&search={}",
                    urlencoding::encode(&compact_q)
                );
                if let Some(wp_body) = self.fetch_cached(&wp_url).await {
                    for item in parse_wp_json_posts(&wp_body, &ep.name, &self.lexicon, claim_text) {
                        if seen_urls.insert(item.url.clone()) {
                            matches.push(item);
                        }
                    }
                }
            }

            if !ep.feed_url.trim().is_empty() {
                if let Some(rss_body) = self.fetch_cached(&ep.feed_url).await {
                    for item in parse_rss_items(&rss_body, &ep.name, &self.lexicon, claim_text) {
                        if item.similarity >= 0.12 && seen_urls.insert(item.url.clone()) {
                            matches.push(item);
                        }
                    }
                }
            }

            if !ep.search_url_template.trim().is_empty() {
                let search_url = ep
                    .search_url_template
                    .replace("{query}", &urlencoding::encode(&compact_q));
                if let Some(html_body) = self.fetch_cached(&search_url).await {
                    for item in parse_html_archive_results(
                        &html_body,
                        base,
                        &ep.name,
                        &self.lexicon,
                        claim_text,
                    ) {
                        if item.similarity >= 0.12 && seen_urls.insert(item.url.clone()) {
                            matches.push(item);
                        }
                    }
                }
            }
        }

        if let Some(fc_key) = resolve_secret("GOOGLE_FACTCHECK_API_KEY") {
            let claim_review_url = format!(
                "https://factchecktools.googleapis.com/v1alpha1/claims:search?query={}&languageCode={}&pageSize=6&key={}",
                urlencoding::encode(&compact_q),
                urlencoding::encode(&self.language_code),
                urlencoding::encode(&fc_key)
            );
            if let Some(cr_body) = self.fetch_cached(&claim_review_url).await {
                for item in parse_claim_review_json(&cr_body, &self.lexicon, claim_text) {
                    if seen_urls.insert(item.url.clone()) {
                        matches.push(item);
                    }
                }
            }
        }

        matches.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(10);

        let _ = ProviderErrorKind::Internal;
        Ok(matches)
    }
}

fn parse_wp_json_posts(
    raw_json: &str,
    archive_name: &str,
    lexicon: &LexiconNormalizer,
    claim_text: &str,
) -> Vec<ArchiveMatch> {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(raw_json) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for obj in arr {
        let Some(url) = obj.get("link").and_then(|v| v.as_str()) else {
            continue;
        };
        let title_html = obj
            .get("title")
            .and_then(|t| t.get("rendered"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let excerpt_html = obj
            .get("excerpt")
            .and_then(|t| t.get("rendered"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let title = strip_html_tags(title_html);
        let summary = strip_html_tags(excerpt_html);
        let published_at = obj.get("date").and_then(|v| v.as_str()).map(|s| s.to_string());
        let combined = format!("{title} {summary}");
        let sim = compute_lemma_trigram_similarity(lexicon, claim_text, &combined);
        out.push(ArchiveMatch {
            archive_name: archive_name.to_string(),
            url: url.to_string(),
            title,
            summary,
            verdict_text: None,
            published_at,
            similarity: (sim * 1000.0).round() / 1000.0,
        });
    }
    out
}

fn parse_rss_items(
    xml: &str,
    archive_name: &str,
    lexicon: &LexiconNormalizer,
    claim_text: &str,
) -> Vec<ArchiveMatch> {
    let mut out = Vec::new();
    for chunk in xml.split("<item>").skip(1) {
        let item_xml = chunk.split("</item>").next().unwrap_or(chunk);
        let title = strip_html_tags(&extract_xml_tag(item_xml, "title").unwrap_or_default());
        let link = extract_xml_tag(item_xml, "link").unwrap_or_default().trim().to_string();
        if link.is_empty() || !link.starts_with("http") {
            continue;
        }
        let desc = strip_html_tags(&extract_xml_tag(item_xml, "description").unwrap_or_default());
        let pub_date = extract_xml_tag(item_xml, "pubDate");
        let combined = format!("{title} {desc}");
        let sim = compute_lemma_trigram_similarity(lexicon, claim_text, &combined);
        out.push(ArchiveMatch {
            archive_name: archive_name.to_string(),
            url: link,
            title,
            summary: desc,
            verdict_text: None,
            published_at: pub_date,
            similarity: (sim * 1000.0).round() / 1000.0,
        });
    }
    out
}

fn parse_html_archive_results(
    html: &str,
    base_url: &str,
    archive_name: &str,
    lexicon: &LexiconNormalizer,
    claim_text: &str,
) -> Vec<ArchiveMatch> {
    let doc = Html::parse_document(html);
    let Ok(a_sel) = Selector::parse("article a[href], h2 a[href], h3 a[href], .search-results a[href]") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for el in doc.select(&a_sel) {
        let Some(href) = el.value().attr("href") else {
            continue;
        };
        let full_url = if href.starts_with("http://") || href.starts_with("https://") {
            href.to_string()
        } else if href.starts_with('/') {
            format!("{base_url}{href}")
        } else {
            continue;
        };
        if !seen.insert(full_url.clone()) {
            continue;
        }
        let title = el.text().collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
        if title.chars().count() < 12 {
            continue;
        }
        let sim = compute_lemma_trigram_similarity(lexicon, claim_text, &title);
        out.push(ArchiveMatch {
            archive_name: archive_name.to_string(),
            url: full_url,
            title: title.clone(),
            summary: String::new(),
            verdict_text: None,
            published_at: None,
            similarity: (sim * 1000.0).round() / 1000.0,
        });
    }
    out
}

fn parse_claim_review_json(
    raw_json: &str,
    lexicon: &LexiconNormalizer,
    claim_text: &str,
) -> Vec<ArchiveMatch> {
    let Ok(val) = serde_json::from_str::<Value>(raw_json) else {
        return Vec::new();
    };
    let Some(claims) = val.get("claims").and_then(|c| c.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for c in claims {
        let c_text = c.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let Some(reviews) = c.get("claimReview").and_then(|r| r.as_array()) else {
            continue;
        };
        for rev in reviews {
            let Some(url) = rev.get("url").and_then(|u| u.as_str()) else {
                continue;
            };
            let title = rev.get("title").and_then(|t| t.as_str()).unwrap_or(c_text);
            let pub_name = rev
                .get("publisher")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("ClaimReview");
            let textual_rating = rev
                .get("textualRating")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string());
            let review_date = rev
                .get("reviewDate")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            let combined = format!("{c_text} {title}");
            let sim = compute_lemma_trigram_similarity(lexicon, claim_text, &combined);
            out.push(ArchiveMatch {
                archive_name: pub_name.to_string(),
                url: url.to_string(),
                title: title.to_string(),
                summary: c_text.to_string(),
                verdict_text: textual_rating,
                published_at: review_date,
                similarity: (sim * 1000.0).round() / 1000.0,
            });
        }
    }
    out
}

pub fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let raw = xml[start..end].trim();
    let unwrapped = raw
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
        .unwrap_or(raw);
    Some(unwrapped.trim().to_string())
}

pub fn strip_html_tags(input: &str) -> String {
    let frag = Html::parse_fragment(input);
    frag.root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn compute_lemma_trigram_similarity(
    lexicon: &LexiconNormalizer,
    a: &str,
    b: &str,
) -> f64 {
    let a_lemmas = lemma_set(lexicon, a);
    let b_lemmas = lemma_set(lexicon, b);
    let lemma_jaccard = if a_lemmas.is_empty() || b_lemmas.is_empty() {
        0.0
    } else {
        let inter = a_lemmas.intersection(&b_lemmas).count() as f64;
        let union = a_lemmas.union(&b_lemmas).count().max(1) as f64;
        inter / union
    };

    let a_tri = char_trigrams(a);
    let b_tri = char_trigrams(b);
    let trigram_dice = if a_tri.is_empty() || b_tri.is_empty() {
        0.0
    } else {
        let inter = a_tri.intersection(&b_tri).count() as f64;
        (2.0 * inter) / ((a_tri.len() + b_tri.len()).max(1) as f64)
    };

    0.65 * lemma_jaccard + 0.35 * trigram_dice
}

fn lemma_set(lexicon: &LexiconNormalizer, text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 || w.chars().any(|c| c.is_ascii_digit()))
        .map(|w| lexicon.lemmatize_word(w))
        .collect()
}

fn char_trigrams(text: &str) -> HashSet<String> {
    let norm: Vec<char> = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    if norm.len() < 3 {
        return HashSet::new();
    }
    let mut set = HashSet::with_capacity(norm.len());
    for win in norm.windows(3) {
        set.insert(win.iter().collect());
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    const BNR_ROBOTS: &str = "User-agent: *\nDisallow: /*/search\nDisallow: /*/find\nAllow: /bg/news\nSitemap: https://bnr.bg/sitemap.xml\n";

    #[test]
    fn robots_rules_block_disallowed_search_and_allow_feeds() {
        assert!(!check_robots_rules(BNR_ROBOTS, "/bg/search?q=test"));
        assert!(!check_robots_rules(BNR_ROBOTS, "/bg/find"));
        assert!(check_robots_rules(BNR_ROBOTS, "/bg/news/item"));
        assert!(check_robots_rules(BNR_ROBOTS, "/sitemap.xml"));
        assert!(check_robots_rules("", "/anything"));
    }

    #[test]
    fn robots_longest_allow_overrides_shorter_disallow() {
        let txt = "User-agent: *\nDisallow: /private\nAllow: /private/public\n";
        assert!(!check_robots_rules(txt, "/private/secret"));
        assert!(check_robots_rules(txt, "/private/public/page"));
    }

    #[test]
    fn xml_and_html_helpers_unwrap_cdata_and_strip_markup() {
        let item = "<title><![CDATA[Заглавие <b>тест</b>]]></title><link>https://a.example.invalid/x</link>";
        assert_eq!(
            extract_xml_tag(item, "link").as_deref(),
            Some("https://a.example.invalid/x")
        );
        let title = extract_xml_tag(item, "title").unwrap();
        assert_eq!(strip_html_tags(&title), "Заглавие тест");
        assert!(extract_xml_tag(item, "missing").is_none());
    }

    #[test]
    fn char_trigram_similarity_signals_overlap() {
        let a = char_trigrams("цените на тока поскъпват");
        let b = char_trigrams("цените на тока поскъпнаха");
        let c = char_trigrams("времето утре ще бъде слънчево");
        let overlap_ab = a.intersection(&b).count();
        let overlap_ac = a.intersection(&c).count();
        assert!(overlap_ab > overlap_ac);
    }
}
