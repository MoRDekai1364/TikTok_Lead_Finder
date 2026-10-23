use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use url::Url;
use crate::traits::{ProviderError, ProviderErrorKind};

#[derive(Debug, Clone)]
pub struct PolitenessPolicy {
    pub requests_per_minute: u32,
    pub breaker_failures: u32,
    pub breaker_cooldown: Duration,
    pub cache_ttl: Duration,
}

impl Default for PolitenessPolicy {
    fn default() -> Self {
        Self {
            requests_per_minute: 12,
            breaker_failures: 5,
            breaker_cooldown: Duration::from_secs(30 * 60),
            cache_ttl: Duration::from_secs(6 * 3600),
        }
    }
}

#[derive(Debug, Clone)]
struct HostBucketState {
    tokens: f64,
    last_refill: Instant,
    consecutive_failures: u32,
    open_until: Option<Instant>,
    open_until_epoch_s: Option<u64>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBreakerStatus {
    pub host: String,
    pub is_open: bool,
    pub consecutive_failures: u32,
    pub open_until_epoch_s: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpTransportMode {
    LiveWithCache,
    RecordFixtures,
    ReplayFixturesOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedHttpEntry {
    pub url: String,
    pub status: u16,
    pub body: String,
    pub content_type: Option<String>,
    pub stored_at_epoch_s: u64,
}

#[derive(Clone)]
pub struct PolitenessGuard {
    policy: PolitenessPolicy,
    hosts: Arc<Mutex<HashMap<String, HostBucketState>>>,
    cache_dir: PathBuf,
    fixture_dir: Option<PathBuf>,
    mode: HttpTransportMode,
}

impl PolitenessGuard {
    pub fn new(policy: PolitenessPolicy, cache_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&cache_dir);
        Self {
            policy,
            hosts: Arc::new(Mutex::new(HashMap::new())),
            cache_dir,
            fixture_dir: None,
            mode: HttpTransportMode::LiveWithCache,
        }
    }

    pub fn with_fixture_mode(mut self, fixture_dir: PathBuf, mode: HttpTransportMode) -> Self {
        let _ = std::fs::create_dir_all(&fixture_dir);
        self.fixture_dir = Some(fixture_dir);
        self.mode = mode;
        self
    }

    pub fn mode(&self) -> HttpTransportMode {
        self.mode
    }

    pub fn extract_host(raw_url: &str) -> String {
        Url::parse(raw_url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_lowercase()))
            .unwrap_or_else(|| "unknown.local".to_string())
    }

    pub async fn acquire_slot(&self, host: &str, provider_name: &str) -> Result<(), ProviderError> {
        let sleep_needed = {
            let mut map = self.hosts.lock().await;
            let now = Instant::now();
            let capacity = (self.policy.requests_per_minute.max(1) as f64).min(4.0);
            let refill_per_sec = self.policy.requests_per_minute.max(1) as f64 / 60.0;

            let state = map.entry(host.to_string()).or_insert_with(|| HostBucketState {
                tokens: 1.0,
                last_refill: now,
                consecutive_failures: 0,
                open_until: None,
                open_until_epoch_s: None,
                last_error: None,
            });

            if let Some(until) = state.open_until {
                if now < until {
                    let remaining_s = (until - now).as_secs();
                    return Err(ProviderError::new(
                        ProviderErrorKind::CircuitOpen,
                        provider_name,
                        format!("circuit breaker open for host {host} ({remaining_s}s remaining)"),
                        true,
                    ));
                }
                state.open_until = None;
                state.open_until_epoch_s = None;
            }

            let elapsed = now.duration_since(state.last_refill).as_secs_f64();
            state.tokens = (state.tokens + elapsed * refill_per_sec).min(capacity);
            state.last_refill = now;

            if state.tokens >= 1.0 {
                state.tokens -= 1.0;
                Duration::from_millis(rand::thread_rng().gen_range(80..320))
            } else {
                let deficit = 1.0 - state.tokens;
                state.tokens = 0.0;
                let base_wait_ms = ((deficit / refill_per_sec) * 1000.0).ceil() as u64;
                let jitter_ms = rand::thread_rng().gen_range(120..550);
                Duration::from_millis(base_wait_ms + jitter_ms)
            }
        };

        if !sleep_needed.is_zero() {
            tokio::time::sleep(sleep_needed).await;
        }
        Ok(())
    }

    pub async fn record_success(&self, host: &str) {
        let mut map = self.hosts.lock().await;
        if let Some(state) = map.get_mut(host) {
            state.consecutive_failures = 0;
            state.open_until = None;
            state.open_until_epoch_s = None;
            state.last_error = None;
        }
    }

    pub async fn record_failure(&self, host: &str, reason: &str) -> bool {
        let mut map = self.hosts.lock().await;
        let now = Instant::now();
        let state = map.entry(host.to_string()).or_insert_with(|| HostBucketState {
            tokens: 0.0,
            last_refill: now,
            consecutive_failures: 0,
            open_until: None,
            open_until_epoch_s: None,
            last_error: None,
        });
        state.consecutive_failures += 1;
        state.last_error = Some(reason.to_string());
        if state.consecutive_failures >= self.policy.breaker_failures {
            let until = now + self.policy.breaker_cooldown;
            let now_epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            state.open_until = Some(until);
            state.open_until_epoch_s = Some(now_epoch + self.policy.breaker_cooldown.as_secs());
            return true;
        }
        false
    }

    pub async fn breaker_statuses(&self) -> Vec<HostBreakerStatus> {
        let map = self.hosts.lock().await;
        let now = Instant::now();
        map.iter()
            .map(|(host, st)| HostBreakerStatus {
                host: host.clone(),
                is_open: st.open_until.map(|u| now < u).unwrap_or(false),
                consecutive_failures: st.consecutive_failures,
                open_until_epoch_s: st.open_until_epoch_s,
                last_error: st.last_error.clone(),
            })
            .collect()
    }

    fn url_key(url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn read_entry(dir: &Path, url: &str) -> Option<CachedHttpEntry> {
        let path = dir.join(format!("{}.json", Self::url_key(url)));
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn write_entry(dir: &Path, entry: &CachedHttpEntry) {
        let _ = std::fs::create_dir_all(dir);
        let path = dir.join(format!("{}.json", Self::url_key(&entry.url)));
        if let Ok(serialized) = serde_json::to_string_pretty(entry) {
            let _ = std::fs::write(path, serialized);
        }
    }

    pub fn get_cached(&self, url: &str) -> Option<CachedHttpEntry> {
        if self.mode == HttpTransportMode::ReplayFixturesOnly {
            if let Some(dir) = &self.fixture_dir {
                if let Some(hit) = Self::read_entry(dir, url) {
                    return Some(hit);
                }
            }
            return None;
        }

        let hit = Self::read_entry(&self.cache_dir, url)?;
        let now_s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now_s.saturating_sub(hit.stored_at_epoch_s) <= self.policy.cache_ttl.as_secs() {
            Some(hit)
        } else {
            None
        }
    }

    pub fn put_cached(&self, url: &str, status: u16, body: String, content_type: Option<String>) {
        let now_s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = CachedHttpEntry {
            url: url.to_string(),
            status,
            body,
            content_type,
            stored_at_epoch_s: now_s,
        };
        Self::write_entry(&self.cache_dir, &entry);
        if self.mode == HttpTransportMode::RecordFixtures {
            if let Some(dir) = &self.fixture_dir {
                Self::write_entry(dir, &entry);
            }
        }
    }
}

pub fn compute_backoff_delay(attempts: u32) -> Duration {
    let exp = attempts.min(7);
    let base_secs = 2u64.saturating_pow(exp);
    let jitter_ms = rand::thread_rng().gen_range(100..900);
    Duration::from_secs(base_secs) + Duration::from_millis(jitter_ms)
}
