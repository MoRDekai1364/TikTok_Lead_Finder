# Implementation Progress

Last updated: 2026-10-23 · Current active phase: Phase 1 complete, ready for Phase 2.

## Phase Summary

| Phase | Title | Status |
| --- | --- | --- |
| Phase 0 | Owner decisions and scaffold | PASS |
| Phase 1 | Core skeleton, paste-a-link, discovery spike (go/no-go) | PASS |
| Phase 2 | Bulgarian single-video pipeline | IN PROGRESS |
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
