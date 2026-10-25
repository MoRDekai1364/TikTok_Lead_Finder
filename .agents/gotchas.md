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
