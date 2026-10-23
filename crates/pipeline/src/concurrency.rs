use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use tokio::sync::{Notify, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AimdStage {
    ScreenCheap,
    Acquire,
    Extract,
    Claims,
    Verify,
}

impl AimdStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ScreenCheap => "screen_cheap",
            Self::Acquire => "acquire",
            Self::Extract => "extract",
            Self::Claims => "claims",
            Self::Verify => "verify",
        }
    }

    pub fn from_stage_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "screen_cheap" | "screencheap" => Some(Self::ScreenCheap),
            "acquire" => Some(Self::Acquire),
            "extract" => Some(Self::Extract),
            "claims" | "checkworthy" => Some(Self::Claims),
            "verify" => Some(Self::Verify),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageAimdConfig {
    pub min_concurrency: usize,
    pub max_concurrency: usize,
    pub initial_concurrency: usize,
    pub additive_step: usize,
    pub multiplicative_factor: f64,
    pub success_streak_threshold: usize,
    pub latency_sla_ms: u64,
}

impl Default for StageAimdConfig {
    fn default() -> Self {
        Self {
            min_concurrency: 1,
            max_concurrency: 4,
            initial_concurrency: 2,
            additive_step: 1,
            multiplicative_factor: 0.5,
            success_streak_threshold: 3,
            latency_sla_ms: 15_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageConcurrencySnapshot {
    pub stage: AimdStage,
    pub current_concurrency: usize,
    pub active_tasks: usize,
    pub max_concurrency: usize,
    pub min_concurrency: usize,
    pub consecutive_successes: usize,
    pub total_completed: u64,
    pub total_failed: u64,
    pub last_latency_ms: u64,
}

#[derive(Debug)]
pub struct StageAimdState {
    pub config: StageAimdConfig,
    pub current_concurrency: usize,
    pub active_tasks: usize,
    pub consecutive_successes: usize,
    pub total_completed: u64,
    pub total_failed: u64,
    pub last_latency_ms: u64,
}

impl StageAimdState {
    pub fn new(config: StageAimdConfig) -> Self {
        let initial = config
            .initial_concurrency
            .clamp(config.min_concurrency, config.max_concurrency);
        Self {
            config,
            current_concurrency: initial,
            active_tasks: 0,
            consecutive_successes: 0,
            total_completed: 0,
            total_failed: 0,
            last_latency_ms: 0,
        }
    }

    pub fn record_completion(&mut self, duration_ms: u64, success: bool) {
        self.last_latency_ms = duration_ms;
        if success && duration_ms <= self.config.latency_sla_ms {
            self.total_completed += 1;
            self.consecutive_successes += 1;
            if self.consecutive_successes >= self.config.success_streak_threshold {
                self.current_concurrency = (self.current_concurrency + self.config.additive_step)
                    .min(self.config.max_concurrency);
                self.consecutive_successes = 0;
            }
        } else {
            if !success {
                self.total_failed += 1;
            } else {
                self.total_completed += 1;
            }
            self.consecutive_successes = 0;
            let decreased =
                (self.current_concurrency as f64 * self.config.multiplicative_factor).floor()
                    as usize;
            self.current_concurrency = decreased.max(self.config.min_concurrency);
        }
    }

    pub fn snapshot(&self, stage: AimdStage) -> StageConcurrencySnapshot {
        StageConcurrencySnapshot {
            stage,
            current_concurrency: self.current_concurrency,
            active_tasks: self.active_tasks,
            max_concurrency: self.config.max_concurrency,
            min_concurrency: self.config.min_concurrency,
            consecutive_successes: self.consecutive_successes,
            total_completed: self.total_completed,
            total_failed: self.total_failed,
            last_latency_ms: self.last_latency_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AimdSnapshot {
    pub stages: HashMap<String, StageConcurrencySnapshot>,
    pub backpressure_active: bool,
    pub constrained_stage: Option<String>,
}

pub struct AimdConcurrencyManager {
    stages: RwLock<HashMap<AimdStage, StageAimdState>>,
    notify: Notify,
    backpressure_active: RwLock<bool>,
    backpressure_threshold: usize,
    backpressure_resume_threshold: usize,
}

impl AimdConcurrencyManager {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(
            AimdStage::ScreenCheap,
            StageAimdState::new(StageAimdConfig {
                min_concurrency: 1,
                max_concurrency: 8,
                initial_concurrency: 4,
                additive_step: 1,
                multiplicative_factor: 0.5,
                success_streak_threshold: 4,
                latency_sla_ms: 1_000,
            }),
        );
        map.insert(
            AimdStage::Acquire,
            StageAimdState::new(StageAimdConfig {
                min_concurrency: 1,
                max_concurrency: 4,
                initial_concurrency: 2,
                additive_step: 1,
                multiplicative_factor: 0.5,
                success_streak_threshold: 3,
                latency_sla_ms: 20_000,
            }),
        );
        map.insert(
            AimdStage::Extract,
            StageAimdState::new(StageAimdConfig {
                min_concurrency: 1,
                max_concurrency: 2,
                initial_concurrency: 1,
                additive_step: 1,
                multiplicative_factor: 0.5,
                success_streak_threshold: 4,
                latency_sla_ms: 45_000,
            }),
        );
        map.insert(
            AimdStage::Claims,
            StageAimdState::new(StageAimdConfig {
                min_concurrency: 1,
                max_concurrency: 2,
                initial_concurrency: 1,
                additive_step: 1,
                multiplicative_factor: 0.5,
                success_streak_threshold: 3,
                latency_sla_ms: 30_000,
            }),
        );
        map.insert(
            AimdStage::Verify,
            StageAimdState::new(StageAimdConfig {
                min_concurrency: 1,
                max_concurrency: 4,
                initial_concurrency: 2,
                additive_step: 1,
                multiplicative_factor: 0.5,
                success_streak_threshold: 3,
                latency_sla_ms: 25_000,
            }),
        );

        Self {
            stages: RwLock::new(map),
            notify: Notify::new(),
            backpressure_active: RwLock::new(false),
            backpressure_threshold: 15,
            backpressure_resume_threshold: 5,
        }
    }

    pub async fn update_backpressure(&self, downstream_queue_depth: usize) -> bool {
        let mut bp = self.backpressure_active.write().await;
        if downstream_queue_depth >= self.backpressure_threshold {
            if !*bp {
                *bp = true;
                let mut stages = self.stages.write().await;
                if let Some(acq) = stages.get_mut(&AimdStage::Acquire) {
                    acq.current_concurrency = acq.config.min_concurrency;
                }
            }
        } else if downstream_queue_depth <= self.backpressure_resume_threshold && *bp {
            *bp = false;
        }
        *bp
    }

    pub async fn is_backpressure_active(&self) -> bool {
        *self.backpressure_active.read().await
    }

    pub async fn acquire_permit(self: &Arc<Self>, stage: AimdStage) -> AimdPermit {
        loop {
            {
                let mut stages = self.stages.write().await;
                if let Some(state) = stages.get_mut(&stage) {
                    if state.active_tasks < state.current_concurrency {
                        state.active_tasks += 1;
                        return AimdPermit {
                            manager: Arc::clone(self),
                            stage,
                            start_time: Instant::now(),
                            completed: false,
                        };
                    }
                }
            }
            self.notify.notified().await;
        }
    }

    pub async fn try_acquire_permit(self: &Arc<Self>, stage: AimdStage) -> Option<AimdPermit> {
        let mut stages = self.stages.write().await;
        if let Some(state) = stages.get_mut(&stage) {
            if state.active_tasks < state.current_concurrency {
                state.active_tasks += 1;
                return Some(AimdPermit {
                    manager: Arc::clone(self),
                    stage,
                    start_time: Instant::now(),
                    completed: false,
                });
            }
        }
        None
    }

    pub async fn snapshot(&self) -> AimdSnapshot {
        let stages = self.stages.read().await;
        let mut map = HashMap::new();
        let mut constrained: Option<String> = None;
        let mut lowest_ratio = f64::MAX;

        for (k, v) in stages.iter() {
            let snap = v.snapshot(*k);
            let ratio = snap.current_concurrency as f64 / snap.max_concurrency.max(1) as f64;
            if ratio < lowest_ratio {
                lowest_ratio = ratio;
                constrained = Some(k.as_str().to_string());
            }
            map.insert(k.as_str().to_string(), snap);
        }

        let bp = *self.backpressure_active.read().await;
        AimdSnapshot {
            stages: map,
            backpressure_active: bp,
            constrained_stage: constrained,
        }
    }

    async fn release_permit(&self, stage: AimdStage, duration_ms: u64, success: bool) {
        {
            let mut stages = self.stages.write().await;
            if let Some(state) = stages.get_mut(&stage) {
                if state.active_tasks > 0 {
                    state.active_tasks -= 1;
                }
                state.record_completion(duration_ms, success);
            }
        }
        self.notify.notify_waiters();
    }
}

pub struct AimdPermit {
    manager: Arc<AimdConcurrencyManager>,
    stage: AimdStage,
    start_time: Instant,
    completed: bool,
}

impl AimdPermit {
    pub async fn finish(mut self, success: bool) {
        if !self.completed {
            self.completed = true;
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            self.manager
                .release_permit(self.stage, elapsed, success)
                .await;
        }
    }

    pub fn finish_sync(mut self, success: bool) {
        if !self.completed {
            self.completed = true;
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            let mgr = Arc::clone(&self.manager);
            let st = self.stage;
            tokio::spawn(async move {
                mgr.release_permit(st, elapsed, success).await;
            });
        }
    }
}

impl Drop for AimdPermit {
    fn drop(&mut self) {
        if !self.completed {
            self.completed = true;
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            let mgr = Arc::clone(&self.manager);
            let st = self.stage;
            tokio::spawn(async move {
                mgr.release_permit(st, elapsed, false).await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn aimd_additive_increase_and_multiplicative_decrease() {
        let mgr = Arc::new(AimdConcurrencyManager::new());
        let stage = AimdStage::Claims;

        {
            let snap = mgr.snapshot().await;
            assert_eq!(snap.stages.get("claims").unwrap().current_concurrency, 1);
        }

        for _ in 0..3 {
            let permit = mgr.acquire_permit(stage).await;
            permit.finish(true).await;
        }

        {
            let snap = mgr.snapshot().await;
            assert_eq!(snap.stages.get("claims").unwrap().current_concurrency, 2);
        }

        {
            let permit = mgr.acquire_permit(stage).await;
            permit.finish(false).await;
        }

        {
            let snap = mgr.snapshot().await;
            assert_eq!(snap.stages.get("claims").unwrap().current_concurrency, 1);
        }
    }

    #[tokio::test]
    async fn backpressure_reduces_upstream_concurrency() {
        let mgr = Arc::new(AimdConcurrencyManager::new());
        assert!(!mgr.is_backpressure_active().await);

        mgr.update_backpressure(20).await;
        assert!(mgr.is_backpressure_active().await);

        let snap = mgr.snapshot().await;
        assert_eq!(snap.stages.get("acquire").unwrap().current_concurrency, 1);

        mgr.update_backpressure(3).await;
        assert!(!mgr.is_backpressure_active().await);
    }
}
