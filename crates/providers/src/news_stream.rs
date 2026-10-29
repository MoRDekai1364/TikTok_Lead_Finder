use anyhow::Result;
use chrono::Utc;
use leadfinder_core::EntityIntelEvent;
use reqwest::Client;
use std::time::Duration;

pub struct DynamicNewsStream {
    client: Client,
}

impl Default for DynamicNewsStream {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicNewsStream {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub async fn fetch_events_for_entity(
        &self,
        person_id: i64,
        name: &str,
        aliases: &[String],
        limit: usize,
    ) -> Result<Vec<EntityIntelEvent>> {
        let clean_name = name.trim();
        if clean_name.is_empty() {
            return Ok(Vec::new());
        }

        let mut events = Vec::new();
        let google_rss_url = format!(
            "https://news.google.com/rss/search?q={}&hl=bg&gl=BG&ceid=BG:bg",
            urlencoding::encode(clean_name)
        );

        if let Ok(g_events) = self.fetch_rss_url(&google_rss_url, "Google News BG", person_id, clean_name).await {
            events.extend(g_events);
        }

        if events.len() < limit && !aliases.is_empty() {
            if let Some(alias) = aliases.first() {
                let alias_rss_url = format!(
                    "https://news.google.com/rss/search?q={}&hl=bg&gl=BG&ceid=BG:bg",
                    urlencoding::encode(alias.trim())
                );
                if let Ok(a_events) = self.fetch_rss_url(&alias_rss_url, "Google News BG", person_id, alias).await {
                    for ev in a_events {
                        if !events.iter().any(|x| x.url == ev.url || x.title == ev.title) {
                            events.push(ev);
                        }
                    }
                }
            }
        }

        let bta_url = "https://www.bta.bg/bg/rss";
        if let Ok(bta_events) = self.fetch_rss_url(bta_url, "БТА Новини", person_id, clean_name).await {
            for ev in bta_events {
                if !events.iter().any(|x| x.url == ev.url || x.title == ev.title) {
                    events.push(ev);
                }
            }
        }

        let bnr_url = "https://bnr.bg/rss";
        if let Ok(bnr_events) = self.fetch_rss_url(bnr_url, "БНР Новини", person_id, clean_name).await {
            for ev in bnr_events {
                if !events.iter().any(|x| x.url == ev.url || x.title == ev.title) {
                    events.push(ev);
                }
            }
        }

        events.truncate(limit);
        Ok(events)
    }

    async fn fetch_rss_url(
        &self,
        url: &str,
        fallback_source: &str,
        person_id: i64,
        target_name: &str,
    ) -> Result<Vec<EntityIntelEvent>> {
        let resp = self.client.get(url).send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let xml_text = resp.text().await?;
        let parsed = Self::parse_rss_items(&xml_text, fallback_source, person_id, target_name);
        Ok(parsed)
    }

    fn parse_rss_items(
        xml: &str,
        fallback_source: &str,
        person_id: i64,
        target_name: &str,
    ) -> Vec<EntityIntelEvent> {
        let mut results = Vec::new();
        let target_tokens: Vec<String> = target_name
            .to_lowercase()
            .split_whitespace()
            .filter(|s| s.chars().count() >= 3)
            .map(|s| s.to_string())
            .collect();

        let items: Vec<&str> = xml.split("<item>").skip(1).collect();
        for item_block in items {
            let item_content = item_block.split("</item>").next().unwrap_or("");
            if item_content.is_empty() {
                continue;
            }

            let title = Self::extract_tag_value(item_content, "title");
            let link = Self::extract_tag_value(item_content, "link");
            let pub_date = Self::extract_tag_value(item_content, "pubDate");
            let description = Self::extract_tag_value(item_content, "description");
            let source = Self::extract_tag_value(item_content, "source");

            if title.is_empty() || link.is_empty() {
                continue;
            }

            let clean_title = Self::clean_xml_text(&title);
            let clean_snippet = Self::clean_xml_text(&description);
            let final_source = if !source.is_empty() {
                Self::clean_xml_text(&source)
            } else {
                fallback_source.to_string()
            };

            let combined = format!("{} {}", clean_title, clean_snippet).to_lowercase();
            let mut matches_entity = target_tokens.is_empty();
            if !target_tokens.is_empty() {
                for tok in &target_tokens {
                    if combined.contains(tok) {
                        matches_entity = true;
                        break;
                    }
                }
            }

            if matches_entity {
                let discovered_at = if !pub_date.is_empty() {
                    chrono::DateTime::parse_from_rfc2822(&pub_date)
                        .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
                        .unwrap_or_else(|_| Utc::now().to_rfc3339())
                } else {
                    Utc::now().to_rfc3339()
                };

                results.push(EntityIntelEvent {
                    id: 0,
                    person_id,
                    source_name: final_source,
                    title: clean_title,
                    url: link.trim().to_string(),
                    discovered_at,
                    snippet: if clean_snippet.is_empty() {
                        "Позоваване в новинарски поток".to_string()
                    } else {
                        clean_snippet
                    },
                });
            }
        }

        results
    }

    fn extract_tag_value(xml: &str, tag: &str) -> String {
        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);

        if let Some(start) = xml.find(&open_tag) {
            let after = &xml[start + open_tag.len()..];
            if let Some(end) = after.find(&close_tag) {
                return after[..end].trim().to_string();
            }
        }

        let prefix = format!("<{}", tag);
        if let Some(start) = xml.find(&prefix) {
            let after = &xml[start..];
            if let Some(gt) = after.find('>') {
                let body = &after[gt + 1..];
                if let Some(end) = body.find(&close_tag) {
                    return body[..end].trim().to_string();
                }
            }
        }

        String::new()
    }

    fn clean_xml_text(s: &str) -> String {
        let mut text = s.replace("<![CDATA[", "").replace("]]>", "");
        while let Some(st) = text.find('<') {
            if let Some(en) = text[st..].find('>') {
                text.replace_range(st..st + en + 1, " ");
            } else {
                break;
            }
        }
        text = text
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ");

        text.split_whitespace().collect::<Vec<&str>>().join(" ")
    }
}
