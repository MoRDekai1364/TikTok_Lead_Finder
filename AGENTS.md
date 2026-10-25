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
- `crates/providers/src/passages.rs`: Passage extraction and HTML sanitizer regex.
- `crates/pipeline/src/engine.rs`: Pipeline worker loop, stage transitions, event emissions, panic-supervised job tasks.
- `scripts/google_audio_transcribe.py` & `tools/google_audio_transcribe.py`: Hybrid speech transcription script, sequence word alignment, Gemini multimodal audio segmentation, Google Web Speech API.
- `tools/whisper-cli.exe`: Vulkan-accelerated local whisper engine on Intel GPU.
- `models/whisper/`: GGML models (`ggml-tiny.bin`, `ggml-large-v3-turbo-q5_0.bin`).
- `release_stage/leadfinder.exe`: Staged release binary.

## Commands
- Build release: `cargo build --release -p leadfinder-cli`
- Run workspace tests: `cargo test --workspace`
- Run transcriber test: `python scripts/google_audio_transcribe.py --audio <path> --whisper-json <path>`

## Conventions & Constraints
- Strictly zero comments across all codebase files, configuration files, and git commits.
- All file links must use `file:///` format with forward slashes.
- Token shield output pipe discipline on verbose build and test outputs.
