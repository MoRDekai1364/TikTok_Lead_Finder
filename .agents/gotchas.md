# Gotchas & Engineered Resolutions

## Regex Backreferences Not Supported by `regex` Crate
- **Symptom**: Background worker loop silently crashes on first fact verification task; jobs remain stuck in `running` or `queued` status forever. The thread panics with:
  `called Result::unwrap() on an Err value: regex parse error: backreferences are not supported`
- **Root Cause**: `regex::Regex` does not support backreferences like `(?is)<(script|style|iframe)\b[^>]*>[\s\S]*?</\1>`.
- **Resolution**: Use explicit alternation without backreferences:
  `(?is)<script\b[^>]*>[\s\S]*?</script>|<style\b[^>]*>[\s\S]*?</style>|<iframe\b[^>]*>[\s\S]*?</iframe>|<noscript\b[^>]*>[\s\S]*?</noscript>|<svg\b[^>]*>[\s\S]*?</svg>|<iframe\b[^>]*>`
  Additionally, enclose per-job worker execution inside `tokio::spawn` to catch any runtime panic and prevent background worker thread termination.
