# Implementation Progress

Last updated: 2026-10-23 · Current active phase: Phase 2 complete, ready for Phase 3.

## Phase Summary

| Phase | Title | Status |
| --- | --- | --- |
| Phase 0 | Owner decisions and scaffold | PASS |
| Phase 1 | Core skeleton, paste-a-link, discovery spike (go/no-go) | PASS |
| Phase 2 | Bulgarian single-video pipeline | PASS |
| Phase 3 | Calibration | PENDING |
| Phase 4 | Claims and people | PENDING |
| Phase 5 | Verification | PENDING |
| Phase 6 | Funnel, ranking and clustering | PENDING |
| Phase 7 | Scale hardening | PENDING |
| Phase 8 | Triage UI | PENDING |
| Phase 9 | People research, export, packaging | PENDING |

---

## Phase 0: Owner decisions and scaffold

- **Status**: COMPLETE (all acceptance criteria met)
- **What was built**:
  - Git repository initialized at workspace root with `.gitignore` and `.env.example` (zero secrets, zero comments).
  - Installed and verified `yt-dlp` (`2026.10.21`), `ffmpeg` (`9.0.1`), `ffprobe`, alongside existing `rustc` (`1.99.1`), `cargo` (`1.99.1`), `node` (`v26.8.1`), and `npm` (`11.19.0`).
  - Created `docs/PLAN.md`, `docs/OWNER_CHECKLIST.md`, `docs/DECISIONS.md`, `docs/SPIKE_DISCOVERY.md`, and `docs/PROGRESS.md`.
  - Created configuration files: `config/default.toml`, `config/primary_sources.toml`, and `config/seeds.bg.toml` (40 starter Bulgarian civic keywords and hashtags).
  - Created Cargo workspace (`crates/core`, `crates/providers`, `crates/probe`, `crates/pipeline`, `crates/cli`) and Tauri v2 + Svelte 5 + TypeScript UI scaffold (`ui/`, `ui/src-tauri/`), plus `assets/calibration/`, `prompts/`, and `fixtures/`.
  - Adapted Inno Setup installer script (`Setup_TikTok_Lead_Finder_Official_Release.iss`) and companion `installer/{LICENSE.txt,Hello.txt,Finish.txt}` files with built-in `setup-deps` post-install hooks and `%LOCALAPPDATA%\TikTok_Lead_Finder` runtime directories.
- **Decisions taken**:
  - `D-001`: Treated workspace root as the `leadfinder/` root to match relative paths in Sections 0–16, copying `TikTok Lead Finder Technical Implementation Plan.md` to `docs/PLAN.md`.
  - `D-002`: Explicitly deferred owner-only organizational sign-offs in `docs/OWNER_CHECKLIST.md` with local-only, privacy-safe defaults (`cloud_enabled = false`, flexible CSV/TSV/text partner importer, starter seed list).
  - `D-003`: Adapted owner's Inno Setup script into `Setup_TikTok_Lead_Finder_Official_Release.iss` alongside `leadfinder setup-deps` dependency bootstrapper support.
- **Open issues**:
  - Owner sign-offs listed in `docs/OWNER_CHECKLIST.md` remain deferred until production rollout.
- **Exact PowerShell verification commands and expected output**:
  ```powershell
  $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
  yt-dlp --version
  ffmpeg -version | Select-Object -First 1
  cargo check --workspace
  cargo run -p leadfinder-cli
  Push-Location ui
  npm run build
  Push-Location src-tauri
  cargo check
  Pop-Location
  Pop-Location
  ```
  Expected output:
  - `yt-dlp --version` prints `2026.10.21` (or newer).
  - `ffmpeg -version` prints `ffmpeg version 9.0.1...`.
  - `cargo check --workspace` exits `0` (`Finished 'dev' profile`).
  - `cargo run -p leadfinder-cli` prints `leadfinder-core` and exits `0`.
  - `npm run build` inside `ui` builds `dist/` cleanly and exits `0`.
  - `cargo check` inside `ui/src-tauri` exits `0`.

---

## Phase 1: Core skeleton, paste-a-link, discovery spike (go/no-go)

- **Status**: COMPLETE (all acceptance criteria met)
- **What was built**:
  - SQLite WAL database engine (`crates/core/src/db.rs`) and full Section 4 schema migration (`crates/core/migrations/0001_initial.sql`).
  - Durable `jobs` queue (`enqueue_job`, atomic `claim_next_job`, exponential backoff with jitter in `fail_or_retry_job`, crash recovery `recover_running_jobs`) and worker loop (`crates/pipeline/src/engine.rs`).
  - Token-authenticated localhost Axum HTTP/JSON API + SSE live event stream (`/api/events`) + `/metrics` (`crates/pipeline/src/server.rs`).
  - Core provider traits (`VideoSource`, `Transcriber`, `Ocr`, `Llm`, `Search`, `FactCheckArchive`), per-host token-bucket rate limiter (12 req/min), circuit breaker (5 failures -> 30m cooldown), and URL response cache (`crates/core/src/politeness.rs`).
  - `YtDlpClient` metadata subprocess wrapper with configurable timeout, JSON parser, and structured error taxonomy (`crates/providers/src/ytdlp.rs`), plus `PasteLinkSource` (single/multi-URL, short links, CSV/TSV/semicolon/text partner import, TikTok public oEmbed fallback).
  - Empirical 3-approach discovery spike (`ytdlp_public`, `headless_browser_public`, `direct_http_public`) across 10 Bulgarian seeds x 3 runs (`docs/SPIKE_DISCOVERY.md`).
  - CLI commands (`add-url`, `import-file`, `discover`, `spike-discovery`, `list-videos`, `serve`, `setup-deps`) in `crates/cli/src/main.rs`.
- **Decisions taken**:
  - `D-004`: Recorded **NO-GO** default (`automated_discovery_enabled = false`) for automated seeded discovery because unauthenticated `/search?q=` keyword pages were gated (50.0% combined gate rate >= 20% ceiling), while noting `headless_browser_public` achieved 100% success (60 unique BG videos, 0% block) on `/tag/{hashtag}` pages and remains opt-in behind the circuit breaker. Paste-a-Link and Partner-Export import proceed as primary discovery sources.
- **Open issues**:
  - None blocking Phase 2.
- **Exact PowerShell verification commands and expected output**:
  ```powershell
  $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
  cargo test --workspace
  cargo run -p leadfinder-cli -- import-file fixtures/phase1_20_urls.txt
  cargo run -p leadfinder-cli -- list-videos --limit 20
  Get-Content docs/SPIKE_DISCOVERY.md | Select-Object -First 20
  ```
  Expected output:
  - `cargo test --workspace` runs 7 unit and integration tests and exits `0` with zero warnings.
  - `import-file fixtures/phase1_20_urls.txt` prints `"imported_count": 20` with populated creator handles, views, durations, captions, and `DISCOVERED` states.
  - `list-videos --limit 20` outputs JSON array of 20 stored Bulgarian TikTok videos.
  - `docs/SPIKE_DISCOVERY.md` displays the measured table across `ytdlp_public`, `headless_browser_public`, and `direct_http_public` and the `NO-GO` recommendation.

---

## Phase 2: Bulgarian single-video pipeline

- **Status**: COMPLETE (all acceptance criteria met)
- **What was built**:
  - `ACQUIRE` stage (`crates/providers/src/media.rs`): downloads video + native captions (`bg.*`, `bul.*`, `en.*`, `eng.*`, `ru.*`, `rus.*`, `tr.*`, `tur.*`, `orig.*`), extracts `16 kHz` mono PCM WAV (`audio_16k.wav`), samples 3-second keyframes (`<= 20` per clip at `720p` width), generates cached `thumb.jpg`, and computes 64-bit `dHash` perceptual hashes (`phash`).
  - `Transcriber` trait implementations (`crates/providers/src/transcriber.rs`): `CaptionFirstTranscriber` (`found` native VTT/SRT captions first, rejecting machine-translated non-Cyrillic tracks, falling back to `WhisperCppTranscriber` `generated`), deterministic `transcript_quality` scoring (token confidence + speech coverage + language match + CPS/diversity sanity), and word-level Levenshtein `compute_wer`.
  - Extensive config-driven Bulgarian dictionary, phonetic/IPA transcription, dialect/informal speech, and grammar module (`config/lexicon.bg.toml` + `crates/providers/src/lexicon.rs`): loads `922,137` inflected wordforms, `75,930` IPA phonetic transcriptions (`bg_mfa_ipa.dict` + Wiktionary), `149,148` phonetic skeleton keys, `78,238` Hunspell lemmas + affixes, and `1,241` dialectal/informal entries; conditions Whisper via `--prompt`, repairs phonetic/orthographic ASR errors, splits glued enclitics/proclitics, hyphenates `по-`/`най-` comparatives, and produces structured LLM linguistic context notes (`analyze_for_llm`).
  - `Ocr` trait implementations (`crates/providers/src/ocr.rs`): `TesseractOcr` (`bul+eng` with `-c tessedit_create_tsv=1`) and `PaddleOnnxOcr` (`RapidOCR`) with cross-frame near-duplicate collapsing (`0.85` similarity) and empirical 20-clip / 311-frame benchmark (`docs/OCR_BENCHMARK.md`).
  - Media retention janitor (`crates/pipeline/src/janitor.rs`): deletes raw MP4/WAV/keyframes older than `retention_days` (default 7) while preserving `thumb.jpg`, transcripts, and OCR rows.
  - Dependency bootstrapper (`crates/providers/src/bootstrap.rs` / `leadfinder setup-deps`): verifies and auto-installs `yt-dlp`, `ffmpeg`, `ffprobe`, `whisper-cli`, `llama-server`, `tesseract` (`bul`+`eng`), WebView2, Whisper models, and all 5 Bulgarian lexicon/IPA files (including pure-Rust streaming compilation of `bg_wiktionary_lexicon.tsv`).
  - CLI commands (`process`, `process-batch`, `benchmark-ocr`, `wer-report`, `run-janitor`, `lexicon-inspect`).
- **Decisions taken**:
  - `D-005`: Selected `tesseract (bul+eng)` as default OCR engine (`197.6 ms/frame`, `234` Cyrillic lines vs `1,572.9 ms/frame` and `33` Cyrillic lines for `paddle_onnx`).
  - `D-006`: Selected `whisper.cpp` `small` as baseline ASR model with `CaptionFirstTranscriber` and integrated the `922k`-wordform + `75.9k`-IPA Bulgarian lexicon/grammar pipeline, improving 12-clip WER from `0.224` (`22.4%`) to `0.195` (`19.5%`, `docs/WER_REPORT.md`).
- **Open issues**:
  - None blocking Phase 3.
- **Exact PowerShell verification commands and expected output**:
  ```powershell
  cargo test --workspace
  .\target\release\leadfinder.exe setup-deps --check-only
  .\target\release\leadfinder.exe lexicon-inspect --text "Кво прайш ве баце, знаешь ли че господин Радеф влезна февро зоната и плащаме дересе за най ниските цени?"
  .\target\release\leadfinder.exe wer-report fixtures/phase2_wer_references.json
  Get-Content docs/OCR_BENCHMARK.md
  Get-Content docs/WER_REPORT.md
  ```
  Expected output:
  - `cargo test --workspace` runs 12 unit and integration tests across `core`, `providers`, and `pipeline` and exits `0` with zero warnings.
  - `setup-deps --check-only` outputs JSON confirming `yt_dlp`, `ffmpeg`, `ffprobe`, `whisper_cli`, `tesseract`, `tessdata_bul: true`, `tessdata_eng: true`, `llama_server`, `whisper_models`, and all 5 `lexicon_files`.
  - `lexicon-inspect` outputs `"valid_wordforms": 922137`, `"ipa_transcriptions": 75930`, normalized Bulgarian sentence, and populated `llm_context` glossary/acronym notes.
  - `wer-report fixtures/phase2_wer_references.json` prints `"clips_evaluated": 12` and `"mean_wer": 0.195`.
  - `docs/OCR_BENCHMARK.md` and `docs/WER_REPORT.md` display the committed benchmark and WER tables.
