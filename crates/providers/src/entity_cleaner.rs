use serde::{Deserialize, Serialize};
pub use leadfinder_core::{DedupActionType, DedupProposal, DedupSummary};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecomposedEntityResult {
    pub name: String,
    pub role: Option<String>,
    pub tag: Option<String>,
}

const BULGARIAN_STOPWORDS: &[&str] = &[
    "който", "която", "които", "чиито", "чий", "чия", "чии", "във", "със", "за", "на", "от", "до",
    "че", "ако", "но", "или", "като", "при", "без", "през", "след", "под", "над",
    "господин", "госпожа", "госпожица", "г-н", "г-жа", "имам", "има", "няма", "казва", "каза",
    "прави", "вижте", "един", "една", "едно",
];

const TOPIC_WORDS: &[&str] = &[
    "политика", "новини", "скандал", "интервю", "избори", "правителство", "партия", "парламент",
    "държава", "общество", "видео", "българия", "foryou", "fyp", "foryourpage", "viral", "trend",
    "bulgaria", "news", "politics",
];

const KNOWN_CANONICAL_PEOPLE: &[(&str, &str)] = &[
    ("Делян Пеевски", "политик"),
    ("Румен Радев", "президент"),
    ("Бойко Борисов", "политик"),
    ("Радослав Ревански", "кмет"),
    ("Кирил Петков", "политик"),
    ("Асен Василев", "политик"),
    ("Илияна Йотова", "вицепрезидент"),
    ("Андрей Гюров", "финансист"),
    ("Костадин Костадинов", "политик"),
    ("Слави Трифонов", "политик"),
];

pub fn is_topic_word(w: &str) -> bool {
    let low = w.trim().to_lowercase();
    TOPIC_WORDS.contains(&low.as_str())
}

pub fn is_stopword_or_pronoun(w: &str) -> bool {
    let low = w.trim().to_lowercase();
    BULGARIAN_STOPWORDS.contains(&low.as_str())
}

pub fn resolve_single_surname_or_alias(token: &str) -> Option<&'static str> {
    let clean = token.trim().to_lowercase();
    for &(canonical, _) in KNOWN_CANONICAL_PEOPLE {
        let parts: Vec<&str> = canonical.split_whitespace().collect();
        if let Some(surname) = parts.last() {
            if clean == surname.to_lowercase() {
                return Some(canonical);
            }
        }
        let compact = canonical.to_lowercase().replace(' ', "");
        if clean == compact {
            return Some(canonical);
        }
    }
    None
}

pub fn sanitize_text_for_name_scanning(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut prev_was_hash = false;
    for ch in text.chars() {
        if ch == '#' {
            out.push_str(". ");
            prev_was_hash = true;
        } else if ch == ',' || ch == ';' || ch == ':' {
            out.push_str(". ");
            prev_was_hash = false;
        } else {
            out.push(ch);
            prev_was_hash = false;
        }
    }
    if prev_was_hash {
        out.push(' ');
    }
    out
}

pub fn split_concatenated_token(token: &str) -> Vec<String> {
    let clean = token.trim();
    if clean.is_empty() {
        return Vec::new();
    }

    let mut parts = Vec::new();
    let mut cur = String::new();
    for ch in clean.chars() {
        if ch.is_uppercase() && !cur.is_empty() {
            parts.push(cur);
            cur = String::new();
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        parts.push(cur);
    }

    if parts.len() > 1 {
        return parts;
    }

    let low = clean.to_lowercase();
    for &(canonical, _) in KNOWN_CANONICAL_PEOPLE {
        let compact = canonical.to_lowercase().replace(' ', "");
        if low == compact {
            return canonical.split_whitespace().map(|s| s.to_string()).collect();
        }
    }

    vec![clean.to_string()]
}

pub fn decompose_and_clean_entity(raw_name: &str) -> Vec<DecomposedEntityResult> {
    let trimmed = raw_name.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let low = trimmed.to_lowercase();

    for top in TOPIC_WORDS {
        if low.contains(top) {
            let stripped = low.replace(top, "");
            let clean_sub = stripped.trim().replace(' ', "");
            for &(canonical, role) in KNOWN_CANONICAL_PEOPLE {
                let compact = canonical.to_lowercase().replace(' ', "");
                let surname = canonical.split_whitespace().last().unwrap_or("").to_lowercase();
                if clean_sub == compact || clean_sub == surname {
                    return vec![DecomposedEntityResult {
                        name: canonical.to_string(),
                        role: Some(role.to_string()),
                        tag: Some((*top).to_string()),
                    }];
                }
            }
        }
    }

    let mut filtered_words = Vec::new();
    for word in trimmed.split_whitespace() {
        let w_low = word.to_lowercase();
        if !is_stopword_or_pronoun(&w_low) && !is_topic_word(&w_low) {
            filtered_words.push(word);
        }
    }

    if filtered_words.is_empty() {
        return Vec::new();
    }

    if filtered_words.len() == 1 {
        let single = filtered_words[0];
        if let Some(canonical) = resolve_single_surname_or_alias(single) {
            return vec![DecomposedEntityResult {
                name: canonical.to_string(),
                role: None,
                tag: None,
            }];
        }
    }

    if filtered_words.len() == 2 {
        let w1 = filtered_words[0];
        let w2 = filtered_words[1];
        let r1 = resolve_single_surname_or_alias(w1);
        let r2 = resolve_single_surname_or_alias(w2);
        if let (Some(p1), Some(p2)) = (r1, r2) {
            if p1 != p2 {
                return vec![
                    DecomposedEntityResult {
                        name: p1.to_string(),
                        role: None,
                        tag: None,
                    },
                    DecomposedEntityResult {
                        name: p2.to_string(),
                        role: None,
                        tag: None,
                    },
                ];
            }
        }
    }

    let compact_all = low.replace(' ', "");
    let mut detected_persons = Vec::new();
    for &(canonical, role) in KNOWN_CANONICAL_PEOPLE {
        let compact = canonical.to_lowercase().replace(' ', "");
        let surname = canonical.split_whitespace().last().unwrap_or("").to_lowercase();
        if compact_all.contains(&compact) {
            if !detected_persons.iter().any(|d: &DecomposedEntityResult| d.name == canonical) {
                detected_persons.push(DecomposedEntityResult {
                    name: canonical.to_string(),
                    role: Some(role.to_string()),
                    tag: None,
                });
            }
        } else if filtered_words.iter().any(|w| w.to_lowercase() == surname) {
            if !detected_persons.iter().any(|d: &DecomposedEntityResult| d.name == canonical) {
                detected_persons.push(DecomposedEntityResult {
                    name: canonical.to_string(),
                    role: Some(role.to_string()),
                    tag: None,
                });
            }
        }
    }

    if detected_persons.len() >= 2 {
        return detected_persons;
    } else if detected_persons.len() == 1 {
        let detected = &detected_persons[0];
        let detected_compact = detected.name.to_lowercase().replace(' ', "");
        let rem = compact_all.replace(&detected_compact, "");
        if rem.is_empty() || is_topic_word(&rem) || is_stopword_or_pronoun(&rem) {
            return detected_persons;
        }
    }

    let re_joined = filtered_words.join(" ");
    vec![DecomposedEntityResult {
        name: re_joined,
        role: None,
        tag: None,
    }]
}

pub fn generate_dedup_proposals_for_person(
    source_id: i64,
    source_name: &str,
    all_people: &[(i64, String)],
) -> Option<DedupProposal> {
    let clean = source_name.trim();
    if clean.is_empty() {
        return None;
    }

    let decomposed = decompose_and_clean_entity(clean);
    if decomposed.is_empty() {
        return None;
    }

    if decomposed.len() == 1 {
        let item = &decomposed[0];
        if item.name.eq_ignore_ascii_case(clean) && item.tag.is_none() {
            return None;
        }

        let target_entry = all_people
            .iter()
            .find(|(_, name)| name.eq_ignore_ascii_case(&item.name));

        if let Some(tag) = &item.tag {
            return Some(DedupProposal {
                id: format!("clean_tag_{source_id}"),
                action_type: DedupActionType::CleanupTag,
                confidence: 0.96,
                title: format!("Преобразуване на '{}' в таг за '{}'", clean, item.name),
                description: format!(
                    "Записът '{}' съдържа тематичен термин '{}'. Предлага се прехвърляне на видеата към '{}' и добавяне на таг #{}.",
                    clean, tag, item.name, tag
                ),
                source_id,
                source_name: clean.to_string(),
                target_names: vec![item.name.clone()],
                target_ids: target_entry.map(|(id, _)| vec![*id]).unwrap_or_default(),
                tag: Some(tag.clone()),
            });
        }

        if let Some((target_id, target_name)) = target_entry {
            if *target_id != source_id {
                return Some(DedupProposal {
                    id: format!("merge_{source_id}_{target_id}"),
                    action_type: DedupActionType::Merge,
                    confidence: 0.94,
                    title: format!("Обединяване на '{}' в '{}'", clean, target_name),
                    description: format!(
                        "Открит е дублиращ се запис за '{}'. Предлага се обединяване на профилите и видеозаписите в каноничния профил '{}'.",
                        clean, target_name
                    ),
                    source_id,
                    source_name: clean.to_string(),
                    target_names: vec![target_name.clone()],
                    target_ids: vec![*target_id],
                    tag: None,
                });
            }
        }
    } else if decomposed.len() >= 2 {
        let target_names: Vec<String> = decomposed.iter().map(|d| d.name.clone()).collect();
        let mut target_ids = Vec::new();
        for tname in &target_names {
            if let Some((tid, _)) = all_people
                .iter()
                .find(|(_, name)| name.eq_ignore_ascii_case(tname))
            {
                target_ids.push(*tid);
            }
        }

        return Some(DedupProposal {
            id: format!("split_{source_id}"),
            action_type: DedupActionType::SplitCompound,
            confidence: 0.95,
            title: format!(
                "Разделяне на съставния запис '{}' на {} лица",
                clean,
                target_names.len()
            ),
            description: format!(
                "Записът '{}' обединява няколко отделни публични фигури ({}). Предлага се разделяне на самостоятелни профили и свързване на видеата с всяко от лицата.",
                clean,
                target_names.join(" и ")
            ),
            source_id,
            source_name: clean.to_string(),
            target_names,
            target_ids,
            tag: None,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peevski_variants() {
        let dec_peevski = decompose_and_clean_entity("Пеевски");
        assert_eq!(dec_peevski.len(), 1);
        assert_eq!(dec_peevski[0].name, "Делян Пеевски");

        let dec_peevski_koito = decompose_and_clean_entity("Пеевски Който");
        assert_eq!(dec_peevski_koito.len(), 1);
        assert_eq!(dec_peevski_koito[0].name, "Делян Пеевски");

        let dec_peevski_borisov = decompose_and_clean_entity("Пеевски Борисов");
        assert_eq!(dec_peevski_borisov.len(), 2);
        assert_eq!(dec_peevski_borisov[0].name, "Делян Пеевски");
        assert_eq!(dec_peevski_borisov[1].name, "Бойко Борисов");

        let dec_rumenradev_peevski = decompose_and_clean_entity("Руменрадев Пеевски");
        assert_eq!(dec_rumenradev_peevski.len(), 2);
        assert!(dec_rumenradev_peevski.iter().any(|p| p.name == "Румен Радев"));
        assert!(dec_rumenradev_peevski.iter().any(|p| p.name == "Делян Пеевски"));

        let dec_revanski_peevski = decompose_and_clean_entity("Радославревански Делянпеевски");
        assert_eq!(dec_revanski_peevski.len(), 2);
        assert!(dec_revanski_peevski.iter().any(|p| p.name == "Радослав Ревански"));
        assert!(dec_revanski_peevski.iter().any(|p| p.name == "Делян Пеевски"));

        let dec_peevski_politika = decompose_and_clean_entity("Делянпеевски Политика");
        assert_eq!(dec_peevski_politika.len(), 1);
        assert_eq!(dec_peevski_politika[0].name, "Делян Пеевски");
        assert_eq!(dec_peevski_politika[0].tag.as_deref(), Some("политика"));

        let dec_mr_radev = decompose_and_clean_entity("Господин Радев");
        assert_eq!(dec_mr_radev.len(), 1);
        assert_eq!(dec_mr_radev[0].name, "Румен Радев");

        let dec_radev_imam = decompose_and_clean_entity("Радев Имам");
        assert_eq!(dec_radev_imam.len(), 1);
        assert_eq!(dec_radev_imam[0].name, "Румен Радев");
    }
}
