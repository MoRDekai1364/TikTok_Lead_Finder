use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use tokio::sync::{broadcast, RwLock};
use leadfinder_core::{
    load_app_config, load_primary_sources, load_seed_entries, resolve_or_create_api_token,
    AppConfig, CoreEvent, Database, DiscoveryQuery, PolitenessGuard, PolitenessPolicy,
    PrimarySourceEntry, ProviderErrorKind, RuntimePaths, Stage, Video, VideoRef, VideoSource,
};
use leadfinder_providers::{
    DirectHttpDiscoverySource, HeadlessBrowserDiscoverySource, PasteLinkSource,
    YtDlpClient, YtDlpDiscoverySource,
};

#[derive(Clone)]
pub struct PipelineContext {
    pub paths: RuntimePaths,
    pub config: Arc<RwLock<AppConfig>>,
    pub primary_sources: Arc<RwLock<Vec<PrimarySourceEntry>>>,
    pub db: Database,
    pub api_token: String,
    pub guard: PolitenessGuard,
    pub ytdlp: YtDlpClient,
    pub paste_source: PasteLinkSource,
    pub events_tx: broadcast::Sender<CoreEvent>,
}

impl PipelineContext {
    pub async fn init() -> Result<Self> {
        let paths = RuntimePaths::discover()?;
        Self::init_with_paths(paths).await
    }

    pub async fn init_with_paths(paths: RuntimePaths) -> Result<Self> {
        let cfg = load_app_config(&paths.config_dir)?;
        let primary = load_primary_sources(&paths.config_dir).unwrap_or_default();
        let seeds = load_seed_entries(&paths.config_dir).unwrap_or_default();

        let db = Database::connect(&paths.db_path).await?;
        let _ = db.seed_defaults(&seeds).await?;
        let _ = db.recover_running_jobs().await?;

        let api_token = resolve_or_create_api_token(&paths.data_dir)?;

        let policy = PolitenessPolicy {
            requests_per_minute: cfg.discovery.requests_per_minute,
            breaker_failures: cfg.discovery.breaker_failures,
            breaker_cooldown: Duration::from_secs(cfg.discovery.breaker_cooldown_minutes * 60),
            cache_ttl: Duration::from_secs(cfg.discovery.cache_ttl_hours * 3600),
        };
        let guard = PolitenessGuard::new(policy, paths.cache_dir.clone());
        let ytdlp = YtDlpClient::new(
            &paths,
            Duration::from_secs(cfg.discovery.yt_dlp_timeout_seconds),
            guard.clone(),
        );
        let paste_source = PasteLinkSource::new(ytdlp.clone(), guard.clone());
        let (events_tx, _) = broadcast::channel(512);

        Ok(Self {
            paths,
            config: Arc::new(RwLock::new(cfg)),
            primary_sources: Arc::new(RwLock::new(primary)),
            db,
            api_token,
            guard,
            ytdlp,
            paste_source,
            events_tx,
        })
    }

    pub fn emit(&self, event: CoreEvent) {
        let _ = self.events_tx.send(event);
    }

    pub async fn ingest_video_refs(&self, refs: Vec<VideoRef>, expansion_depth: i64, enqueue_next: bool) -> Result<Vec<Video>> {
        let mut stored = Vec::with_capacity(refs.len());
        for vref in refs {
            let (video, _created) = self.db.upsert_video_discovered(&vref, expansion_depth).await?;
            if enqueue_next {
                let job = self.db.enqueue_job(Some(&video.id), Stage::ScreenCheap).await?;
                self.emit(CoreEvent::JobUpdated {
                    job_id: job.id,
                    video_id: Some(video.id.clone()),
                    stage: job.stage,
                    status: job.status,
                });
            }
            self.emit(CoreEvent::VideoUpdated {
                video_id: video.id.clone(),
                state: video.state.clone(),
                lane: video.lane.clone(),
            });
            stored.push(video);
        }
        Ok(stored)
    }

    pub async fn paste_urls(&self, raw_text: &str) -> Result<Vec<Video>> {
        let query = DiscoveryQuery::PasteText {
            raw_text: raw_text.to_string(),
        };
        let refs = self.paste_source.discover(&query).await?;
        self.ingest_video_refs(refs, 0, true).await
    }

    pub async fn import_partner_file(&self, file_path: &Path) -> Result<Vec<Video>> {
        let query = DiscoveryQuery::PartnerImport {
            file_path: file_path.to_path_buf(),
        };
        let refs = self.paste_source.discover(&query).await?;
        self.ingest_video_refs(refs, 0, true).await
    }

    pub async fn import_partner_content(&self, content: &str, label: &str) -> Result<Vec<Video>> {
        let query = DiscoveryQuery::PartnerImportContent {
            content: content.to_string(),
            source_label: label.to_string(),
        };
        let refs = self.paste_source.discover(&query).await?;
        self.ingest_video_refs(refs, 0, true).await
    }

    pub async fn run_automated_discovery(&self, max_per_seed: usize) -> Result<DiscoveryRunSummary> {
        let cfg = self.config.read().await.clone();
        let mut summary = DiscoveryRunSummary::default();

        if !cfg.discovery.automated_discovery_enabled {
            summary.skipped_reason = Some(
                "automated_discovery_enabled is false in config; paste-a-link and partner import remain active"
                    .to_string(),
            );
            return Ok(summary);
        }

        let ytdlp_source = YtDlpDiscoverySource::new(self.ytdlp.clone());
        let http_source = DirectHttpDiscoverySource::new(self.guard.clone());
        let headless_source = HeadlessBrowserDiscoverySource::new(
            self.guard.clone(),
            Duration::from_secs(25),
        );

        let seeds = self.db.list_seeds(true).await?;
        for seed in seeds {
            let query = match seed.kind.as_str() {
                "hashtag" => DiscoveryQuery::Hashtag {
                    tag: seed.value.clone(),
                    max_results: max_per_seed,
                },
                "creator" => DiscoveryQuery::Creator {
                    handle: seed.value.clone(),
                    max_results: max_per_seed,
                    expansion_depth: 0,
                },
                _ => DiscoveryQuery::Keyword {
                    keyword: seed.value.clone(),
                    max_results: max_per_seed,
                },
            };

            summary.seeds_attempted += 1;
            let res = match ytdlp_source.discover(&query).await {
                Ok(v) if !v.is_empty() => Ok(v),
                Err(e) if e.kind == ProviderErrorKind::CircuitOpen => Err(e),
                _ => match headless_source.discover(&query).await {
                    Ok(v) if !v.is_empty() => Ok(v),
                    _ => http_source.discover(&query).await,
                },
            };

            match res {
                Ok(refs) => {
                    let stored = self.ingest_video_refs(refs, 0, true).await?;
                    summary.videos_discovered += stored.len();
                    let _ = self.db.mark_seed_ran(seed.id).await;
                }
                Err(err) => {
                    summary.errors.push(err.to_string());
                    if err.kind == ProviderErrorKind::CircuitOpen {
                        summary.paused_by_breaker = true;
                        self.emit(CoreEvent::DiscoveryStatus {
                            paused: true,
                            reason: Some(err.message),
                            active_breakers: vec!["www.tiktok.com".to_string()],
                        });
                        break;
                    }
                }
            }
        }

        let creators = self.db.list_creators().await?;
        for creator in creators.into_iter().filter(|c| c.watchlist) {
            let query = DiscoveryQuery::Creator {
                handle: creator.handle.clone(),
                max_results: cfg.expansion.max_videos_per_creator,
                expansion_depth: creator.expansion_depth,
            };
            if let Ok(refs) = ytdlp_source.discover(&query).await {
                let stored = self
                    .ingest_video_refs(refs, creator.expansion_depth, true)
                    .await?;
                summary.videos_discovered += stored.len();
            }
        }

        Ok(summary)
    }

    pub fn spawn_worker_loop(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match self.db.claim_next_job(Some(Stage::ScreenCheap)).await {
                    Ok(Some(job)) => {
                        let _ = self.db.complete_job(job.id).await;
                        self.emit(CoreEvent::JobUpdated {
                            job_id: job.id,
                            video_id: job.video_id.clone(),
                            stage: job.stage,
                            status: "succeeded".to_string(),
                        });
                    }
                    _ => {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        })
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DiscoveryRunSummary {
    pub seeds_attempted: usize,
    pub videos_discovered: usize,
    pub paused_by_breaker: bool,
    pub skipped_reason: Option<String>,
    pub errors: Vec<String>,
}
