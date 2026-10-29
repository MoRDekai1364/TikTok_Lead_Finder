use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use tokio::process::Command;
use leadfinder_core::{
    ProviderError, ProviderErrorKind, RuntimePaths, TranscribeInput, Transcriber,
    TranscriptRecord, TranscriptSegment,
};
use crate::lexicon::{global_lexicon, LexiconNormalizer};

#[derive(Clone)]
pub struct CaptionFirstTranscriber {
    whisper: WhisperCppTranscriber,
}

impl CaptionFirstTranscriber {
    pub fn new(whisper: WhisperCppTranscriber) -> Self {
        Self { whisper }
    }
}

#[async_trait]
impl Transcriber for CaptionFirstTranscriber {
    fn name(&self) -> &str {
        "caption_then_whisper"
    }

    async fn transcribe(&self, input: &TranscribeInput) -> Result<TranscriptRecord, ProviderError> {
        let chosen_track = select_original_caption_track(&input.caption_tracks);
        if let Some((lang_raw, path)) = chosen_track {
            if let Ok(raw_vtt) = std::fs::read_to_string(path) {
                let segments = parse_vtt_or_srt(&raw_vtt);
                let full_text = merge_segment_texts(&segments);
                if full_text.chars().count() >= 8 {
                    let norm_lang = normalize_lang_code(lang_raw, &full_text);
                    let duration_ms = segments.last().map(|s| s.end_ms).unwrap_or(10_000).max(1000);
                    let quality = compute_transcript_quality(
                        0.88,
                        &segments,
                        duration_ms as f64 / 1000.0,
                        &norm_lang,
                        &input.expected_languages,
                        &full_text,
                    );
                    let mut rec = TranscriptRecord {
                        source: "found".to_string(),
                        engine: "tiktok_captions".to_string(),
                        model: "native_vtt".to_string(),
                        language: norm_lang,
                        quality,
                        text: full_text,
                        segments,
                    };
                    if let Some(ref lex) = self.whisper.lexicon {
                        lex.normalize_transcript(&mut rec);
                    }
                    return Ok(rec);
                }
            }
        }

        if let Some(ref wav_path) = input.audio_wav_path {
            if wav_path.is_file() {
                let detected_lang = self.whisper.detect_language(wav_path).await;
                let is_non_bg = detected_lang
                    .as_deref()
                    .map(|l| l != "bg")
                    .unwrap_or(false);

                if is_non_bg {
                    return self.whisper.transcribe(input).await;
                }

                let py_script = [
                    PathBuf::from("scripts/google_audio_transcribe.py"),
                    PathBuf::from("tools/google_audio_transcribe.py"),
                ]
                .into_iter()
                .find(|p| p.is_file());

                if let Some(script) = py_script {
                    let whisper_res = self.whisper.transcribe_with_json_file(input).await.ok();
                    let (whisper_rec, whisper_json_path) = match whisper_res {
                        Some((rec, path)) => (Some(rec), Some(path)),
                        None => (None, None),
                    };

                    let mut cmd = Command::new("python");
                    cmd.env("PYTHONIOENCODING", "utf-8");
                    if let Some(key) = leadfinder_core::resolve_secret("GEMINI_API_KEY") {
                        cmd.env("GEMINI_API_KEY", key);
                    }
                    cmd.arg(&script).arg("--audio").arg(wav_path);
                    if let Some(ref jpath) = whisper_json_path {
                        cmd.arg("--whisper-json").arg(jpath);
                    }
                    cmd.stdout(Stdio::piped());
                    cmd.stderr(Stdio::piped());
                    #[cfg(windows)]
                    {
                        cmd.creation_flags(0x08000000);
                    }
                    let py_output = tokio::time::timeout(Duration::from_secs(45), cmd.output()).await;

                    if let Some(ref jpath) = whisper_json_path {
                        let _ = std::fs::remove_file(jpath);
                    }

                    if let Ok(Ok(output)) = py_output {
                        if output.status.success() {
                            if let Ok(val) = serde_json::from_slice::<Value>(&output.stdout) {
                                let corrected = val
                                    .get("corrected_text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .trim();
                                if corrected.chars().count() >= 5 {
                                    let parsed_segs = val
                                        .get("segments")
                                        .and_then(|v| v.as_array())
                                        .and_then(|arr| {
                                            serde_json::from_value::<Vec<TranscriptSegment>>(
                                                Value::Array(arr.clone()),
                                            )
                                            .ok()
                                        })
                                        .unwrap_or_default();
                                    let src_model = val
                                        .get("source")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("google_speech")
                                        .to_string();
                                    let mut rec = TranscriptRecord {
                                        source: "generated".to_string(),
                                        engine: "google_ai_speech".to_string(),
                                        model: src_model,
                                        language: "bg".to_string(),
                                        quality: 0.95,
                                        text: corrected.to_string(),
                                        segments: parsed_segs,
                                    };
                                    if let Some(ref lex) = self.whisper.lexicon {
                                        lex.normalize_transcript(&mut rec);
                                    }
                                    return Ok(rec);
                                }
                            }
                        }
                    }

                    if let Some(w_rec) = whisper_rec {
                        return Ok(w_rec);
                    }
                }
            }
        }

        self.whisper.transcribe(input).await
    }
}

fn select_original_caption_track(tracks: &[(String, PathBuf)]) -> Option<&(String, PathBuf)> {
    for item in tracks {
        let low = item.0.to_lowercase();
        if low.starts_with("bg") || low == "orig" || low.contains("bul") {
            return Some(item);
        }
    }
    for item in tracks {
        let low = item.0.to_lowercase();
        if (low.starts_with("ru") || low.starts_with("tr")) && !low.contains("translat") {
            return Some(item);
        }
    }
    if tracks.len() == 1 {
        let low = tracks[0].0.to_lowercase();
        if low.starts_with("en") {
            if let Ok(content) = std::fs::read_to_string(&tracks[0].1) {
                if content.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
                    return Some(&tracks[0]);
                }
            }
            return None;
        }
        return Some(&tracks[0]);
    }
    None
}

#[derive(Clone)]
pub struct WhisperCppTranscriber {
    binary_path: Option<PathBuf>,
    models_dir: PathBuf,
    bundled_models_dir: PathBuf,
    model_name: String,
    timeout: Duration,
    force_cpu: bool,
    pub lexicon: Option<Arc<LexiconNormalizer>>,
}

impl WhisperCppTranscriber {
    pub fn new(paths: &RuntimePaths, model_name: impl Into<String>, timeout: Duration) -> Self {
        let bin = paths
            .resolve_binary("whisper-cli")
            .or_else(|| paths.resolve_binary("main"));
        let lex_file = leadfinder_core::load_app_config(&paths.config_dir)
            .map(|c| c.normalisation.lexicon_config_file)
            .unwrap_or_else(|_| "lexicon.bg.toml".to_string());
        let lex = global_lexicon(paths, &lex_file);
        Self {
            binary_path: bin,
            models_dir: paths.models_dir.join("whisper"),
            bundled_models_dir: paths.static_root.join("models").join("whisper"),
            model_name: model_name.into(),
            timeout,
            force_cpu: false,
            lexicon: Some(lex),
        }
    }

    pub fn with_force_cpu(mut self, force_cpu: bool) -> Self {
        self.force_cpu = force_cpu;
        self
    }

    pub fn with_model(mut self, model_name: impl Into<String>) -> Self {
        self.model_name = model_name.into();
        self
    }

    pub fn with_lexicon(mut self, lexicon: Option<Arc<LexiconNormalizer>>) -> Self {
        self.lexicon = lexicon;
        self
    }

    pub fn resolve_model_file(&self) -> Option<PathBuf> {
        let candidates = [
            self.models_dir.join(format!("ggml-{}.bin", self.model_name)),
            self.models_dir.join(&self.model_name),
            self.bundled_models_dir.join(format!("ggml-{}.bin", self.model_name)),
            self.bundled_models_dir.join(&self.model_name),
            self.models_dir.join("ggml-large-v3-turbo-q5_0.bin"),
            self.models_dir.join("ggml-large-v3-turbo.bin"),
            self.models_dir.join("ggml-medium.bin"),
            self.models_dir.join("ggml-small.bin"),
            self.bundled_models_dir.join("ggml-small.bin"),
            self.models_dir.join("ggml-base.bin"),
            self.models_dir.join("ggml-tiny.bin"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }

    pub fn is_ready(&self) -> bool {
        self.binary_path.is_some() && self.resolve_model_file().is_some()
    }

    pub async fn detect_language(&self, audio_wav_path: &Path) -> Option<String> {
        let bin = self.binary_path.as_deref()?;
        let model_file = [
            self.models_dir.join("ggml-tiny.bin"),
            self.bundled_models_dir.join("ggml-tiny.bin"),
            self.models_dir.join("ggml-base.bin"),
            self.bundled_models_dir.join("ggml-base.bin"),
        ]
        .into_iter()
        .find(|p| p.is_file())
        .or_else(|| self.resolve_model_file())?;

        let mut cmd = Command::new(bin);
        cmd.args([
            "-m",
            &model_file.to_string_lossy(),
            "-f",
            &audio_wav_path.to_string_lossy(),
            "-dl",
        ]);
        if self.force_cpu {
            cmd.arg("-ng");
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let output = tokio::time::timeout(Duration::from_secs(8), cmd.output()).await.ok()?.ok()?;
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        for line in combined.lines() {
            if let Some(pos) = line.find("auto-detected language:") {
                let rest = &line[pos + "auto-detected language:".len()..];
                if let Some(token) = rest.split_whitespace().next() {
                    let clean = token.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
                    if !clean.is_empty() {
                        return Some(clean);
                    }
                }
            }
        }
        None
    }

    pub async fn transcribe_with_json_file(
        &self,
        input: &TranscribeInput,
    ) -> Result<(TranscriptRecord, PathBuf), ProviderError> {
        let audio = input.audio_wav_path.as_deref().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::InvalidOutput,
                "whisper_cpp",
                "audio_wav_path is missing for Whisper transcription",
                false,
            )
        })?;
        let bin = self.binary_path.as_deref().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::BinaryMissing,
                "whisper_cpp",
                "whisper-cli.exe not found in tools/ or PATH",
                false,
            )
        })?;
        let model_file = self.resolve_model_file().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::BinaryMissing,
                "whisper_cpp",
                format!("Whisper model file not found in {}", self.models_dir.display()),
                false,
            )
        })?;

        let out_prefix = std::env::temp_dir().join(format!(
            "leadfinder_whisper_{}_{}",
            input.video_id,
            uuid::Uuid::new_v4()
        ));
        let out_json_path = PathBuf::from(format!("{}.json", out_prefix.display()));

        let primary_lang = input
            .expected_languages
            .first()
            .map(|s| s.as_str())
            .unwrap_or("bg");

        let threads = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(4, 16))
            .unwrap_or(8)
            .to_string();

        let mut cmd = Command::new(bin);
        cmd.args([
            "-t",
            &threads,
            "-m",
            &model_file.to_string_lossy(),
            "-f",
            &audio.to_string_lossy(),
            "-l",
            primary_lang,
            "-sns",
            "-ojf",
            "-of",
            &out_prefix.to_string_lossy(),
        ]);
        if let Some(prompt) = self
            .lexicon
            .as_ref()
            .and_then(|l| l.whisper_prompt(&input.expected_languages))
        {
            cmd.args(["--prompt", prompt]);
        }
        if self.force_cpu {
            cmd.arg("-ng");
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let output = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| {
                ProviderError::new(
                    ProviderErrorKind::Timeout,
                    "whisper_cpp",
                    "whisper-cli timed out",
                    true,
                )
            })?
            .map_err(|e| {
                ProviderError::new(ProviderErrorKind::Internal, "whisper_cpp", e.to_string(), false)
            })?;

        if !output.status.success() && !out_json_path.is_file() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(ProviderError::new(
                ProviderErrorKind::Internal,
                "whisper_cpp",
                format!("whisper-cli failed: {stderr}"),
                false,
            ));
        }

        let raw_json = std::fs::read_to_string(&out_json_path).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::ParseError,
                "whisper_cpp",
                format!("missing whisper JSON output: {e}"),
                false,
            )
        })?;

        let parsed: Value = serde_json::from_str(&raw_json).map_err(|e| {
            ProviderError::new(ProviderErrorKind::ParseError, "whisper_cpp", e.to_string(), false)
        })?;

        let detected_lang = parsed
            .get("result")
            .and_then(|r| r.get("language"))
            .and_then(|v| v.as_str())
            .unwrap_or(primary_lang)
            .to_string();

        let mut segments = Vec::new();
        let mut token_probs = Vec::new();

        if let Some(arr) = parsed.get("transcription").and_then(|v| v.as_array()) {
            for item in arr {
                let start_ms = item
                    .get("offsets")
                    .and_then(|o| o.get("from"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let end_ms = item
                    .get("offsets")
                    .and_then(|o| o.get("to"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(start_ms + 500);
                let txt = item
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();

                let mut seg_probs = Vec::new();
                if let Some(toks) = item.get("tokens").and_then(|v| v.as_array()) {
                    for tok in toks {
                        let tok_txt = tok.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        if !tok_txt.starts_with('[') {
                            if let Some(p) = tok.get("p").and_then(|v| v.as_f64()) {
                                seg_probs.push(p);
                                token_probs.push(p);
                            }
                        }
                    }
                }
                let seg_conf = if seg_probs.is_empty() {
                    None
                } else {
                    Some(seg_probs.iter().sum::<f64>() / seg_probs.len() as f64)
                };

                if !txt.is_empty() && !is_whisper_non_speech_marker(&txt) {
                    segments.push(TranscriptSegment {
                        start_ms,
                        end_ms,
                        text: txt,
                        confidence: seg_conf,
                    });
                }
            }
        }

        let full_text = merge_segment_texts(&segments);
        let mean_conf = if token_probs.is_empty() {
            if full_text.is_empty() {
                0.15
            } else {
                0.65
            }
        } else {
            token_probs.iter().sum::<f64>() / token_probs.len() as f64
        };

        let audio_duration_s = estimate_wav_duration_seconds(audio).unwrap_or_else(|| {
            segments
                .last()
                .map(|s| s.end_ms as f64 / 1000.0)
                .unwrap_or(10.0)
                .max(1.0)
        });

        let norm_lang = normalize_lang_code(&detected_lang, &full_text);
        let quality = compute_transcript_quality(
            mean_conf,
            &segments,
            audio_duration_s,
            &norm_lang,
            &input.expected_languages,
            &full_text,
        );

        let actual_model_stem = model_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&self.model_name)
            .trim_start_matches("ggml-")
            .to_string();

        let mut rec = TranscriptRecord {
            source: "generated".to_string(),
            engine: "whisper_cpp".to_string(),
            model: actual_model_stem,
            language: norm_lang,
            quality,
            text: full_text,
            segments,
        };
        if let Some(ref lex) = self.lexicon {
            lex.normalize_transcript(&mut rec);
        }
        Ok((rec, out_json_path))
    }
}

#[async_trait]
impl Transcriber for WhisperCppTranscriber {
    fn name(&self) -> &str {
        "whisper_cpp"
    }

    async fn transcribe(&self, input: &TranscribeInput) -> Result<TranscriptRecord, ProviderError> {
        let (rec, json_path) = self.transcribe_with_json_file(input).await?;
        let _ = std::fs::remove_file(&json_path);
        Ok(rec)
    }
}

fn is_whisper_non_speech_marker(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')'))
}

pub fn estimate_wav_duration_seconds(wav_path: &Path) -> Option<f64> {
    let bytes = std::fs::metadata(wav_path).ok()?.len();
    if bytes <= 44 {
        return None;
    }
    Some((bytes - 44) as f64 / 32000.0)
}

pub fn compute_transcript_quality(
    mean_token_confidence: f64,
    segments: &[TranscriptSegment],
    duration_s: f64,
    detected_language: &str,
    expected_languages: &[String],
    full_text: &str,
) -> f64 {
    if full_text.trim().is_empty() {
        return 0.05;
    }

    let conf_score = mean_token_confidence.clamp(0.0, 1.0);

    let covered_ms: i64 = segments
        .iter()
        .map(|s| (s.end_ms - s.start_ms).max(0))
        .sum();
    let total_ms = (duration_s.max(1.0) * 1000.0) as i64;
    let raw_ratio = (covered_ms as f64 / total_ms as f64).clamp(0.0, 1.0);
    let speech_ratio_score = (raw_ratio / 0.35).clamp(0.1, 1.0);

    let lang_low = detected_language.to_lowercase();
    let lang_score = if expected_languages
        .first()
        .map(|p| p.eq_ignore_ascii_case(&lang_low))
        .unwrap_or(false)
    {
        1.0
    } else if expected_languages.iter().any(|l| l.eq_ignore_ascii_case(&lang_low)) {
        0.85
    } else {
        0.30
    };

    let char_count = full_text.chars().count() as f64;
    let cps = char_count / duration_s.max(1.0);
    let unique_words: HashSet<String> = full_text
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();
    let total_words = full_text.split_whitespace().count().max(1);
    let diversity = (unique_words.len() as f64 / total_words as f64).clamp(0.0, 1.0);
    let cps_sanity = if cps < 1.0 {
        0.25
    } else if cps > 35.0 {
        0.35
    } else {
        1.0
    };
    let length_sanity = (0.6 * cps_sanity + 0.4 * (diversity / 0.35).clamp(0.2, 1.0)).clamp(0.0, 1.0);

    let combined = 0.40 * conf_score + 0.20 * speech_ratio_score + 0.20 * lang_score + 0.20 * length_sanity;
    (combined * 1000.0).round() / 1000.0
}

fn normalize_lang_code(raw: &str, text: &str) -> String {
    let low = raw.trim().to_lowercase();
    if low.starts_with("bg") || low.starts_with("bul") {
        return "bg".to_string();
    }
    if low.starts_with("ru") {
        return "ru".to_string();
    }
    if low.starts_with("tr") {
        return "tr".to_string();
    }
    if low.starts_with("en") {
        if text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
            return "bg".to_string();
        }
        return "en".to_string();
    }
    if text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
        "bg".to_string()
    } else {
        low.split('-').next().unwrap_or("bg").to_string()
    }
}

pub fn parse_vtt_or_srt(content: &str) -> Vec<TranscriptSegment> {
    let cue_re = Regex::new(
        r"(?:(\d{1,2}):)?(\d{2}):(\d{2})[.,](\d{3})\s*-->\s*(?:(\d{1,2}):)?(\d{2}):(\d{2})[.,](\d{3})",
    )
    .unwrap();
    let tag_re = Regex::new(r"<[^>]+>").unwrap();

    let mut segments: Vec<TranscriptSegment> = Vec::new();
    let mut current_times: Option<(i64, i64)> = None;
    let mut current_lines: Vec<String> = Vec::new();

    let flush = |times: Option<(i64, i64)>, lines: &mut Vec<String>, out: &mut Vec<TranscriptSegment>| {
        if let Some((start_ms, end_ms)) = times {
            let joined = lines.join(" ").trim().to_string();
            let clean = tag_re.replace_all(&joined, "").trim().to_string();
            if !clean.is_empty() {
                if let Some(prev) = out.last_mut() {
                    if prev.text == clean {
                        prev.end_ms = prev.end_ms.max(end_ms);
                        lines.clear();
                        return;
                    }
                }
                out.push(TranscriptSegment {
                    start_ms,
                    end_ms,
                    text: clean,
                    confidence: Some(0.90),
                });
            }
        }
        lines.clear();
    };

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            flush(current_times.take(), &mut current_lines, &mut segments);
            continue;
        }
        if line.starts_with("WEBVTT") || line.starts_with("Kind:") || line.starts_with("Language:") || line.starts_with("NOTE") {
            continue;
        }
        if let Some(cap) = cue_re.captures(line) {
            flush(current_times.take(), &mut current_lines, &mut segments);
            let h1 = cap.get(1).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);
            let m1 = cap.get(2).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);
            let s1 = cap.get(3).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);
            let ms1 = cap.get(4).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);

            let h2 = cap.get(5).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);
            let m2 = cap.get(6).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);
            let s2 = cap.get(7).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);
            let ms2 = cap.get(8).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);

            let start_ms = ((h1 * 3600 + m1 * 60 + s1) * 1000) + ms1;
            let end_ms = ((h2 * 3600 + m2 * 60 + s2) * 1000) + ms2;
            current_times = Some((start_ms, end_ms));
            continue;
        }
        if current_times.is_some() {
            current_lines.push(line.to_string());
        }
    }
    flush(current_times.take(), &mut current_lines, &mut segments);
    segments
}

fn merge_segment_texts(segments: &[TranscriptSegment]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for seg in segments {
        let t = seg.text.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(last) = parts.last() {
            if last == t {
                continue;
            }
        }
        parts.push(t.to_string());
    }
    parts.join(" ")
}

pub fn compute_wer(reference: &str, hypothesis: &str) -> f64 {
    let tokenize = |s: &str| -> Vec<String> {
        s.chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c.to_lowercase().to_string()
                } else {
                    " ".to_string()
                }
            })
            .collect::<String>()
            .split_whitespace()
            .map(|w| w.to_string())
            .collect()
    };

    let ref_words = tokenize(reference);
    let hyp_words = tokenize(hypothesis);

    if ref_words.is_empty() {
        return if hyp_words.is_empty() { 0.0 } else { 1.0 };
    }

    let n = ref_words.len();
    let m = hyp_words.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n {
        dp[i][0] = i;
    }
    for j in 0..=m {
        dp[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if ref_words[i - 1] == hyp_words[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[n][m] as f64 / n as f64
}
