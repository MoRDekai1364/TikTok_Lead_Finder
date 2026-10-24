use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use leadfinder_core::{NormalisationConfig, PersonInput, PolitenessGuard};
use crate::lexicon::LexiconNormalizer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoTextSources {
    pub transcript: String,
    pub ocr: String,
    pub caption: String,
    pub hashtags: Vec<String>,
    pub creator_handle: Option<String>,
    pub creator_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPersonRecord {
    pub canonical_name: String,
    pub variants: Vec<String>,
    pub role: Option<String>,
    pub public_figure: bool,
    pub wikidata_id: Option<String>,
    pub summary: Option<String>,
    pub mentions: i64,
    pub found_in: String,
    pub entity_type: String,
}

pub fn detect_entity_type(name: &str, role_hint: Option<&str>) -> &'static str {
    let lower = name.trim().to_lowercase();
    let r_lower = role_hint.map(|r| r.trim().to_lowercase()).unwrap_or_default();

    let has_person_title = lower.starts_with("инж. ")
        || lower.starts_with("инженер ")
        || lower.starts_with("д-р ")
        || lower.starts_with("доктор ")
        || lower.starts_with("проф. ")
        || lower.starts_with("професор ")
        || lower.starts_with("доц. ")
        || lower.starts_with("доцент ")
        || lower.starts_with("адв. ")
        || lower.starts_with("адвокат ");

    let is_human_role = r_lower.contains("кмет")
        || r_lower.contains("министър")
        || r_lower.contains("съдия")
        || r_lower.contains("прокурор")
        || r_lower.contains("депутат")
        || r_lower.contains("председател")
        || r_lower.contains("директор")
        || r_lower.contains("политик")
        || r_lower.contains("член")
        || r_lower.contains("лекар")
        || r_lower.contains("журналист")
        || r_lower.contains("водещ")
        || r_lower.contains("автор")
        || r_lower.contains("създател");

    if has_person_title {
        return "person";
    }

    if lower.contains("партия")
        || lower.contains("коалиция")
        || lower.contains("движение")
        || lower.contains("съюз")
        || lower.contains("асоциация")
        || lower.contains("синдикат")
        || lower.contains("фондация")
        || lower.contains("герб")
        || lower.contains("възраждане")
        || lower.contains("бсп")
        || lower.contains("дпс")
        || lower.contains("итн")
        || lower.contains("пп-дб")
        || lower.contains("демократична българия")
        || lower.contains("продължаваме промяната")
        || lower.contains("прогресивна българия")
        || lower.contains("величие")
        || lower.contains("мечове")
        || lower.contains("църква")
        || lower.contains("секта")
        || lower.contains("организация")
        || (!is_human_role && (r_lower.contains("партия") || r_lower.contains("движение") || r_lower.contains("коалиция")))
    {
        return "group";
    }

    if lower.contains("община")
        || lower.contains("съд")
        || lower.contains("прокуратура")
        || lower.contains("следствие")
        || lower.contains("комисия")
        || lower.contains("европейска комисия")
        || lower.contains("министерство")
        || lower.contains("съвет")
        || lower.contains("агенция")
        || lower.contains("парламент")
        || lower.contains("събрание")
        || lower.contains("народно събрание")
        || lower.contains("инспекторат")
        || lower.contains("институт")
        || lower.contains("комитет")
        || lower.contains("фонд")
        || lower.contains("бюро")
        || lower.contains("дирекция")
        || lower.contains("управление")
        || lower.contains("европейски съюз")
        || lower.contains("nato")
        || lower.contains("нато")
        || lower.contains("оон")
        || lower.contains("юнеско")
        || lower.contains("сзо")
        || lower.contains("мвр")
        || lower.contains("данс")
        || lower.contains("гдбоп")
        || lower.contains("нап")
        || lower.contains("цик")
        || lower.contains("бнб")
        || lower.contains("кзк")
        || lower.contains("крс")
        || lower.contains("пентагон")
        || lower.contains("кремъл")
        || lower.contains("бял дом")
        || lower.contains("болница")
        || lower.contains("бта")
        || lower.contains("български лев")
        || lower.contains("еврозона")
        || lower.contains("президентство")
        || lower.contains("кметство")
        || lower.contains("университет")
        || lower.contains("академия")
        || lower.contains("бан")
        || (!is_human_role && (r_lower.contains("институция") || r_lower.contains("орган") || r_lower.contains("комисия") || r_lower.contains("министерство") || r_lower.contains("съд") || r_lower.contains("община") || r_lower.contains("болница")))
    {
        return "institution";
    }

    let words: Vec<&str> = lower.split_whitespace().collect();
    let has_tv_token = words.iter().any(|w| *w == "тв" || *w == "tv");

    if lower.contains("the clashers")
        || lower.contains("clashers")
        || lower.ends_with(".bg")
        || lower.contains(".bg ")
        || lower.contains("новини софия")
        || lower.contains("балкан президент")
        || lower.contains("канал")
        || lower.contains("телевизия")
        || has_tv_token
        || lower.contains("podcast")
        || lower.contains("подкаст")
        || lower.contains("шоу")
        || lower.contains("медия")
        || lower.contains("новини")
        || lower.contains("студио")
        || lower.contains("studio")
        || lower.contains("youtube")
        || lower.contains("тикток")
        || lower.contains("tiktok")
        || lower.contains("бтв")
        || lower.contains("btv")
        || lower.contains("нова тв")
        || lower.contains("нова телевизия")
        || lower.contains("бнт")
        || lower.contains("bnt")
        || lower.contains("евронюз")
        || lower.contains("свободна европа")
        || lower.contains("дойче веле")
        || lower.contains("цанов")
        || lower.contains("господари")
        || (!is_human_role && (r_lower.contains("канал") || r_lower.contains("медия") || r_lower.contains("предаване")))
    {
        return "channel";
    }

    if lower.starts_with("република ")
        || lower.starts_with("кралство ")
        || lower.starts_with("държава")
        || lower.starts_with("федерална република")
        || lower.starts_with("съединени американски")
        || lower.contains("българия")
        || lower.contains("русия")
        || lower.contains("сащ")
        || lower.contains("украйна")
        || lower.contains("германия")
        || lower.contains("франция")
        || lower.contains("великобритания")
        || lower.contains("китай")
        || lower.contains("турция")
        || lower.contains("гърция")
        || lower.contains("гръцко")
        || lower.contains("румъния")
        || lower.contains("сърбия")
        || lower.contains("северна македония")
        || lower.contains("македония")
        || lower.contains("израел")
        || lower.contains("палестина")
        || lower.contains("иран")
        || lower.contains("ирак")
        || lower.contains("сирия")
        || lower.contains("полша")
        || lower.contains("италия")
        || lower.contains("испания")
        || lower.contains("словения")
        || lower.contains("словакия")
        || lower.contains("унгария")
        || lower.contains("австрия")
        || lower.contains("швейцария")
        || lower.contains("европа")
        || lower.contains("холандия")
        || lower.contains("нидерландия")
        || lower.contains("белгия")
        || lower.contains("чехия")
        || lower.contains("дания")
        || lower.contains("швеция")
        || lower.contains("норвегия")
        || lower.contains("финландия")
        || (!is_human_role && (r_lower.contains("държава") || r_lower.contains("страна")))
    {
        return "country";
    }

    "person"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikidataEntityMatch {
    pub wikidata_id: String,
    pub label: String,
    pub description: Option<String>,
    pub is_human: bool,
    pub has_public_office_or_party: bool,
    pub sitelink_count: usize,
    pub public_figure: bool,
}

#[derive(Clone)]
pub struct PeopleResolver {
    norm_cfg: NormalisationConfig,
    lexicon: Arc<LexiconNormalizer>,
    guard: PolitenessGuard,
    client: Client,
    api_base_url: String,
    offline_fixtures: Option<Arc<HashMap<String, WikidataEntityMatch>>>,
}

impl PeopleResolver {
    pub fn new(
        norm_cfg: NormalisationConfig,
        lexicon: Arc<LexiconNormalizer>,
        guard: PolitenessGuard,
    ) -> Self {
        let client = Client::builder()
            .user_agent("TikTokLeadFinder/0.1 (local fact-checking research assistant)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            norm_cfg,
            lexicon,
            guard,
            client,
            api_base_url: "https://www.wikidata.org/w/api.php".to_string(),
            offline_fixtures: None,
        }
    }

    pub fn with_offline_fixtures(mut self, map: HashMap<String, WikidataEntityMatch>) -> Self {
        self.offline_fixtures = Some(Arc::new(map));
        self
    }

    pub fn normalize_name_and_role(
        &self,
        raw_name: &str,
        role_hint: Option<&str>,
    ) -> (String, Option<String>) {
        let mut working = raw_name
            .trim()
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_string();
        let mut inferred_role = role_hint
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty());

        let mut sorted_prefixes = self.norm_cfg.role_prefixes.clone();
        sorted_prefixes.sort_by(|a, b| b.len().cmp(&a.len()));

        loop {
            let low = working.to_lowercase();
            let mut stripped_any = false;
            for prefix in &sorted_prefixes {
                let p_low = prefix.to_lowercase();
                if low.starts_with(&p_low) {
                    let rest = working.get(prefix.len()..).unwrap_or("").trim_start();
                    let boundary_ok = working
                        .get(prefix.len()..)
                        .and_then(|s| s.chars().next())
                        .map(|ch| ch.is_whitespace() || ch == '.')
                        .unwrap_or(false);
                    if boundary_ok && !rest.is_empty() {
                        if inferred_role.is_none() {
                            inferred_role = Some(prefix.trim_end_matches('.').to_string());
                        }
                        working = rest.trim_start_matches('.').trim_start().to_string();
                        stripped_any = true;
                        break;
                    }
                }
            }
            if !stripped_any {
                break;
            }
        }

        let mut cleaned_tokens = Vec::new();
        for tok in working.split_whitespace() {
            let trimmed = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
            if trimmed.is_empty() {
                continue;
            }
            let de_articled = self.strip_definite_suffix(trimmed);
            cleaned_tokens.push(title_case_token(&de_articled));
        }

        let canonical = cleaned_tokens.join(" ");
        (canonical, inferred_role)
    }

    fn strip_definite_suffix(&self, token: &str) -> String {
        let low = token.to_lowercase();
        if low.chars().count() < 5 {
            return token.to_string();
        }
        for suf in &self.norm_cfg.definite_suffixes {
            let s_low = suf.to_lowercase();
            if low.ends_with(&s_low) {
                let stem_byte_len = token.len().saturating_sub(suf.len());
                if let Some(stem) = token.get(..stem_byte_len) {
                    let stem_low = stem.to_lowercase();
                    let matches_surname_suffix = self
                        .norm_cfg
                        .surname_suffixes
                        .iter()
                        .any(|ss| stem_low.ends_with(&ss.to_lowercase()));
                    if stem.chars().count() >= 4
                        && (self.lexicon.is_known_word(&stem_low) || matches_surname_suffix)
                    {
                        return stem.to_string();
                    }
                }
            }
        }
        token.to_string()
    }

    pub fn person_phonetic_key(&self, name: &str) -> String {
        let (norm, _) = self.normalize_name_and_role(name, None);
        norm.split_whitespace()
            .map(transliterate_to_canonical_latin_key)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub async fn extract_and_resolve_people(
        &self,
        llm_people: &[PersonInput],
        sources: &VideoTextSources,
    ) -> Result<Vec<ResolvedPersonRecord>> {
        let mut combined_inputs: Vec<PersonInput> = llm_people.to_vec();
        for heuristic_person in self.scan_capitalized_name_pairs(sources) {
            combined_inputs.push(heuristic_person);
        }

        #[derive(Default)]
        struct CandidateGroup {
            canonical_name: String,
            variants: Vec<String>,
            role: Option<String>,
            entity_type: Option<String>,
        }

        let mut groups: HashMap<String, CandidateGroup> = HashMap::new();
        let mut group_order: Vec<String> = Vec::new();

        for item in combined_inputs {
            let (canon, role) = self.normalize_name_and_role(&item.name, item.role_hint.as_deref());
            if canon.chars().count() < 3 || !canon.chars().any(|c| c.is_alphabetic()) {
                continue;
            }
            if is_generic_non_person(&canon, &self.norm_cfg) {
                continue;
            }
            let key = self.person_phonetic_key(&canon);
            if key.len() < 3 {
                continue;
            }

            let item_ent_type = item.entity_type.clone().or_else(|| {
                let d = detect_entity_type(&canon, role.as_deref());
                if d != "person" {
                    Some(d.to_string())
                } else {
                    None
                }
            });

            let entry = groups.entry(key.clone()).or_insert_with(|| {
                group_order.push(key.clone());
                CandidateGroup {
                    canonical_name: canon.clone(),
                    variants: Vec::new(),
                    role: None,
                    entity_type: item_ent_type.clone(),
                }
            });

            if has_cyrillic(&canon) && !has_cyrillic(&entry.canonical_name) {
                entry.canonical_name = canon.clone();
            } else if canon.split_whitespace().count() > entry.canonical_name.split_whitespace().count()
                && has_cyrillic(&canon) == has_cyrillic(&entry.canonical_name)
            {
                entry.canonical_name = canon.clone();
            }

            push_unique_ci(&mut entry.variants, &canon);
            push_unique_ci(&mut entry.variants, item.name.trim());
            for v in &item.variants {
                if !v.trim().is_empty() {
                    push_unique_ci(&mut entry.variants, v.trim());
                }
            }
            if entry.role.is_none() && role.is_some() {
                entry.role = role;
            }
            if entry.entity_type.is_none() && item_ent_type.is_some() {
                entry.entity_type = item_ent_type;
            }
        }

        let full_keys: Vec<String> = group_order
            .iter()
            .filter(|k| k.split_whitespace().count() >= 2)
            .cloned()
            .collect();
        let single_keys: Vec<String> = group_order
            .iter()
            .filter(|k| k.split_whitespace().count() == 1)
            .cloned()
            .collect();

        for skey in single_keys {
            let matching_full: Vec<String> = full_keys
                .iter()
                .filter(|fk| fk.split_whitespace().any(|part| part == skey))
                .cloned()
                .collect();
            if matching_full.len() == 1 {
                if let Some(single_grp) = groups.remove(&skey) {
                    if let Some(target_grp) = groups.get_mut(&matching_full[0]) {
                        for v in single_grp.variants {
                            push_unique_ci(&mut target_grp.variants, &v);
                        }
                        if target_grp.role.is_none() {
                            target_grp.role = single_grp.role;
                        }
                        if target_grp.entity_type.is_none() {
                            target_grp.entity_type = single_grp.entity_type;
                        }
                    }
                    group_order.retain(|k| k != &skey);
                }
            }
        }

        let mut results = Vec::new();
        for key in group_order {
            let Some(grp) = groups.remove(&key) else {
                continue;
            };

            let (mentions, found_in_list) = count_mentions_in_sources(&grp.variants, sources);
            if mentions == 0 || found_in_list.is_empty() {
                continue;
            }

            let wd_match = self
                .lookup_wikidata(&grp.canonical_name, &grp.variants)
                .await
                .ok()
                .flatten();

            let (final_canon, final_role, public_figure, wikidata_id, summary) = match wd_match {
                Some(wd) if wd.is_human => {
                    let chosen_canon = if !wd.label.is_empty() && !wd.label.starts_with('Q') {
                        wd.label.clone()
                    } else {
                        grp.canonical_name.clone()
                    };
                    let chosen_role = wd.description.clone().or_else(|| grp.role.clone());
                    let summary_text = if wd.public_figure {
                        Some(match &wd.description {
                            Some(desc) if !desc.is_empty() => {
                                format!("{} ({}) — {}", chosen_canon, wd.wikidata_id, desc)
                            }
                            _ => format!("{} ({})", chosen_canon, wd.wikidata_id),
                        })
                    } else {
                        None
                    };
                    (
                        chosen_canon,
                        chosen_role,
                        wd.public_figure,
                        Some(wd.wikidata_id),
                        summary_text,
                    )
                }
                _ => (grp.canonical_name.clone(), grp.role.clone(), false, None, None),
            };

            let mut final_variants = grp.variants;
            push_unique_ci(&mut final_variants, &final_canon);

            let ent_type = grp
                .entity_type
                .unwrap_or_else(|| detect_entity_type(&final_canon, final_role.as_deref()).to_string());

            results.push(ResolvedPersonRecord {
                canonical_name: final_canon,
                variants: final_variants,
                role: final_role,
                public_figure,
                wikidata_id,
                summary,
                mentions,
                found_in: found_in_list.join(","),
                entity_type: ent_type,
            });
        }

        Ok(results)
    }

    fn scan_capitalized_name_pairs(&self, sources: &VideoTextSources) -> Vec<PersonInput> {
        let mut out = Vec::new();
        let combined = format!(
            "{}\n{}\n{}",
            sources.transcript, sources.caption, sources.ocr
        );
        for sentence in combined.split(|c: char| matches!(c, '.' | '!' | '?' | '\n' | ';' | ':')) {
            let words: Vec<&str> = sentence.split_whitespace().collect();
            if words.len() < 2 {
                continue;
            }
            for idx in 0..words.len() - 1 {
                let w1 = words[idx].trim_matches(|c: char| !c.is_alphabetic() && c != '-');
                let w2 = words[idx + 1].trim_matches(|c: char| !c.is_alphabetic() && c != '-');
                if w1.chars().count() < 3 || w2.chars().count() < 3 {
                    continue;
                }
                if !is_title_cased_word(w1) || !is_title_cased_word(w2) {
                    continue;
                }
                let full = format!("{w1} {w2}");
                if is_extracted_noise(&full) {
                    continue;
                }
                let w2_low = w2.to_lowercase();
                let has_surname_ending = self
                    .norm_cfg
                    .surname_suffixes
                    .iter()
                    .any(|ss| w2_low.ends_with(&ss.to_lowercase()));
                if idx == 0 && self.lexicon.is_known_word(&w1.to_lowercase()) && !has_surname_ending {
                    continue;
                }
                if !has_surname_ending && idx == 0 {
                    continue;
                }
                let full = format!("{w1} {w2}");
                let detected = detect_entity_type(&full, None);
                out.push(PersonInput {
                    name: full.clone(),
                    variants: vec![full],
                    role_hint: None,
                    entity_type: Some(detected.to_string()),
                });
            }
        }
        out
    }

    pub async fn lookup_wikidata(
        &self,
        canonical_name: &str,
        variants: &[String],
    ) -> Result<Option<WikidataEntityMatch>> {
        if let Some(ref fixtures) = self.offline_fixtures {
            let key = self.person_phonetic_key(canonical_name);
            if let Some(found) = fixtures.get(&key) {
                return Ok(Some(found.clone()));
            }
            for v in variants {
                let vk = self.person_phonetic_key(v);
                if let Some(found) = fixtures.get(&vk) {
                    return Ok(Some(found.clone()));
                }
            }
            return Ok(None);
        }

        if canonical_name.split_whitespace().count() < 2 {
            return Ok(None);
        }

        let langs = if self.norm_cfg.wikidata_languages.is_empty() {
            vec!["en".to_string()]
        } else {
            self.norm_cfg.wikidata_languages.clone()
        };

        let mut search_terms = vec![canonical_name.to_string()];
        for v in variants {
            if v.split_whitespace().count() >= 2
                && !search_terms.iter().any(|s| s.eq_ignore_ascii_case(v))
            {
                search_terms.push(v.clone());
            }
        }

        let mut candidate_qids: Vec<String> = Vec::new();
        for lang in &langs {
            for term in search_terms.iter().take(2) {
                let encoded = urlencoding::encode(term);
                let search_url = format!(
                    "{}?action=wbsearchentities&search={encoded}&language={lang}&uselang={lang}&type=item&limit=5&format=json",
                    self.api_base_url
                );
                if let Some(body) = self.cached_get(&search_url).await? {
                    if let Ok(val) = serde_json::from_str::<Value>(&body) {
                        if let Some(arr) = val.get("search").and_then(|s| s.as_array()) {
                            for item in arr {
                                if let Some(qid) = item.get("id").and_then(|x| x.as_str()) {
                                    if !candidate_qids.iter().any(|q| q == qid) {
                                        candidate_qids.push(qid.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                if !candidate_qids.is_empty() {
                    break;
                }
            }
            if !candidate_qids.is_empty() {
                break;
            }
        }

        if candidate_qids.is_empty() {
            return Ok(None);
        }

        let joined_ids = candidate_qids
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("|");
        let joined_langs = langs.join("|");
        let entity_url = format!(
            "{}?action=wbgetentities&ids={}&props=claims|labels|descriptions|sitelinks&languages={}&format=json",
            self.api_base_url,
            urlencoding::encode(&joined_ids),
            urlencoding::encode(&joined_langs)
        );

        let Some(entity_body) = self.cached_get(&entity_url).await? else {
            return Ok(None);
        };
        let parsed: Value = serde_json::from_str(&entity_body)?;
        let Some(entities_obj) = parsed.get("entities").and_then(|e| e.as_object()) else {
            return Ok(None);
        };

        let target_key = self.person_phonetic_key(canonical_name);

        for qid in &candidate_qids {
            let Some(ent) = entities_obj.get(qid) else {
                continue;
            };
            let claims = ent.get("claims");
            let is_human = claim_has_entity_id(claims, "P31", "Q5");
            if !is_human {
                continue;
            }

            let has_office = claims
                .and_then(|c| c.get("P39"))
                .and_then(|a| a.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            let has_party = claims
                .and_then(|c| c.get("P102"))
                .and_then(|a| a.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            let sitelink_count = ent
                .get("sitelinks")
                .and_then(|s| s.as_object())
                .map(|o| o.len())
                .unwrap_or(0);

            let label = pick_localized_string(ent.get("labels"), &langs)
                .unwrap_or_else(|| canonical_name.to_string());
            let description = pick_localized_string(ent.get("descriptions"), &langs);

            let label_key = self.person_phonetic_key(&label);
            if !keys_compatible(&target_key, &label_key) {
                continue;
            }

            let public_figure = has_office || has_party || sitelink_count >= 2;

            return Ok(Some(WikidataEntityMatch {
                wikidata_id: qid.clone(),
                label,
                description,
                is_human,
                has_public_office_or_party: has_office || has_party,
                sitelink_count,
                public_figure,
            }));
        }

        Ok(None)
    }

    async fn cached_get(&self, url: &str) -> Result<Option<String>> {
        if let Some(cached) = self.guard.get_cached(url) {
            return Ok(Some(cached.body));
        }
        if self.guard.mode() == leadfinder_core::HttpTransportMode::ReplayFixturesOnly {
            return Ok(None);
        }
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "www.wikidata.org".to_string());

        if self.guard.acquire_slot(&host, "wikidata").await.is_err() {
            return Ok(None);
        }

        match self.client.get(url).send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                if resp.status().is_success() {
                    let txt = resp.text().await?;
                    self.guard.record_success(&host).await;
                    self.guard
                        .put_cached(url, code, txt.clone(), Some("application/json".to_string()));
                    Ok(Some(txt))
                } else {
                    self.guard
                        .record_failure(&host, &format!("HTTP {code}"))
                        .await;
                    Ok(None)
                }
            }
            Err(e) => {
                self.guard.record_failure(&host, &e.to_string()).await;
                Ok(None)
            }
        }
    }
}

fn claim_has_entity_id(claims: Option<&Value>, prop: &str, target_qid: &str) -> bool {
    let Some(arr) = claims.and_then(|c| c.get(prop)).and_then(|p| p.as_array()) else {
        return false;
    };
    arr.iter().any(|stmt| {
        stmt.get("mainsnak")
            .and_then(|m| m.get("datavalue"))
            .and_then(|d| d.get("value"))
            .and_then(|v| v.get("id"))
            .and_then(|id| id.as_str())
            == Some(target_qid)
    })
}

fn pick_localized_string(map_val: Option<&Value>, langs: &[String]) -> Option<String> {
    let obj = map_val?.as_object()?;
    for lang in langs {
        if let Some(s) = obj
            .get(lang)
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
        {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    obj.values()
        .find_map(|v| v.get("value").and_then(|s| s.as_str()))
        .map(|s| s.trim().to_string())
}

fn keys_compatible(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a_parts: Vec<&str> = a.split_whitespace().collect();
    let b_parts: Vec<&str> = b.split_whitespace().collect();
    if a_parts.is_empty() || b_parts.is_empty() {
        return false;
    }
    let last_match = match (a_parts.last(), b_parts.last()) {
        (Some(la), Some(lb)) => la == lb || edit_distance_le_one(la, lb),
        _ => false,
    };
    let first_match = match (a_parts.first(), b_parts.first()) {
        (Some(fa), Some(fb)) => fa == fb || edit_distance_le_one(fa, fb),
        _ => false,
    };
    first_match && last_match
}

fn edit_distance_le_one(a: &str, b: &str) -> bool {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    if ac.len().abs_diff(bc.len()) > 1 {
        return false;
    }
    let mut i = 0;
    let mut j = 0;
    let mut diffs = 0;
    while i < ac.len() && j < bc.len() {
        if ac[i] == bc[j] {
            i += 1;
            j += 1;
        } else {
            diffs += 1;
            if diffs > 1 {
                return false;
            }
            if ac.len() > bc.len() {
                i += 1;
            } else if bc.len() > ac.len() {
                j += 1;
            } else {
                i += 1;
                j += 1;
            }
        }
    }
    if i < ac.len() || j < bc.len() {
        diffs += 1;
    }
    diffs <= 1
}

fn count_mentions_in_sources(variants: &[String], sources: &VideoTextSources) -> (i64, Vec<String>) {
    let mut total_mentions = 0i64;
    let mut found_in = Vec::new();
    let hashtag_str = sources.hashtags.join(" ");
    let creator_str = format!(
        "{} {}",
        sources.creator_handle.as_deref().unwrap_or(""),
        sources.creator_display_name.as_deref().unwrap_or("")
    );

    let buckets = [
        ("transcript", sources.transcript.as_str()),
        ("ocr", sources.ocr.as_str()),
        ("caption", sources.caption.as_str()),
        ("hashtags", hashtag_str.as_str()),
        ("creator", creator_str.as_str()),
    ];

    for (label, haystack) in buckets {
        if haystack.trim().is_empty() {
            continue;
        }
        let hay_low = haystack.to_lowercase();
        let hay_lat = transliterate_to_canonical_latin_key(haystack);
        let mut bucket_hits = 0i64;

        for var in variants {
            let v_low = var.trim().to_lowercase();
            if v_low.chars().count() < 3 {
                continue;
            }
            let direct = hay_low.matches(&v_low).count() as i64;
            if direct > 0 {
                bucket_hits = bucket_hits.max(direct);
                continue;
            }
            for part in v_low.split_whitespace() {
                if part.chars().count() >= 5 && hay_low.contains(part) {
                    bucket_hits = bucket_hits.max(1);
                }
            }
            let v_lat = transliterate_to_canonical_latin_key(&v_low);
            if v_lat.len() >= 5 && hay_lat.contains(&v_lat) {
                bucket_hits = bucket_hits.max(1);
            }
        }

        if bucket_hits > 0 {
            total_mentions += bucket_hits;
            found_in.push(label.to_string());
        }
    }

    (total_mentions, found_in)
}

fn has_latin(s: &str) -> bool {
    s.chars().any(|c| ('a'..='z').contains(&c) || ('A'..='Z').contains(&c))
}

pub fn is_extracted_noise(name: &str) -> bool {
    let low = name.trim().to_lowercase();
    if low.is_empty() {
        return true;
    }
    let static_noise = [
        "no time",
        "time to",
        "to die",
        "ai vie",
        "ly ма",
        "my wc",
        "ui ат",
        "да fay",
        "жо fy",
        "запазен запазен",
        "класа кове",
        "павликеки запазен",
        "захарсът дигнах",
        "дигнах сътсемно",
        "анатем мосвамви",
        "гедебог тукбургаз",
        "радъв бългурското",
        "хора",
        "охранител",
        "водеща",
        "пиян полицай",
        "инфлация икономика",
        "икономика славивасилев",
        "желад",
        "софия вар",
        "стоимен врагове",
    ];
    if static_noise.contains(&low.as_str()) {
        return true;
    }
    let words: Vec<&str> = low.split_whitespace().collect();
    if words.is_empty() || words.len() > 4 {
        return true;
    }
    let has_any_cyr = words.iter().any(|w| has_cyrillic(w));
    let has_any_lat = words.iter().any(|w| has_latin(w));
    if has_any_cyr && has_any_lat {
        return true;
    }
    if words.len() == 2 && words.iter().any(|w| w.chars().count() < 3) {
        return true;
    }
    false
}

fn is_generic_non_person(name: &str, cfg: &NormalisationConfig) -> bool {
    if is_extracted_noise(name) {
        return true;
    }
    let low = name.trim().to_lowercase();
    if cfg
        .role_prefixes
        .iter()
        .any(|p| p.trim_end_matches('.').to_lowercase() == low)
    {
        return true;
    }
    let words: Vec<&str> = low.split_whitespace().collect();
    words.is_empty() || words.len() > 4
}

fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

fn is_title_cased_word(w: &str) -> bool {
    let mut chars = w.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_uppercase() {
        return false;
    }
    let rest: Vec<char> = chars.collect();
    !rest.is_empty() && rest.iter().any(|c| c.is_lowercase())
}

fn title_case_token(tok: &str) -> String {
    if tok.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()) && tok.chars().count() <= 4 {
        return tok.to_string();
    }
    let mut chars = tok.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = first.to_uppercase().to_string();
    out.push_str(&chars.as_str().to_lowercase());
    out
}

fn push_unique_ci(vec: &mut Vec<String>, item: &str) {
    let trimmed = item.trim();
    if trimmed.is_empty() {
        return;
    }
    if !vec
        .iter()
        .any(|x| x.to_lowercase() == trimmed.to_lowercase())
    {
        vec.push(trimmed.to_string());
    }
}

pub fn transliterate_to_canonical_latin_key(input: &str) -> String {
    let mut raw = String::with_capacity(input.len() * 2);
    for ch in input.to_lowercase().chars() {
        match ch {
            '\u{0430}' => raw.push('a'),
            '\u{0431}' => raw.push('b'),
            '\u{0432}' => raw.push('v'),
            '\u{0433}' => raw.push('g'),
            '\u{0434}' => raw.push('d'),
            '\u{0435}' | '\u{044d}' => raw.push('e'),
            '\u{0436}' => raw.push_str("zh"),
            '\u{0437}' => raw.push('z'),
            '\u{0438}' | '\u{0439}' | '\u{044b}' => raw.push('i'),
            '\u{043a}' => raw.push('k'),
            '\u{043b}' => raw.push('l'),
            '\u{043c}' => raw.push('m'),
            '\u{043d}' => raw.push('n'),
            '\u{043e}' => raw.push('o'),
            '\u{043f}' => raw.push('p'),
            '\u{0440}' => raw.push('r'),
            '\u{0441}' => raw.push('s'),
            '\u{0442}' => raw.push('t'),
            '\u{0443}' => raw.push('u'),
            '\u{0444}' => raw.push('f'),
            '\u{0445}' => raw.push('h'),
            '\u{0446}' => raw.push_str("ts"),
            '\u{0447}' => raw.push_str("ch"),
            '\u{0448}' => raw.push_str("sh"),
            '\u{0449}' => raw.push_str("sht"),
            '\u{044a}' => raw.push('a'),
            '\u{044c}' => {}
            '\u{044e}' => raw.push_str("yu"),
            '\u{044f}' => raw.push_str("ya"),
            'y' => raw.push('i'),
            'w' => raw.push('v'),
            'q' => raw.push('k'),
            'x' => raw.push_str("ks"),
            c if c.is_ascii_alphanumeric() => raw.push(c),
            c if c.is_whitespace() || c == '-' => raw.push(' '),
            _ => {}
        }
    }

    let mut collapsed = String::with_capacity(raw.len());
    let mut prev: Option<char> = None;
    for ch in raw.chars() {
        if prev == Some(ch) && ch != ' ' {
            continue;
        }
        collapsed.push(ch);
        prev = Some(ch);
    }

    let mut seen_space = true;
    let mut clean = String::new();
    for ch in collapsed.trim().chars() {
        if ch == ' ' {
            if !seen_space {
                clean.push(' ');
                seen_space = true;
            }
        } else {
            clean.push(ch);
            seen_space = false;
        }
    }
    clean
}
