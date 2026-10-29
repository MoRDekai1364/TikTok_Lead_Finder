use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use leadfinder_core::{
    ExtractionConfig, FrameSample, Ocr, OcrLine, ProviderError, ProviderErrorKind, RuntimePaths,
};

#[derive(Clone)]
pub struct TesseractOcr {
    binary_path: Option<PathBuf>,
    tessdata_dir: Option<PathBuf>,
    languages: String,
    dedup_similarity: f64,
    timeout: Duration,
}

impl TesseractOcr {
    pub fn new(paths: &RuntimePaths, cfg: &ExtractionConfig) -> Self {
        let bin = paths.resolve_binary("tesseract");
        let td_candidates = [
            paths.tools_dir.join("tessdata"),
            paths.bundled_tools_dir.join("tessdata"),
            PathBuf::from(r"C:\Program Files\Tesseract-OCR\tessdata"),
        ];
        let tessdata_dir = td_candidates
            .into_iter()
            .find(|d| d.join("bul.traineddata").is_file() || d.join("eng.traineddata").is_file());

        let langs = if cfg.ocr_languages.is_empty() {
            "bul+eng".to_string()
        } else {
            cfg.ocr_languages.join("+")
        };

        Self {
            binary_path: bin,
            tessdata_dir,
            languages: langs,
            dedup_similarity: cfg.ocr_dedup_similarity,
            timeout: Duration::from_secs(cfg.ocr_timeout_seconds),
        }
    }

        async fn ocr_with_prep(&self, bin: &Path, frame: &FrameSample) -> Result<Vec<OcrLine>, ProviderError> {
        let mut lines = Vec::new();
        for band in prepare_frame_bands(frame) {
            let res = self.ocr_single_frame(bin, &band).await;
            let _ = std::fs::remove_file(&band.image_path);
            if let Ok(Some(line)) = res {
                lines.push(line);
            }
        }
        if lines.is_empty() {
            if let Ok(Some(line)) = self.ocr_single_frame(bin, frame).await {
                lines.push(line);
            }
        }
        Ok(lines)
    }

    pub fn is_available(&self) -> bool {
        self.binary_path.is_some()
    }

    async fn ocr_single_frame(&self, bin: &Path, frame: &FrameSample) -> Result<Option<OcrLine>, ProviderError> {
        let mut cmd = Command::new(bin);
        cmd.arg(&frame.image_path)
            .arg("stdout")
            .arg("-l")
            .arg(&self.languages)
            .arg("--psm")
            .arg("11");

        if let Some(td) = &self.tessdata_dir {
            cmd.arg("--tessdata-dir").arg(td);
        }
        cmd.args(["-c", "tessedit_create_tsv=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let out = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| {
                ProviderError::new(ProviderErrorKind::Timeout, "tesseract", "tesseract timeout", true)
            })?
            .map_err(|e| {
                ProviderError::new(ProviderErrorKind::Internal, "tesseract", e.to_string(), false)
            })?;

        if !out.status.success() {
            return Ok(None);
        }

        let tsv = String::from_utf8_lossy(&out.stdout);
        let mut words = Vec::new();
        let mut confs = Vec::new();

        for (idx, line) in tsv.lines().enumerate() {
            if idx == 0 {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() >= 12 {
                let conf = cols[10].parse::<f64>().unwrap_or(-1.0);
                let word = cols[11].trim();
                if conf >= 30.0 && !word.is_empty() && word.chars().any(|c| c.is_alphanumeric()) {
                    words.push(word.to_string());
                    confs.push(conf / 100.0);
                }
            }
        }

        if words.is_empty() {
            return Ok(None);
        }
        let joined = words.join(" ");
        if joined.chars().filter(|c| c.is_alphanumeric()).count() < 3 {
            return Ok(None);
        }
        let mean_conf = confs.iter().sum::<f64>() / confs.len() as f64;
        if mean_conf < 0.35 {
            return Ok(None);
        }

        Ok(Some(OcrLine {
            frame_ms: frame.frame_ms,
            text: joined,
            confidence: (mean_conf * 1000.0).round() / 1000.0,
        }))
    }
}

#[async_trait]
impl Ocr for TesseractOcr {
    fn name(&self) -> &str {
        "tesseract"
    }

    async fn extract_frames(&self, frames: &[FrameSample]) -> Result<Vec<OcrLine>, ProviderError> {
        let bin = self.binary_path.as_deref().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::BinaryMissing,
                "tesseract",
                "tesseract.exe not found",
                false,
            )
        })?;

        let mut results: Vec<Result<Vec<OcrLine>, ProviderError>> = Vec::new();
        for chunk in frames.chunks(4) {
            let futures = chunk.iter().map(|frame| self.ocr_with_prep(bin, frame));
            results.extend(futures::future::join_all(futures).await);
        }

        let mut raw_lines = Vec::new();
        for res in results {
            if let Ok(lines) = res {
                raw_lines.extend(lines);
            }
        }

        Ok(deduplicate_ocr_lines(raw_lines, self.dedup_similarity))
    }
}

#[derive(Clone)]
pub struct PaddleOnnxOcr {
    python_bin: Option<PathBuf>,
    dedup_similarity: f64,
    timeout: Duration,
}

impl PaddleOnnxOcr {
    pub fn new(paths: &RuntimePaths, cfg: &ExtractionConfig) -> Self {
        Self {
            python_bin: paths.resolve_binary("python"),
            dedup_similarity: cfg.ocr_dedup_similarity,
            timeout: Duration::from_secs(cfg.ocr_timeout_seconds),
        }
    }
}

#[async_trait]
impl Ocr for PaddleOnnxOcr {
    fn name(&self) -> &str {
        "paddle_onnx"
    }

    async fn extract_frames(&self, frames: &[FrameSample]) -> Result<Vec<OcrLine>, ProviderError> {
        let py = self.python_bin.as_deref().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::BinaryMissing,
                "paddle_onnx",
                "python not found for rapidocr_onnxruntime",
                false,
            )
        })?;
        if frames.is_empty() {
            return Ok(Vec::new());
        }

        let payload: Vec<Value> = frames
            .iter()
            .map(|f| {
                serde_json::json!({
                    "frame_ms": f.frame_ms,
                    "path": f.image_path.to_string_lossy(),
                })
            })
            .collect();
        let payload_json = serde_json::to_string(&payload).unwrap_or_else(|_| "[]".to_string());

        let script = "import sys, json\nfrom rapidocr_onnxruntime import RapidOCR\ndef make_engine():\n    try:\n        from huggingface_hub import hf_hub_download\n        repo = 'monkt/paddleocr-onnx'\n        return RapidOCR(det_model_path=hf_hub_download(repo, 'detection/v5/det.onnx'), rec_model_path=hf_hub_download(repo, 'languages/eslav/rec.onnx'), rec_keys_path=hf_hub_download(repo, 'languages/eslav/dict.txt'))\n    except Exception:\n        return RapidOCR()\nengine = make_engine()\nitems = json.loads(sys.stdin.read())\nout = []\nfor it in items:\n    res, _ = engine(it['path'])\n    if res:\n        txts = [r[1].strip() for r in res if float(r[2]) >= 0.45 and len(r[1].strip()) > 0]\n        confs = [float(r[2]) for r in res if float(r[2]) >= 0.45 and len(r[1].strip()) > 0]\n        if txts:\n            joined = ' '.join(txts)\n            if sum(c.isalnum() for c in joined) >= 3:\n                out.append({'frame_ms': it['frame_ms'], 'text': joined, 'confidence': round(sum(confs)/len(confs), 3)})\nprint(json.dumps(out, ensure_ascii=False))\n";

        let mut cmd = Command::new(py);
        cmd.args(["-c", script])
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd.spawn().map_err(|e| {
            ProviderError::new(ProviderErrorKind::Internal, "paddle_onnx", e.to_string(), false)
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(payload_json.as_bytes()).await;
        }

        let out = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| {
                ProviderError::new(ProviderErrorKind::Timeout, "paddle_onnx", "paddle_onnx timed out", true)
            })?
            .map_err(|e| {
                ProviderError::new(ProviderErrorKind::Internal, "paddle_onnx", e.to_string(), false)
            })?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(ProviderError::new(ProviderErrorKind::Internal, "paddle_onnx", err, false));
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let raw_lines: Vec<OcrLine> = serde_json::from_str(&stdout).map_err(|e| {
            ProviderError::new(ProviderErrorKind::ParseError, "paddle_onnx", e.to_string(), false)
        })?;

        Ok(deduplicate_ocr_lines(raw_lines, self.dedup_similarity))
    }
}

fn prepare_frame_bands(frame: &FrameSample) -> Vec<FrameSample> {
    use std::hash::{Hash, Hasher};
    let img = match image::open(&frame.image_path) {
        Ok(i) => i,
        Err(_) => return Vec::new(),
    };
    let gray = img.grayscale();
    let (w, h) = (gray.width(), gray.height());
    let scale = if w < 1400 { 2 } else { 1 };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    frame.image_path.hash(&mut hasher);
    let key = hasher.finish();
    let spans: [(f32, f32); 3] = [(0.0, 0.38), (0.31, 0.69), (0.62, 1.0)];
    let mut out = Vec::new();
    for (idx, &(a, b)) in spans.iter().enumerate() {
        let y0 = (h as f32 * a) as u32;
        let y1 = ((h as f32 * b) as u32).min(h);
        if y1 <= y0 + 8 {
            continue;
        }
        let band_h = y1 - y0;
        let crop = gray.crop_imm(0, y0, w, band_h);
        let resized = if scale > 1 {
            crop.resize_exact(w * scale, band_h * scale, image::imageops::FilterType::Lanczos3)
        } else {
            crop
        };
        let mut luma = resized.to_luma8();
        for p in luma.pixels_mut() {
            let v = p.0[0] as f32;
            p.0[0] = ((v - 128.0) * 1.4 + 128.0).clamp(0.0, 255.0) as u8;
        }
        let path = std::env::temp_dir().join(format!("lf_ocr_{:x}_{}.png", key, idx));
        if luma.save(&path).is_ok() {
            out.push(FrameSample {
                frame_ms: frame.frame_ms,
                image_path: path,
            });
        }
    }
    out
}

#[derive(Clone)]
pub struct CombinedOcr {
    tesseract: TesseractOcr,
    paddle: PaddleOnnxOcr,
    dedup_similarity: f64,
}

impl CombinedOcr {
    pub fn new(paths: &RuntimePaths, cfg: &ExtractionConfig) -> Self {
        Self {
            tesseract: TesseractOcr::new(paths, cfg),
            paddle: PaddleOnnxOcr::new(paths, cfg),
            dedup_similarity: cfg.ocr_dedup_similarity,
        }
    }
}

#[async_trait]
impl Ocr for CombinedOcr {
    fn name(&self) -> &str {
        "combined"
    }

    async fn extract_frames(&self, frames: &[FrameSample]) -> Result<Vec<OcrLine>, ProviderError> {
        let (t, p) = tokio::join!(
            self.tesseract.extract_frames(frames),
            self.paddle.extract_frames(frames)
        );
        let mut merged: Vec<OcrLine> = Vec::new();
        let mut failures: Vec<ProviderError> = Vec::new();
        for res in [p, t] {
            match res {
                Ok(v) => merged.extend(v),
                Err(e) => failures.push(e),
            }
        }
        if merged.is_empty() && failures.len() == 2 {
            return Err(failures.remove(0));
        }
        merged.sort_by_key(|l| l.frame_ms);
        Ok(deduplicate_ocr_lines(merged, self.dedup_similarity))
    }
}

pub fn deduplicate_ocr_lines(lines: Vec<OcrLine>, similarity_threshold: f64) -> Vec<OcrLine> {  
    let mut kept: Vec<OcrLine> = Vec::new();
    let mut lines = lines;
    lines.sort_by_key(|l| l.frame_ms);
    for candidate in lines {
        let cand_norm = normalize_ocr_text(&candidate.text);
        if cand_norm.len() < 3 {
            continue;
        }
        let mut duplicate = false;
        for existing in &mut kept {
            let ex_norm = normalize_ocr_text(&existing.text);
            if dice_bigram_similarity(&cand_norm, &ex_norm) >= similarity_threshold {
                if candidate.confidence > existing.confidence && candidate.text.len() >= existing.text.len() {
                    existing.text = candidate.text.clone();
                    existing.confidence = candidate.confidence;
                }
                duplicate = true;
                break;
            }
        }
        if !duplicate {
            kept.push(candidate);
        }
    }
    kept
}

fn normalize_ocr_text(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn dice_bigram_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    if a_chars.len() < 2 || b_chars.len() < 2 {
        return 0.0;
    }
    let bigrams = |chars: &[char]| -> HashSet<(char, char)> {
        chars.windows(2).map(|w| (w[0], w[1])).collect()
    };
    let set_a = bigrams(&a_chars);
    let set_b = bigrams(&b_chars);
    let inter = set_a.intersection(&set_b).count();
    (2.0 * inter as f64) / (set_a.len() + set_b.len()) as f64
}
