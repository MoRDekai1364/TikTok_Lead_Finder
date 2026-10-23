# TikTok Lead Finder (Bulgaria-first): Technical Implementation Plan

Version 1.0 · 23 October 2026 · For the implementing coding agent and the project owner

## 0. How to use this document

This is the single source of truth. Implement it phase by phase (section 13).

1. Phases are sequential. Start phase N+1 only when phase N's acceptance criteria pass.
2. After every phase, update docs/PROGRESS.md with status, what was built, decisions taken, open issues, and the exact PowerShell commands the owner runs to verify. Any other session or account must be able to resume from PROGRESS.md alone.
3. Log every deviation from this plan in docs/DECISIONS.md with the reason.
4. No comments in code, SQL, config or scripts. Names and structure carry the meaning.
5. Primary platform is Windows 10/11. Give PowerShell commands, never bash.
6. Make small, reviewable changes. After each task, list the commands to build and test and the output to expect.
7. Free by default: every stage works with zero API keys and zero spend. Keyed or paid providers are optional plug-ins.
8. Never commit secrets. Keys live in the OS credential store or environment variables. Commit only .env.example. Assume anything in the workspace may be sent to the coding agent's provider.
9. Ask the owner only when blocked on something only they can answer (access, accounts, terms). Otherwise pick the simplest option, record it in docs/DECISIONS.md and continue.

## 1. Product

A local Windows desktop app that discovers Bulgarian TikTok videos, transcribes and analyses them, and gives a human fact-checker a short ranked list of leads that probably contain false or misleading claims.

The end user is a professional fact-checker. Her TikTok feed is personalised by the algorithm, so today she watches hours of irrelevant video to find a few false-information videos. The tool replaces that with a daily queue of roughly 20 to 30 leads, each with a one-line reason.

**It produces leads, not verdicts. A human decides.**

Non-goals: auto-publishing; final verdicts; face recognition or any biometric identification; bypassing TikTok access controls; collecting data on private individuals; countries other than Bulgaria in v1 (but never hard-code Bulgaria: region, language and sources come from config).

Principles: precision over recall at the top of the queue; every verdict links to evidence; “no evidence found” never means “false”; degrade gracefully (pasting a link always works); calm UI with detail only on request.

## 2. Fixed decisions

| Area | Decision |
| --- | --- |
| Language and region | Bulgarian first, region BG. UI strings in Bulgarian and English through i18n keys, Bulgarian default. No Bulgaria-specific constants in code. |
| Cost | Fully free to run. API keys optional. |
| Shape | Local-first Windows desktop app, one installer. The core runs as a local service with an HTTP API from day one. |
| Core | Rust, Tokio, Axum, sqlx with SQLite in WAL mode. |
| UI | Tauri 2, Svelte, TypeScript. The UI talks to the core only through the HTTP API and SSE, using a configurable core\_url, so the core can later run on another machine. Do not build remote mode now. |
| Acquisition | yt-dlp and ffmpeg as subprocesses with timeouts and output parsing. |
| ASR | whisper.cpp (CLI or whisper-rs). Model chosen by calibration. |
| OCR | Tesseract (Bulgarian and English) or PaddleOCR via ONNX. Decide in phase 2 by benchmark on the 20-clip set. |
| Local LLM | llama.cpp server with GGUF models. Default family INSAIT BgGPT (2.6B, 9B or 27B, chosen by calibration). Verify the latest release and licence terms at implementation time. |
| Cloud LLM (optional) | Gemini API Flash models with the owner's key. Off by default (section 11). |
| Search | Bulgarian fact-check archives, owner-editable primary-source domains, optional self-hosted SearXNG. |
| Forbidden | Using Google Antigravity, any IDE session or any consumer chat session as an LLM backend; extracting or reusing login tokens; CAPTCHA solving; account automation; fingerprint or IP spoofing; face recognition. |

## 3. Architecture

```text
Tauri UI (Svelte)  <-- HTTP/JSON + SSE -->  Core service (Rust, localhost, token-authenticated)
                                              |-- durable job queue + stage workers
                                              |-- SQLite (WAL)
                                              |-- providers: VideoSource, Transcriber, Ocr, Llm, Search, FactCheckArchive
                                              `-- subprocesses: yt-dlp, ffmpeg, whisper.cpp, llama.cpp server
```

The core is a library crate plus a thin binary. Every provider is a Rust trait with implementations and a fallback chain declared in config.

Video states: DISCOVERED, SCREENED\_OUT, ACQUIRED, EXTRACTED, CLAIMS\_DONE, VERIFIED, READY, KEPT, DISCARDED, FAILED.

Stages. Each is idempotent and resumable, retries with exponential backoff and jitter, and ends in FAILED with a stored reason after the configured maximum attempts.

1. DISCOVER: produce video references (id, url, creator, caption, hashtags, stats, discovered\_via).
2. SCREEN\_CHEAP: dedup by video id and perceptual hash; virality gate; relevance gate; budget gate.
3. ACQUIRE: download the video, extract 16 kHz mono audio and keyframes.
4. EXTRACT: transcript (captions first, otherwise Whisper), OCR, metadata.
5. CLAIMS: LLM extracts claims and people.
6. CHECKWORTHY: score claims, drop those below threshold.
7. VERIFY: retrieval-grounded verification.
8. RANK: compute lead\_score, assign lane, apply the daily cap.
9. DECISION: the user keeps, discards or sends for more research.

Funnel intent: stages 1 and 2 are nearly free, stages 3 and 4 are local compute, stages 5 to 7 use LLMs and search and must only see the surviving minority.

## 4. Data model (SQLite)

```sql
CREATE TABLE creators (id TEXT PRIMARY KEY, handle TEXT NOT NULL, display_name TEXT, region TEXT, follower_count INTEGER, first_seen_at TEXT NOT NULL, watchlist INTEGER NOT NULL DEFAULT 0, flagged_count INTEGER NOT NULL DEFAULT 0, expansion_depth INTEGER NOT NULL DEFAULT 0);
CREATE TABLE videos (id TEXT PRIMARY KEY, creator_id TEXT REFERENCES creators(id), url TEXT NOT NULL, caption TEXT, hashtags TEXT, posted_at TEXT, views INTEGER, likes INTEGER, shares INTEGER, comments INTEGER, duration_s REAL, discovered_via TEXT NOT NULL, discovered_at TEXT NOT NULL, phash TEXT, state TEXT NOT NULL, state_reason TEXT, media_path TEXT, lead_score REAL, lane TEXT, cluster_id INTEGER);
CREATE TABLE transcripts (video_id TEXT PRIMARY KEY REFERENCES videos(id), source TEXT NOT NULL CHECK (source IN ('found','generated')), engine TEXT, model TEXT, language TEXT, quality REAL, text TEXT NOT NULL, segments_json TEXT, edited_text TEXT);
CREATE TABLE ocr_texts (video_id TEXT REFERENCES videos(id), frame_ms INTEGER, text TEXT, confidence REAL, PRIMARY KEY (video_id, frame_ms));
CREATE TABLE claims (id INTEGER PRIMARY KEY AUTOINCREMENT, video_id TEXT NOT NULL REFERENCES videos(id), text_bg TEXT NOT NULL, claim_type TEXT NOT NULL, speaker TEXT, attribution_json TEXT, start_ms INTEGER, end_ms INTEGER, checkworthiness REAL, verdict TEXT, label TEXT, reasoning_bg TEXT, evidence_strength REAL, verified_at TEXT, cluster_id INTEGER);
CREATE TABLE evidence (id INTEGER PRIMARY KEY AUTOINCREMENT, claim_id INTEGER NOT NULL REFERENCES claims(id), passage_id TEXT NOT NULL, source_url TEXT NOT NULL, source_name TEXT, source_kind TEXT NOT NULL CHECK (source_kind IN ('primary','news','factcheck','other')), published_at TEXT, passage TEXT NOT NULL, stance TEXT, retrieved_at TEXT NOT NULL);
CREATE TABLE people (id INTEGER PRIMARY KEY AUTOINCREMENT, canonical_name TEXT NOT NULL, variants_json TEXT, role TEXT, public_figure INTEGER NOT NULL DEFAULT 0, wikidata_id TEXT, summary TEXT, researched_at TEXT);
CREATE TABLE video_people (video_id TEXT REFERENCES videos(id), person_id INTEGER REFERENCES people(id), mentions INTEGER NOT NULL DEFAULT 1, found_in TEXT, PRIMARY KEY (video_id, person_id));
CREATE TABLE decisions (id INTEGER PRIMARY KEY AUTOINCREMENT, video_id TEXT NOT NULL REFERENCES videos(id), action TEXT NOT NULL CHECK (action IN ('keep','discard','research_more','undo')), note TEXT, decided_at TEXT NOT NULL);
CREATE TABLE seeds (id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL CHECK (kind IN ('keyword','hashtag','creator')), value TEXT NOT NULL, topic TEXT, active INTEGER NOT NULL DEFAULT 1, last_run_at TEXT, UNIQUE (kind, value));
CREATE TABLE jobs (id INTEGER PRIMARY KEY AUTOINCREMENT, video_id TEXT REFERENCES videos(id), stage TEXT NOT NULL, status TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT, run_after TEXT, updated_at TEXT NOT NULL);
CREATE TABLE provider_usage (provider TEXT NOT NULL, model TEXT NOT NULL, day TEXT NOT NULL, requests INTEGER NOT NULL DEFAULT 0, tokens_in INTEGER NOT NULL DEFAULT 0, tokens_out INTEGER NOT NULL DEFAULT 0, errors_429 INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (provider, model, day));
CREATE TABLE profile (id INTEGER PRIMARY KEY CHECK (id = 1), hardware_json TEXT NOT NULL, chosen_json TEXT NOT NULL, capacity_per_day REAL, measured_at TEXT NOT NULL);
CREATE TABLE settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL);
```

## 5. Discovery

Sources in priority order:

1. Paste-a-link inbox (always on): accepts tiktok.com URLs and short links (vm.tiktok.com, tiktok.com/t/...), resolves the video id and enqueues it. Also accepts a text file or clipboard with many URLs.
2. Partner-export import: a CSV or text list of URLs or ids exported from the owner's team tools. The format is decided in phase 0.
3. Seeded search: Bulgarian keywords and hashtags from the seeds table, run from a logged-out session on the machine's own Bulgarian IP.
4. Creator watchlist: creators the user pins, plus automatic expansion.

Hard boundaries: no login, no account automation, no CAPTCHA solving, no fingerprint or IP spoofing, no proxy rotation. If blocked, the circuit breaker opens, the UI shows discovery as paused, and paste-a-link and import keep working.

Politeness defaults: token bucket per host, jittered delays, at most 12 requests per minute per host, exponential backoff on any 4xx or 5xx, circuit breaker after 5 consecutive failures with a 30-minute cool-down, every response cached by URL for the configured TTL.

Creator expansion: a creator becomes eligible when at least 3 of their videos were flagged as leads within 30 days. Expansion fetches at most 20 recent videos per creator per run, depth limit 2 (a creator found through expansion may expand once more, never further), and counts against the global daily budget.

## 6. Extraction

Transcript:

1. If TikTok-provided captions can be obtained through yt-dlp, use them and set source to found.
2. Otherwise run Whisper on the 16 kHz mono audio and set source to generated.

Always store engine, model, detected language, quality and timestamped segments. Expected languages are bg, en, ru and tr. Quality score from 0 to 1 combines mean token confidence, speech ratio, language match and length sanity. A low score demotes a lead and shows a badge, it never hides the video. The transcript is editable by the user.

OCR: sample keyframes (1 per 3 seconds, at most 20), run OCR in Bulgarian and English, de-duplicate near-identical lines across frames, store with frame time.

Metadata: caption, hashtags, creator, stats, posted\_at, duration, sound title if available.

Media retention: delete video and frames after 7 days (config), keep all text.

## 7. Claims and people

One LLM call per surviving video. Output is JSON only, validated against the schema, with one automatic repair retry. Use schema-constrained decoding when the server supports it. If still invalid, fail the stage with a reason.

```ts
type ClaimsOutput = {
  claims: {
    text_bg: string;
    claim_type: 'factual' | 'attribution' | 'statistical' | 'health' | 'quote' | 'prediction' | 'opinion';
    speaker: string | null;
    attribution: { actor: string; act: 'said' | 'agreed' | 'proposed' | 'voted' | 'denied' | 'other'; target: string | null; about: string }[];
    start_ms: number;
    end_ms: number;
    checkworthiness: number;
  }[];
  people: { name: string; variants: string[]; role_hint: string | null }[];
};
```

Rules:

- Extraction never produces verdicts.
- Attribution claims (X said Y, X agreed with Y, X voted for Z) are split into two parts: that the speaker asserts it (always true, from the transcript) and that the attributed act really happened. Only the second part is verified.
- Opinion claims are stored and excluded from verification.
- Check-worthiness combines the LLM score with rule features (contains numbers, names a public figure, health or election topic, urges sharing), weights in config.

People:

- Names come only from transcript, OCR, caption, hashtags and the creator profile. No face recognition.
- Normalise Cyrillic and Latin spellings of one person, Bulgarian definite-article suffixes and inflected forms, titles and roles.
- Resolve public figures through Wikidata. Web research runs only for people resolved as public figures. Unresolved names are shown as text with no research.

## 8. Verification

Retrieval-grounded. The LLM compares the claim to retrieved passages and never answers from its own memory.

Retrieval order per claim, stopping when evidence is sufficient:

1. Existing fact-checks: Bulgarian archives (Factcheck.bg, AFP's Bulgarian fact-check section, the BROD database, BNR and BNT fact-check sections) through official search, feeds or sitemaps, respecting robots.txt and terms. Optional: Google Fact Check Tools API (ClaimReview) when a key is configured. Record in docs/DECISIONS.md which archives are actually accessible.
2. Primary sources: queries restricted to owner-editable domains in config/primary\_sources.toml (parliament, government, presidency, ministries, national news agency, institutions).
3. General web search: SearXNG or a configured provider, preferring sources inside the claim's date window.

Passages: fetch the page, extract main text, chunk, rank by BM25 (embedding re-rank optional), keep the top 8 per claim with source metadata.

Verdicts: SUPPORTED, CONTRADICTED, UNVERIFIABLE, NOT\_CHECKABLE, ALREADY\_CHECKED (requires a link). Labels: false\_claim, misleading, missing\_context, satire\_or\_opinion, unknown. Never emit “disinformation”: intent cannot be known.

```ts
type VerificationOutput = {
  claim_id: number;
  verdict: 'SUPPORTED' | 'CONTRADICTED' | 'UNVERIFIABLE' | 'NOT_CHECKABLE' | 'ALREADY_CHECKED';
  label: 'false_claim' | 'misleading' | 'missing_context' | 'satire_or_opinion' | 'unknown';
  evidence: { passage_id: string; stance: 'supports' | 'contradicts' | 'neutral' }[];
  reasoning_bg: string;
  evidence_strength: number;
};
```

Enforced by code, not by prompt:

- SUPPORTED or CONTRADICTED needs at least one passage from a primary source or fact-check, or at least two independent news outlets. Otherwise the verdict is forced to UNVERIFIABLE.
- Every passage\_id must exist in the retrieved set. Otherwise reject, retry once, then UNVERIFIABLE.
- No passages retrieved means UNVERIFIABLE, never CONTRADICTED.
- For an attributed act (X said or agreed Y), search X's own statements inside a window around the video date. The speaker's statement in the video is never evidence for itself.

Confidence is three separate numbers: transcript\_quality, checkworthiness, evidence\_strength. Never merge them into one displayed percentage. lead\_score (section 9) exists only for ordering.

## 9. Ranking and funnel

Cheapest checks first:

1. Dedup by video id and perceptual hash of keyframes.
2. Virality gate: views and views per hour since posting above the configured minimums. Defaults are starting points to tune for Bulgaria's smaller audience.
3. Relevance gate: matches a seed topic, or contains a check-worthy claim.
4. Budget gate: remaining daily LLM and search budget.

Claim clustering: normalise claims and cluster near-duplicates across videos (embedding or n-gram similarity with a threshold). A cluster is one lead showing video count and total views, because fact-checkers prioritise narratives rather than single videos.

```text
lead_score = w_check * max_checkworthiness + w_viral * virality_norm + w_verdict * verdict_value - w_q * (1 - transcript_quality)
```

verdict\_value: CONTRADICTED 1.0, UNVERIFIABLE 0.6, NOT\_CHECKABLE 0.1, SUPPORTED 0.0. ALREADY\_CHECKED items go to their own lane instead of being scored.

Lanes: Leads, Already checked, Low quality (transcript quality below the threshold), Failed. The Leads lane has a daily cap (default 25). Overflow stays queued behind the cap and can be pulled with one action.

Tuning: after 200 decisions, the CLI command tune fits a logistic regression on keep versus discard and proposes new weights. Weights change only after owner confirmation.

## 10. Calibration (hardware probe)

Goal: choose models, providers and concurrency automatically. Never ask the user about hardware.

Detect: CPU model and cores, RAM, free disk, GPU vendor, name and VRAM, available compute backends (CUDA, Vulkan, CPU).

Assets in assets/calibration: a 30-second Bulgarian speech sample with a reference transcript (a clip whose licence allows redistribution, or one recorded by the owner), and 3 reference claim-extraction prompts with expected outputs.

Measure:

- Whisper candidates (tiny, base, small, medium, large-v3, quantised variants where available): real-time factor and word error rate against the reference.
- LLM candidates (BgGPT 2.6B, 9B and 27B in 4-bit GGUF where memory allows): tokens per second and JSON validity over 3 runs per prompt.

Capacity:

```text
capacity_per_day = processing_window_s / (avg_video_s * rtf + overhead_s)
```

Defaults: 12-hour window, 45-second average video, 10 seconds overhead per video.

Selection:

- ASR: the lowest-WER model whose capacity meets target\_videos\_per\_day (default 300). If none does, take the highest-capacity model under the WER ceiling, show the shortfall, and offer optional cloud assist.
- LLM: the largest model with tokens per second above the threshold and JSON validity of at least 95 percent. Otherwise the next smaller. Otherwise the cloud LLM if configured. Otherwise heuristics-only mode (rule-based flags, no LLM claims) with a visible banner.

Store the result in the profile table and show it in the UI as how many videos per day this machine handles. Re-run on demand, after a hardware change, and as a monthly reminder. Every per-stage choice can be overridden.

Runtime: an AIMD concurrency controller per stage. When queue wait grows, reduce concurrency on the constrained resource first.

Limit: calibration measures speed and transcript accuracy only. Verification quality is validated by the gold set (section 14).

## 11. Providers and fallbacks

Traits: VideoSource, Transcriber, Ocr, Llm, Search, FactCheckArchive. Config declares each implementation and its fallback chain.

LLM chain default: local llama.cpp with BgGPT, then the Gemini API Flash model (only if the owner configured a key and enabled cloud), then an optional paid key, then heuristics-only.

Clients: an OpenAI-compatible HTTP client (local servers and compatible endpoints) and a native Gemini adapter.

Cloud rules: cloud providers are off until the owner enables them. The UI shows a data-flow notice when enabled. Only text is ever sent, never video or audio. The Gemini free tier has per-project limits that are not published as a fixed table (read them in the key's rate-limit page), and free-tier data may be used to improve Google models and read by reviewers. Before enabling it for an organisation, verify the current terms, including whether commercial use is allowed.

Quota guard: count requests and tokens per provider per day in provider\_usage. On HTTP 429 or a quota error, back off, switch to the next provider in the chain, or defer the job until the reset time. Never retry in a tight loop.

Prompts live in prompts/ with a version number in the file name. Changing a prompt requires re-running the gold set.

Google Antigravity, IDE sessions and consumer chat sessions are not providers.

## 12. UI (Tauri, Svelte)

Goal: one screen for fast triage, detail only on request.

Top bar: paste-a-link field (accepts several URLs), discovery status, queue status, capacity.

Inbox (default screen): lane tabs (Leads, Already checked, Low quality, Failed) over a virtualised list of lead cards. Each card shows, left to right: thumbnail with duration; creator and views; the top claim in Bulgarian on one line; verdict chip and label; three small meters (transcript, check-worthiness, evidence) with tooltips; badges (transcript found or generated, cluster size, already checked, research running); a one-line reason it was flagged.

Keyboard: J and K move, Space opens or closes the drawer, Enter keeps, Backspace discards, R sends for more research, U undoes, / focuses the filter, Shift extends selection for batch actions. Every action also has a mouse equivalent on hover.

Detail drawer (nothing loads until it is opened): video player on the local file with the transcript synced to playback; claims highlighted in the transcript and clickable to seek; evidence per claim with source, date, stance and link; people with research summaries; creator summary and history; raw metadata collapsed. The transcript is editable and edits can re-run claim extraction for that video.

Research more: queues a deeper job (more retrieval, people research, creator history). The card shows a progress badge and updates live while the user keeps working.

Undo: every decision is reversible and logged in decisions.

Filters and sort: verdict, label, score, views, creator, topic, date; saved filters. Daily cap indicator with a pull-more action.

Other screens: Creators (watchlist, flagged counts, expansion state), Seeds (editable keywords, hashtags, topics), Calibration, Settings, Export.

First-run wizard: calibration, optional keys, a test paste-a-link, a starter seed list.

Performance: 2,000 cards scroll smoothly; updates arrive by SSE within 1 second; thumbnails cached on disk; no blocking work on the UI thread.

Accessibility and i18n: all strings through keys, Bulgarian default and English; dark and light themes; fully keyboard-operable; AA contrast.

## 13. Phases

Each phase lists tasks, then acceptance. Do not skip acceptance.

### Phase 0: Owner decisions and scaffold

Owner tasks: find out whether the partner fact-checker tool used by the owner's team can export URL or id lists, and in what format; get the employer's approval for the tool and its data handling; accept that unofficial TikTok discovery can break and may conflict with TikTok's terms; check the Gemini free-tier terms for an organisation; supply 30 to 50 Bulgarian seed keywords and hashtags and a creator watchlist; create the repository; install Rust, Node, ffmpeg and yt-dlp.

Agent tasks: create docs/OWNER\_CHECKLIST.md, the repository scaffold from section 15, and empty docs/PROGRESS.md and docs/DECISIONS.md.

Acceptance: each owner item answered or explicitly deferred; the scaffold builds.

### Phase 1: Core skeleton, paste-a-link, discovery spike (go/no-go)

Tasks: Cargo workspace; Axum service with token-authenticated localhost API and SSE; sqlx migrations from section 4; durable jobs table and worker loop; VideoSource trait; PasteLinkSource (URLs, short links, bulk text); yt-dlp metadata wrapper with timeout, parsing and an error taxonomy; rate limiter and circuit breaker; a spike of seeded search through three approaches (yt-dlp extractors for tag and user pages, a headless browser on public pages, direct HTTP on public pages); CLI commands add-url, import-file, discover.

Deliverables: working paste-a-link flow; docs/SPIKE\_DISCOVERY.md with, per approach, success rate over 3 runs on 10 seeds, block rate, p95 latency and a recommendation.

Acceptance: 20 pasted URLs are stored with correct metadata; the spike report contains measured numbers; a go/no-go is recorded in docs/DECISIONS.md. If no approach returns at least 30 Bulgarian videos across 10 seeds with a block rate under 20 percent, automated discovery is NO-GO and the project continues with paste-a-link and partner-export import.

### Phase 2: Bulgarian single-video pipeline

Tasks: ACQUIRE (download, 16 kHz mono audio, keyframes); Transcriber trait with captions first and Whisper second; OCR benchmark and engine choice; quality score; media retention janitor; CLI command process with a video id that prints the result as JSON.

Acceptance: 20 hand-picked Bulgarian clips process end to end; every transcript is tagged found or generated; a WER report covers at least 10 clips with owner-written reference transcripts; the OCR benchmark table is committed.

### Phase 3: Calibration

Tasks: probe crate implementing section 10; bundled assets; CLI command calibrate; profile storage; capacity estimate; override support.

Acceptance: runs on two hardware configurations (or two forced configurations, GPU and CPU-only) and writes a profile; a second run is idempotent; the capacity estimate is within 25 percent of a measured 20-video run.

### Phase 4: Claims and people

Tasks: Llm trait; OpenAI-compatible client and native Gemini adapter; prompt files; schema validation with one repair retry; constrained decoding where supported; people extraction, normalisation and Wikidata lookup; storage.

Acceptance: at least 90 percent of outputs are valid on the first try across the 20 clips; the owner reviews 50 claims and at least 80 percent are correct extractions; every attribution trap case is split into an asserted part and an attributed part.

### Phase 5: Verification

Tasks: Search and FactCheckArchive traits and implementations; passage fetch, extraction, chunking and ranking; verification prompt; the code-enforced rules from section 8; ALREADY\_CHECKED detection by matching claims against existing fact-check titles and summaries, requiring a link.

Acceptance: on the gold set no trap case receives SUPPORTED; every verdict other than UNVERIFIABLE and NOT\_CHECKABLE has stored evidence meeting the forcing rules; agreement with published verdicts is reported (target at least 70 percent recall on CONTRADICTED); the gold set replays offline against recorded HTTP responses.

### Phase 6: Funnel, ranking and clustering

Tasks: screen gates; perceptual hash dedup; claim clustering; lead\_score; lanes; daily cap; decision logging; tune command.

Acceptance: replaying 300 recorded videos yields at most the daily cap in the Leads lane; seeded duplicates cluster correctly; precision at 25 is reported against an owner-labelled fixture.

### Phase 7: Scale hardening

Tasks: durable queue recovery; per-stage AIMD concurrency; provider quota guard with fallback and deferral; circuit breakers; disk janitor; structured logs to file; a local metrics endpoint; backpressure.

Acceptance: killing the process mid-run and restarting resumes without duplicate work; a 300-video soak run completes; quota exhaustion triggers fallback or deferral and is logged; memory and disk stay bounded.

### Phase 8: Triage UI

8a: app shell, inbox list, paste bar, against a mock API. 8b: detail drawer with synced player and evidence. 8c: research-more flow, creators, seeds. 8d: calibration screen, settings, i18n. 8e: keyboard map, undo, batch actions, export.

Acceptance: the owner completes a 50-video triage session using only the keyboard; 2,000 cards render smoothly; every action is reflected within 1 second; the UI works with the core on a different configured core\_url.

### Phase 9: People research, export, packaging

Tasks: research-more job (deeper retrieval, public-figure research, creator history); Markdown, CSV and JSON export of kept leads with transcript and evidence links; Tauri installer; updater; first-run wizard; a short Bulgarian quick-start guide.

Acceptance: the installer on a clean Windows machine reaches a working paste-a-link flow without any API keys.

## 14. Evaluation

Gold set of 30 to 50 Bulgarian claims with known outcomes, drawn from published fact-checks (the owner supplies their team's own, plus public archives), including at least 10 traps:

- fabricated attribution (a speaker says another named person agreed or said something they never did)
- genuine quote used in a false context
- old video presented as new
- satire or parody
- correct statistic attributed to the wrong source
- claim with no retrievable evidence (must be UNVERIFIABLE, not CONTRADICTED)

Metrics: trap pass rate (target 100 percent with zero SUPPORTED), agreement with published verdicts, precision at 25 for the Leads lane, transcript WER on the 20-clip set, schema validity rate, throughput against the calibration estimate.

All tests run offline against recorded fixtures (record and replay HTTP). Add unit tests for parsers, validators, the forcing rules and the rate limiter.

## 15. Repository layout

```text
leadfinder/
  Cargo.toml
  crates/
    core/
    pipeline/
    providers/
    probe/
    cli/
  ui/
  assets/calibration/
  config/
    default.toml
    primary_sources.toml
    seeds.bg.toml
  prompts/
  fixtures/
  docs/
    PLAN.md
    PROGRESS.md
    DECISIONS.md
    OWNER_CHECKLIST.md
    SPIKE_DISCOVERY.md
```

## 16. Config defaults

```toml
[discovery]
region = 'BG'
languages = ['bg', 'en', 'ru', 'tr']
requests_per_minute = 12
breaker_failures = 5
breaker_cooldown_minutes = 30
cache_ttl_hours = 6

[funnel]
min_views = 2000
min_views_per_hour = 100
daily_lead_cap = 25
low_quality_threshold = 0.4
retention_days = 7

[expansion]
min_flagged_videos = 3
window_days = 30
max_videos_per_creator = 20
max_depth = 2

[capacity]
target_videos_per_day = 300
processing_window_hours = 12
avg_video_seconds = 45
overhead_seconds = 10

[providers]
llm_chain = ['local', 'gemini', 'heuristics']
cloud_enabled = false

[verification]
min_independent_news_outlets = 2
passages_per_claim = 8
retrieval_window_days = 30
```

## 17. Risks and fallbacks

| Risk | Mitigation |
| --- | --- |
| Automated discovery is blocked or breaks | Paste-a-link and partner-export import always work; circuit breaker; go/no-go spike in phase 1 |
| Free quota exhausted | Fallback chain, deferral to reset, local models first |
| Local hardware too slow | Calibration picks smaller models, capacity estimate shown, overnight batching |
| Bulgarian speech recognition errors | Quality badge, editable transcript, demotion rather than hiding |
| LLM hallucinated verdicts | Retrieval-grounded, code-enforced evidence rules, UNVERIFIABLE default |
| Wrong person named | No face recognition, public-figure resolution through Wikidata, research only for resolved public figures |
| Terms or legal exposure | Owner and employer confirm TikTok terms risk and data-protection obligations before real use; text-only cloud calls; this document is not legal advice |
| Agent drift between sessions | PROGRESS.md and DECISIONS.md updated every phase |

## 18. Deferred decisions

Decide during implementation and record in docs/DECISIONS.md: OCR engine; Whisper variant and quantisation; which discovery approach survives the spike; which Bulgarian fact-check archives expose usable search or feeds; the latest BgGPT release and its licence; the exact format of the partner export; embedding model for clustering and re-ranking.
