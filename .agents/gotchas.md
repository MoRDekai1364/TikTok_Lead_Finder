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

## Interactive "Research More" Queue Starvation & UI Inactivity
- **Symptom**: Clicking the "Проучи" (Research More) button appears to do nothing; no UI state change, toast, or spinner appears, and the video's verification never completes.
- **Root Cause**:
  1. `handle_decision` in `crates/pipeline/src/server.rs` merely enqueued `Stage::ResearchMore` into the DB `jobs` table without initiating execution.
  2. In `crates/core/src/db.rs`, `claim_next_job` sorted strictly by `j.id ASC`. Retrying failing background jobs (e.g. yt-dlp 429/captcha blocks) with low IDs monopolized the FIFO queue, completely starving newly enqueued interactive research jobs.
  3. In `ui/src/components/VirtualInbox.svelte`, `selectedCard` referenced `filteredCards` before declaration (TDZ), and the button had no loading state, disabled state, or spinner.
- **Resolution**:
  1. `handle_decision` now spawns `run_research_more_job` immediately in a background `tokio::spawn` task while updating the job status to `running` and emitting live SSE events.
  2. `claim_next_job` in `crates/core/src/db.rs` prioritizes `Stage::ResearchMore` (Priority 0) and sorts by `j.attempts ASC, j.id ASC` to prevent failing retries from blocking fresh tasks.
  3. `VirtualInbox.svelte` and `LeadCard.svelte` now display an animated spinner, disable the button during active research, show a progress track, and `App.svelte` displays live toast notifications.

## Single-Turn JSON Truncation in 10k-Word OSINT Generation
- **Symptom**: Dossiers requested to be 5,000–10,000 words generated only ~200–500 words despite system prompts asking for 10k words.
- **Root Cause**: When LLMs output structured JSON, token budgets and JSON syntax completion pressures cause them to truncate text strings to 1–2 paragraphs to maintain syntactic integrity.
- **Resolution**: Refactored `handle_research_person` to synthesize the dossier across 3 sequential chapter blocks (Chapters 1–3, 4–6, 7–9), generating 2,500–3,500 words per block. Each block is concatenated into an exhaustive 10,000-word investigative markdown document, followed by a lightweight schema metadata extraction pass.

## OpenSanctions and Live Dynamic Media Stream Resiliency
- **Symptom**: Relying on static whitelists for OSINT news and hardcoded sanction bodies misses breaking investigations, regional entities, and new PEP declarations.
- **Root Cause**: Hardcoded search templates and fixed jurisdiction filters cannot keep up with real-world corporate restructuring, new sanctions, or breaking news.
- **Resolution**: Implemented `leadfinder_providers::SanctionsMatcher` (live OpenSanctions API with multi-jurisdiction fallback) and `leadfinder_providers::DynamicNewsStream` (real-time Google News RSS, BTA, and BNR parser with entity deduplication and background worker polling). Corporate entities (`ООД`, `ЕООД`, `АД`, `Холдинг`) are extracted on the fly via `extract_entity_pivots` for 2nd-degree recursive sub-searches.
