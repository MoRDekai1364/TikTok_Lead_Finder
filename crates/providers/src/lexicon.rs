use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use leadfinder_core::{RuntimePaths, TranscriptRecord};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PairRule {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepairRule {
    pub wrong: String,
    pub right: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DialectGlossaryEntry {
    pub term: String,
    pub standard: String,
    pub register: String,
    #[serde(default)]
    pub ipa: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AcronymEntry {
    pub abbr: String,
    pub expansion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LexiconConfig {
    pub language_code: String,
    #[serde(default)]
    pub frequency_file: String,
    #[serde(default)]
    pub hunspell_dic_file: String,
    #[serde(default)]
    pub hunspell_aff_file: String,
    #[serde(default)]
    pub mfa_ipa_file: String,
    #[serde(default)]
    pub wiktionary_tsv_file: String,
    #[serde(default)]
    pub whisper_initial_prompt: String,
    #[serde(default)]
    pub word_end_strip_chars: Vec<String>,
    #[serde(default)]
    pub enclitics: Vec<String>,
    #[serde(default)]
    pub proclitic_devoiced_prefix: String,
    #[serde(default)]
    pub proclitic_restored_word: String,
    #[serde(default)]
    pub comparative_particles: Vec<String>,
    #[serde(default)]
    pub renarrated_auxiliaries: Vec<String>,
    #[serde(default)]
    pub compound_join_left: Vec<String>,
    #[serde(default)]
    pub domain_vocabulary: Vec<String>,
    #[serde(default)]
    pub char_replacements: Vec<PairRule>,
    #[serde(default)]
    pub cluster_replacements: Vec<PairRule>,
    #[serde(default)]
    pub asr_repairs: Vec<RepairRule>,
    #[serde(default)]
    pub orth_phonetic_clusters: Vec<PairRule>,
    #[serde(default)]
    pub orth_phonetic_chars: Vec<PairRule>,
    #[serde(default)]
    pub orth_final_devoice: Vec<PairRule>,
    #[serde(default)]
    pub ipa_phonetic_clusters: Vec<PairRule>,
    #[serde(default)]
    pub ipa_phonetic_chars: Vec<PairRule>,
    #[serde(default)]
    pub dialect_glossary: Vec<DialectGlossaryEntry>,
    #[serde(default)]
    pub acronyms: Vec<AcronymEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LexiconStats {
    pub language_code: String,
    pub valid_wordforms: usize,
    pub frequency_entries: usize,
    pub hunspell_lemmas: usize,
    pub ipa_transcriptions: usize,
    pub dialect_and_informal_entries: usize,
    pub phonetic_keys: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryMatch {
    pub matched_term: String,
    pub standard_meaning: String,
    pub register: String,
    pub ipa: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinguisticContext {
    pub glossary_matches: Vec<GlossaryMatch>,
    pub acronym_matches: Vec<AcronymEntry>,
    pub renarrated_evidential_markers: Vec<String>,
    pub lemmas: Vec<String>,
    pub prompt_notes: String,
}

#[derive(Debug, Clone)]
struct NonStandardEntry {
    standard: String,
    register: String,
    ipa: String,
}

#[derive(Debug, Clone)]
struct HunspellAffRule {
    strip: String,
    append: String,
}

pub struct LexiconNormalizer {
    cfg: LexiconConfig,
    valid_words: HashSet<String>,
    word_freq: HashMap<String, u64>,
    form_to_lemma: HashMap<String, String>,
    word_to_ipa: HashMap<String, String>,
    pure_adj_adv: HashSet<String>,
    non_adj_adv: HashSet<String>,
    renarrated_forms: HashSet<String>,
    phonetic_index: HashMap<String, Vec<String>>,
    delete_index: HashMap<String, Vec<String>>,
    glossary_map: HashMap<String, DialectGlossaryEntry>,
    wiktionary_nonstd: HashMap<String, NonStandardEntry>,
    acronym_map: HashMap<String, AcronymEntry>,
    compound_left_set: HashSet<String>,
    renarrated_aux_set: HashSet<String>,
    single_word_repairs: HashMap<String, String>,
    bigram_repairs: HashMap<(String, String), String>,
    hunspell_lemma_count: usize,
}

static GLOBAL_LEXICON: OnceLock<Arc<LexiconNormalizer>> = OnceLock::new();

pub fn global_lexicon(paths: &RuntimePaths, lexicon_config_file: &str) -> Arc<LexiconNormalizer> {
    GLOBAL_LEXICON
        .get_or_init(|| Arc::new(LexiconNormalizer::load(paths, lexicon_config_file)))
        .clone()
}

impl LexiconNormalizer {
    pub fn empty() -> Self {
        Self {
            cfg: LexiconConfig::default(),
            valid_words: HashSet::new(),
            word_freq: HashMap::new(),
            form_to_lemma: HashMap::new(),
            word_to_ipa: HashMap::new(),
            pure_adj_adv: HashSet::new(),
            non_adj_adv: HashSet::new(),
            renarrated_forms: HashSet::new(),
            phonetic_index: HashMap::new(),
            delete_index: HashMap::new(),
            glossary_map: HashMap::new(),
            wiktionary_nonstd: HashMap::new(),
            acronym_map: HashMap::new(),
            compound_left_set: HashSet::new(),
            renarrated_aux_set: HashSet::new(),
            single_word_repairs: HashMap::new(),
            bigram_repairs: HashMap::new(),
            hunspell_lemma_count: 0,
        }
    }

    pub fn load(paths: &RuntimePaths, lexicon_config_file: &str) -> Self {
        let cfg_path = paths.config_dir.join(lexicon_config_file);
        let cfg: LexiconConfig = std::fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|raw| toml::from_str(&raw).ok())
            .unwrap_or_default();

        let mut norm = Self::empty();
        norm.cfg = cfg.clone();

        for left in &cfg.compound_join_left {
            norm.compound_left_set.insert(left.to_lowercase());
        }
        for aux in &cfg.renarrated_auxiliaries {
            norm.renarrated_aux_set.insert(aux.to_lowercase());
        }
        for g in &cfg.dialect_glossary {
            let key = g.term.to_lowercase();
            norm.valid_words.insert(key.clone());
            if !g.ipa.is_empty() {
                norm.word_to_ipa.insert(key.clone(), g.ipa.clone());
            }
            norm.glossary_map.insert(key, g.clone());
        }
        for a in &cfg.acronyms {
            let key = a.abbr.to_lowercase();
            norm.valid_words.insert(key.clone());
            norm.word_freq.entry(key.clone()).or_insert(50_000);
            norm.acronym_map.insert(key, a.clone());
        }
        for dom in &cfg.domain_vocabulary {
            let low = dom.to_lowercase();
            norm.valid_words.insert(low.clone());
            let entry = norm.word_freq.entry(low).or_insert(0);
            *entry = (*entry).max(25_000);
        }
        for rep in &cfg.asr_repairs {
            let parts: Vec<String> = rep
                .wrong
                .split_whitespace()
                .map(|s| s.to_lowercase())
                .collect();
            if parts.len() == 1 {
                norm.single_word_repairs
                    .insert(parts[0].clone(), rep.right.clone());
            } else if parts.len() == 2 {
                norm.bigram_repairs
                    .insert((parts[0].clone(), parts[1].clone()), rep.right.clone());
            }
        }

        if !cfg.frequency_file.is_empty() {
            if let Some(p) = paths.resolve_model_path(&cfg.frequency_file) {
                norm.load_frequency_file(&p);
            }
        }

        let aff_rules = if !cfg.hunspell_aff_file.is_empty() {
            paths
                .resolve_model_path(&cfg.hunspell_aff_file)
                .map(|p| parse_hunspell_sfx(&p))
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        if !cfg.hunspell_dic_file.is_empty() {
            if let Some(p) = paths.resolve_model_path(&cfg.hunspell_dic_file) {
                norm.load_hunspell_dic(&p, &aff_rules);
            }
        }

        if !cfg.wiktionary_tsv_file.is_empty() {
            if let Some(p) = paths.resolve_model_path(&cfg.wiktionary_tsv_file) {
                norm.load_wiktionary_tsv(&p);
            }
        }

        if !cfg.mfa_ipa_file.is_empty() {
            if let Some(p) = paths.resolve_model_path(&cfg.mfa_ipa_file) {
                norm.load_mfa_ipa_dict(&p);
            }
        }

        norm.build_indexes();
        norm
    }

    pub fn stats(&self) -> LexiconStats {
        LexiconStats {
            language_code: self.cfg.language_code.clone(),
            valid_wordforms: self.valid_words.len(),
            frequency_entries: self.word_freq.len(),
            hunspell_lemmas: self.hunspell_lemma_count,
            ipa_transcriptions: self.word_to_ipa.len(),
            dialect_and_informal_entries: self.glossary_map.len() + self.wiktionary_nonstd.len(),
            phonetic_keys: self.phonetic_index.len(),
        }
    }

    pub fn whisper_prompt(&self, expected_languages: &[String]) -> Option<&str> {
        if self.cfg.whisper_initial_prompt.is_empty() {
            return None;
        }
        let matches_lang = expected_languages
            .first()
            .map(|l| l.eq_ignore_ascii_case(&self.cfg.language_code))
            .unwrap_or(true);
        if matches_lang {
            Some(self.cfg.whisper_initial_prompt.as_str())
        } else {
            None
        }
    }

    pub fn phonetic_transcription(&self, word: &str) -> Option<&str> {
        let low = word.trim().to_lowercase();
        self.word_to_ipa.get(&low).map(|s| s.as_str())
    }

    pub fn lemmatize_word(&self, word: &str) -> String {
        let low = strip_stress_marks(word.trim()).to_lowercase();
        if let Some(lem) = self.form_to_lemma.get(&low) {
            return lem.clone();
        }
        low
    }

    pub fn is_known_word(&self, word: &str) -> bool {
        let low = word.trim().to_lowercase();
        self.valid_words.contains(&low)
    }

    pub fn normalize_transcript(&self, record: &mut TranscriptRecord) {
        if self.cfg.language_code.is_empty()
            || !record.language.eq_ignore_ascii_case(&self.cfg.language_code)
        {
            return;
        }
        for seg in &mut record.segments {
            seg.text = self.normalize_text(&seg.text);
        }
        record.text = self.normalize_text(&record.text);
    }

    pub fn normalize_text(&self, input: &str) -> String {
        if self.valid_words.is_empty() || input.trim().is_empty() {
            return input.to_string();
        }

        let raw_tokens: Vec<&str> = input.split_whitespace().collect();
        if raw_tokens.is_empty() {
            return input.to_string();
        }

        let mut stage1_tokens: Vec<String> = Vec::with_capacity(raw_tokens.len() + 4);
        for tok in raw_tokens {
            let (lead, core, trail) = split_punct(tok);
            if core.is_empty() {
                stage1_tokens.push(tok.to_string());
                continue;
            }
            let cleaned = self.clean_orthography(&core);
            let low = cleaned.to_lowercase();

            if let Some(rep) = self.single_word_repairs.get(&low) {
                let cased = match_case(&cleaned, rep);
                push_possibly_multiword(&mut stage1_tokens, &lead, &cased, &trail);
                continue;
            }

            if self.valid_words.contains(&low) || !is_all_alphabetic_non_ascii(&low) {
                stage1_tokens.push(format!("{lead}{cleaned}{trail}"));
                continue;
            }

            if let Some((stem, clitic)) = self.try_split_enclitic(&low) {
                let cased_stem = match_case(&cleaned, &stem);
                stage1_tokens.push(format!("{lead}{cased_stem}"));
                stage1_tokens.push(format!("{clitic}{trail}"));
                continue;
            }

            if let Some((prep, rest)) = self.try_split_proclitic(&low) {
                let cased_prep = match_case(&cleaned, &prep);
                stage1_tokens.push(format!("{lead}{cased_prep}"));
                stage1_tokens.push(format!("{rest}{trail}"));
                continue;
            }

            if let Some(repaired) = self.try_repair_oov(&low) {
                let cased = match_case(&cleaned, &repaired);
                stage1_tokens.push(format!("{lead}{cased}{trail}"));
                continue;
            }

            stage1_tokens.push(format!("{lead}{cleaned}{trail}"));
        }

        let split_items: Vec<(String, String, String)> =
            stage1_tokens.iter().map(|t| split_punct(t)).collect();

        let mut out_words: Vec<String> = Vec::with_capacity(split_items.len());
        let mut idx = 0;
        while idx < split_items.len() {
            let (ref lead1, ref core1, ref trail1) = split_items[idx];
            if core1.is_empty() {
                out_words.push(format!("{lead1}{trail1}"));
                idx += 1;
                continue;
            }
            let low1 = core1.to_lowercase();

            if idx + 1 < split_items.len() && trail1.is_empty() && split_items[idx + 1].0.is_empty() {
                let (_, ref core2, ref trail2) = split_items[idx + 1];
                if !core2.is_empty() {
                    let low2 = core2.to_lowercase();

                    if let Some(rep) = self.bigram_repairs.get(&(low1.clone(), low2.clone())) {
                        let cased = match_case(core1, rep);
                        out_words.push(format!("{lead1}{cased}{trail2}"));
                        idx += 2;
                        continue;
                    }

                    let joined = format!("{low1}{low2}");
                    let joined_freq = self.word_freq.get(&joined).copied().unwrap_or(0);
                    let left_is_prefix = self.compound_left_set.contains(&low1)
                        || !self.valid_words.contains(&low1);
                    if self.valid_words.contains(&joined) && joined_freq >= 20 && left_is_prefix {
                        let cased = match_case(core1, &joined);
                        out_words.push(format!("{lead1}{cased}{trail2}"));
                        idx += 2;
                        continue;
                    }

                    if self.cfg.comparative_particles.iter().any(|p| p == &low1)
                        && self.is_comparative_target(&low1, &low2)
                    {
                        let p_cased = match_case(core1, &low1);
                        out_words.push(format!("{lead1}{p_cased}-{core2}{trail2}"));
                        idx += 2;
                        continue;
                    }
                }
            }

            out_words.push(format!("{lead1}{core1}{trail1}"));
            idx += 1;
        }

        out_words.join(" ")
    }

    pub fn analyze_for_llm(&self, text: &str) -> LinguisticContext {
        let mut ctx = LinguisticContext::default();
        if text.trim().is_empty() {
            return ctx;
        }

        let mut seen_glossary = HashSet::new();
        let mut seen_acronyms = HashSet::new();
        let mut seen_renarrated = HashSet::new();
        let mut seen_lemmas = HashSet::new();

        let tokens: Vec<String> = text
            .split_whitespace()
            .map(|t| {
                let (_, core, _) = split_punct(t);
                core
            })
            .filter(|s| !s.is_empty())
            .collect();

        for (i, tok) in tokens.iter().enumerate() {
            let low = tok.to_lowercase();

            if let Some(acr) = self.acronym_map.get(&low) {
                if seen_acronyms.insert(low.clone()) {
                    ctx.acronym_matches.push(acr.clone());
                }
            }

            if let Some(g) = self.glossary_map.get(&low) {
                if seen_glossary.insert(low.clone()) {
                    ctx.glossary_matches.push(GlossaryMatch {
                        matched_term: tok.clone(),
                        standard_meaning: g.standard.clone(),
                        register: g.register.clone(),
                        ipa: if g.ipa.is_empty() {
                            None
                        } else {
                            Some(g.ipa.clone())
                        },
                    });
                }
            } else if let Some(ns) = self.wiktionary_nonstd.get(&low) {
                if !ns.standard.is_empty() && seen_glossary.insert(low.clone()) {
                    ctx.glossary_matches.push(GlossaryMatch {
                        matched_term: tok.clone(),
                        standard_meaning: ns.standard.clone(),
                        register: ns.register.clone(),
                        ipa: if ns.ipa.is_empty() {
                            None
                        } else {
                            Some(ns.ipa.clone())
                        },
                    });
                }
            }

            if self.renarrated_aux_set.contains(&low) {
                let phrase = if i + 1 < tokens.len() {
                    format!("{} {}", tok, tokens[i + 1])
                } else {
                    tok.clone()
                };
                if seen_renarrated.insert(phrase.to_lowercase()) {
                    ctx.renarrated_evidential_markers.push(phrase);
                }
            } else if self.renarrated_forms.contains(&low) && low.chars().count() >= 5 {
                if seen_renarrated.insert(low.clone()) && ctx.renarrated_evidential_markers.len() < 6 {
                    ctx.renarrated_evidential_markers.push(tok.clone());
                }
            }

            if low.chars().count() >= 4 {
                let lem = self.lemmatize_word(&low);
                if seen_lemmas.insert(lem.clone()) && ctx.lemmas.len() < 48 {
                    ctx.lemmas.push(lem);
                }
            }
        }

        let mut note_lines = Vec::new();
        if !ctx.glossary_matches.is_empty() {
            let items: Vec<String> = ctx
                .glossary_matches
                .iter()
                .take(10)
                .map(|m| {
                    if let Some(ref ipa) = m.ipa {
                        format!("\"{}\" [{}] -> {} ({})", m.matched_term, ipa, m.standard_meaning, m.register)
                    } else {
                        format!("\"{}\" -> {} ({})", m.matched_term, m.standard_meaning, m.register)
                    }
                })
                .collect();
            note_lines.push(format!("Dialect/informal lexicon: {}", items.join("; ")));
        }
        if !ctx.acronym_matches.is_empty() {
            let items: Vec<String> = ctx
                .acronym_matches
                .iter()
                .take(8)
                .map(|a| format!("{} = {}", a.abbr, a.expansion))
                .collect();
            note_lines.push(format!("Acronyms: {}", items.join("; ")));
        }
        if !ctx.renarrated_evidential_forms_summary().is_empty() {
            note_lines.push(format!(
                "Renarrated/non-witnessed grammatical forms (check whether attributed/hearsay): {}",
                ctx.renarrated_evidential_forms_summary()
            ));
        }
        ctx.prompt_notes = note_lines.join("\n");
        ctx
    }

    fn clean_orthography(&self, word: &str) -> String {
        let was_upper = word.chars().all(|c| !c.is_alphabetic() || c.is_uppercase())
            && word.chars().any(|c| c.is_uppercase());
        let first_upper = word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);

        let mut cur = word.to_lowercase();
        for rule in &self.cfg.char_replacements {
            if cur.contains(&rule.from) {
                cur = cur.replace(&rule.from, &rule.to);
            }
        }
        for rule in &self.cfg.cluster_replacements {
            if cur.contains(&rule.from) {
                cur = cur.replace(&rule.from, &rule.to);
            }
        }
        for strip_end in &self.cfg.word_end_strip_chars {
            if cur.chars().count() >= 3 && cur.ends_with(strip_end) {
                let new_len = cur.len() - strip_end.len();
                cur.truncate(new_len);
            }
        }

        if cur == word.to_lowercase() {
            return word.to_string();
        }
        if was_upper {
            cur.to_uppercase()
        } else if first_upper {
            capitalize_first(&cur)
        } else {
            cur
        }
    }

    fn is_comparative_target(&self, particle: &str, next_low: &str) -> bool {
        if next_low.chars().count() < 3 {
            return false;
        }
        if particle == "най" {
            self.pure_adj_adv.contains(next_low)
        } else {
            self.pure_adj_adv.contains(next_low) && !self.non_adj_adv.contains(next_low)
        }
    }

    fn try_split_enclitic(&self, low: &str) -> Option<(String, String)> {
        let total_chars = low.chars().count();
        if total_chars < 5 {
            return None;
        }
        for clitic in &self.cfg.enclitics {
            if low.ends_with(clitic) {
                let stem_bytes = low.len() - clitic.len();
                let stem = &low[..stem_bytes];
                let stem_chars = stem.chars().count();
                if stem_chars < 3 {
                    continue;
                }
                if self.valid_words.contains(stem) {
                    let freq = self.word_freq.get(stem).copied().unwrap_or(10);
                    if freq >= 10 || stem_chars >= 4 {
                        return Some((stem.to_string(), clitic.clone()));
                    }
                }
                if stem_chars >= 4 {
                    let pkey = self.orth_phonetic_key(stem);
                    if let Some(cands) = self.phonetic_index.get(&pkey) {
                        for cand in cands {
                            if char_edit_distance(stem, cand) <= 1 {
                                let freq = self.word_freq.get(cand).copied().unwrap_or(0);
                                if freq >= 25 {
                                    return Some((cand.clone(), clitic.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn try_split_proclitic(&self, low: &str) -> Option<(String, String)> {
        if self.cfg.proclitic_devoiced_prefix.is_empty()
            || self.cfg.proclitic_restored_word.is_empty()
        {
            return None;
        }
        if low.chars().count() < 5 || !low.starts_with(&self.cfg.proclitic_devoiced_prefix) {
            return None;
        }
        let rest = &low[self.cfg.proclitic_devoiced_prefix.len()..];
        if self.valid_words.contains(rest) {
            let freq = self.word_freq.get(rest).copied().unwrap_or(0);
            if freq >= 80 {
                return Some((self.cfg.proclitic_restored_word.clone(), rest.to_string()));
            }
        }
        None
    }

    fn try_repair_oov(&self, low: &str) -> Option<String> {
        let n_chars = low.chars().count();
        if n_chars < 4 {
            return None;
        }

        let pkey = self.orth_phonetic_key(low);
        if !pkey.is_empty() {
            if let Some(cands) = self.phonetic_index.get(&pkey) {
                let mut best: Option<(&String, u64, usize)> = None;
                for cand in cands {
                    let dist = char_edit_distance(low, cand);
                    let len_diff = (cand.chars().count() as isize - n_chars as isize).unsigned_abs();
                    if dist <= 2 && len_diff <= 1 {
                        let freq = self.word_freq.get(cand).copied().unwrap_or(5);
                        if freq < 20 {
                            continue;
                        }
                        let better = match best {
                            None => true,
                            Some((_, bf, bd)) => {
                                if dist != bd {
                                    dist < bd
                                } else {
                                    freq > bf
                                }
                            }
                        };
                        if better {
                            best = Some((cand, freq, dist));
                        }
                    }
                }
                if let Some((winner, _, _)) = best {
                    return Some(winner.clone());
                }
            }
        }

        if n_chars >= 7 {
            let mut best_1edit: Option<(String, u64)> = None;

            if let Some(cands) = self.delete_index.get(low) {
                for cand in cands {
                    if is_single_insertion_or_deletion(low, cand) {
                        let freq = self.word_freq.get(cand).copied().unwrap_or(0);
                        if freq >= 60
                            && best_1edit.as_ref().map(|(_, bf)| freq > *bf).unwrap_or(true)
                        {
                            best_1edit = Some((cand.clone(), freq));
                        }
                    }
                }
            }

            let chars: Vec<char> = low.chars().collect();
            for del_idx in 0..chars.len() {
                let reduced: String = chars
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, &ch)| if idx == del_idx { None } else { Some(ch) })
                    .collect();
                if self.valid_words.contains(&reduced) {
                    let freq = self.word_freq.get(&reduced).copied().unwrap_or(0);
                    if freq >= 60
                        && best_1edit.as_ref().map(|(_, bf)| freq > *bf).unwrap_or(true)
                    {
                        best_1edit = Some((reduced, freq));
                    }
                }
            }

            if let Some((winner, _)) = best_1edit {
                return Some(winner);
            }
        }

        None
    }

    pub fn orth_phonetic_key(&self, word: &str) -> String {
        let mut cur = word.trim().to_lowercase();
        for cl in &self.cfg.orth_phonetic_clusters {
            if cur.contains(&cl.from) {
                cur = cur.replace(&cl.from, &cl.to);
            }
        }
        let mut mapped = String::with_capacity(cur.len());
        for ch in cur.chars() {
            if ch.is_ascii_uppercase() {
                mapped.push(ch);
                continue;
            }
            let s = ch.to_string();
            if let Some(rule) = self.cfg.orth_phonetic_chars.iter().find(|r| r.from == s) {
                mapped.push_str(&rule.to);
            } else if ch.is_alphabetic() {
                mapped.push(ch);
            }
        }
        for fin in &self.cfg.orth_final_devoice {
            if mapped.ends_with(&fin.from) {
                let keep = mapped.len() - fin.from.len();
                mapped.truncate(keep);
                mapped.push_str(&fin.to);
                break;
            }
        }
        dedup_adjacent_ascii(&mapped)
    }

    pub fn ipa_phonetic_key(&self, ipa: &str) -> String {
        let mut cur: String = ipa
            .chars()
            .filter(|&c| {
                !matches!(
                    c,
                    'ˈ' | 'ˌ' | '.' | ' ' | '\t' | '[' | ']' | '/' | '̪' | 'ʲ' | 'ː' | 'ˑ'
                )
            })
            .collect();
        for cl in &self.cfg.ipa_phonetic_clusters {
            if cur.contains(&cl.from) {
                cur = cur.replace(&cl.from, &cl.to);
            }
        }
        let mut mapped = String::with_capacity(cur.len());
        for ch in cur.chars() {
            if ch.is_ascii_uppercase() {
                mapped.push(ch);
                continue;
            }
            let s = ch.to_string();
            if let Some(rule) = self.cfg.ipa_phonetic_chars.iter().find(|r| r.from == s) {
                mapped.push_str(&rule.to);
            }
        }
        for fin in &self.cfg.orth_final_devoice {
            if mapped.ends_with(&fin.from) {
                let keep = mapped.len() - fin.from.len();
                mapped.truncate(keep);
                mapped.push_str(&fin.to);
                break;
            }
        }
        dedup_adjacent_ascii(&mapped)
    }

    fn load_frequency_file(&mut self, path: &Path) {
        let Ok(file) = std::fs::File::open(path) else {
            return;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let mut parts = line.split_whitespace();
            let Some(raw_w) = parts.next() else {
                continue;
            };
            let freq = parts
                .next()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(1);
            let w = raw_w.trim().to_lowercase();
            if w.chars().count() >= 1 && is_all_alphabetic_non_ascii(&w) {
                self.valid_words.insert(w.clone());
                let entry = self.word_freq.entry(w).or_insert(0);
                *entry = (*entry).max(freq);
            }
        }
    }

    fn load_hunspell_dic(&mut self, path: &Path, aff_rules: &HashMap<char, Vec<HunspellAffRule>>) {
        let Ok(file) = std::fs::File::open(path) else {
            return;
        };
        for (line_idx, line) in BufReader::new(file).lines().map_while(Result::ok).enumerate() {
            if line_idx == 0 && line.trim().chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (raw_lemma, flags_opt) = match trimmed.split_once('/') {
                Some((l, f)) => (l.trim(), Some(f.trim())),
                None => (trimmed, None),
            };
            let lemma = raw_lemma.to_lowercase();
            if !is_all_alphabetic_non_ascii(&lemma) {
                continue;
            }
            self.hunspell_lemma_count += 1;
            self.valid_words.insert(lemma.clone());
            self.word_freq.entry(lemma.clone()).or_insert(25);
            self.form_to_lemma.entry(lemma.clone()).or_insert_with(|| lemma.clone());

            if let Some(flags) = flags_opt {
                for flag_ch in flags.chars() {
                    if let Some(rules) = aff_rules.get(&flag_ch) {
                        for rule in rules {
                            let expanded = if rule.strip.is_empty() {
                                Some(format!("{lemma}{}", rule.append))
                            } else if lemma.ends_with(&rule.strip) && lemma.len() > rule.strip.len() {
                                let base = &lemma[..lemma.len() - rule.strip.len()];
                                Some(format!("{base}{}", rule.append))
                            } else {
                                None
                            };
                            if let Some(form) = expanded {
                                if is_all_alphabetic_non_ascii(&form) {
                                    self.valid_words.insert(form.clone());
                                    self.word_freq.entry(form.clone()).or_insert(12);
                                    self.form_to_lemma
                                        .entry(form)
                                        .or_insert_with(|| lemma.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn load_wiktionary_tsv(&mut self, path: &Path) {
        let Ok(file) = std::fs::File::open(path) else {
            return;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 5 {
                continue;
            }
            let form = cols[0].trim().to_lowercase();
            if form.is_empty() {
                continue;
            }
            let lemma = cols[1].trim().to_lowercase();
            let pos = cols[2].trim();
            let ipa = cols[3].trim();
            let reg = cols[4].trim();
            let note = cols.get(5).copied().unwrap_or("").trim();

            self.valid_words.insert(form.clone());
            let base_freq = if reg == "standard" { 20 } else { 8 };
            self.word_freq.entry(form.clone()).or_insert(base_freq);
            if !lemma.is_empty() {
                self.form_to_lemma.insert(form.clone(), lemma);
            }
            if !ipa.is_empty() {
                self.word_to_ipa.entry(form.clone()).or_insert_with(|| ipa.to_string());
            }

            let base_pos = pos.split(':').next().unwrap_or(pos);
            if base_pos == "adj" || base_pos == "adv" {
                self.pure_adj_adv.insert(form.clone());
            } else if !base_pos.is_empty() {
                self.non_adj_adv.insert(form.clone());
            }
            if pos.ends_with(":renarrated") {
                self.renarrated_forms.insert(form.clone());
            }

            if reg != "standard" && !note.is_empty() {
                self.wiktionary_nonstd.insert(
                    form,
                    NonStandardEntry {
                        standard: note.to_string(),
                        register: reg.to_string(),
                        ipa: ipa.to_string(),
                    },
                );
            }
        }
    }

    fn load_mfa_ipa_dict(&mut self, path: &Path) {
        let Ok(file) = std::fs::File::open(path) else {
            return;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 2 {
                continue;
            }
            let w = cols[0].trim().to_lowercase();
            if w.starts_with('<') || !is_all_alphabetic_non_ascii(&w) {
                continue;
            }
            let phones = cols.last().copied().unwrap_or("").trim();
            if phones.is_empty() || phones == "sil" || phones == "spn" {
                continue;
            }
            self.valid_words.insert(w.clone());
            self.word_freq.entry(w.clone()).or_insert(18);
            self.word_to_ipa.entry(w).or_insert_with(|| phones.to_string());
        }
    }

    fn build_indexes(&mut self) {
        for w in &self.valid_words {
            let n_chars = w.chars().count();
            if n_chars < 4 {
                continue;
            }
            let freq = self.word_freq.get(w).copied().unwrap_or(0);
            if freq < 18 {
                continue;
            }

            let okey = self.orth_phonetic_key(w);
            if okey.len() >= 3 {
                let list = self.phonetic_index.entry(okey).or_default();
                if !list.contains(w) && list.len() < 12 {
                    list.push(w.clone());
                }
            }

            if let Some(ipa) = self.word_to_ipa.get(w) {
                let ikey = self.ipa_phonetic_key(ipa);
                if ikey.len() >= 3 {
                    let list = self.phonetic_index.entry(ikey).or_default();
                    if !list.contains(w) && list.len() < 12 {
                        list.push(w.clone());
                    }
                }
            }

            if n_chars >= 7 && freq >= 60 {
                let chars: Vec<char> = w.chars().collect();
                for del_idx in 0..chars.len() {
                    let key: String = chars
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, &ch)| if idx == del_idx { None } else { Some(ch) })
                        .collect();
                    let list = self.delete_index.entry(key).or_default();
                    if list.len() < 6 {
                        list.push(w.clone());
                    }
                }
            }
        }
    }
}

impl LinguisticContext {
    pub fn renarrated_evidential_forms_summary(&self) -> String {
        self.renarrated_evidential_markers
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn parse_hunspell_sfx(path: &Path) -> HashMap<char, Vec<HunspellAffRule>> {
    let mut map: HashMap<char, Vec<HunspellAffRule>> = HashMap::new();
    let Ok(file) = std::fs::File::open(path) else {
        return map;
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 && parts[0] == "SFX" {
            let Some(flag_ch) = parts[1].chars().next() else {
                continue;
            };
            if parts[2] == "Y" || parts[2] == "N" {
                continue;
            }
            let strip = if parts[2] == "0" {
                String::new()
            } else {
                parts[2].to_lowercase()
            };
            let append_raw = parts[3].split('/').next().unwrap_or(parts[3]);
            let append = if append_raw == "0" {
                String::new()
            } else {
                append_raw.to_lowercase()
            };
            map.entry(flag_ch)
                .or_default()
                .push(HunspellAffRule { strip, append });
        }
    }
    map
}

pub fn compile_wiktionary_jsonl_to_tsv(jsonl_path: &Path, out_tsv: &Path) -> Result<usize> {
    let file = std::fs::File::open(jsonl_path)?;
    let reader = BufReader::new(file);

    let mut entries: HashMap<String, (String, String, String, String, String)> = HashMap::new();
    let skip_tags = ["table-tags", "inflection-template", "romanization", "class"];
    let nonstd_keys = [
        "dialectal",
        "colloquial",
        "slang",
        "informal",
        "regional",
        "vernacular",
        "nonstandard",
        "contraction",
        "abbreviation",
    ];

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(obj) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let raw_word = obj.get("word").and_then(|v| v.as_str()).unwrap_or("").trim();
        let lemma = strip_stress_marks(raw_word).to_lowercase();
        if lemma.is_empty() || !is_all_alphabetic_non_ascii(&lemma) {
            continue;
        }
        let pos = obj
            .get("pos")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();

        let mut ipa = String::new();
        if let Some(sounds) = obj.get("sounds").and_then(|v| v.as_array()) {
            for s in sounds {
                if let Some(cand) = s.get("ipa").and_then(|v| v.as_str()) {
                    let trimmed = cand.trim().trim_matches(|c| c == '[' || c == ']' || c == '/');
                    if !trimmed.is_empty() {
                        ipa = trimmed.to_string();
                        break;
                    }
                }
            }
        }

        let mut has_std = false;
        let mut reg_tag = String::new();
        let mut target_note = String::new();

        if let Some(senses) = obj.get("senses").and_then(|v| v.as_array()) {
            for s in senses {
                let mut tags = Vec::new();
                if let Some(arr) = s.get("tags").and_then(|v| v.as_array()) {
                    for t in arr {
                        if let Some(ts) = t.as_str() {
                            tags.push(ts.to_lowercase());
                        }
                    }
                }
                if let Some(arr) = s.get("raw_tags").and_then(|v| v.as_array()) {
                    for t in arr {
                        if let Some(ts) = t.as_str() {
                            tags.push(ts.to_lowercase());
                        }
                    }
                }
                let is_nonstd = nonstd_keys.iter().any(|k| tags.iter().any(|t| t == k));
                if !is_nonstd {
                    has_std = true;
                } else {
                    if reg_tag.is_empty() {
                        reg_tag = nonstd_keys
                            .iter()
                            .find(|&&k| tags.iter().any(|t| t == k))
                            .copied()
                            .unwrap_or("colloquial")
                            .to_string();
                    }
                    if target_note.is_empty() {
                        let alt = s
                            .get("alt_of")
                            .or_else(|| s.get("form_of"))
                            .and_then(|v| v.as_array())
                            .and_then(|a| a.first())
                            .and_then(|x| x.get("word"))
                            .and_then(|v| v.as_str());
                        let syn = s
                            .get("synonyms")
                            .and_then(|v| v.as_array())
                            .and_then(|a| a.first())
                            .and_then(|x| x.get("word"))
                            .and_then(|v| v.as_str());
                        let gloss = s
                            .get("glosses")
                            .and_then(|v| v.as_array())
                            .and_then(|a| a.first())
                            .and_then(|v| v.as_str());

                        if let Some(w) = alt.or(syn) {
                            target_note = strip_stress_marks(w)
                                .split(|c: char| c.is_whitespace() || c == '(')
                                .next()
                                .unwrap_or("")
                                .to_lowercase();
                        } else if let Some(g) = gloss {
                            target_note = g
                                .replace(['\t', '\n', '\r'], " ")
                                .chars()
                                .take(90)
                                .collect();
                        }
                    }
                }
            }
        }

        let final_reg = if has_std || reg_tag.is_empty() {
            "standard".to_string()
        } else {
            reg_tag
        };
        let final_note = if final_reg == "standard" {
            String::new()
        } else {
            target_note
        };

        let replace_entry = match entries.get(&lemma) {
            None => true,
            Some(prev) => (prev.3 != "standard" && final_reg == "standard") || (prev.2.is_empty() && !ipa.is_empty()),
        };
        if replace_entry {
            entries.insert(
                lemma.clone(),
                (
                    lemma.clone(),
                    pos.clone(),
                    ipa.clone(),
                    final_reg.clone(),
                    final_note.clone(),
                ),
            );
        }

        if let Some(forms) = obj.get("forms").and_then(|v| v.as_array()) {
            for frm in forms {
                let mut ftags = Vec::new();
                if let Some(arr) = frm.get("tags").and_then(|v| v.as_array()) {
                    for t in arr {
                        if let Some(ts) = t.as_str() {
                            ftags.push(ts.to_lowercase());
                        }
                    }
                }
                if skip_tags.iter().any(|sk| ftags.iter().any(|t| t == sk)) {
                    continue;
                }
                let raw_f = frm.get("form").and_then(|v| v.as_str()).unwrap_or("").trim();
                let clean_f = strip_stress_marks(raw_f).to_lowercase();
                if clean_f.chars().count() < 2 || !is_all_alphabetic_non_ascii(&clean_f) {
                    continue;
                }
                let gram_pos = if ftags.iter().any(|t| t == "participle")
                    && (ftags.iter().any(|t| t == "aorist") || ftags.iter().any(|t| t == "imperfect"))
                    && ftags.iter().any(|t| t == "active")
                    && ftags.iter().any(|t| t == "indefinite")
                {
                    format!("{pos}:renarrated")
                } else if ftags.iter().any(|t| t == "definite") {
                    format!("{pos}:def")
                } else if ftags.iter().any(|t| t == "vocative") {
                    format!("{pos}:voc")
                } else {
                    pos.clone()
                };

                entries.entry(clean_f).or_insert_with(|| {
                    (
                        lemma.clone(),
                        gram_pos,
                        String::new(),
                        final_reg.clone(),
                        String::new(),
                    )
                });
            }
        }
    }

    if let Some(parent) = out_tsv.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut keys: Vec<&String> = entries.keys().collect();
    keys.sort();
    let mut out_file = std::io::BufWriter::new(std::fs::File::create(out_tsv)?);
    for k in &keys {
        let (lem, pos, ipa, reg, note) = &entries[*k];
        writeln!(out_file, "{k}\t{lem}\t{pos}\t{ipa}\t{reg}\t{note}")?;
    }
    out_file.flush()?;
    Ok(keys.len())
}

fn push_possibly_multiword(out: &mut Vec<String>, lead: &str, phrase: &str, trail: &str) {
    let parts: Vec<&str> = phrase.split_whitespace().collect();
    if parts.is_empty() {
        out.push(format!("{lead}{trail}"));
    } else if parts.len() == 1 {
        out.push(format!("{lead}{}{trail}", parts[0]));
    } else {
        out.push(format!("{lead}{}", parts[0]));
        for mid in &parts[1..parts.len() - 1] {
            out.push((*mid).to_string());
        }
        out.push(format!("{}{trail}", parts[parts.len() - 1]));
    }
}

fn strip_stress_marks(s: &str) -> String {
    s.chars()
        .filter(|&c| c != '\u{0301}' && c != '\u{0300}')
        .collect()
}

fn is_all_alphabetic_non_ascii(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphabetic() && !c.is_ascii())
}

fn split_punct(tok: &str) -> (String, String, String) {
    let chars: Vec<char> = tok.chars().collect();
    let mut start = 0;
    while start < chars.len() && !chars[start].is_alphanumeric() {
        start += 1;
    }
    if start == chars.len() {
        return (tok.to_string(), String::new(), String::new());
    }
    let mut end = chars.len();
    while end > start && !chars[end - 1].is_alphanumeric() {
        end -= 1;
    }
    let lead: String = chars[..start].iter().collect();
    let core: String = chars[start..end].iter().collect();
    let trail: String = chars[end..].iter().collect();
    (lead, core, trail)
}

fn match_case(source: &str, replacement: &str) -> String {
    if replacement.chars().any(|c| c.is_uppercase()) {
        let all_src_upper = source.chars().all(|c| !c.is_alphabetic() || c.is_uppercase())
            && source.chars().any(|c| c.is_uppercase());
        if all_src_upper {
            return replacement.to_uppercase();
        }
        return replacement.to_string();
    }
    let all_src_upper = source.chars().all(|c| !c.is_alphabetic() || c.is_uppercase())
        && source.chars().any(|c| c.is_uppercase())
        && source.chars().filter(|c| c.is_alphabetic()).count() > 1;
    if all_src_upper {
        replacement.to_uppercase()
    } else if source.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        capitalize_first(replacement)
    } else {
        replacement.to_string()
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out: String = first.to_uppercase().collect();
    out.extend(chars);
    out
}

fn dedup_adjacent_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev: Option<char> = None;
    for ch in s.chars() {
        if prev != Some(ch) {
            out.push(ch);
            prev = Some(ch);
        }
    }
    out
}

fn is_single_insertion_or_deletion(a: &str, b: &str) -> bool {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let (shorter, longer) = if ca.len() + 1 == cb.len() {
        (&ca, &cb)
    } else if cb.len() + 1 == ca.len() {
        (&cb, &ca)
    } else {
        return false;
    };
    let mut i = 0;
    let mut j = 0;
    let mut skipped = false;
    while i < shorter.len() && j < longer.len() {
        if shorter[i] == longer[j] {
            i += 1;
            j += 1;
        } else if !skipped {
            skipped = true;
            j += 1;
        } else {
            return false;
        }
    }
    true
}

fn char_edit_distance(a: &str, b: &str) -> usize {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let n = ca.len();
    let m = cb.len();
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = usize::from(ca[i - 1] != cb[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}
