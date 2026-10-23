use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use anyhow::Result;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use leadfinder_core::RuntimePaths;
use crate::lexicon::compile_wiktionary_jsonl_to_tsv;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub yt_dlp: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub whisper_cli: Option<PathBuf>,
    pub tesseract: Option<PathBuf>,
    pub tessdata_bul: bool,
    pub tessdata_eng: bool,
    pub llama_server: Option<PathBuf>,
    pub whisper_models: Vec<String>,
    pub llm_models: Vec<String>,
    pub lexicon_files: Vec<String>,
    pub webview2_installed: bool,
}

pub fn inspect_dependencies(paths: &RuntimePaths) -> DependencyStatus {
    let td = paths.tools_dir.join("tessdata");
    let bundled_td = paths.bundled_tools_dir.join("tessdata");
    let pf_td = PathBuf::from(r"C:\Program Files\Tesseract-OCR\tessdata");

    let has_lang = |fname: &str| {
        td.join(fname).is_file() || bundled_td.join(fname).is_file() || pf_td.join(fname).is_file()
    };

    let list_files = |dir: PathBuf, ext: &str| -> Vec<String> {
        std::fs::read_dir(dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some(ext) {
                    p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect()
    };

    let list_any = |dir1: PathBuf, dir2: PathBuf| -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for dir in [dir1, dir2] {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_file() {
                        if let Some(s) = p.file_name().and_then(|s| s.to_str()) {
                            if !names.iter().any(|x| x == s) {
                                names.push(s.to_string());
                            }
                        }
                    }
                }
            }
        }
        names.sort();
        names
    };

    DependencyStatus {
        yt_dlp: paths.resolve_binary("yt-dlp"),
        ffmpeg: paths.resolve_binary("ffmpeg"),
        ffprobe: paths.resolve_binary("ffprobe"),
        whisper_cli: paths
            .resolve_binary("whisper-cli")
            .or_else(|| paths.resolve_binary("main")),
        tesseract: paths.resolve_binary("tesseract"),
        tessdata_bul: has_lang("bul.traineddata"),
        tessdata_eng: has_lang("eng.traineddata"),
        llama_server: paths.resolve_binary("llama-server"),
        whisper_models: list_files(paths.models_dir.join("whisper"), "bin"),
        llm_models: list_files(paths.models_dir.join("llm"), "gguf"),
        lexicon_files: list_any(
            paths.models_dir.join("lexicon"),
            paths.static_root.join("models").join("lexicon"),
        ),
        webview2_installed: detect_webview2(),
    }
}

pub fn detect_webview2() -> bool {
    let candidates = [
        r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application",
        r"C:\Program Files\Microsoft\EdgeWebView\Application",
    ];
    candidates.iter().any(|p| Path::new(p).is_dir())
}

pub async fn install_missing_dependencies(
    paths: &RuntimePaths,
    ensure_webview2: bool,
    with_models: bool,
) -> Result<DependencyStatus> {
    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .user_agent("leadfinder-bootstrap/1.0")
        .build()?;

    std::fs::create_dir_all(&paths.tools_dir)?;
    std::fs::create_dir_all(paths.tools_dir.join("tessdata"))?;
    std::fs::create_dir_all(paths.models_dir.join("whisper"))?;
    std::fs::create_dir_all(paths.models_dir.join("llm"))?;
    std::fs::create_dir_all(paths.models_dir.join("lexicon"))?;

    if ensure_webview2 && !detect_webview2() {
        let wv_installer = paths.tools_dir.join("MicrosoftEdgeWebview2Setup.exe");
        download_file(
            &client,
            "https://go.microsoft.com/fwlink/p/?LinkId=2124703",
            &wv_installer,
        )
        .await?;
        let _ = Command::new(&wv_installer)
            .args(["/silent", "/install"])
            .status()
            .await;
    }

    if paths.resolve_binary("yt-dlp").is_none() {
        download_file(
            &client,
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
            &paths.tools_dir.join("yt-dlp.exe"),
        )
        .await?;
    }

    if paths.resolve_binary("ffmpeg").is_none() {
        let _ = Command::new("winget")
            .args([
                "install",
                "--id",
                "Gyan.FFmpeg.Essentials",
                "-e",
                "--accept-source-agreements",
                "--accept-package-agreements",
                "--silent",
            ])
            .stdin(Stdio::null())
            .status()
            .await;
    }

    if paths.resolve_binary("tesseract").is_none() {
        let _ = Command::new("winget")
            .args([
                "install",
                "--id",
                "UB-Mannheim.TesseractOCR",
                "-e",
                "--accept-source-agreements",
                "--accept-package-agreements",
                "--silent",
            ])
            .stdin(Stdio::null())
            .status()
            .await;
    }

    let td = paths.tools_dir.join("tessdata");
    if !td.join("bul.traineddata").is_file() {
        download_file(
            &client,
            "https://github.com/tesseract-ocr/tessdata_fast/raw/main/bul.traineddata",
            &td.join("bul.traineddata"),
        )
        .await?;
    }
    if !td.join("eng.traineddata").is_file() {
        download_file(
            &client,
            "https://github.com/tesseract-ocr/tessdata_fast/raw/main/eng.traineddata",
            &td.join("eng.traineddata"),
        )
        .await?;
    }

    if paths.resolve_binary("whisper-cli").is_none() {
        let zip_path = paths.tools_dir.join("whisper-bin-x64.zip");
        download_file(
            &client,
            "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip",
            &zip_path,
        )
        .await?;
        extract_zip_flat(&zip_path, &paths.tools_dir).await?;
    }

    if paths.resolve_binary("llama-server").is_none() {
        let zip_path = paths.tools_dir.join("llama-win-vulkan-x64.zip");
        download_file(
            &client,
            "https://github.com/ggml-org/llama.cpp/releases/download/b11836/llama-b11836-bin-win-vulkan-x64.zip",
            &zip_path,
        )
        .await?;
        extract_zip_flat(&zip_path, &paths.tools_dir).await?;
    }

    let lex_dir = paths.models_dir.join("lexicon");
    let lex_downloads = [
        (
            "lexicon/bg_50k.txt",
            "bg_50k.txt",
            "https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/bg/bg_50k.txt",
        ),
        (
            "lexicon/bg_BG.dic",
            "bg_BG.dic",
            "https://raw.githubusercontent.com/LibreOffice/dictionaries/master/bg_BG/bg_BG.dic",
        ),
        (
            "lexicon/bg_BG.aff",
            "bg_BG.aff",
            "https://raw.githubusercontent.com/LibreOffice/dictionaries/master/bg_BG/bg_BG.aff",
        ),
        (
            "lexicon/bg_mfa_ipa.dict",
            "bg_mfa_ipa.dict",
            "https://github.com/MontrealCorpusTools/mfa-models/releases/download/dictionary-bulgarian_mfa-v2.0.0a/bulgarian_mfa.dict",
        ),
    ];
    for (rel, fname, url) in lex_downloads {
        if paths.resolve_model_path(rel).is_none() {
            download_file(&client, url, &lex_dir.join(fname)).await?;
        }
    }

    if with_models {
        let small_bin = paths.models_dir.join("whisper").join("ggml-small.bin");
        if paths.resolve_model_path("whisper/ggml-small.bin").is_none() {
            download_file(
                &client,
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
                &small_bin,
            )
            .await?;
        }
        if paths
            .resolve_model_path("lexicon/bg_wiktionary_lexicon.tsv")
            .is_none()
        {
            let jsonl_cache = paths.cache_dir.join("kaikki_bg.jsonl");
            if !jsonl_cache.is_file() {
                download_file(
                    &client,
                    "https://kaikki.org/dictionary/Bulgarian/kaikki.org-dictionary-Bulgarian.jsonl",
                    &jsonl_cache,
                )
                .await?;
            }
            let out_tsv = lex_dir.join("bg_wiktionary_lexicon.tsv");
            let jsonl_clone = jsonl_cache.clone();
            tokio::task::spawn_blocking(move || compile_wiktionary_jsonl_to_tsv(&jsonl_clone, &out_tsv))
                .await??;
        }
    }

    Ok(inspect_dependencies(paths))
}

pub async fn download_file(client: &Client, url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("part");
    let resp = client.get(url).send().await?.error_for_status()?;
    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp).await?;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        file.write_all(&bytes).await?;
    }
    file.flush().await?;
    drop(file);
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

async fn extract_zip_flat(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    let sub = dest_dir.join("_extract_tmp");
    let _ = std::fs::remove_dir_all(&sub);
    std::fs::create_dir_all(&sub)?;

    let status = Command::new("tar")
        .args([
            "-xf",
            &zip_path.to_string_lossy(),
            "-C",
            &sub.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .status()
        .await?;

    if status.success() {
        copy_files_recursive_flat(&sub, dest_dir)?;
    }
    let _ = std::fs::remove_dir_all(&sub);
    let _ = std::fs::remove_file(zip_path);
    Ok(())
}

fn copy_files_recursive_flat(src: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            copy_files_recursive_flat(&p, dest)?;
        } else if let Some(fname) = p.file_name() {
            std::fs::copy(&p, dest.join(fname))?;
        }
    }
    Ok(())
}
