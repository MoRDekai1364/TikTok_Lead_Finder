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
