# Gotchas & Engineered Resolutions

## Regex Backreferences Not Supported by `regex` Crate
- **Symptom**: Background worker loop silently crashes on first fact verification task; jobs remain stuck in `running` or `queued` status forever. The thread panics with:
  `called Result::unwrap() on an Err value: regex parse error: backreferences are not supported`
- **Root Cause**: `regex::Regex` does not support backreferences like `(?is)<(script|style|iframe)\b[^>]*>[\s\S]*?</\1>`.
- **Resolution**: Use explicit alternation without backreferences:
  `(?is)<script\b[^>]*>[\s\S]*?</script>|<style\b[^>]*>[\s\S]*?</style>|<iframe\b[^>]*>[\s\S]*?</iframe>|<noscript\b[^>]*>[\s\S]*?</noscript>|<svg\b[^>]*>[\s\S]*?</svg>|<iframe\b[^>]*>`
  Additionally, enclose per-job worker execution inside `tokio::spawn` to catch any runtime panic and prevent background worker thread termination.

## Google Speech API Lack of Bulgarian Sentence Boundaries
- **Symptom**: Google AI speech recognition produces high Bulgarian lexical accuracy but outputs unpunctuated text without sentence boundaries.
- **Root Cause**: Web Speech API does not punctuate or divide sentences for Bulgarian.
- **Resolution**: Whisper runs via Vulkan GPU in ~1.2s to generate acoustic sentence boundaries, timestamps, and punctuation. The sequence alignment engine (`align_google_words_to_whisper_segments`) aligns Google's speech recognition text into Whisper's acoustic sentence segments, keeping exact timestamps and sentence punctuation. If Google speech recognition fails, Whisper's complete transcript is used as a fallback. For non-Bulgarian audio, audio routes directly to Whisper.

## SQLite Transcripts Table Check Constraint Failure
- **Symptom**: Videos take 10+ minutes stuck in processing; jobs in `ACQUIRE` repeatedly crash after full download and speech recognition with: `error returned from database: (code: 275) CHECK constraint failed: source IN ('found','generated')`.
- **Root Cause**: `crates/providers/src/transcriber.rs` returned `source: "asr".to_string()`, which violates the SQLite table constraint `source IN ('found','generated')` on table `transcripts`.
- **Resolution**: Updated `transcriber.rs` to set `source: "generated".to_string()` and added defensive fallback mapping in `engine.rs` before `db.save_transcript`.

## Worker Loop Serialization & Sequential Frame OCR
- **Symptom**: Pipeline throughput capped at 1 video at a time regardless of available CPU/GPU hardware; 20-frame OCR takes 8-10 seconds per video.
- **Root Cause**: `spawn_worker_loop` in `crates/pipeline/src/engine.rs` directly awaited each job on the loop instead of spawning tasks concurrently under `AimdConcurrencyManager` stage permits. Additionally, `crates/providers/src/ocr.rs` ran `ocr_single_frame` sequentially.
- **Resolution**: Refactored `spawn_worker_loop` to launch claimed jobs in independent `tokio::spawn` tasks with stage AIMD permits, and parallelized keyframe OCR using `futures::future::join_all`.
