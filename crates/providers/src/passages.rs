use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use scraper::{Html, Selector};
use leadfinder_core::{
    HttpTransportMode, PolitenessGuard, SearchHit,
};
use crate::archive::RobotsTxtChecker;
use crate::lexicon::LexiconNormalizer;

#[derive(Debug, Clone)]
pub struct RankedPassage {
    pub passage_id: String,
    pub source_url: String,
    pub source_name: Option<String>,
    pub source_kind: String,
    pub published_at: Option<String>,
    pub text: String,
    pub score: f64,
}

pub struct PassageExtractorAndRanker {
    guard: PolitenessGuard,
    robots: RobotsTxtChecker,
    lexicon: Arc<LexiconNormalizer>,
    client: reqwest::Client,
}

impl PassageExtractorAndRanker {
    pub fn new(
        guard: PolitenessGuard,
        lexicon: Arc<LexiconNormalizer>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("TikTokLeadFinder/0.1 (local fact-checking research assistant)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let robots = RobotsTxtChecker::new(guard.clone(), client.clone());
        Self {
            guard,
            robots,
            lexicon,
            client,
        }
    }

    pub async fn fetch_html_cached(&self, url: &str) -> Option<String> {
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
        if self.guard.acquire_slot(&host, "fetch_passage").await.is_err() {
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

    pub fn extract_main_text(&self, html: &str) -> String {
        let doc = Html::parse_document(html);
        let article_selectors = [
            "article",
            "main",
            "[role='main']",
            ".article-body",
            ".entry-content",
            ".post-content",
            ".news-article",
            ".story-content",
            "#content",
        ];

        for sel_str in article_selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                let mut full = String::new();
                for el in doc.select(&sel) {
                    let text = el.text().collect::<Vec<_>>().join(" ");
                    full.push_str(&text);
                    full.push('\n');
                }
                let clean = clean_extracted_text(&full);
                if clean.chars().count() >= 120 {
                    return clean;
                }
            }
        }

        let body_sel = Selector::parse("body").unwrap();
        if let Some(body) = doc.select(&body_sel).next() {
            clean_extracted_text(&body.text().collect::<Vec<_>>().join(" "))
        } else {
            String::new()
        }
    }

    pub fn chunk_text(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let sentences: Vec<&str> = text
            .split(|c: char| c == '.' || c == '!' || c == '?' || c == '\n')
            .map(|s| s.trim())
            .filter(|s| s.chars().count() >= 15)
            .collect();

        if sentences.is_empty() {
            return chunks;
        }

        let mut cur = String::new();
        for s in sentences {
            if cur.chars().count() + s.chars().count() > 380 && !cur.is_empty() {
                chunks.push(cur.clone());
                cur.clear();
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(s);
            cur.push('.');
        }
        if !cur.is_empty() {
            chunks.push(cur);
        }
        chunks
    }

    pub fn rank_passages(
        &self,
        claim_text: &str,
        candidates: &[(SearchHit, String)],
        top_n: usize,
    ) -> Vec<RankedPassage> {
        let mut all_passages = Vec::new();
        let query_lemmas = extract_lemmas(&self.lexicon, claim_text);
        if query_lemmas.is_empty() {
            return Vec::new();
        }

        for (hit, full_text) in candidates {
            let chunks = if full_text.trim().is_empty() {
                if !hit.snippet.trim().is_empty() {
                    vec![hit.snippet.clone()]
                } else {
                    Vec::new()
                }
            } else {
                let mut c = self.chunk_text(full_text);
                if !hit.snippet.trim().is_empty() {
                    c.push(hit.snippet.clone());
                }
                c
            };

            for chunk in chunks {
                if chunk.chars().count() < 30 {
                    continue;
                }
                let pass_lemmas = extract_lemmas(&self.lexicon, &chunk);
                let score = compute_bm25_score(&query_lemmas, &pass_lemmas);
                if score < 0.25 {
                    continue;
                }
                let kind_bonus = match hit.source_kind.as_str() {
                    "factcheck" => 1.35,
                    "primary" => 1.25,
                    "news" => 1.10,
                    _ => 1.0,
                };
                let final_score = score * kind_bonus;
                all_passages.push((
                    final_score,
                    hit.url.clone(),
                    hit.source_name.clone(),
                    hit.source_kind.clone(),
                    hit.published_at.clone(),
                    chunk,
                ));
            }
        }

        all_passages.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut seen_texts = HashSet::new();
        let mut out = Vec::new();
        for (score, url, name, kind, pub_date, text) in all_passages {
            let clean_prefix: String = text.chars().take(80).collect();
            if !seen_texts.insert(clean_prefix) {
                continue;
            }
            let pid = format!("p{}", out.len() + 1);
            out.push(RankedPassage {
                passage_id: pid,
                source_url: url,
                source_name: name,
                source_kind: kind,
                published_at: pub_date,
                text,
                score: (score * 1000.0).round() / 1000.0,
            });
            if out.len() >= top_n {
                break;
            }
        }

        out
    }
}

fn clean_extracted_text(raw: &str) -> String {
    let mut lines = Vec::new();
    for l in raw.lines() {
        let trimmed = l.trim();
        if trimmed.is_empty() {
            continue;
        }
        let low = trimmed.to_lowercase();
        if low.starts_with("cookie")
            || low.contains("всички права запазени")
            || low.contains("all rights reserved")
            || low.contains("политика за бисквитки")
            || low.contains("абонирайте се")
            || low.contains("var ")
            || low.contains("function(")
            || low.contains("function ")
            || low.contains("window[")
            || low.contains("document.")
            || low.contains("urlsearchparams")
            || low.contains("addeventlistener")
            || low.contains("{var ")
            || low.contains("bind(")
            || low.starts_with("-- function")
            || low.starts_with("!function")
            || low.contains("display:none")
        {
            continue;
        }
        lines.push(trimmed);
    }
    lines.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_lemmas(lexicon: &LexiconNormalizer, text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 || w.chars().any(|c| c.is_ascii_digit()))
        .map(|w| lexicon.lemmatize_word(w))
        .collect()
}

fn compute_bm25_score(query: &[String], passage: &[String]) -> f64 {
    let mut tf: HashMap<&str, usize> = HashMap::new();
    for word in passage {
        *tf.entry(word.as_str()).or_insert(0) += 1;
    }

    let k1 = 1.2;
    let b = 0.75;
    let avg_doc_len = 50.0;
    let doc_len = passage.len() as f64;

    let mut score = 0.0;
    let mut query_unique = HashSet::new();
    for q in query {
        if query_unique.insert(q.as_str()) {
            let count = tf.get(q.as_str()).copied().unwrap_or(0) as f64;
            if count > 0.0 {
                let idf = 1.5;
                let num = count * (k1 + 1.0);
                let den = count + k1 * (1.0 - b + b * (doc_len / avg_doc_len));
                score += idf * (num / den);
            }
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bm25_prefers_passages_with_more_query_terms() {
        let q: Vec<String> = ["цени", "ток", "поскъпване"].iter().map(|s| s.to_string()).collect();
        let strong: Vec<String> = ["цени", "ток", "поскъпване", "април"].iter().map(|s| s.to_string()).collect();
        let weak: Vec<String> = ["времето", "слънчево", "цени"].iter().map(|s| s.to_string()).collect();
        assert!(compute_bm25_score(&q, &strong) > compute_bm25_score(&q, &weak));
        assert_eq!(compute_bm25_score(&q, &[]), 0.0);
    }

    #[test]
    fn boilerplate_lines_are_removed_from_extracted_text() {
        let raw = "Основен текст на статията.\nCookie политика\nВсички права запазени\nvar queryParams = new URLSearchParams(window.search);\n-- function oamSetHiddenInput(formname, name, value) {\nОще текст.";
        let clean = clean_extracted_text(raw);
        assert!(clean.contains("Основен текст"));
        assert!(clean.contains("Още текст"));
        assert!(!clean.to_lowercase().contains("cookie"));
        assert!(!clean.contains("права запазени"));
        assert!(!clean.contains("queryParams"));
        assert!(!clean.contains("oamSetHiddenInput"));
    }
}
