use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedSnapshot {
    pub url: String,
    pub timestamp: String,
    pub original_url: String,
    pub service: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternetArchiveDoc {
    pub identifier: String,
    pub title: String,
    pub date: Option<String>,
    pub mediatype: Option<String>,
    pub detail_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiRevision {
    pub user: String,
    pub timestamp: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiBioExtract {
    pub title: String,
    pub extract: String,
    pub language: String,
    pub revisions: Vec<WikiRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AegisTemporalAudit {
    pub target: String,
    pub snapshots_checked: usize,
    pub oldest_capture: Option<String>,
    pub newest_capture: Option<String>,
    pub historical_snapshots: Vec<ArchivedSnapshot>,
    pub internet_archive_docs: Vec<InternetArchiveDoc>,
    pub wiki_bio: Option<WikiBioExtract>,
    pub scrubbed_or_altered_risk: String,
    pub provenance_notes: Vec<String>,
}

#[derive(Clone)]
pub struct WebArchiveEngine {
    client: Client,
}

impl Default for WebArchiveEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WebArchiveEngine {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("TikTokLeadFinder/0.1 (OSINT Temporal Forensics; +https://archive.org)")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub async fn lookup_wayback_availability(&self, target_url: &str) -> Option<ArchivedSnapshot> {
        let clean = target_url.trim();
        if clean.is_empty() {
            return None;
        }
        let ep = format!("https://archive.org/wayback/available?url={}", urlencoding::encode(clean));
        let resp = self.client.get(&ep).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let val: Value = resp.json().await.ok()?;
        let closest = val.get("archived_snapshots")?.get("closest")?;
        let avail = closest.get("available")?.as_bool().unwrap_or(false);
        if !avail {
            return None;
        }
        let snap_url = closest.get("url")?.as_str()?.to_string();
        let timestamp = closest.get("timestamp")?.as_str().unwrap_or_default().to_string();
        let status = closest.get("status")?.as_str().unwrap_or("200").to_string();

        Some(ArchivedSnapshot {
            url: snap_url,
            timestamp,
            original_url: clean.to_string(),
            service: "Wayback Machine".to_string(),
            status,
        })
    }

    pub async fn lookup_wayback_cdx(&self, domain_or_url: &str, limit: usize) -> Vec<ArchivedSnapshot> {
        let clean = domain_or_url.trim().trim_start_matches("https://").trim_start_matches("http://");
        if clean.is_empty() {
            return Vec::new();
        }
        let target = clean.split('/').next().unwrap_or(clean);
        let ep = format!(
            "https://web.archive.org/cdx/search/cdx?url={}&output=json&limit={}&fl=timestamp,original,mimetype,statuscode",
            urlencoding::encode(target),
            limit.clamp(1, 20)
        );

        let mut results = Vec::new();
        if let Ok(resp) = self.client.get(&ep).send().await {
            if resp.status().is_success() {
                if let Ok(Value::Array(rows)) = resp.json::<Value>().await {
                    for row in rows.into_iter().skip(1) {
                        if let Value::Array(cols) = row {
                            if cols.len() >= 4 {
                                let ts = cols[0].as_str().unwrap_or_default().to_string();
                                let orig = cols[1].as_str().unwrap_or_default().to_string();
                                let status = cols[3].as_str().unwrap_or("200").to_string();
                                if !ts.is_empty() && !orig.is_empty() {
                                    let snap_url = format!("https://web.archive.org/web/{}/{}", ts, orig);
                                    results.push(ArchivedSnapshot {
                                        url: snap_url,
                                        timestamp: ts,
                                        original_url: orig,
                                        service: "Wayback CDX".to_string(),
                                        status,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        results
    }

    pub async fn search_internet_archive(&self, query: &str, limit: usize) -> Vec<InternetArchiveDoc> {
        let clean = query.trim();
        if clean.is_empty() {
            return Vec::new();
        }
        let ep = format!(
            "https://archive.org/advancedsearch.php?q={}&fl[]=identifier,title,date,description,mediatype&output=json&rows={}",
            urlencoding::encode(clean),
            limit.clamp(1, 15)
        );

        let mut docs = Vec::new();
        if let Ok(resp) = self.client.get(&ep).send().await {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<Value>().await {
                    if let Some(arr) = val.get("response").and_then(|r| r.get("docs")).and_then(|d| d.as_array()) {
                        for item in arr {
                            if let Some(id) = item.get("identifier").and_then(|v| v.as_str()) {
                                let title = item.get("title").and_then(|v| v.as_str()).unwrap_or(id).to_string();
                                let date = item.get("date").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let mediatype = item.get("mediatype").and_then(|v| v.as_str()).map(|s| s.to_string());
                                docs.push(InternetArchiveDoc {
                                    identifier: id.to_string(),
                                    title,
                                    date,
                                    mediatype,
                                    detail_url: format!("https://archive.org/details/{}", id),
                                });
                            }
                        }
                    }
                }
            }
        }
        docs
    }

    pub async fn fetch_wikipedia_biography(&self, name: &str) -> Option<WikiBioExtract> {
        let clean = name.trim();
        if clean.is_empty() {
            return None;
        }

        if let Some(bio) = self.fetch_wikipedia_for_lang(clean, "bg").await {
            if !bio.extract.trim().is_empty() {
                return Some(bio);
            }
        }

        self.fetch_wikipedia_for_lang(clean, "en").await
    }

    async fn fetch_wikipedia_for_lang(&self, name: &str, lang: &str) -> Option<WikiBioExtract> {
        let mut title_to_fetch = name.to_string();

        let search_url = format!(
            "https://{}.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&utf8=1&srlimit=1",
            lang,
            urlencoding::encode(name)
        );

        if let Ok(resp) = self.client.get(&search_url).send().await {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<Value>().await {
                    if let Some(first) = val.get("query").and_then(|q| q.get("search")).and_then(|s| s.as_array()).and_then(|a| a.first()) {
                        if let Some(t) = first.get("title").and_then(|v| v.as_str()) {
                            if !t.is_empty() {
                                title_to_fetch = t.to_string();
                            }
                        }
                    }
                }
            }
        }

        let extract_url = format!(
            "https://{}.wikipedia.org/w/api.php?action=query&prop=extracts|revisions&explaintext=1&redirects=1&rvprop=timestamp|user|comment&rvlimit=8&format=json&titles={}",
            lang,
            urlencoding::encode(&title_to_fetch)
        );

        let resp = self.client.get(&extract_url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let val: Value = resp.json().await.ok()?;
        let pages = val.get("query")?.get("pages")?.as_object()?;
        let (_, page) = pages.iter().next()?;
        let extract = page.get("extract")?.as_str()?.to_string();
        let canonical_title = page.get("title").and_then(|v| v.as_str()).unwrap_or(&title_to_fetch).to_string();

        if extract.trim().len() < 50 {
            return None;
        }

        let mut revisions = Vec::new();
        if let Some(rev_arr) = page.get("revisions").and_then(|v| v.as_array()) {
            for r in rev_arr {
                let user = r.get("user").and_then(|v| v.as_str()).unwrap_or("anon").to_string();
                let timestamp = r.get("timestamp").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let comment = r.get("comment").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                revisions.push(WikiRevision { user, timestamp, comment });
            }
        }

        Some(WikiBioExtract {
            title: canonical_title,
            extract,
            language: lang.to_string(),
            revisions,
        })
    }

    pub async fn run_aegis_temporal_audit(
        &self,
        name: &str,
        entity_type: &str,
        candidate_urls: &[String],
    ) -> AegisTemporalAudit {
        let (wiki_res, ia_docs) = tokio::join!(
            self.fetch_wikipedia_biography(name),
            self.search_internet_archive(name, 6)
        );

        let mut snapshots = Vec::new();
        let mut domains_to_probe = Vec::new();

        for u in candidate_urls {
            if let Ok(parsed) = url::Url::parse(u) {
                if let Some(host) = parsed.host_str() {
                    let d = host.to_lowercase();
                    if !domains_to_probe.contains(&d) && !d.contains("google.") && !d.contains("tiktok.") && !d.contains("bing.") {
                        domains_to_probe.push(d);
                    }
                }
            }
        }

        let name_low = name.to_lowercase();
        if (name_low.contains("радев") || name_low.contains("президент")) && !domains_to_probe.iter().any(|d| d.contains("president.bg")) {
            domains_to_probe.push("president.bg".to_string());
        } else if name_low.contains("министерски") || name_low.contains("премиер") {
            domains_to_probe.push("gov.bg".to_string());
        }

        for domain in domains_to_probe.iter().take(3) {
            let cdx_res = self.lookup_wayback_cdx(domain, 3).await;
            for s in cdx_res {
                snapshots.push(s);
            }
            if let Some(avail) = self.lookup_wayback_availability(&format!("https://{}", domain)).await {
                if !snapshots.iter().any(|s| s.url == avail.url) {
                    snapshots.push(avail);
                }
            }
        }

        snapshots.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        let oldest = snapshots.first().map(|s| s.timestamp.clone());
        let newest = snapshots.last().map(|s| s.timestamp.clone());

        let mut provenance_notes = Vec::new();
        let mut scrubbed_or_altered_risk = "Ниска вероятност за заличаване (стабилен времеви отпечатък)".to_string();

        if let Some(ref w) = wiki_res {
            provenance_notes.push(format!("Енциклопедичен запис в Уикипедия [{}] с обем от {} символа.", w.language, w.extract.chars().count()));
            if !w.revisions.is_empty() {
                provenance_notes.push(format!("Последна редакция на статията: {} от потребител '{}' (Коментар: '{}').", w.revisions[0].timestamp, w.revisions[0].user, w.revisions[0].comment));
            }
        } else {
            provenance_notes.push("Липсва потвърдена статия в Уикипедия - субектът няма самостоятелен енциклопедичен отпечатък.".to_string());
        }

        if !snapshots.is_empty() {
            provenance_notes.push(format!("Wayback Machine регистрира най-ранен архив: {} и най-нов архив: {}.", oldest.as_deref().unwrap_or("неизвестен"), newest.as_deref().unwrap_or("неизвестен")));
        } else {
            provenance_notes.push("Няма открити директни Wayback Machine архиви за асоциираните уеб адреси.".to_string());
        }

        if !ia_docs.is_empty() {
            provenance_notes.push(format!("Internet Archive открива {} архивирани медийни записа, документи или радиопредавания.", ia_docs.len()));
        }

        if snapshots.is_empty() && wiki_res.is_none() && (entity_type == "person" || entity_type == "institution") {
            scrubbed_or_altered_risk = "Умерена към висока: липсват дългосрочни дигитални архиви преди появата в социалните мрежи.".to_string();
        }

        AegisTemporalAudit {
            target: name.to_string(),
            snapshots_checked: snapshots.len(),
            oldest_capture: oldest,
            newest_capture: newest,
            historical_snapshots: snapshots,
            internet_archive_docs: ia_docs,
            wiki_bio: wiki_res,
            scrubbed_or_altered_risk,
            provenance_notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_initializes_with_default() {
        let eng = WebArchiveEngine::default();
        assert_eq!(std::mem::size_of_val(&eng) > 0, true);
    }
}
