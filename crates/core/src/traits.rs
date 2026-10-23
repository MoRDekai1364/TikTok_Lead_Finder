use std::path::PathBuf;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::models::{TranscriptSegment, VideoRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    Timeout,
    RateLimited,
    BlockedOrCaptcha,
    CircuitOpen,
    NotFoundOrPrivate,
    GeoRestricted,
    NetworkError,
    ParseError,
    BinaryMissing,
    QuotaExhausted,
    InvalidOutput,
    Unsupported,
    Internal,
}

#[derive(Debug, Error, Clone, Serialize, Deserialize)]
#[error("[{kind:?}] {provider}: {message}")]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub provider: String,
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, provider: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            kind,
            provider: provider.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryQuery {
    PasteText { raw_text: String },
    PartnerImport { file_path: PathBuf },
    PartnerImportContent { content: String, source_label: String },
    Keyword { keyword: String, max_results: usize },
    Hashtag { tag: String, max_results: usize },
    Creator { handle: String, max_results: usize, expansion_depth: i64 },
}

#[async_trait]
pub trait VideoSource: Send + Sync {
    fn name(&self) -> &str;
    async fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<VideoRef>, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct TranscribeInput {
    pub video_id: String,
    pub audio_wav_path: Option<PathBuf>,
    pub caption_tracks: Vec<(String, PathBuf)>,
    pub expected_languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRecord {
    pub source: String,
    pub engine: String,
    pub model: String,
    pub language: String,
    pub quality: f64,
    pub text: String,
    pub segments: Vec<TranscriptSegment>,
}

#[async_trait]
pub trait Transcriber: Send + Sync {
    fn name(&self) -> &str;
    async fn transcribe(&self, input: &TranscribeInput) -> Result<TranscriptRecord, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct FrameSample {
    pub frame_ms: i64,
    pub image_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrLine {
    pub frame_ms: i64,
    pub text: String,
    pub confidence: f64,
}

#[async_trait]
pub trait Ocr: Send + Sync {
    fn name(&self) -> &str;
    async fn extract_frames(&self, frames: &[FrameSample]) -> Result<Vec<OcrLine>, ProviderError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmJsonRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub json_schema: Option<serde_json::Value>,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmJsonResponse {
    pub provider: String,
    pub model: String,
    pub raw_text: String,
    pub parsed_json: serde_json::Value,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

#[async_trait]
pub trait Llm: Send + Sync {
    fn name(&self) -> &str;
    fn model_name(&self) -> &str;
    async fn complete_json(&self, req: &LlmJsonRequest) -> Result<LlmJsonResponse, ProviderError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub restrict_domains: Vec<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub max_results: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub source_name: Option<String>,
    pub source_kind: String,
    pub published_at: Option<String>,
}

#[async_trait]
pub trait Search: Send + Sync {
    fn name(&self) -> &str;
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, ProviderError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMatch {
    pub archive_name: String,
    pub url: String,
    pub title: String,
    pub summary: String,
    pub verdict_text: Option<String>,
    pub published_at: Option<String>,
    pub similarity: f64,
}

#[async_trait]
pub trait FactCheckArchive: Send + Sync {
    fn name(&self) -> &str;
    async fn find_fact_checks(&self, claim_text: &str) -> Result<Vec<ArchiveMatch>, ProviderError>;
}
