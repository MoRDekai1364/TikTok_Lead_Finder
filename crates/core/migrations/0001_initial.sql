CREATE TABLE IF NOT EXISTS creators (
    id TEXT PRIMARY KEY,
    handle TEXT NOT NULL,
    display_name TEXT,
    region TEXT,
    follower_count INTEGER,
    first_seen_at TEXT NOT NULL,
    watchlist INTEGER NOT NULL DEFAULT 0,
    flagged_count INTEGER NOT NULL DEFAULT 0,
    expansion_depth INTEGER NOT NULL DEFAULT 0,
    risk_assessment_json TEXT
);

CREATE TABLE IF NOT EXISTS videos (
    id TEXT PRIMARY KEY,
    creator_id TEXT REFERENCES creators(id),
    url TEXT NOT NULL,
    caption TEXT,
    hashtags TEXT,
    posted_at TEXT,
    views INTEGER,
    likes INTEGER,
    shares INTEGER,
    comments INTEGER,
    duration_s REAL,
    discovered_via TEXT NOT NULL,
    discovered_at TEXT NOT NULL,
    phash TEXT,
    state TEXT NOT NULL,
    state_reason TEXT,
    media_path TEXT,
    lead_score REAL,
    lane TEXT,
    cluster_id INTEGER
);

CREATE TABLE IF NOT EXISTS transcripts (
    video_id TEXT PRIMARY KEY REFERENCES videos(id),
    source TEXT NOT NULL CHECK (source IN ('found','generated')),
    engine TEXT,
    model TEXT,
    language TEXT,
    quality REAL,
    text TEXT NOT NULL,
    segments_json TEXT,
    edited_text TEXT
);

CREATE TABLE IF NOT EXISTS ocr_texts (
    video_id TEXT REFERENCES videos(id),
    frame_ms INTEGER,
    text TEXT,
    confidence REAL,
    PRIMARY KEY (video_id, frame_ms)
);

CREATE TABLE IF NOT EXISTS claims (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    video_id TEXT NOT NULL REFERENCES videos(id),
    text_bg TEXT NOT NULL,
    claim_type TEXT NOT NULL,
    speaker TEXT,
    attribution_json TEXT,
    start_ms INTEGER,
    end_ms INTEGER,
    checkworthiness REAL,
    verdict TEXT,
    label TEXT,
    reasoning_bg TEXT,
    evidence_strength REAL,
    verified_at TEXT,
    cluster_id INTEGER
);

CREATE TABLE IF NOT EXISTS evidence (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    claim_id INTEGER NOT NULL REFERENCES claims(id),
    passage_id TEXT NOT NULL,
    source_url TEXT NOT NULL,
    source_name TEXT,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('primary','news','factcheck','other')),
    published_at TEXT,
    passage TEXT NOT NULL,
    stance TEXT,
    retrieved_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS people (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    canonical_name TEXT NOT NULL,
    variants_json TEXT,
    role TEXT,
    public_figure INTEGER NOT NULL DEFAULT 0,
    wikidata_id TEXT,
    summary TEXT,
    researched_at TEXT
);

CREATE TABLE IF NOT EXISTS video_people (
    video_id TEXT REFERENCES videos(id),
    person_id INTEGER REFERENCES people(id),
    mentions INTEGER NOT NULL DEFAULT 1,
    found_in TEXT,
    PRIMARY KEY (video_id, person_id)
);

CREATE TABLE IF NOT EXISTS decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    video_id TEXT NOT NULL REFERENCES videos(id),
    action TEXT NOT NULL CHECK (action IN ('keep','discard','research_more','undo')),
    note TEXT,
    decided_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS seeds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL CHECK (kind IN ('keyword','hashtag','creator')),
    value TEXT NOT NULL,
    topic TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    last_run_at TEXT,
    UNIQUE (kind, value)
);

CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    video_id TEXT REFERENCES videos(id),
    stage TEXT NOT NULL,
    status TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    run_after TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_usage (
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    day TEXT NOT NULL,
    requests INTEGER NOT NULL DEFAULT 0,
    tokens_in INTEGER NOT NULL DEFAULT 0,
    tokens_out INTEGER NOT NULL DEFAULT 0,
    errors_429 INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (provider, model, day)
);

CREATE TABLE IF NOT EXISTS profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    hardware_json TEXT NOT NULL,
    chosen_json TEXT NOT NULL,
    capacity_per_day REAL,
    measured_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_status_stage_run_after ON jobs(status, stage, run_after);
CREATE INDEX IF NOT EXISTS idx_jobs_video_stage ON jobs(video_id, stage);
CREATE INDEX IF NOT EXISTS idx_videos_state_lane_score ON videos(state, lane, lead_score DESC);
CREATE INDEX IF NOT EXISTS idx_videos_creator_id ON videos(creator_id);
CREATE INDEX IF NOT EXISTS idx_videos_phash ON videos(phash);
CREATE INDEX IF NOT EXISTS idx_claims_video_id ON claims(video_id);
CREATE INDEX IF NOT EXISTS idx_claims_cluster_id ON claims(cluster_id);
CREATE INDEX IF NOT EXISTS idx_evidence_claim_id ON evidence(claim_id);
CREATE INDEX IF NOT EXISTS idx_decisions_video_decided ON decisions(video_id, decided_at DESC);
