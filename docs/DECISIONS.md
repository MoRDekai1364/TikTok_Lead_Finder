# Architecture & Implementation Decisions Log

Every deviation from `docs/PLAN.md` and every resolution of a deferred decision (Section 18) is recorded here with its rationale.

## D-001: Workspace Root Layout (`leadfinder/`)
- **Date**: 2026-10-23
- **Phase**: 0
- **Context**: Section 15 depicts the tree rooted under `leadfinder/`, while Section 0 and Section 13 reference relative paths (`docs/PROGRESS.md`, `docs/DECISIONS.md`, `config/primary_sources.toml`, `assets/calibration`, `prompts/`) directly at the repository root where `TikTok Lead Finder Technical Implementation Plan.md` resides.
- **Decision**: Treat the workspace root (`D:\Programming\Personal_projects_D\TikTok_Scan`) as the `leadfinder/` repository root (`Cargo.toml` package workspace name `leadfinder`), and copy `TikTok Lead Finder Technical Implementation Plan.md` to `docs/PLAN.md` while keeping the original file intact.
- **Reason**: Preserves exact relative paths across Section 0–16 without nested directory indirection and keeps the user's original specification untouched.

## D-002: Explicit Deferral of Owner-Only Sign-offs with Safe Local Defaults
- **Date**: 2026-10-23
- **Phase**: 0
- **Context**: Phase 0 lists organizational sign-offs (employer approval, TikTok terms acceptance, Gemini organizational terms, exact partner export format, curated seed/watchlist review) and allows each owner item to be answered or explicitly deferred.
- **Decision**: Record explicit deferrals in `docs/OWNER_CHECKLIST.md`, set `cloud_enabled = false`, support flexible CSV/TSV/text URL & ID partner exports out of the box, and ship a 40-item Bulgarian civic starter seed list in `config/seeds.bg.toml`.
- **Reason**: Follows Section 0 Rule 9 and Phase 0 acceptance criteria while guaranteeing zero-cost, zero-cloud, privacy-preserving operation by default.

## D-003: Inno Setup Packaging (`Setup_TikTok_Lead_Finder_Official_Release.iss`) and Built-in Dependency Bootstrapper
- **Date**: 2026-10-23
- **Phase**: 0 (prepared for Phase 9 packaging)
- **Context**: The owner supplied `Setup_Horizon_Browser_Official_Release.iss` from a prior project and requested adapting it for TikTok Lead Finder so the installer and program can install all required dependencies automatically.
- **Decision**: Adapted the script into `Setup_TikTok_Lead_Finder_Official_Release.iss` (removing all `;` comments per Section 0 Rule 4, generating a dedicated `AppId`, replacing hardcoded `E:\` paths with relative `release_stage\*` and `installer\*` files, removing browser `.html/.pdf` file associations, creating `%LOCALAPPDATA%\TikTok_Lead_Finder` writable directories, and wiring `leadfinder.exe setup-deps` tasks for WebView2, `yt-dlp`, `ffmpeg`, `whisper.cpp`, `llama.cpp`, OCR language packs, and models alongside Tauri bundling).
- **Reason**: Satisfies both Section 2/Phase 9 single-installer requirement and the owner's explicit Inno Setup workflow while ensuring clean installs under `{autopf}` never hit read-only directory errors.

## D-004: Phase 1 Automated Discovery Go/No-Go Decision (NO-GO Default)
- **Date**: 2026-10-23
- **Phase**: 1
- **Context**: Section 13 (Phase 1) and Section 18 require benchmarking three unauthenticated discovery approaches (`ytdlp_public`, `headless_browser_public`, `direct_http_public`) across 10 Bulgarian seeds (5 hashtags + 5 keywords) over 3 runs (30 attempts per approach) and declaring NO-GO unless an approach returns >= 30 Bulgarian videos with < 20% block rate.
- **Measured Findings**:
  - `ytdlp_public`: 0.0% success, p95 = 2,294 ms (`ERROR: No working app info is available` on unauthenticated tag/playlist extractors).
  - `direct_http_public`: 0.0% success, 100.0% JS/WAF gate rate, p95 = 2,187 ms.
  - `headless_browser_public` (Edge `--headless=new --dump-dom`, logged-out, no spoofing): 50.0% overall success rate, 50.0% gate rate, p95 = 13,998 ms, 60 unique Bulgarian videos extracted. Specifically, public `/tag/{hashtag}` pages succeeded 15/15 (100% success, 0% block, 60 unique BG videos), whereas `/search?q={keyword}` pages were gated 15/15 (100% gated for logged-out sessions).
- **Decision**: Record **NO-GO** for automated seeded search by default (`automated_discovery_enabled = false` in `config/default.toml`) because the 10-seed block/gate rate (50.0%) exceeds the 20% ceiling. Proceed with Paste-a-Link inbox (single/multi-URL + `yt-dlp` metadata + TikTok public oEmbed fallback) and Partner-Export CSV/TSV/text import as primary discovery sources, while retaining `HeadlessBrowserDiscoverySource` behind the opt-in config toggle and 5-failure circuit breaker for public hashtag pages if the owner chooses to enable it.
- **Reason**: Strictly adheres to the Phase 1 empirical acceptance rule and Section 5 hard boundaries.

## D-005: OCR Engine Selection (`tesseract (bul+eng)`)
- **Date**: 2026-10-23
- **Phase**: 2
- **Context**: Section 6, Section 13 (Phase 2), and Section 18 require benchmarking Tesseract (`bul+eng`) against PaddleOCR ONNX (`RapidOCR`) on real Bulgarian TikTok keyframes and recording the engine choice.
- **Measured Findings** (`docs/OCR_BENCHMARK.md`, 20 clips / 311 frames):
  - `tesseract (bul+eng)`: `197.6 ms/frame`, `248` deduped lines, `234` Cyrillic lines (`94.4%` Cyrillic share), mean confidence `0.695`. Notice passing `-c tessedit_create_tsv=1` instead of positional `tsv` works cleanly with standalone `--tessdata-dir`.
  - `paddle_onnx (RapidOCR)`: `1,572.9 ms/frame`, `233` deduped lines, `33` Cyrillic lines (`14.2%` Cyrillic share, default Latin/CJK recognition weights).
- **Decision**: Select `tesseract` (`bul+eng`) as default OCR engine (`ocr_engine = 'tesseract'` in `config/default.toml`), keeping `PaddleOnnxOcr` available behind config.
- **Reason**: 8.0x faster per frame, 7.1x higher Bulgarian Cyrillic recall, and zero Python runtime dependency.

## D-006: Whisper ASR Variant & Local Bulgarian Morphological + Phonetic/IPA + Dialect/Grammar Lexicon
- **Date**: 2026-10-23
- **Phase**: 2
- **Context**: Section 18 defers Whisper variant selection, and the owner requested downloading an extensive local Bulgarian dictionary covering dialects, informal speech, phonetic transcriptions, and grammar structures integrated with both the ASR transcription model and the LLM model.
- **Decision**:
  - Default ASR engine to `whisper.cpp` (`b5454`) with `ggml-small.bin` (`whisper_model = 'small'`, calibrated dynamically in Phase 3 against `tiny`, `base`, `small`, `small-q5_1`), always preferring native original TikTok captions (`found` via `CaptionFirstTranscriber` supporting both ISO-639-1 `bg` and ISO-639-2 `bul-BG`) before running Whisper (`generated`).
  - Implement a 100% config-driven lexicon & phonetic/grammar engine (`config/lexicon.bg.toml` + `crates/providers/src/lexicon.rs`, zero hardcoded Bulgarian constants in Rust) backed by `models/lexicon/{bg_50k.txt,bg_BG.dic,bg_BG.aff,bg_mfa_ipa.dict,bg_wiktionary_lexicon.tsv}` (`922,137` inflected wordforms, `75,930` IPA phonetic transcriptions, `149,148` phonetic skeleton keys, `1,241` dialect/informal entries), auto-provisioned by `leadfinder setup-deps`.
  - Feed Whisper `--prompt` conditioning + post-ASR orthographic, enclitic/proclitic, comparative `по-`/`най-`, and IPA/phonetic OOV repair, reducing 12-clip real TikTok WER from `0.224` (`22.4%`) to `0.195` (`19.5%`, `-12.9%` relative error reduction, `docs/WER_REPORT.md`), while exposing `analyze_for_llm` linguistic context notes (dialect/informal glosses with IPA, institutional acronyms, and renarrated-mood `преизказно наклонение` hearsay markers) for Phase 4–5 LLM models.
- **Reason**: Improves real Bulgarian TikTok transcription accuracy measurably while enabling downstream LLM claim/entity/attribution models to handle regional dialects, informal speech, and Bulgarian grammatical evidentiality offline.

## D-007: Latest INSAIT BgGPT Release, License Verification, and Calibration Model Selection
- **Date**: 2026-10-23
- **Phase**: 3
- **Context**: Section 2, Section 10, and Section 18 require verifying the latest INSAIT BgGPT GGUF release and its license terms at implementation time and selecting the default local LLM via calibration (`>= 8 tok/s` and `>= 95%` JSON schema validity over 3 prompts x 3 runs).
- **Verified Findings**:
  - Latest official INSAIT HuggingFace GGUF family (`INSAIT-Institute`) as of October 2026 is **`BgGPT-Gemma-3-{4B,12B,27B}-IT-GGUF`** (`gated: false`), succeeding `BgGPT-Gemma-2-{2.6B,9B,27B}-IT-v1.0-GGUF`.
  - **License**: Distributed under the **Google Gemma Terms of Use** (`gemma` license tag on HuggingFace, un-gated download; permits local research and commercial/organizational inference subject to the Gemma Prohibited Use Policy; weights are downloaded locally per install via `leadfinder setup-deps --with-models` rather than redistributed inside third-party source trees).
  - **Empirical Calibration (`llama-server b11836`, Vulkan on RTX 5070 Ti 12 GB)**:
    - `BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf` (`2.49 GB`): **`48.97 tok/s`**, **`100.0%` JSON schema validity (`9/9`)**, `5.05 s` mean call latency -> **PASSES** (`>= 95%` validity & `>= 8 tok/s`).
    - `BgGPT-Gemma-2-2B-IT-v1.0.Q4_K_M.gguf` (`1.71 GB`): `63.24 tok/s`, `66.7%` JSON schema validity (`6/9`), `6.02 s` mean call latency -> rejected by the `95%` validity gate.
  - **Redistributable Calibration Audio**: Bundled `assets/calibration/sample_bg_30s.wav` (`30.0 s`, `16 kHz` mono PCM WAV) compiled from Wikimedia Commons Lingua Libre native Bulgarian recordings (`CC BY-SA 4.0`, attribution in `assets/calibration/ATTRIBUTION.md`).
- **Decision**: Default local LLM bootstrapper and config to `BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf` (while supporting larger `9B`/`12B`/`27B` GGUFs when present in `models/llm/` and falling back to CPU or heuristics-only banner mode on low-power machines).
- **Reason**: `BgGPT-Gemma-3-4B-IT-Q4_K_M` is un-gated, fits comfortably in 4–12 GB VRAM or 8+ GB RAM, and achieves 100% first-try JSON validity on Bulgarian claim extraction.

---

## D-008: Bulgarian Fact-Check Archives Accessibility, robots.txt Rules, and WAF Findings
- **Date**: 2026-10-23
- **Phase**: 5
- **Context**: Section 8, Section 13 (Phase 5), and Section 18 require auditing Bulgarian fact-check archives (Factcheck.bg, AFP's Bulgarian fact-check section, the BROD database, BNR and BNT fact-check sections) through official search, feeds, or sitemaps, respecting `robots.txt` and terms, and recording which archives are actually accessible.
- **Empirical Findings**:
  - `Factcheck.bg` (`https://factcheck.bg`): `robots.txt` returns `200 OK` with full public crawl allowance (`User-agent: *`, `Allow: /`). The WordPress REST API (`/wp-json/wp/v2/posts?per_page=8&search={query}`) is live and returns structured JSON with titles, excerpts, links, and dates without rate-walling. The RSS feed (`/feed/`) is live and returns full recent articles (`268 KB`). HTML search (`/?s={query}`) is also functional. -> accessible at first audit. Re-test later the same day: after a burst of larger requests (per_page=100), the WordPress REST API and RSS feed both returned 403 Forbidden for the project's honest user-agent while obots.txt still returned 200. Treated as a temporary rate or WAF block; no retry loop, no user-agent spoofing. The circuit breaker in PolitenessGuard opens after 5 failures and verification degrades to primary-source and web search.
  - `BNT` (`https://bntnews.bg`): `robots.txt` returns `200 OK` (`User-agent: *`, `Disallow:` none). Public search endpoint (`/search?q={query}`) returns `200 OK` with structured news and fact-check results. Sitemap is available at `/sitemap.xml`. -> **Accessible Archive**.
  - `BNR` (`https://bnr.bg`): `robots.txt` returns `200 OK`, but contains explicit disallow rules: `Disallow: /*/search` and `Disallow: /*/find`, while exposing `Sitemap: https://bnr.bg/sitemap.xml`. To maintain strict compliance with `robots.txt`, Lead Finder's `RobotsTxtChecker` blocks dynamic query dispatch to `/search` on `bnr.bg`; archival access operates via allowed RSS category feeds and sitemaps only.
  - `BROD` (`https://brodhub.eu`): Central European EDMO fact-checking hub; `robots.txt` disallows `/XWiki/*/Main/Search` and `/XWiki/*/rest/` while providing `Sitemap: https://brodhub.eu/XWiki/EDMO/Hub/SiteMap?xpage=plain&outputSyntax=plain`. Feeds and sitemaps are used.
  - `AFP Провери` (`https://proveri.afp.com`): Unauthenticated HTTP/HTTPS GET requests from local machines (including `/robots.txt` and `/rss.xml`) return `403 Forbidden` due to automated edge bot protection / Cloudflare WAF. Section 2 and Section 5 explicitly forbid user-agent spoofing, CAPTCHA solving, or IP proxy rotation.
- **Decision**:
  - Set `factcheck_bg` and `bnt_news` as active archives (`enabled = true`) in `config/default.toml`.
  - Disable `afp_proveri` by default (`enabled = false`) in `config/default.toml` to avoid unnecessary 403 request overhead.
  - Enforce automated `RobotsTxtChecker` checks in `ConfiguredFactCheckArchive` and `WebAndPrimarySearch` to guarantee that all archive and search queries respect `robots.txt` disallows dynamically.
- **Reason**: Strictly honors Section 0 Rule 7, Section 2 hard boundaries, and Section 8 retrieval compliance rules.


## D-009: Gold Set Is a Synthetic Offline Fixture, Not Published Fact-Checks
- **Date**: 2026-10-23
- **Phase**: 5
- **Context**: Section 14 asks for 30 to 50 Bulgarian claims drawn from published fact-checks with recorded HTTP responses. The owner has not yet supplied the team's fact-checks, and Factcheck.bg returned 403 Forbidden on re-test (see D-008), so real archive pages could not be recorded.
- **Decision**: ixtures/gold_set/claims.json (40 claims, 12 traps covering all 6 Section 14 categories, flagged "synthetic": true) is authored test data. People, institutions, passages and URLs are fictional (*.test.invalid) so that no invented statement is attributed to a real person or outlet. It exercises the code-enforced forcing rules and the model's passage-grounded reasoning offline through leadfinder eval-gold-set. The loader accepts the same schema for real claims.
- **Consequence**: docs/GOLD_SET_REPORT.md reports trap pass rate, forcing-rule compliance and CONTRADICTED recall on this synthetic fixture only. It does not measure agreement with published verdicts. That metric is open until the owner supplies real fact-checks (or archive access recovers) and is listed in docs/OWNER_CHECKLIST.md.
- **Reason**: Honest reporting. A synthetic pass must not be presented as validation on real fact-checks.
## D-010: Proceeding to Phase 5 With Phase 4 Marked PARTIAL
- **Date**: 2026-10-23
- **Phase**: 4 and 5
- **Context**: Phase 4 acceptance asks for 80% correct extractions on an owner review of 50 claims. The corpus yields 39 claims after filtering and the agent's review scored 66.7%. The owner review cannot be done by the agent.
- **Decision**: Record Phase 4 as PARTIAL in `docs/PROGRESS.md` and continue to Phase 5, whose verification stage depends only on claim text and the forcing rules. Do not report Phase 4 as passed.
- **Reason**: Section 0 rule 9 lets the agent continue when blocked on an owner-only item. The deviation from strict phase sequencing is logged here.
