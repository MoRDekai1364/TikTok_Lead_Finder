# Discovery Spike Report (Phase 1)

- **Date**: 2026-10-23
- **Seeds tested**: 10 (`hashtag:българия`, `hashtag:българскиятлев`, `hashtag:еврозона`, `hashtag:енергетика`, `hashtag:здраве`, `keyword:антибиотици забрана аптеки`, `keyword:ваксини странични ефекти`, `keyword:гмо храни магазини българия`, `keyword:данъци нов закон парламент`, `keyword:дигитално евро контрол`)
- **Runs per seed**: 3
- **Overall Decision**: **NO-GO**

## Measured Results Summary

| Approach | Attempts | Success Rate | Block / Gate Rate | p50 Latency (ms) | p95 Latency (ms) | Unique Videos | Unique BG Videos | Pass Criteria (>=30 BG & <20% Block) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `ytdlp_public` | 30 | 0.0% | 0.0% | 1935 | 2294 | 0 | 0 | FAIL (NO-GO) |
| `headless_browser_public` | 30 | 50.0% | 50.0% | 11663 | 13998 | 60 | 60 | FAIL (NO-GO) |
| `direct_http_public` | 30 | 0.0% | 100.0% | 1436 | 2187 | 0 | 0 | FAIL (NO-GO) |

## Recommendation

NO-GO for automated public-page scraping: no unauthenticated approach achieved >= 30 Bulgarian videos across 10 seeds with < 20% block rate under hard safety boundaries (no login, no CAPTCHA solving, no fingerprint spoofing). Proceed with Paste-a-Link inbox and Partner-Export CSV/text import as primary discovery sources.

## Sample Observations & Error Taxonomy

### `ytdlp_public`
- `българия: [Internal] ERROR: No working app info is available`
- `българскиятлев: [Internal] ERROR: No working app info is available`
- `еврозона: [Internal] ERROR: No working app info is available`
- `енергетика: [Internal] ERROR: No working app info is available`
- `здраве: [Internal] ERROR: No working app info is available`

### `headless_browser_public`
- `антибиотици забрана аптеки: empty unauthenticated response (JS/challenge gated)`
- `ваксини странични ефекти: empty unauthenticated response (JS/challenge gated)`
- `гмо храни магазини българия: empty unauthenticated response (JS/challenge gated)`
- `данъци нов закон парламент: empty unauthenticated response (JS/challenge gated)`
- `дигитално евро контрол: empty unauthenticated response (JS/challenge gated)`
- Sample URLs extracted: 25

### `direct_http_public`
- `българия: empty unauthenticated response (JS/challenge gated)`
- `българскиятлев: empty unauthenticated response (JS/challenge gated)`
- `еврозона: empty unauthenticated response (JS/challenge gated)`
- `енергетика: empty unauthenticated response (JS/challenge gated)`
- `здраве: empty unauthenticated response (JS/challenge gated)`

