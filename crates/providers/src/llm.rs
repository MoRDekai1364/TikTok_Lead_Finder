use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use leadfinder_core::{
    resolve_secret, AppConfig, Database, Llm, LlmJsonRequest, LlmJsonResponse, ProviderError,
    ProviderErrorKind, RuntimePaths,
};

struct ManagedLlamaInstance {
    port: u16,
    model_path: PathBuf,
    use_gpu: bool,
    child: Child,
}

static MANAGED_LLAMA: OnceLock<Arc<Mutex<Option<ManagedLlamaInstance>>>> = OnceLock::new();

fn managed_llama_slot() -> Arc<Mutex<Option<ManagedLlamaInstance>>> {
    MANAGED_LLAMA
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone()
}

pub async fn stop_managed_llama_server() {
    let slot = managed_llama_slot();
    let mut guard = slot.lock().await;
    if let Some(mut inst) = guard.take() {
        let _ = inst.child.kill().await;
    }
}

#[derive(Debug, Clone)]
pub struct LocalLlamaLaunchSpec {
    pub llama_bin: PathBuf,
    pub model_path: PathBuf,
    pub use_gpu: bool,
    pub context_size: u32,
}

pub struct OpenAiCompatLlm {
    provider_label: String,
    base_url: String,
    model: String,
    api_key: Option<String>,
    client: Client,
    auto_launch: Option<LocalLlamaLaunchSpec>,
}

impl OpenAiCompatLlm {
    pub fn new(
        provider_label: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
        timeout: Duration,
    ) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            provider_label: provider_label.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            api_key,
            client,
            auto_launch: None,
        }
    }

    pub fn with_local_autolaunch(mut self, spec: LocalLlamaLaunchSpec) -> Self {
        self.auto_launch = Some(spec);
        self
    }

    async fn ensure_server_ready(&self) -> Result<(), ProviderError> {
        let Some(ref spec) = self.auto_launch else {
            return Ok(());
        };
        if !spec.llama_bin.is_file() {
            return Err(ProviderError::new(
                ProviderErrorKind::BinaryMissing,
                &self.provider_label,
                format!("llama-server binary missing at {}", spec.llama_bin.display()),
                false,
            ));
        }
        if !spec.model_path.is_file() {
            return Err(ProviderError::new(
                ProviderErrorKind::BinaryMissing,
                &self.provider_label,
                format!("GGUF model missing at {}", spec.model_path.display()),
                false,
            ));
        }

        let parsed_url = reqwest::Url::parse(&self.base_url).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::Internal,
                &self.provider_label,
                format!("invalid local_llm_url {}: {e}", self.base_url),
                false,
            )
        })?;
        let host = parsed_url.host_str().unwrap_or("127.0.0.1");
        if host != "127.0.0.1" && host != "localhost" {
            return Ok(());
        }
        let port = parsed_url.port_or_known_default().unwrap_or(18080);

        let slot = managed_llama_slot();
        let mut guard = slot.lock().await;

        let need_restart = match guard.as_mut() {
            Some(existing) => {
                let exited = existing.child.try_wait().ok().flatten().is_some();
                exited
                    || existing.port != port
                    || existing.model_path != spec.model_path
                    || existing.use_gpu != spec.use_gpu
            }
            None => false,
        };

        if need_restart {
            if let Some(mut old) = guard.take() {
                let _ = old.child.kill().await;
            }
        }

        if let Ok(resp) = self
            .client
            .get(format!("{}/health", self.base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            if resp.status().is_success() {
                return Ok(());
            }
        }

        if let Some(mut dead) = guard.take() {
            let _ = dead.child.kill().await;
        }

        let ngl = if spec.use_gpu { "99" } else { "0" };
        let threads = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(4, 16))
            .unwrap_or(8)
            .to_string();

        let mut cmd = Command::new(&spec.llama_bin);
        cmd.args([
            "-m",
            &spec.model_path.to_string_lossy(),
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "-ngl",
            ngl,
            "-c",
            &spec.context_size.to_string(),
            "-t",
            &threads,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd.spawn().map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::Internal,
                &self.provider_label,
                format!("failed spawning llama-server: {e}"),
                true,
            )
        })?;

        let mut ready = false;
        for _ in 0..100 {
            if let Ok(resp) = self
                .client
                .get(format!("{}/health", self.base_url))
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                if resp.status().is_success() {
                    ready = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        if !ready {
            let _ = child.kill().await;
            return Err(ProviderError::new(
                ProviderErrorKind::Timeout,
                &self.provider_label,
                "llama-server failed health check within 50s",
                true,
            ));
        }

        *guard = Some(ManagedLlamaInstance {
            port,
            model_path: spec.model_path.clone(),
            use_gpu: spec.use_gpu,
            child,
        });

        Ok(())
    }

    async fn send_completion(
        &self,
        req: &LlmJsonRequest,
        use_strict_schema: bool,
    ) -> Result<reqwest::Response, ProviderError> {
        let response_format = match (&req.json_schema, use_strict_schema) {
            (Some(schema), true) => json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "structured_response",
                    "strict": true,
                    "schema": schema
                }
            }),
            _ => json!({ "type": "json_object" }),
        };

        let payload = json!({
            "model": self.model,
            "temperature": req.temperature,
            "max_tokens": req.max_tokens,
            "messages": [
                { "role": "system", "content": req.system_prompt },
                { "role": "user", "content": req.user_prompt }
            ],
            "response_format": response_format
        });

        let endpoint = format!("{}/v1/chat/completions", self.base_url);
        let mut builder = self.client.post(&endpoint).json(&payload);
        if let Some(ref key) = self.api_key {
            if !key.trim().is_empty() {
                builder = builder.bearer_auth(key.trim());
            }
        }

        builder.send().await.map_err(|e| {
            let kind = if e.is_timeout() {
                ProviderErrorKind::Timeout
            } else {
                ProviderErrorKind::NetworkError
            };
            ProviderError::new(kind, &self.provider_label, format!("HTTP error: {e}"), true)
        })
    }
}

#[async_trait]
impl Llm for OpenAiCompatLlm {
    fn name(&self) -> &str {
        &self.provider_label
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn complete_json(&self, req: &LlmJsonRequest) -> Result<LlmJsonResponse, ProviderError> {
        self.ensure_server_ready().await?;

        let mut resp = self.send_completion(req, req.json_schema.is_some()).await?;
        if resp.status() == StatusCode::BAD_REQUEST && req.json_schema.is_some() {
            resp = self.send_completion(req, false).await?;
        }

        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        if status == StatusCode::TOO_MANY_REQUESTS {
            let kind = if body_text.to_lowercase().contains("quota") {
                ProviderErrorKind::QuotaExhausted
            } else {
                ProviderErrorKind::RateLimited
            };
            return Err(ProviderError::new(
                kind,
                &self.provider_label,
                format!("HTTP 429: {}", truncate_chars(&body_text, 240)),
                true,
            ));
        }
        if !status.is_success() {
            return Err(ProviderError::new(
                ProviderErrorKind::NetworkError,
                &self.provider_label,
                format!("HTTP {}: {}", status.as_u16(), truncate_chars(&body_text, 240)),
                status.is_server_error(),
            ));
        }

        let outer: Value = serde_json::from_str(&body_text).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::ParseError,
                &self.provider_label,
                format!("invalid completion envelope: {e}"),
                false,
            )
        })?;

        let raw_text = outer
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let tokens_in = outer
            .get("usage")
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| estimate_tokens(&req.system_prompt) + estimate_tokens(&req.user_prompt));
        let tokens_out = outer
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| estimate_tokens(&raw_text));

        let cleaned = extract_json_substring(&raw_text);
        let parsed_json: Value = serde_json::from_str(&cleaned).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::InvalidOutput,
                &self.provider_label,
                format!(
                    "model output is not valid JSON: {e} | raw={}",
                    truncate_chars(&raw_text, 200)
                ),
                false,
            )
        })?;

        Ok(LlmJsonResponse {
            provider: self.provider_label.clone(),
            model: self.model.clone(),
            raw_text,
            parsed_json,
            tokens_in,
            tokens_out,
        })
    }
}

pub struct GeminiLlm {
    model: String,
    api_key: Option<String>,
    cloud_enabled: bool,
    base_url: String,
    client: Client,
}

impl GeminiLlm {
    pub fn new(
        model: impl Into<String>,
        api_key: Option<String>,
        cloud_enabled: bool,
        timeout: Duration,
    ) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            model: model.into(),
            api_key,
            cloud_enabled,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            client,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }
}

#[async_trait]
impl Llm for GeminiLlm {
    fn name(&self) -> &str {
        "gemini"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn complete_json(&self, req: &LlmJsonRequest) -> Result<LlmJsonResponse, ProviderError> {
        if !self.cloud_enabled {
            return Err(ProviderError::new(
                ProviderErrorKind::Unsupported,
                "gemini",
                "cloud_enabled is false",
                false,
            ));
        }
        let Some(ref key) = self.api_key else {
            return Err(ProviderError::new(
                ProviderErrorKind::Unsupported,
                "gemini",
                "GEMINI_API_KEY not configured",
                false,
            ));
        };
        if key.trim().is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::Unsupported,
                "gemini",
                "GEMINI_API_KEY is empty",
                false,
            ));
        }

        let mut gen_cfg = json!({
            "temperature": req.temperature,
            "maxOutputTokens": req.max_tokens,
            "responseMimeType": "application/json"
        });
        if let Some(ref schema) = req.json_schema {
            if let Some(obj) = gen_cfg.as_object_mut() {
                obj.insert(
                    "responseSchema".to_string(),
                    sanitize_schema_for_gemini(schema),
                );
            }
        }

        let payload = json!({
            "systemInstruction": {
                "parts": [{ "text": req.system_prompt }]
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": req.user_prompt }]
                }
            ],
            "generationConfig": gen_cfg
        });

        let url = format!("{}/models/{}:generateContent", self.base_url, self.model);
        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", key.trim())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                let kind = if e.is_timeout() {
                    ProviderErrorKind::Timeout
                } else {
                    ProviderErrorKind::NetworkError
                };
                ProviderError::new(kind, "gemini", format!("Gemini request error: {e}"), true)
            })?;

        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        if status == StatusCode::TOO_MANY_REQUESTS || body_text.contains("RESOURCE_EXHAUSTED") {
            return Err(ProviderError::new(
                ProviderErrorKind::QuotaExhausted,
                "gemini",
                format!("Gemini quota/429: {}", truncate_chars(&body_text, 240)),
                true,
            ));
        }
        if !status.is_success() {
            return Err(ProviderError::new(
                ProviderErrorKind::NetworkError,
                "gemini",
                format!(
                    "Gemini HTTP {}: {}",
                    status.as_u16(),
                    truncate_chars(&body_text, 240)
                ),
                status.is_server_error(),
            ));
        }

        let outer: Value = serde_json::from_str(&body_text).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::ParseError,
                "gemini",
                format!("invalid Gemini envelope: {e}"),
                false,
            )
        })?;

        let raw_text = outer
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let tokens_in = outer
            .get("usageMetadata")
            .and_then(|u| u.get("promptTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| estimate_tokens(&req.system_prompt) + estimate_tokens(&req.user_prompt));
        let tokens_out = outer
            .get("usageMetadata")
            .and_then(|u| u.get("candidatesTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| estimate_tokens(&raw_text));

        let cleaned = extract_json_substring(&raw_text);
        let parsed_json: Value = serde_json::from_str(&cleaned).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::InvalidOutput,
                "gemini",
                format!("Gemini output is not valid JSON: {e}"),
                false,
            )
        })?;

        Ok(LlmJsonResponse {
            provider: "gemini".to_string(),
            model: self.model.clone(),
            raw_text,
            parsed_json,
            tokens_in,
            tokens_out,
        })
    }
}

pub struct HeuristicLlm;

impl HeuristicLlm {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HeuristicLlm {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Llm for HeuristicLlm {
    fn name(&self) -> &str {
        "heuristics"
    }

    fn model_name(&self) -> &str {
        "rule_based"
    }

    async fn complete_json(&self, req: &LlmJsonRequest) -> Result<LlmJsonResponse, ProviderError> {
        let props = req
            .json_schema
            .as_ref()
            .and_then(|s| s.get("properties"));
        let is_verification = props.and_then(|p| p.get("verdict")).is_some();
        let is_chapter_block = props.and_then(|p| p.get("chapter_markdown")).is_some();
        let is_person_meta = props.and_then(|p| p.get("public_figure")).is_some()
            && props.and_then(|p| p.get("tags")).is_some();

        let parsed_json = if is_verification {
            let cid = extract_integer_after_key(&req.user_prompt, "claim_id").unwrap_or(0);
            json!({
                "claim_id": cid,
                "verdict": "UNVERIFIABLE",
                "label": "unknown",
                "evidence": [],
                "reasoning_bg": "heuristics_only_mode",
                "evidence_strength": 0.0
            })
        } else if is_chapter_block {
            json!({
                "chapter_markdown": ""
            })
        } else if is_person_meta {
            json!({
                "role": "",
                "summary": "",
                "public_figure": false,
                "tags": []
            })
        } else {
            heuristic_extract_claims_json(&req.user_prompt)
        };

        let raw_text = serde_json::to_string(&parsed_json).unwrap_or_else(|_| "{}".to_string());
        Ok(LlmJsonResponse {
            provider: "heuristics".to_string(),
            model: "rule_based".to_string(),
            raw_text,
            parsed_json,
            tokens_in: 0,
            tokens_out: 0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedLlmResult {
    pub response: LlmJsonResponse,
    pub first_try_valid: bool,
    pub repair_used: bool,
}

pub struct LlmChainRunner {
    providers: Vec<Arc<dyn Llm>>,
    daily_budget: u64,
}

impl LlmChainRunner {
    pub fn from_providers(providers: Vec<Arc<dyn Llm>>, daily_budget: u64) -> Self {
        Self {
            providers,
            daily_budget,
        }
    }

    pub fn primary_provider_name(&self) -> &str {
        self.providers
            .first()
            .map(|p| p.name())
            .unwrap_or("heuristics")
    }

    pub async fn from_runtime(paths: &RuntimePaths, cfg: &AppConfig, db: &Database) -> Self {
        let db_cloud_enabled = db
            .get_setting("cloud_enabled")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let effective_cloud_enabled = cfg.providers.cloud_enabled || db_cloud_enabled;
        let profile = db.get_profile().await.ok().flatten();
        let effective_provider = profile
            .as_ref()
            .and_then(|p| p.chosen_json.get("effective"))
            .and_then(|e| e.get("llm_provider"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let effective_model = profile
            .as_ref()
            .and_then(|p| p.chosen_json.get("effective"))
            .and_then(|e| e.get("llm_model"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let compute_backend = profile
            .as_ref()
            .and_then(|p| p.chosen_json.get("effective"))
            .and_then(|e| e.get("compute_backend"))
            .and_then(|v| v.as_str())
            .unwrap_or("vulkan");
        let use_gpu = compute_backend != "cpu";

        let chosen_gguf_name = effective_model
            .filter(|m| m.ends_with(".gguf"))
            .unwrap_or_else(|| {
                if cfg.providers.local_llm_model.ends_with(".gguf") {
                    cfg.providers.local_llm_model.clone()
                } else {
                    "BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf".to_string()
                }
            });

        let model_rel = format!("llm/{chosen_gguf_name}");
        let resolved_gguf = paths
            .resolve_model_path(&model_rel)
            .or_else(|| find_any_gguf(paths))
            .unwrap_or_else(|| paths.models_dir.join("llm").join(&chosen_gguf_name));
        let llama_bin = paths
            .resolve_binary("llama-server")
            .unwrap_or_else(|| paths.tools_dir.join("llama-server.exe"));

        let mut chain_names = Vec::new();
        if let Some(ref pref) = effective_provider {
            chain_names.push(pref.clone());
        }
        for item in &cfg.providers.llm_chain {
            if !chain_names.iter().any(|x| x == item) {
                chain_names.push(item.clone());
            }
        }

        let mut providers: Vec<Arc<dyn Llm>> = Vec::new();
        for name in chain_names {
            match name.as_str() {
                "local" => {
                    let local_client = OpenAiCompatLlm::new(
                        "local",
                        &cfg.providers.local_llm_url,
                        &chosen_gguf_name,
                        None,
                        Duration::from_secs(120),
                    )
                    .with_local_autolaunch(LocalLlamaLaunchSpec {
                        llama_bin: llama_bin.clone(),
                        model_path: resolved_gguf.clone(),
                        use_gpu,
                        context_size: 8192,
                    });
                    providers.push(Arc::new(local_client));
                }
                "gemini" => {
                    let gem_key = resolve_secret("GEMINI_API_KEY");
                    let gem = GeminiLlm::new(
                        &cfg.providers.gemini_model,
                        gem_key,
                        effective_cloud_enabled,
                        Duration::from_secs(60),
                    );
                    providers.push(Arc::new(gem));
                }
                "paid" | "openai" => {
                    if effective_cloud_enabled {
                        if let Some(paid_key) = resolve_secret("OPENAI_API_KEY") {
                            let paid_url = resolve_secret("OPENAI_BASE_URL")
                                .unwrap_or_else(|| "https://api.openai.com".to_string());
                            let paid_model = resolve_secret("OPENAI_MODEL")
                                .unwrap_or_else(|| "gpt-4o-mini".to_string());
                            providers.push(Arc::new(OpenAiCompatLlm::new(
                                "paid",
                                paid_url,
                                paid_model,
                                Some(paid_key),
                                Duration::from_secs(60),
                            )));
                        }
                    }
                }
                "heuristics" => {
                    providers.push(Arc::new(HeuristicLlm::new()));
                }
                _ => {}
            }
        }

        if providers.is_empty() {
            providers.push(Arc::new(HeuristicLlm::new()));
        }

        Self {
            providers,
            daily_budget: cfg.funnel.daily_llm_budget,
        }
    }

    pub async fn from_role(
        role: &str,
        paths: &RuntimePaths,
        cfg: &AppConfig,
        db: &Database,
    ) -> Self {
        let db_cloud_enabled = db
            .get_setting("cloud_enabled")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let effective_cloud_enabled = cfg.providers.cloud_enabled || db_cloud_enabled;
        let mut role_provider: Option<Arc<dyn Llm>> = None;
        if let Ok(Some(roles_val)) = db.get_setting("model_roles").await {
            if let Some(role_obj) = roles_val.get("roles").and_then(|r| r.get(role)) {
                let provider_type = role_obj.get("provider").and_then(|v| v.as_str()).unwrap_or("local");
                let model_name = role_obj.get("model_name").and_then(|v| v.as_str()).unwrap_or("");
                let custom_url = role_obj.get("custom_url").and_then(|v| v.as_str());

                match provider_type {
                    "ollama" => {
                        let raw_url = custom_url.unwrap_or("http://localhost:11434");
                        let norm_url = if raw_url.ends_with("/v1") {
                            raw_url.to_string()
                        } else {
                            format!("{}/v1", raw_url.trim_end_matches('/'))
                        };
                        role_provider = Some(Arc::new(OpenAiCompatLlm::new(
                            "ollama",
                            norm_url,
                            model_name,
                            None,
                            Duration::from_secs(120),
                        )));
                    }
                    "lmstudio" => {
                        let raw_url = custom_url.unwrap_or("http://localhost:1234");
                        let norm_url = if raw_url.ends_with("/v1") {
                            raw_url.to_string()
                        } else {
                            format!("{}/v1", raw_url.trim_end_matches('/'))
                        };
                        role_provider = Some(Arc::new(OpenAiCompatLlm::new(
                            "lmstudio",
                            norm_url,
                            model_name,
                            None,
                            Duration::from_secs(120),
                        )));
                    }
                    "gemini" => {
                        let gem_key = resolve_secret("GEMINI_API_KEY");
                        let m = if model_name.is_empty() { &cfg.providers.gemini_model } else { model_name };
                        let allow_cloud = effective_cloud_enabled || gem_key.is_some();
                        role_provider = Some(Arc::new(GeminiLlm::new(
                            m,
                            gem_key,
                            allow_cloud,
                            Duration::from_secs(60),
                        )));
                    }
                    "local" => {
                        if !model_name.is_empty() {
                            let model_rel = format!("llm/{model_name}");
                            let resolved = paths
                                .resolve_model_path(&model_rel)
                                .or_else(|| paths.resolve_model_path(model_name))
                                .unwrap_or_else(|| paths.models_dir.join("llm").join(model_name));
                            let llama_bin = paths
                                .resolve_binary("llama-server")
                                .unwrap_or_else(|| paths.tools_dir.join("llama-server.exe"));
                            let local_client = OpenAiCompatLlm::new(
                                "local",
                                &cfg.providers.local_llm_url,
                                model_name,
                                None,
                                Duration::from_secs(120),
                            )
                            .with_local_autolaunch(LocalLlamaLaunchSpec {
                                llama_bin,
                                model_path: resolved,
                                use_gpu: true,
                                context_size: 8192,
                            });
                            role_provider = Some(Arc::new(local_client));
                        }
                    }
                    "heuristics" => {
                        role_provider = Some(Arc::new(HeuristicLlm::new()));
                    }
                    _ => {}
                }
            } else if role == "consensus_secondary" {
                if let Some(cons_obj) = roles_val.get("consensus") {
                    let sec_provider = cons_obj.get("secondary_provider").and_then(|v| v.as_str()).unwrap_or("gemini");
                    let sec_model = cons_obj.get("secondary_model").and_then(|v| v.as_str()).unwrap_or("gemini-2.5-flash");
                    let custom_url = cons_obj.get("secondary_custom_url").and_then(|v| v.as_str());

                    match sec_provider {
                        "gemini" => {
                            let gem_key = resolve_secret("GEMINI_API_KEY");
                            role_provider = Some(Arc::new(GeminiLlm::new(
                                sec_model,
                                gem_key,
                                cfg.providers.cloud_enabled,
                                Duration::from_secs(60),
                            )));
                        }
                        "ollama" => {
                            let raw_url = custom_url.unwrap_or("http://localhost:11434");
                            let norm_url = if raw_url.ends_with("/v1") {
                                raw_url.to_string()
                            } else {
                                format!("{}/v1", raw_url.trim_end_matches('/'))
                            };
                            role_provider = Some(Arc::new(OpenAiCompatLlm::new(
                                "ollama",
                                norm_url,
                                sec_model,
                                None,
                                Duration::from_secs(120),
                            )));
                        }
                        "local" => {
                            let model_rel = format!("llm/{sec_model}");
                            let resolved = paths
                                .resolve_model_path(&model_rel)
                                .or_else(|| paths.resolve_model_path(sec_model))
                                .unwrap_or_else(|| paths.models_dir.join("llm").join(sec_model));
                            let llama_bin = paths
                                .resolve_binary("llama-server")
                                .unwrap_or_else(|| paths.tools_dir.join("llama-server.exe"));
                            let local_client = OpenAiCompatLlm::new(
                                "local",
                                &cfg.providers.local_llm_url,
                                sec_model,
                                None,
                                Duration::from_secs(120),
                            )
                            .with_local_autolaunch(LocalLlamaLaunchSpec {
                                llama_bin,
                                model_path: resolved,
                                use_gpu: true,
                                context_size: 8192,
                            });
                            role_provider = Some(Arc::new(local_client));
                        }
                        _ => {}
                    }
                }
            }
        }

        let fallback_runner = Self::from_runtime(paths, cfg, db).await;
        if let Some(primary) = role_provider {
            let mut combined = vec![primary];
            for p in fallback_runner.providers {
                if !combined.iter().any(|existing| existing.name() == p.name()) {
                    combined.push(p);
                }
            }
            Self {
                providers: combined,
                daily_budget: cfg.funnel.daily_llm_budget,
            }
        } else {
            fallback_runner
        }
    }

    pub async fn complete_validated(
        &self,
        req: &LlmJsonRequest,
        db: &Database,
    ) -> Result<ValidatedLlmResult, ProviderError> {
        let mut last_err: Option<ProviderError> = None;

        for provider in &self.providers {
            let pname = provider.name();
            if pname != "local" && pname != "heuristics" && self.daily_budget > 0 {
                if let Ok(used) = db.total_requests_today_for_prefix(pname).await {
                    if used >= self.daily_budget {
                        last_err = Some(ProviderError::new(
                            ProviderErrorKind::QuotaExhausted,
                            pname,
                            format!("daily LLM budget ({}) reached for {pname}", self.daily_budget),
                            true,
                        ));
                        continue;
                    }
                }
            }

            match run_single_provider_with_repair(provider.as_ref(), req, db).await {
                Ok(ok) => return Ok(ok),
                Err(err) => {
                    if matches!(
                        err.kind,
                        ProviderErrorKind::RateLimited | ProviderErrorKind::QuotaExhausted
                    ) {
                        tokio::time::sleep(Duration::from_millis(400)).await;
                    }
                    last_err = Some(err);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::Internal,
                "llm_chain",
                "no LLM providers available in chain",
                false,
            )
        }))
    }
}

async fn run_single_provider_with_repair(
    provider: &dyn Llm,
    req: &LlmJsonRequest,
    db: &Database,
) -> Result<ValidatedLlmResult, ProviderError> {
    let first_attempt = provider.complete_json(req).await;

    let (bad_raw, validation_err) = match first_attempt {
        Ok(resp) => {
            let _ = db
                .record_provider_usage(
                    provider.name(),
                    provider.model_name(),
                    resp.tokens_in,
                    resp.tokens_out,
                    false,
                )
                .await;

            match validate_optional_schema(&resp.parsed_json, req.json_schema.as_ref()) {
                Ok(()) => {
                    return Ok(ValidatedLlmResult {
                        response: resp,
                        first_try_valid: true,
                        repair_used: false,
                    });
                }
                Err(verr) => (resp.raw_text, verr),
            }
        }
        Err(err) => {
            if matches!(
                err.kind,
                ProviderErrorKind::RateLimited | ProviderErrorKind::QuotaExhausted
            ) {
                let _ = db
                    .record_provider_usage(provider.name(), provider.model_name(), 0, 0, true)
                    .await;
            }
            if err.kind != ProviderErrorKind::InvalidOutput {
                return Err(err);
            }
            (err.message.clone(), err.message)
        }
    };

    let repair_req = LlmJsonRequest {
        system_prompt: format!(
            "{}\nSCHEMA_REPAIR_ERROR: {validation_err}. Return strictly valid JSON conforming to the schema.",
            req.system_prompt
        ),
        user_prompt: format!(
            "{}\n\nINVALID_PRIOR_JSON:\n{}",
            req.user_prompt,
            truncate_chars(&bad_raw, 1500)
        ),
        json_schema: req.json_schema.clone(),
        temperature: 0.0,
        max_tokens: req.max_tokens.max(2560),
    };

    let second = provider.complete_json(&repair_req).await?;
    let _ = db
        .record_provider_usage(
            provider.name(),
            provider.model_name(),
            second.tokens_in,
            second.tokens_out,
            false,
        )
        .await;

    validate_optional_schema(&second.parsed_json, req.json_schema.as_ref()).map_err(|verr| {
        ProviderError::new(
            ProviderErrorKind::InvalidOutput,
            provider.name(),
            format!("schema validation failed after 1 repair retry: {verr}"),
            false,
        )
    })?;

    Ok(ValidatedLlmResult {
        response: second,
        first_try_valid: false,
        repair_used: true,
    })
}

pub fn validate_optional_schema(value: &Value, schema: Option<&Value>) -> Result<(), String> {
    let Some(s) = schema else {
        return Ok(());
    };
    validate_json_against_schema(value, s, "$")
}

pub fn validate_json_against_schema(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        for branch in any_of {
            if validate_json_against_schema(value, branch, path).is_ok() {
                return Ok(());
            }
        }
        return Err(format!("{path} did not match anyOf branches"));
    }

    if let Some(type_val) = schema.get("type") {
        let allowed_types: Vec<&str> = match type_val {
            Value::String(s) => vec![s.as_str()],
            Value::Array(arr) => arr.iter().filter_map(|x| x.as_str()).collect(),
            _ => Vec::new(),
        };
        if !allowed_types.is_empty() && !matches_any_json_type(value, &allowed_types) {
            return Err(format!(
                "{path} expected type {:?}, got {}",
                allowed_types,
                json_type_name(value)
            ));
        }
    }

    if value.is_null() {
        return Ok(());
    }

    if let Some(enum_vals) = schema.get("enum").and_then(|v| v.as_array()) {
        if !enum_vals.iter().any(|ev| ev == value) {
            return Err(format!("{path} value {value} not in enum {enum_vals:?}"));
        }
    }

    if let Some(num) = value.as_f64() {
        if let Some(min_v) = schema.get("minimum").and_then(|v| v.as_f64()) {
            if num < min_v - 1e-9 {
                return Err(format!("{path} number {num} < minimum {min_v}"));
            }
        }
        if let Some(max_v) = schema.get("maximum").and_then(|v| v.as_f64()) {
            if num > max_v + 1e-9 {
                return Err(format!("{path} number {num} > maximum {max_v}"));
            }
        }
    }

    if let Some(s) = value.as_str() {
        if let Some(min_len) = schema.get("minLength").and_then(|v| v.as_u64()) {
            if (s.chars().count() as u64) < min_len {
                return Err(format!("{path} string shorter than minLength {min_len}"));
            }
        }
    }

    if let Some(arr) = value.as_array() {
        if let Some(max_i) = schema.get("maxItems").and_then(|v| v.as_u64()) {
            if (arr.len() as u64) > max_i {
                return Err(format!("{path} array length {} > maxItems {max_i}", arr.len()));
            }
        }
        if let Some(item_schema) = schema.get("items") {
            for (idx, item) in arr.iter().enumerate() {
                let child_path = format!("{path}[{idx}]");
                validate_json_against_schema(item, item_schema, &child_path)?;
            }
        }
    }

    if let Some(obj) = value.as_object() {
        if let Some(req_fields) = schema.get("required").and_then(|v| v.as_array()) {
            for rf in req_fields.iter().filter_map(|x| x.as_str()) {
                if !obj.contains_key(rf) {
                    return Err(format!("{path} missing required field '{rf}'"));
                }
            }
        }
        if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
            for (k, prop_schema) in props {
                if let Some(v) = obj.get(k) {
                    let child_path = format!("{path}.{k}");
                    validate_json_against_schema(v, prop_schema, &child_path)?;
                }
            }
        }
    }

    Ok(())
}

fn matches_any_json_type(val: &Value, types: &[&str]) -> bool {
    types.iter().any(|t| match *t {
        "null" => val.is_null(),
        "boolean" => val.is_boolean(),
        "object" => val.is_object(),
        "array" => val.is_array(),
        "string" => val.is_string(),
        "number" => val.is_number(),
        "integer" => {
            val.is_i64()
                || val.is_u64()
                || val.as_f64().map(|f| f.fract() == 0.0).unwrap_or(false)
        }
        _ => true,
    })
}

fn json_type_name(val: &Value) -> &'static str {
    match val {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn extract_json_substring(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix("```") {
        let without_lang = stripped
            .strip_prefix("json")
            .or_else(|| stripped.strip_prefix("JSON"))
            .unwrap_or(stripped)
            .trim_start();
        if let Some(end_idx) = without_lang.rfind("```") {
            return without_lang[..end_idx].trim().to_string();
        }
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end >= start {
            return trimmed[start..=end].to_string();
        }
    }
    trimmed.to_string()
}

fn sanitize_schema_for_gemini(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if k == "additionalProperties" {
                    continue;
                }
                if k == "type" {
                    if let Some(arr) = v.as_array() {
                        let non_null: Vec<&str> = arr
                            .iter()
                            .filter_map(|x| x.as_str())
                            .filter(|s| *s != "null")
                            .collect();
                        let has_null = arr.iter().any(|x| x.as_str() == Some("null"));
                        if let Some(primary) = non_null.first() {
                            out.insert(
                                "type".to_string(),
                                Value::String((*primary).to_uppercase()),
                            );
                            if has_null {
                                out.insert("nullable".to_string(), Value::Bool(true));
                            }
                            continue;
                        }
                    } else if let Some(s) = v.as_str() {
                        out.insert("type".to_string(), Value::String(s.to_uppercase()));
                        continue;
                    }
                }
                out.insert(k.clone(), sanitize_schema_for_gemini(v));
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sanitize_schema_for_gemini).collect()),
        other => other.clone(),
    }
}

fn find_any_gguf(paths: &RuntimePaths) -> Option<PathBuf> {
    for dir in [
        paths.models_dir.join("llm"),
        paths.static_root.join("models").join("llm"),
    ] {
        if let Ok(rd) = std::fs::read_dir(dir) {
            let mut files: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("gguf"))
                .collect();
            files.sort();
            if let Some(first) = files.into_iter().next() {
                return Some(first);
            }
        }
    }
    None
}

fn parse_bracket_timestamps_ms(line: &str) -> (i64, i64, &str) {
    let trimmed = line.trim();
    if let Some(rest_after_open) = trimmed.strip_prefix('[') {
        if let Some(close_idx) = rest_after_open.find(']') {
            let inside = rest_after_open[..close_idx].trim();
            let body = rest_after_open[close_idx + 1..].trim();
            let clean_inside = inside.trim_end_matches("ms").trim();
            if let Some((s_str, e_str)) = clean_inside.split_once('-') {
                if let (Ok(s), Ok(e)) = (s_str.trim().parse::<i64>(), e_str.trim().parse::<i64>()) {
                    return (s.max(0), e.max(s.max(0)), body);
                }
            } else if let Some(ocr_ms) = clean_inside.strip_prefix("OCR") {
                if let Ok(ms) = ocr_ms.trim().parse::<i64>() {
                    return (ms.max(0), (ms + 3000).max(0), body);
                }
            }
        }
    }
    (0, 5000, trimmed)
}

fn strip_inline_hashtags(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|tok| !tok.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_concrete_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let ch = chars[i];
        buf.push(ch);
        if ch == '.' || ch == '!' || ch == '?' {
            let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
            let next_digit = i + 1 < len && chars[i + 1].is_ascii_digit();
            if !(ch == '.' && prev_digit && next_digit) {
                let cleaned = strip_inline_hashtags(buf.trim());
                if !cleaned.is_empty() {
                    out.push(cleaned);
                }
                buf.clear();
            }
        }
        i += 1;
    }
    let tail = strip_inline_hashtags(buf.trim());
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

fn score_concrete_factual_sentence(sentence: &str) -> Option<(f64, &'static str)> {
    let trimmed = sentence.trim();
    if trimmed.ends_with('?') || trimmed.contains('?') {
        return None;
    }
    let word_count = trimmed
        .split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphabetic()))
        .count();
    if word_count < 5 || trimmed.chars().count() < 24 {
        return None;
    }
    let cyr_chars = trimmed
        .chars()
        .filter(|c| ('\u{0400}'..='\u{04FF}').contains(c))
        .count();
    if cyr_chars < 12 {
        return None;
    }
    let low = trimmed.to_lowercase();
    let promo_markers = [
        "последвай",
        "сподели",
        "абонирай",
        "следете ",
        "коментирай",
        "линк в био",
    ];
    if promo_markers.iter().any(|p| low.contains(p)) {
        return None;
    }

    let has_digits = trimmed.chars().any(|c| c.is_ascii_digit());
    let quant_markers = [
        "милион", "милиард", "млн", "млрд", "хиляд", "процент", "%", "евро", "лева", "лв",
        "години", "година", "месец", "седмиц",
    ];
    let attr_markers = [
        "заяви", "обяви", "съобщи", "твърди", "каза", "посочи", "по данни на", "според",
        "определи", "потвърди", "опроверга",
    ];
    let action_or_inst_markers = [
        "реши", "прие", "отмени", "гласува", "внесе", "подписа", "назначи", "освободи",
        "оставка", "арестув", "задържа", "обвини", "разследв", "проверк", "одит", "санкц",
        "глоба", "съд", "вас", "вкс", "сес", "прокуратур", "кпконпи", "сметна палата",
        "министерств", "правителств", "парламент", "народното събрание", "президент",
        "премиер", "министър", "кмет", "общин", "полици", "бнб", "апи", "днск", "кзк",
        "бюджет", "договор", "поръчк", "инфлац", "данъц", "цени", "протест", "сблъсъц",
        "безредиц", "пожар", "магистрал", "строителств", "болниц", "училищ",
    ];

    let has_quant = has_digits || quant_markers.iter().any(|m| low.contains(m));
    let has_attr = attr_markers.iter().any(|m| low.contains(m));
    let inst_hits = action_or_inst_markers
        .iter()
        .filter(|m| low.contains(**m))
        .count();

    if !has_quant && !has_attr && inst_hits == 0 {
        return None;
    }

    let mut score: f64 = 0.55;
    if has_digits {
        score += 0.16;
    }
    if has_quant {
        score += 0.08;
    }
    if has_attr {
        score += 0.08;
    }
    score += (inst_hits.min(3) as f64) * 0.06;

    let ctype = if has_quant {
        "statistical"
    } else if has_attr {
        "attribution"
    } else {
        "factual"
    };

    Some((score.min(0.95), ctype))
}

fn heuristic_extract_claims_json(user_prompt: &str) -> Value {
    #[derive(Clone)]
    struct Candidate {
        text: String,
        claim_type: &'static str,
        start_ms: i64,
        end_ms: i64,
        score: f64,
    }

    let mut section = "";
    let mut candidates: Vec<Candidate> = Vec::new();

    for raw_line in user_prompt.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed == "(none)" {
            continue;
        }
        if trimmed.starts_with("VIDEO_METADATA:") {
            section = "meta";
            continue;
        }
        if trimmed.starts_with("EDITED_TRANSCRIPT") {
            section = "edited_transcript";
            continue;
        }
        if trimmed.starts_with("TRANSCRIPT_SEGMENTS:") {
            section = "segments";
            continue;
        }
        if trimmed.starts_with("TRANSCRIPT ") || trimmed == "TRANSCRIPT:" {
            section = "transcript";
            continue;
        }
        if trimmed.starts_with("OCR_LINES:") {
            section = "ocr";
            continue;
        }
        if trimmed.starts_with("LEXICON_NOTES:") {
            section = "lexicon";
            continue;
        }

        match section {
            "meta" => {
                if let Some(cap) = trimmed.strip_prefix("- caption:") {
                    for sent in split_concrete_sentences(cap.trim()) {
                        if let Some((score, ctype)) = score_concrete_factual_sentence(&sent) {
                            candidates.push(Candidate {
                                text: sent,
                                claim_type: ctype,
                                start_ms: 0,
                                end_ms: 5000,
                                score: score - 0.04,
                            });
                        }
                    }
                }
            }
            "segments" | "ocr" => {
                let (s_ms, e_ms, body) = parse_bracket_timestamps_ms(trimmed);
                for sent in split_concrete_sentences(body) {
                    if let Some((score, ctype)) = score_concrete_factual_sentence(&sent) {
                        candidates.push(Candidate {
                            text: sent,
                            claim_type: ctype,
                            start_ms: s_ms,
                            end_ms: e_ms,
                            score: if section == "segments" { score + 0.03 } else { score },
                        });
                    }
                }
            }
            "edited_transcript" | "transcript" => {
                for sent in split_concrete_sentences(trimmed) {
                    if let Some((score, ctype)) = score_concrete_factual_sentence(&sent) {
                        candidates.push(Candidate {
                            text: sent,
                            claim_type: ctype,
                            start_ms: 0,
                            end_ms: 5000,
                            score,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected: Vec<Candidate> = Vec::new();
    for cand in candidates {
        let low = cand.text.to_lowercase();
        let duplicate = selected.iter().any(|existing| {
            let ex_low = existing.text.to_lowercase();
            ex_low == low || ex_low.contains(&low) || low.contains(&ex_low)
        });
        if !duplicate {
            selected.push(cand);
            if selected.len() >= 5 {
                break;
            }
        }
    }

    selected.sort_by_key(|c| c.start_ms);

    let claims: Vec<Value> = selected
        .into_iter()
        .map(|c| {
            json!({
                "text_bg": c.text,
                "claim_type": c.claim_type,
                "speaker": null,
                "attribution": [],
                "start_ms": c.start_ms,
                "end_ms": c.end_ms,
                "checkworthiness": (c.score * 100.0).round() / 100.0
            })
        })
        .collect();

    json!({
        "claims": claims,
        "people": []
    })
}

fn extract_integer_after_key(text: &str, key: &str) -> Option<i64> {
    let idx = text.find(key)?;
    let rest = &text[idx + key.len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<i64>().ok()
}

fn estimate_tokens(s: &str) -> u64 {
    ((s.chars().count() as u64) / 3).max(1)
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod heuristic_fact_tests {
    use super::*;

    #[test]
    fn heuristic_claim_extractor_skips_metadata_and_extracts_concrete_facts() {
        let prompt = "VIDEO_METADATA:\n\
- id: 7697938534824430870\n\
- creator: @blitzbg (blitz)\n\
- posted_at: 2026-10-18T09:33:10+00:00\n\
- duration_s: 64.0\n\
- caption: Протести във Франция #Франция #Новини\n\n\
TRANSCRIPT (lang=bg, source=generated, quality=0.95):\n\
Франция е разтърсена от масови протести с 1200 арестувани и 150 ранени полицаи. Върховният административен съд отмени решението на министерството за обществената поръчка. Ще успеят ли властите да овладеят ситуацията? Следете Блиц за повече подробности!\n\n\
TRANSCRIPT_SEGMENTS:\n\
[0-12000 ms] Франция е разтърсена от масови протести с 1200 арестувани и 150 ранени полицаи.\n\
[12000-24000 ms] Върховният административен съд отмени решението на министерството за обществената поръчка.\n\
[24000-30000 ms] Ще успеят ли властите да овладеят ситуацията?\n\n\
OCR_LINES:\n\
(none)\n";

        let val = heuristic_extract_claims_json(prompt);
        let arr = val.get("claims").and_then(|v| v.as_array()).expect("claims array");
        assert_eq!(arr.len(), 2, "Expected 2 concrete claims, got: {:?}", arr);
        let first_text = arr[0].get("text_bg").and_then(|v| v.as_str()).unwrap_or("");
        let second_text = arr[1].get("text_bg").and_then(|v| v.as_str()).unwrap_or("");
        assert!(!first_text.contains("7697938534824430870"));
        assert!(first_text.contains("1200 арестувани"));
        assert_eq!(arr[0].get("start_ms").and_then(|v| v.as_i64()), Some(0));
        assert_eq!(arr[0].get("end_ms").and_then(|v| v.as_i64()), Some(12000));
        assert!(second_text.contains("Върховният административен съд"));
        assert_eq!(arr[1].get("start_ms").and_then(|v| v.as_i64()), Some(12000));
    }
}
