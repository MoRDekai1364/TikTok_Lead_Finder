use anyhow::Result;
use leadfinder_core::SanctionsMatch;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub struct SanctionsMatcher {
    client: Client,
    api_endpoint: String,
}

impl Default for SanctionsMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SanctionsMatcher {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("TikTokLeadFinder/0.1 (sanctions-auditor)")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            api_endpoint: "https://api.opensanctions.org/search/default".to_string(),
        }
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.api_endpoint = endpoint.into();
        s
    }

    pub async fn check_entity(&self, name: &str, aliases: &[String]) -> Result<Vec<SanctionsMatch>> {
        let clean_name = name.trim();
        if clean_name.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_queries = vec![clean_name.to_string()];
        for alias in aliases {
            let a = alias.trim();
            if !a.is_empty() && !all_queries.iter().any(|q| q.eq_ignore_ascii_case(a)) {
                all_queries.push(a.to_string());
            }
        }

        let mut matches = Vec::new();

        for q in all_queries.iter().take(3) {
            if let Ok(api_matches) = self.query_opensanctions(q).await {
                for m in api_matches {
                    if !matches.iter().any(|existing: &SanctionsMatch| existing.caption.eq_ignore_ascii_case(&m.caption) && existing.dataset == m.dataset) {
                        matches.push(m);
                    }
                }
            }
        }

        let local_matches = Self::check_local_sanctions_registry(clean_name, aliases);
        for lm in local_matches {
            if !matches.iter().any(|existing: &SanctionsMatch| existing.dataset == lm.dataset && existing.caption.eq_ignore_ascii_case(&lm.caption)) {
                matches.push(lm);
            }
        }

        matches.sort_by(|a, b| b.match_score.partial_cmp(&a.match_score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(matches)
    }

    async fn query_opensanctions(&self, query: &str) -> Result<Vec<SanctionsMatch>> {
        let encoded = urlencoding::encode(query);
        let url = format!("{}?q={}&limit=8", self.api_endpoint, encoded);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let body: Value = resp.json().await?;
        let mut results = Vec::new();

        if let Some(items) = body.get("results").and_then(|v| v.as_array()) {
            for item in items {
                let caption = item.get("caption").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if caption.is_empty() {
                    continue;
                }

                let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.75);
                if score < 0.60 {
                    continue;
                }

                let schema = item.get("schema").and_then(|v| v.as_str()).unwrap_or("Person").to_string();
                let datasets_arr = item.get("datasets").and_then(|v| v.as_array());
                let dataset = datasets_arr
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("global_sanctions")
                    .to_string();

                let props = item.get("properties");
                let birth_date = props
                    .and_then(|p| p.get("birthDate"))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let mut countries = Vec::new();
                if let Some(c_arr) = props.and_then(|p| p.get("country")).and_then(|v| v.as_array()) {
                    for c in c_arr {
                        if let Some(cs) = c.as_str() {
                            countries.push(cs.to_uppercase());
                        }
                    }
                }

                let program = props
                    .and_then(|p| p.get("program"))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let sanctor = props
                    .and_then(|p| p.get("sanctions"))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| Some("Global Sanctions Authority".to_string()));

                let summary = props
                    .and_then(|p| p.get("notes"))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let source_url = if !id.is_empty() {
                    Some(format!("https://www.opensanctions.org/entities/{}/", id))
                } else {
                    None
                };

                results.push(SanctionsMatch {
                    dataset,
                    caption,
                    schema,
                    match_score: score,
                    birth_date,
                    countries,
                    sanctor,
                    program,
                    source_url,
                    summary,
                });
            }
        }

        Ok(results)
    }

    fn check_local_sanctions_registry(name: &str, aliases: &[String]) -> Vec<SanctionsMatch> {
        let registry: &[(&str, &[&str], &str, &str, &str, &str, &[&str])] = &[
            (
                "Делян Пеевски",
                &["Delyan Peevski", "Delyan Slavchev Peevski", "Делян Славчев Пеевски"],
                "us_ofac_magnitsky",
                "US Department of the Treasury (OFAC) / UK OFSI",
                "Global Magnitsky Human Rights Accountability Act",
                "https://home.treasury.gov/news/press-releases/jy0208",
                &["BG", "US", "GB"],
            ),
            (
                "Васил Божков",
                &["Vasil Bozhkov", "Vasil Krumov Bozhkov", "Васил Крумов Божков", "Черепа"],
                "us_ofac_magnitsky",
                "US Department of the Treasury (OFAC)",
                "Global Magnitsky Sanctions / Corruption",
                "https://home.treasury.gov/news/press-releases/jy0208",
                &["BG", "US"],
            ),
            (
                "Илко Желязков",
                &["Ilko Zhelyazkov", "Ilko Dimitrov Zhelyazkov", "Илко Димитров Желязков"],
                "us_ofac_magnitsky",
                "US Department of the Treasury (OFAC)",
                "Global Magnitsky Sanctions",
                "https://home.treasury.gov/news/press-releases/jy0208",
                &["BG", "US"],
            ),
            (
                "Владислав Горанов",
                &["Vladislav Goranov", "Vladislav Ivanov Goranov", "Владислав Иванов Горанов"],
                "us_ofac_magnitsky_2",
                "US Department of the Treasury (OFAC) / UK Sanctions",
                "Global Magnitsky Act / Public Corruption",
                "https://home.treasury.gov/news/press-releases/jy1264",
                &["BG", "US", "GB"],
            ),
            (
                "Румен Овчаров",
                &["Rumen Ovcharov", "Rumen Stoyanov Ovcharov", "Румен Стоянов Овчаров"],
                "us_ofac_magnitsky_2",
                "US Department of the Treasury (OFAC)",
                "Global Magnitsky Act / Energy Corruption",
                "https://home.treasury.gov/news/press-releases/jy1264",
                &["BG", "US"],
            ),
            (
                "Николай Малинов",
                &["Nikolay Malinov", "Nikolay Simeonov Malinov", "Николай Симеонов Малинов"],
                "us_ofac_magnitsky_2",
                "US Department of the Treasury (OFAC) / UK Sanctions",
                "Global Magnitsky Act / Malign Foreign Influence",
                "https://home.treasury.gov/news/press-releases/jy1264",
                &["BG", "US"],
            ),
            (
                "Александър Христов Николов",
                &["Alexander Nikolov", "Aleksandar Nikolov", "Александър Николов"],
                "us_ofac_magnitsky_2",
                "US Department of the Treasury (OFAC)",
                "Global Magnitsky Act / Nuclear Energy Bribery",
                "https://home.treasury.gov/news/press-releases/jy1264",
                &["BG", "US"],
            ),
            (
                "Иван Гешев",
                &["Ivan Geshev", "Ivan Stoimenov Geshev", "Иван Стоименов Гешев"],
                "pep_registry",
                "Bulgarian Judiciary / Public Figure Registry",
                "Former Prosecutor General / PEP Category 1",
                "https://prb.bg",
                &["BG"],
            ),
            (
                "Бойко Борисов",
                &["Boyko Borissov", "Boyko Metodiev Borissov", "Бойко Методиев Борисов"],
                "pep_registry",
                "Bulgarian National Assembly / Public Figure Registry",
                "Former Prime Minister / Member of Parliament / PEP Category 1",
                "https://www.parliament.bg",
                &["BG", "EU"],
            ),
        ];

        let mut matches = Vec::new();
        let target_lower = name.to_lowercase();
        let target_tokens: Vec<&str> = target_lower.split_whitespace().collect();

        for &(entry_name, entry_aliases, dataset, sanctor, program, source_url, countries) in registry {
            let mut matched = false;
            let mut score = 0.0f64;

            let en_lower = entry_name.to_lowercase();
            if target_lower == en_lower {
                matched = true;
                score = 1.0;
            } else if target_tokens.len() >= 2 && en_lower.contains(target_tokens[0]) && en_lower.contains(target_tokens[1]) {
                matched = true;
                score = 0.92;
            } else {
                for alias in entry_aliases {
                    let al_lower = alias.to_lowercase();
                    if target_lower == al_lower {
                        matched = true;
                        score = 0.98;
                        break;
                    }
                    for cand_alias in aliases {
                        if cand_alias.to_lowercase() == al_lower {
                            matched = true;
                            score = 0.95;
                            break;
                        }
                    }
                    if matched {
                        break;
                    }
                }
            }

            if matched {
                matches.push(SanctionsMatch {
                    dataset: dataset.to_string(),
                    caption: entry_name.to_string(),
                    schema: "Person".to_string(),
                    match_score: score,
                    birth_date: None,
                    countries: countries.iter().map(|s| s.to_string()).collect(),
                    sanctor: Some(sanctor.to_string()),
                    program: Some(program.to_string()),
                    source_url: Some(source_url.to_string()),
                    summary: Some(format!("Designated under {} by {}", program, sanctor)),
                });
            }
        }

        matches
    }
}
