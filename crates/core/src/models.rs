use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoState {
    #[serde(rename = "DISCOVERED")]
    Discovered,
    #[serde(rename = "SCREENED_OUT")]
    ScreenedOut,
    #[serde(rename = "ACQUIRED")]
    Acquired,
    #[serde(rename = "EXTRACTED")]
    Extracted,
    #[serde(rename = "CLAIMS_DONE")]
    ClaimsDone,
    #[serde(rename = "VERIFIED")]
    Verified,
    #[serde(rename = "READY")]
    Ready,
    #[serde(rename = "KEPT")]
    Kept,
    #[serde(rename = "DISCARDED")]
    Discarded,
    #[serde(rename = "FAILED")]
    Failed,
}

impl VideoState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Discovered => "DISCOVERED",
            Self::ScreenedOut => "SCREENED_OUT",
            Self::Acquired => "ACQUIRED",
            Self::Extracted => "EXTRACTED",
            Self::ClaimsDone => "CLAIMS_DONE",
            Self::Verified => "VERIFIED",
            Self::Ready => "READY",
            Self::Kept => "KEPT",
            Self::Discarded => "DISCARDED",
            Self::Failed => "FAILED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "DISCOVERED" => Self::Discovered,
            "SCREENED_OUT" => Self::ScreenedOut,
            "ACQUIRED" => Self::Acquired,
            "EXTRACTED" => Self::Extracted,
            "CLAIMS_DONE" => Self::ClaimsDone,
            "VERIFIED" => Self::Verified,
            "READY" => Self::Ready,
            "KEPT" => Self::Kept,
            "DISCARDED" => Self::Discarded,
            _ => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    #[serde(rename = "DISCOVER")]
    Discover,
    #[serde(rename = "SCREEN_CHEAP")]
    ScreenCheap,
    #[serde(rename = "ACQUIRE")]
    Acquire,
    #[serde(rename = "EXTRACT")]
    Extract,
    #[serde(rename = "CLAIMS")]
    Claims,
    #[serde(rename = "CHECKWORTHY")]
    Checkworthy,
    #[serde(rename = "VERIFY")]
    Verify,
    #[serde(rename = "RANK")]
    Rank,
    #[serde(rename = "DECISION")]
    Decision,
    #[serde(rename = "RESEARCH_MORE")]
    ResearchMore,
}

impl Stage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Discover => "DISCOVER",
            Self::ScreenCheap => "SCREEN_CHEAP",
            Self::Acquire => "ACQUIRE",
            Self::Extract => "EXTRACT",
            Self::Claims => "CLAIMS",
            Self::Checkworthy => "CHECKWORTHY",
            Self::Verify => "VERIFY",
            Self::Rank => "RANK",
            Self::Decision => "DECISION",
            Self::ResearchMore => "RESEARCH_MORE",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "DISCOVER" => Some(Self::Discover),
            "SCREEN_CHEAP" => Some(Self::ScreenCheap),
            "ACQUIRE" => Some(Self::Acquire),
            "EXTRACT" => Some(Self::Extract),
            "CLAIMS" => Some(Self::Claims),
            "CHECKWORTHY" => Some(Self::Checkworthy),
            "VERIFY" => Some(Self::Verify),
            "RANK" => Some(Self::Rank),
            "DECISION" => Some(Self::Decision),
            "RESEARCH_MORE" => Some(Self::ResearchMore),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Leads,
    AlreadyChecked,
    LowQuality,
    Failed,
    Pasted,
}

impl Lane {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Leads => "leads",
            Self::AlreadyChecked => "already_checked",
            Self::LowQuality => "low_quality",
            Self::Failed => "failed",
            Self::Pasted => "pasted",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "leads" => Some(Self::Leads),
            "already_checked" => Some(Self::AlreadyChecked),
            "low_quality" => Some(Self::LowQuality),
            "failed" => Some(Self::Failed),
            "pasted" => Some(Self::Pasted),
            _ => None,
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    SUPPORTED,
    CONTRADICTED,
    UNVERIFIABLE,
    NOT_CHECKABLE,
    ALREADY_CHECKED,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SUPPORTED => "SUPPORTED",
            Self::CONTRADICTED => "CONTRADICTED",
            Self::UNVERIFIABLE => "UNVERIFIABLE",
            Self::NOT_CHECKABLE => "NOT_CHECKABLE",
            Self::ALREADY_CHECKED => "ALREADY_CHECKED",
        }
    }

    pub fn ranking_value(&self) -> f64 {
        match self {
            Self::CONTRADICTED => 1.0,
            Self::UNVERIFIABLE => 0.6,
            Self::NOT_CHECKABLE => 0.1,
            Self::SUPPORTED => 0.0,
            Self::ALREADY_CHECKED => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimLabel {
    FalseClaim,
    Misleading,
    MissingContext,
    SatireOrOpinion,
    Unknown,
}

impl ClaimLabel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FalseClaim => "false_claim",
            Self::Misleading => "misleading",
            Self::MissingContext => "missing_context",
            Self::SatireOrOpinion => "satire_or_opinion",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimType {
    Factual,
    Attribution,
    Statistical,
    Health,
    Quote,
    Prediction,
    Opinion,
}

impl ClaimType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Factual => "factual",
            Self::Attribution => "attribution",
            Self::Statistical => "statistical",
            Self::Health => "health",
            Self::Quote => "quote",
            Self::Prediction => "prediction",
            Self::Opinion => "opinion",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creator {
    pub id: String,
    pub handle: String,
    pub display_name: Option<String>,
    pub region: Option<String>,
    pub follower_count: Option<i64>,
    pub first_seen_at: String,
    pub watchlist: bool,
    pub flagged_count: i64,
    pub expansion_depth: i64,
    #[serde(default)]
    pub risk_assessment_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorRiskAssessment {
    pub risk_level: String,
    pub risk_score: f64,
    pub confidence: f64,
    pub confidence_level: String,
    pub confidence_reason: String,
    pub rationale: String,
    pub web_sources: Vec<CreatorWebSource>,
    pub local_stats: CreatorLocalStats,
    pub last_calculated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorWebSource {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorLocalStats {
    pub total_videos: i64,
    pub flagged_videos: i64,
    pub contradicted_claims: i64,
    pub supported_claims: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoRef {
    pub id: String,
    pub url: String,
    pub creator_id: Option<String>,
    pub creator_handle: Option<String>,
    pub creator_display_name: Option<String>,
    pub creator_follower_count: Option<i64>,
    pub creator_region: Option<String>,
    pub caption: Option<String>,
    pub hashtags: Vec<String>,
    pub posted_at: Option<String>,
    pub views: Option<i64>,
    pub likes: Option<i64>,
    pub shares: Option<i64>,
    pub comments: Option<i64>,
    pub duration_s: Option<f64>,
    pub sound_title: Option<String>,
    pub subtitle_urls: Vec<SubtitleTrackRef>,
    pub discovered_via: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleTrackRef {
    pub language: String,
    pub ext: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub creator_id: Option<String>,
    pub creator_handle: Option<String>,
    pub creator_display_name: Option<String>,
    pub url: String,
    pub caption: Option<String>,
    pub hashtags: Vec<String>,
    pub posted_at: Option<String>,
    pub views: Option<i64>,
    pub likes: Option<i64>,
    pub shares: Option<i64>,
    pub comments: Option<i64>,
    pub duration_s: Option<f64>,
    pub discovered_via: String,
    pub discovered_at: String,
    pub phash: Option<String>,
    pub state: String,
    pub state_reason: Option<String>,
    pub media_path: Option<String>,
    pub lead_score: Option<f64>,
    pub lane: Option<String>,
    pub cluster_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub video_id: String,
    pub source: String,
    pub engine: Option<String>,
    pub model: Option<String>,
    pub language: Option<String>,
    pub quality: Option<f64>,
    pub text: String,
    pub segments: Vec<TranscriptSegment>,
    pub edited_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrText {
    pub video_id: String,
    pub frame_ms: i64,
    pub text: String,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributionAct {
    pub actor: String,
    pub act: String,
    pub target: Option<String>,
    pub about: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asserted_part: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributed_part: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimInput {
    pub text_bg: String,
    pub claim_type: ClaimType,
    pub speaker: Option<String>,
    #[serde(default)]
    pub attribution: Vec<AttributionAct>,
    pub start_ms: i64,
    pub end_ms: i64,
    pub checkworthiness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonInput {
    pub name: String,
    #[serde(default)]
    pub variants: Vec<String>,
    pub role_hint: Option<String>,
    #[serde(default)]
    pub entity_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimsOutput {
    pub claims: Vec<ClaimInput>,
    #[serde(default)]
    pub people: Vec<PersonInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: i64,
    pub video_id: String,
    pub text_bg: String,
    pub claim_type: String,
    pub speaker: Option<String>,
    pub attribution: Vec<AttributionAct>,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub checkworthiness: Option<f64>,
    pub verdict: Option<String>,
    pub label: Option<String>,
    pub reasoning_bg: Option<String>,
    pub evidence_strength: Option<f64>,
    pub verified_at: Option<String>,
    pub cluster_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceStanceRef {
    pub passage_id: String,
    pub stance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationOutput {
    pub claim_id: i64,
    pub verdict: Verdict,
    pub label: ClaimLabel,
    pub evidence: Vec<EvidenceStanceRef>,
    pub reasoning_bg: String,
    pub evidence_strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: i64,
    pub claim_id: i64,
    pub passage_id: String,
    pub source_url: String,
    pub source_name: Option<String>,
    pub source_kind: String,
    pub published_at: Option<String>,
    pub passage: String,
    pub stance: Option<String>,
    pub retrieved_at: String,
}

fn default_person_rating() -> i64 {
    50
}

fn default_entity_type() -> String {
    "person".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: i64,
    pub canonical_name: String,
    pub variants: Vec<String>,
    pub role: Option<String>,
    pub public_figure: bool,
    pub wikidata_id: Option<String>,
    pub summary: Option<String>,
    pub researched_at: Option<String>,
    pub mentions: Option<i64>,
    pub found_in: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_person_rating")]
    pub rating: i64,
    pub category: Option<String>,
    pub notes: Option<String>,
    pub detailed_profile_json: Option<String>,
    #[serde(default = "default_entity_type")]
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: i64,
    pub video_id: String,
    pub action: String,
    pub note: Option<String>,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seed {
    pub id: i64,
    pub kind: String,
    pub value: String,
    pub topic: Option<String>,
    pub active: bool,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: i64,
    pub video_id: Option<String>,
    pub stage: String,
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub run_after: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub hardware_json: serde_json::Value,
    pub chosen_json: serde_json::Value,
    pub capacity_per_day: Option<f64>,
    pub measured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadCardSummary {
    pub video: Video,
    pub top_claim_bg: Option<String>,
    pub top_verdict: Option<String>,
    pub top_label: Option<String>,
    pub transcript_quality: Option<f64>,
    pub transcript_source: Option<String>,
    pub max_checkworthiness: Option<f64>,
    pub max_evidence_strength: Option<f64>,
    pub cluster_size: i64,
    pub cluster_total_views: i64,
    pub research_running: bool,
    pub reason_flagged: String,
    #[serde(default)]
    pub progress_pct: Option<i64>,
    #[serde(default)]
    pub etr_seconds: Option<i64>,
    #[serde(default)]
    pub stage_desc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDetail {
    pub video: Video,
    pub creator: Option<Creator>,
    pub transcript: Option<Transcript>,
    pub ocr_texts: Vec<OcrText>,
    pub claims: Vec<ClaimWithEvidence>,
    pub people: Vec<Person>,
    pub decisions: Vec<Decision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsageSummary {
    pub provider: String,
    pub model: String,
    pub day: String,
    pub requests: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub errors_429: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimWithEvidence {
    pub claim: Claim,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum CoreEvent {
    VideoUpdated { video_id: String, state: String, lane: Option<String> },
    JobUpdated { job_id: i64, video_id: Option<String>, stage: String, status: String },
    DiscoveryStatus { paused: bool, reason: Option<String>, active_breakers: Vec<String> },
    DecisionLogged { video_id: String, action: String },
    CalibrationUpdated { capacity_per_day: Option<f64> },
    BenchmarkProgress(serde_json::Value),
    PowerChanged(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoleConfig {
    pub role: String,
    pub provider: String,
    pub model_name: String,
    #[serde(default)]
    pub custom_url: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_consensus_threshold")]
    pub threshold: f64,
    #[serde(default = "default_secondary_provider")]
    pub secondary_provider: String,
    #[serde(default = "default_secondary_model")]
    pub secondary_model: String,
    #[serde(default)]
    pub secondary_custom_url: Option<String>,
}

fn default_consensus_threshold() -> f64 {
    0.70
}

fn default_secondary_provider() -> String {
    "gemini".to_string()
}

fn default_secondary_model() -> String {
    "gemini-2.5-flash".to_string()
}

impl Default for ConsensusSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.70,
            secondary_provider: "gemini".to_string(),
            secondary_model: "gemini-2.5-flash".to_string(),
            secondary_custom_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelHubRolesConfig {
    pub roles: std::collections::HashMap<String, ModelRoleConfig>,
    #[serde(default)]
    pub consensus: ConsensusSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModelInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub filename: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub modified_at: String,
    pub recommended_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuratedModelCatalogItem {
    pub id: String,
    pub name: String,
    pub author: String,
    pub kind: String,
    pub download_url: String,
    pub filename: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub vram_estimate: String,
    pub description_bg: String,
    pub description_en: String,
    pub recommended_for: Vec<String>,
    pub is_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelDownloadProgress {
    pub active: bool,
    pub model_id: String,
    pub filename: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub progress_pct: u8,
    pub speed_bps: u64,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAiServerModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parameter_size: Option<String>,
    #[serde(default)]
    pub quantization: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAiServerStatus {
    pub server_type: String,
    pub endpoint_url: String,
    pub is_online: bool,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    pub models: Vec<ExternalAiServerModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTestRequest {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub custom_url: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTestResponse {
    pub success: bool,
    pub elapsed_ms: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub tokens_per_second: f64,
    pub raw_text: String,
    pub parsed_json: Option<serde_json::Value>,
    pub error: Option<String>,
}
