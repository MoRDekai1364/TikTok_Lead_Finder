use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub discovery: DiscoveryConfig,
    pub extraction: ExtractionConfig,
    pub funnel: FunnelConfig,
    pub ranking: RankingConfig,
    pub normalisation: NormalisationConfig,
    pub expansion: ExpansionConfig,
    pub capacity: CapacityConfig,
    pub providers: ProvidersConfig,
    pub verification: VerificationConfig,
    #[serde(default)]
    pub claim_quality: ClaimQualityConfig,
    #[serde(default)]
    pub factcheck_archives: Vec<ArchiveEndpointConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimQualityConfig {
    #[serde(default = "default_min_words")]
    pub min_words: usize,
    #[serde(default = "default_min_alpha_ratio")]
    pub min_alpha_ratio: f64,
    #[serde(default = "default_min_script_share")]
    pub min_script_share: f64,
    #[serde(default)]
    pub script_ranges: Vec<[u32; 2]>,
    #[serde(default)]
    pub generic_actor_terms: Vec<String>,
    #[serde(default = "default_min_known_word_share")]
    pub min_known_word_share: f64,
    #[serde(default = "default_max_foreign_token_share")]
    pub max_foreign_token_share: f64,
    #[serde(default = "default_max_short_token_share")]
    pub max_short_token_share: f64,
    #[serde(default)]
    pub reject_questions: bool,
    #[serde(default)]
    pub rejected_terms: Vec<String>,
}

impl Default for ClaimQualityConfig {
    fn default() -> Self {
        Self {
            min_words: default_min_words(),
            min_alpha_ratio: default_min_alpha_ratio(),
            min_script_share: default_min_script_share(),
            script_ranges: Vec::new(),
            generic_actor_terms: Vec::new(),
            min_known_word_share: default_min_known_word_share(),
            max_foreign_token_share: default_max_foreign_token_share(),
            max_short_token_share: default_max_short_token_share(),
            reject_questions: false,
            rejected_terms: Vec::new(),
        }
    }
}

fn default_min_words() -> usize {
    4
}

fn default_min_alpha_ratio() -> f64 {
    0.75
}

fn default_max_foreign_token_share() -> f64 {
    1.0
}

fn default_max_short_token_share() -> f64 {
    1.0
}

fn default_min_known_word_share() -> f64 {
    0.0
}

fn default_min_script_share() -> f64 {
    0.6
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: String,
    pub db_path: String,
    pub log_dir: String,
    pub cache_dir: String,
    pub prompts_dir: String,
    pub assets_dir: String,
    pub tools_dir: String,
    pub models_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub region: String,
    pub languages: Vec<String>,
    pub requests_per_minute: u32,
    pub breaker_failures: u32,
    pub breaker_cooldown_minutes: u64,
    pub cache_ttl_hours: u64,
    #[serde(default)]
    pub automated_discovery_enabled: bool,
    #[serde(default = "default_yt_dlp_timeout")]
    pub yt_dlp_timeout_seconds: u64,
}

fn default_yt_dlp_timeout() -> u64 {
    45
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionConfig {
    pub ffmpeg_timeout_seconds: u64,
    pub whisper_timeout_seconds: u64,
    pub ocr_timeout_seconds: u64,
    pub keyframe_interval_seconds: u64,
    pub max_keyframes: usize,
    pub ocr_languages: Vec<String>,
    pub ocr_dedup_similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelConfig {
    pub min_views: i64,
    pub min_views_per_hour: f64,
    pub daily_lead_cap: usize,
    pub low_quality_threshold: f64,
    pub retention_days: u64,
    #[serde(default = "default_daily_llm_budget")]
    pub daily_llm_budget: u64,
    #[serde(default = "default_daily_search_budget")]
    pub daily_search_budget: u64,
    #[serde(default = "default_cluster_threshold")]
    pub cluster_similarity_threshold: f64,
    #[serde(default = "default_virality_saturation_views")]
    pub virality_saturation_views: i64,
    #[serde(default = "default_phash_max_hamming")]
    pub phash_max_hamming: u32,
    #[serde(default = "default_tune_min_decisions")]
    pub tune_min_decisions: usize,
    #[serde(default)]
    pub bypass_sources: Vec<String>,
}

fn default_virality_saturation_views() -> i64 {
    1_000_000
}

fn default_phash_max_hamming() -> u32 {
    6
}

fn default_tune_min_decisions() -> usize {
    200
}

fn default_daily_llm_budget() -> u64 {
    250
}

fn default_daily_search_budget() -> u64 {
    500
}

fn default_cluster_threshold() -> f64 {
    0.55
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingConfig {
    pub w_check: f64,
    pub w_viral: f64,
    pub w_verdict: f64,
    pub w_q: f64,
    pub checkworthy_threshold: f64,
    pub weight_llm_score: f64,
    pub weight_contains_numbers: f64,
    pub weight_public_figure: f64,
    pub weight_sensitive_topic: f64,
    pub weight_urges_sharing: f64,
    #[serde(default)]
    pub sensitive_topic_terms: Vec<String>,
    #[serde(default)]
    pub share_urge_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalisationConfig {
    #[serde(default = "default_lexicon_config_file")]
    pub lexicon_config_file: String,
    #[serde(default)]
    pub wikidata_languages: Vec<String>,
    #[serde(default)]
    pub definite_suffixes: Vec<String>,
    #[serde(default)]
    pub role_prefixes: Vec<String>,
    #[serde(default)]
    pub surname_suffixes: Vec<String>,
}

fn default_lexicon_config_file() -> String {
    "lexicon.bg.toml".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionConfig {
    pub min_flagged_videos: i64,
    pub window_days: i64,
    pub max_videos_per_creator: usize,
    pub max_depth: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityConfig {
    pub target_videos_per_day: u64,
    pub processing_window_hours: f64,
    pub avg_video_seconds: f64,
    pub overhead_seconds: f64,
    #[serde(default = "default_wer_ceiling")]
    pub max_wer_ceiling: f64,
    #[serde(default = "default_min_tps")]
    pub min_llm_tokens_per_second: f64,
    #[serde(default = "default_min_json_validity")]
    pub min_llm_json_validity: f64,
}

fn default_wer_ceiling() -> f64 {
    0.40
}

fn default_min_tps() -> f64 {
    8.0
}

fn default_min_json_validity() -> f64 {
    0.95
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersConfig {
    pub llm_chain: Vec<String>,
    pub cloud_enabled: bool,
    pub local_llm_url: String,
    pub local_llm_model: String,
    pub gemini_model: String,
    pub transcriber_engine: String,
    pub whisper_model: String,
    pub ocr_engine: String,
    #[serde(default)]
    pub searxng_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    pub min_independent_news_outlets: usize,
    pub passages_per_claim: usize,
    pub retrieval_window_days: i64,
    #[serde(default = "default_already_checked_sim")]
    pub already_checked_similarity: f64,
    #[serde(default)]
    pub news_domains: Vec<String>,
}

fn default_already_checked_sim() -> f64 {
    0.60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEndpointConfig {
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub feed_url: String,
    #[serde(default)]
    pub sitemap_url: String,
    #[serde(default)]
    pub search_url_template: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimarySourceEntry {
    pub name: String,
    pub domain: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimarySourcesFile {
    #[serde(default)]
    pub sources: Vec<PrimarySourceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedEntry {
    pub kind: String,
    pub value: String,
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedsFile {
    #[serde(default)]
    pub seeds: Vec<SeedEntry>,
}

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub static_root: PathBuf,
    pub mutable_root: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub log_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub prompts_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub tools_dir: PathBuf,
    pub bundled_tools_dir: PathBuf,
    pub models_dir: PathBuf,
}

impl RuntimePaths {
    pub fn discover() -> Result<Self> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| cwd.clone());

        let find_root = |start: &Path| -> Option<PathBuf> {
            start
                .ancestors()
                .take(5)
                .find(|p| p.join("config").join("default.toml").exists())
                .map(|p| p.to_path_buf())
        };

        let found_root = find_root(&cwd).or_else(|| find_root(&exe_dir));
        let is_workspace = found_root
            .as_ref()
            .map(|r| r.join("Cargo.toml").exists())
            .unwrap_or(false)
            || std::env::var("LEADFINDER_PORTABLE").ok().as_deref() == Some("1");

        let static_root = found_root.unwrap_or_else(|| cwd.clone());

        let mutable_root = if let Ok(custom) = std::env::var("LEADFINDER_APP_DATA_DIR") {
            PathBuf::from(custom)
        } else if is_workspace {
            static_root.clone()
        } else if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(local_app_data).join("TikTok_Lead_Finder")
        } else {
            static_root.clone()
        };

        let config_dir = std::env::var("LEADFINDER_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| static_root.join("config"));
        let data_dir = std::env::var("LEADFINDER_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| mutable_root.join("data"));
        let db_path = std::env::var("LEADFINDER_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| data_dir.join("leadfinder.db"));
        let log_dir = mutable_root.join("logs");
        let cache_dir = mutable_root.join("cache");
        let prompts_dir = static_root.join("prompts");
        let assets_dir = static_root.join("assets");
        let tools_dir = std::env::var("LEADFINDER_TOOLS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| mutable_root.join("tools"));
        let bundled_tools_dir = static_root.join("tools");
        let models_dir = std::env::var("LEADFINDER_MODELS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| mutable_root.join("models"));

        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&log_dir)?;
        std::fs::create_dir_all(&cache_dir)?;
        std::fs::create_dir_all(&tools_dir)?;
        std::fs::create_dir_all(&models_dir)?;

        Ok(Self {
            static_root,
            mutable_root,
            config_dir,
            data_dir,
            db_path,
            log_dir,
            cache_dir,
            prompts_dir,
            assets_dir,
            tools_dir,
            bundled_tools_dir,
            models_dir,
        })
    }

    pub fn resolve_binary(&self, binary_stem: &str) -> Option<PathBuf> {
        let exe_name = if cfg!(windows) && !binary_stem.ends_with(".exe") {
            format!("{binary_stem}.exe")
        } else {
            binary_stem.to_string()
        };

        let direct_candidates = [
            self.tools_dir.join(&exe_name),
            self.tools_dir.join(binary_stem).join(&exe_name),
            self.bundled_tools_dir.join(&exe_name),
            self.bundled_tools_dir.join(binary_stem).join(&exe_name),
        ];
        for candidate in direct_candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        if let Ok(path_var) = std::env::var("PATH") {
            for entry in std::env::split_paths(&path_var) {
                let candidate = entry.join(&exe_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }

        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_link = PathBuf::from(&local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join(&exe_name);
            if winget_link.is_file() {
                return Some(winget_link);
            }
        }

        if binary_stem == "tesseract" {
            let pf = PathBuf::from(r"C:\Program Files\Tesseract-OCR\tesseract.exe");
            if pf.is_file() {
                return Some(pf);
            }
        }

        None
    }

    pub fn resolve_model_path(&self, rel: &str) -> Option<PathBuf> {
        let candidates = [
            self.models_dir.join(rel),
            self.static_root.join("models").join(rel),
            self.assets_dir.join(rel),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }
}

pub fn load_app_config(config_dir: &Path) -> Result<AppConfig> {
    let default_path = config_dir.join("default.toml");
    let raw = std::fs::read_to_string(&default_path)
        .with_context(|| format!("failed reading {}", default_path.display()))?;
    let mut cfg: AppConfig = toml::from_str(&raw)
        .with_context(|| format!("failed parsing {}", default_path.display()))?;

    if let Ok(host) = std::env::var("LEADFINDER_HOST") {
        if !host.trim().is_empty() {
            cfg.server.host = host;
        }
    }
    if let Ok(port_str) = std::env::var("LEADFINDER_PORT") {
        if let Ok(port) = port_str.parse::<u16>() {
            cfg.server.port = port;
        }
    }
    if let Ok(llama_url) = std::env::var("LEADFINDER_LLAMA_URL") {
        if !llama_url.trim().is_empty() {
            cfg.providers.local_llm_url = llama_url;
        }
    }
    if let Ok(searxng_url) = std::env::var("LEADFINDER_SEARXNG_URL") {
        if !searxng_url.trim().is_empty() {
            cfg.providers.searxng_url = searxng_url;
        }
    }

    Ok(cfg)
}

pub fn load_primary_sources(config_dir: &Path) -> Result<Vec<PrimarySourceEntry>> {
    let path = config_dir.join("primary_sources.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)?;
    let parsed: PrimarySourcesFile = toml::from_str(&raw)?;
    Ok(parsed.sources)
}

pub fn load_seed_entries(config_dir: &Path) -> Result<Vec<SeedEntry>> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(config_dir) {
        for item in read_dir.flatten() {
            let path = item.path();
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.starts_with("seeds.") && name.ends_with(".toml") {
                let raw = std::fs::read_to_string(&path)?;
                let parsed: SeedsFile = toml::from_str(&raw)?;
                entries.extend(parsed.seeds);
            }
        }
    }
    Ok(entries)
}

pub fn resolve_secret(key_name: &str) -> Option<String> {
    if let Ok(val) = std::env::var(key_name) {
        let trimmed = val.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    if let Ok(entry) = keyring::Entry::new("leadfinder", key_name) {
        if let Ok(secret) = entry.get_password() {
            let trimmed = secret.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

pub fn store_secret_in_os_credential_store(key_name: &str, value: &str) -> Result<()> {
    let entry = keyring::Entry::new("leadfinder", key_name)
        .map_err(|e| anyhow::anyhow!("credential store init error: {e}"))?;
    if value.trim().is_empty() {
        let _ = entry.delete_credential();
    } else {
        entry
            .set_password(value.trim())
            .map_err(|e| anyhow::anyhow!("credential store write error: {e}"))?;
    }
    Ok(())
}

pub fn resolve_or_create_api_token(data_dir: &Path) -> Result<String> {
    if let Some(secret) = resolve_secret("LEADFINDER_API_TOKEN") {
        return Ok(secret);
    }
    let token_path = data_dir.join("api_token");
    if token_path.is_file() {
        let existing = std::fs::read_to_string(&token_path)?.trim().to_string();
        if !existing.is_empty() {
            return Ok(existing);
        }
    }
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    let generated = hex::encode(bytes);
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(&token_path, &generated)?;
    Ok(generated)
}
