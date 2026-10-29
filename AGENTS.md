# TikTok Lead Finder Memory & Architecture

## Stack & Architecture
- **Language**: Rust 2021 (backend/core/pipeline/providers), Python 3 (auxiliary scripts & ASR tools), SvelteKit / TypeScript (frontend UI)
- **Crates**:
  - `leadfinder-core`: Domain models, traits (`Transcriber`, `LlmProvider`, `VisionProber`), error types, event channels.
  - `leadfinder-pipeline`: Job engine, claim worker loop, stage orchestration (`SCREEN_CHEAP` -> `ACQUIRE` -> `CLAIMS` -> `VERIFY`).
  - `leadfinder-providers`: External integrations: WhisperCppTranscriber, CaptionFirstTranscriber, Ollama/OpenAI/Gemini LLMs, SearxngSearchProvider, WebScraper.
  - `leadfinder-probe`: Multimodal frame extraction, OCR probe, visual change detection.
  - `leadfinder-cli`: Command-line interface and background server runner.

## Key Directories & Files
- `crates/providers/src/transcriber.rs`: Hybrid speech transcriber with Whisper sentence segmentation, language detection routing, and Google AI speech integration.
- `crates/providers/src/sanctions.rs`: OpenSanctions live search & multi-jurisdiction sanctions/PEP cross-referencing engine (UN, US OFAC SDN, EU, UK OFSI, SECO, NAZK, Interpol).
- `crates/providers/src/news_stream.rs`: Dynamic real-time news stream engine aggregating Google News RSS, BTA, and BNR with deduplication.
- `crates/pipeline/src/creator_risk.rs`: Web-based author risk calculation engine using Factcheck.bg, BNT, Google search, and AEGIS forensic penalties.
- `crates/pipeline/src/verify.rs`: Verification pipeline with AEGIS deepfake and voice clone detection integrated into claim auditing.
- `crates/pipeline/src/server.rs`: REST API server with chunked 10k-word OSINT dossier synthesis, dynamic query formulation, entity pivoting, and live news streams.
- `crates/pipeline/src/engine.rs`: Pipeline worker loop, stage transitions, event emissions, and background continuous news stream monitoring.
- `tools/aegis_deepfake_detector.py` & `scripts/aegis_deepfake_detector.py`: Forensic audio/video synthetic detection engine (F0, vocoder cutoffs, 2D FFT, Laplacian variance, AV-sync).
- `release_stage/leadfinder.exe`: Staged release binary.

## Commands
- Build release: `cargo build --release -p leadfinder-cli`
- Run workspace tests: `cargo test --workspace`
- Run transcriber test: `python scripts/google_audio_transcribe.py --audio <path> --whisper-json <path>`

## Conventions & Constraints
- Strictly zero comments across all codebase files, configuration files, and git commits.
- All file links must use `file:///` format with forward slashes.
- Token shield output pipe discipline on verbose build and test outputs.
