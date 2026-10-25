use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use chrono::Utc;
use leadfinder_core::{
    load_app_config, load_primary_sources, load_seed_entries, resolve_or_create_api_token,
    AppConfig, CoreEvent, Database, DiscoveryQuery, FrameSample, Ocr, OcrText, PolitenessGuard,
    PolitenessPolicy, PrimarySourceEntry, ProviderError, ProviderErrorKind, RuntimePaths, Stage,
    TranscribeInput, Transcriber, Transcript, Video, VideoDetail, VideoFilter, VideoRef,
    VideoSource, VideoState,
};
use leadfinder_providers::{
    compute_wer, CaptionFirstTranscriber, MediaAcquirer, PaddleOnnxOcr, PasteLinkSource,
    TesseractOcr, WhisperCppTranscriber, YtDlpClient, YtDlpDiscoverySource,
};

use crate::concurrency::{AimdConcurrencyManager, AimdStage};
use crate::funnel::{screen_cheap, BudgetState, FunnelClaim, FunnelVideo, ScreenOutcome};
use crate::janitor::run_media_retention_janitor;
use crate::quota::ProviderQuotaGuard;

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
    pub concurrency: Arc<AimdConcurrencyManager>,
    pub quota_guard: Arc<ProviderQuotaGuard>,
}

impl PipelineContext {
    pub async fn init() -> Result<Self> {
        let paths = RuntimePaths::discover()?;
        Self::init_with_paths(paths).await
    }

    pub async fn init_with_paths(paths: RuntimePaths) -> Result<Self> {
        let _ = leadfinder_core::init_structured_logging(&paths.log_dir);
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

        let concurrency = Arc::new(AimdConcurrencyManager::new());
        let quota_guard = Arc::new(ProviderQuotaGuard::new());

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
            concurrency,
            quota_guard,
        })
    }

    pub fn emit(&self, event: CoreEvent) {
        let _ = self.events_tx.send(event);
    }

    pub fn media_root(&self) -> std::path::PathBuf {
        self.paths.data_dir.join("media")
    }

    pub async fn ingest_video_refs(
        &self,
        refs: Vec<VideoRef>,
        expansion_depth: i64,
        enqueue_next: bool,
    ) -> Result<Vec<Video>> {
        let mut stored = Vec::with_capacity(refs.len());
        for vref in refs {
            let (video, created) = self.db.upsert_video_discovered(&vref, expansion_depth).await?;
            if enqueue_next && (created || vref.discovered_via.starts_with("paste")) {
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

    pub fn spawn_metadata_enrichment(self: &Arc<Self>, videos: &[Video]) {
        for video in videos {
            let ctx = Arc::clone(self);
            let video_id = video.id.clone();
            let url = video.url.clone();
            tokio::spawn(async move {
                let Some(vref) = ctx.paste_source.enrich_metadata(&url, "paste").await else {
                    return;
                };
                if vref.id != video_id {
                    return;
                }
                if let Ok((updated, _)) = ctx.db.upsert_video_discovered(&vref, 0).await {
                    ctx.emit(CoreEvent::VideoUpdated {
                        video_id: updated.id.clone(),
                        state: updated.state.clone(),
                        lane: updated.lane.clone(),
                    });
                }
            });
        }
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

    pub async fn run_screen_cheap_job(&self, video_id: &str) -> Result<()> {
        let video = self
            .db
            .get_video(video_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("video not found: {video_id}"))?;

        let cfg = self.config.read().await.clone();
        let seeds = self.db.list_seeds(true).await.unwrap_or_default();
        let seed_terms: Vec<String> = seeds.into_iter().map(|s| s.value).collect();
        let seen_phashes = self.db.list_known_phashes().await.unwrap_or_default();

        let claims = self.db.get_claims_for_video(video_id).await.unwrap_or_default();
        let fclaims = claims
            .into_iter()
            .map(|c| FunnelClaim {
                id: c.id,
                text: c.text_bg,
                claim_type: c.claim_type,
                checkworthiness: c.checkworthiness.unwrap_or(0.0),
                verdict: c.verdict,
                label: c.label,
            })
            .collect();

        let fvideo = FunnelVideo {
            id: video.id.clone(),
            views: video.views.unwrap_or(0),
            posted_at: video.posted_at.clone(),
            phash: video.phash.clone(),
            discovered_via: video.discovered_via.clone(),
            caption: video.caption.clone().unwrap_or_default(),
            hashtags: video.hashtags.clone(),
            transcript_quality: 0.8,
            failed: video.state == "FAILED",
            claims: fclaims,
            kept_label: None,
            creator: video.creator_handle.clone(),
        };

        let budget = BudgetState {
            llm_used: 0,
            search_used: 0,
        };

        let outcome = screen_cheap(
            &fvideo,
            &cfg.funnel,
            cfg.ranking.checkworthy_threshold,
            &seed_terms,
            &seen_phashes,
            &budget,
            Utc::now(),
        );

        match outcome {
            ScreenOutcome::Out(reason) => {
                self.db
                    .set_video_state(&video.id, VideoState::ScreenedOut, Some(&reason))
                    .await?;
                self.emit(CoreEvent::VideoUpdated {
                    video_id: video.id.clone(),
                    state: VideoState::ScreenedOut.as_str().to_string(),
                    lane: Some("screened_out".to_string()),
                });
            }
            ScreenOutcome::Pass => {
                let _ = self.db.enqueue_job(Some(&video.id), Stage::Acquire).await?;
            }
        }
        Ok(())
    }

    pub async fn process_single_video(&self, video_id_or_url: &str) -> Result<VideoDetail> {
        let video_id = if video_id_or_url.starts_with("http://") || video_id_or_url.starts_with("https://") {
            let ingested = self.paste_urls(video_id_or_url).await?;
            ingested
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("unable to resolve URL {video_id_or_url}"))?
                .id
        } else {
            video_id_or_url.trim().to_string()
        };

        let video = self
            .db
            .get_video(&video_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("video not found in database: {video_id}"))?;

        let cfg = self.config.read().await.clone();
        let acquirer = MediaAcquirer::new(
            &self.paths,
            Duration::from_secs(cfg.discovery.yt_dlp_timeout_seconds.max(60)),
            &cfg.extraction,
            self.guard.clone(),
        );

        let acquired = match acquirer
            .acquire(&video.id, &video.url, &self.media_root(), &cfg.extraction)
            .await
        {
            Ok(acq) => acq,
            Err(err) => {
                self.db
                    .set_video_state(&video.id, VideoState::Failed, Some(&err.to_string()))
                    .await?;
                return Err(anyhow::anyhow!(err));
            }
        };

        self.db
            .set_video_media_and_phash(
                &video.id,
                Some(&acquired.video_path.to_string_lossy()),
                acquired.phash.as_deref(),
            )
            .await?;
        self.db
            .set_video_state(&video.id, VideoState::Acquired, None)
            .await?;
        self.emit(CoreEvent::VideoUpdated {
            video_id: video.id.clone(),
            state: VideoState::Acquired.as_str().to_string(),
            lane: video.lane.clone(),
        });

        let whisper = WhisperCppTranscriber::new(
            &self.paths,
            cfg.providers.whisper_model.clone(),
            Duration::from_secs(cfg.extraction.whisper_timeout_seconds),
        );
        let transcriber = CaptionFirstTranscriber::new(whisper);

        let tr_record = transcriber
            .transcribe(&TranscribeInput {
                video_id: video.id.clone(),
                audio_wav_path: Some(acquired.audio_wav_path.clone()),
                caption_tracks: acquired.caption_files.clone(),
                expected_languages: cfg.discovery.languages.clone(),
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        let existing_edited = self
            .db
            .get_transcript(&video.id)
            .await?
            .and_then(|t| t.edited_text);

        self.db
            .save_transcript(&Transcript {
                video_id: video.id.clone(),
                source: tr_record.source,
                engine: Some(tr_record.engine),
                model: Some(tr_record.model),
                language: Some(tr_record.language),
                quality: Some(tr_record.quality),
                text: tr_record.text,
                segments: tr_record.segments,
                edited_text: existing_edited,
            })
            .await?;

        let ocr_lines = if cfg.providers.ocr_engine == "paddle_onnx" {
            let p = PaddleOnnxOcr::new(&self.paths, &cfg.extraction);
            p.extract_frames(&acquired.keyframes).await.unwrap_or_default()
        } else {
            let t = TesseractOcr::new(&self.paths, &cfg.extraction);
            t.extract_frames(&acquired.keyframes).await.unwrap_or_default()
        };

        let ocr_rows: Vec<OcrText> = ocr_lines
            .into_iter()
            .map(|l| OcrText {
                video_id: video.id.clone(),
                frame_ms: l.frame_ms,
                text: l.text,
                confidence: Some(l.confidence),
            })
            .collect();
        self.db.replace_ocr_texts(&video.id, &ocr_rows).await?;

        self.db
            .set_video_state(&video.id, VideoState::Extracted, None)
            .await?;
        self.emit(CoreEvent::VideoUpdated {
            video_id: video.id.clone(),
            state: VideoState::Extracted.as_str().to_string(),
            lane: video.lane.clone(),
        });

        let _ = self.db.enqueue_job(Some(&video.id), Stage::Claims).await;

        self.db
            .get_video_detail(&video.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing video detail after extraction"))
    }

    pub async fn benchmark_ocr_on_acquired(&self, limit: usize) -> Result<OcrBenchmarkSummary> {
        let cfg = self.config.read().await.clone();
        let videos = self
            .db
            .list_videos(&VideoFilter {
                limit: Some(limit as i64),
                ..Default::default()
            })
            .await?;

        let tesseract = TesseractOcr::new(&self.paths, &cfg.extraction);
        let paddle = PaddleOnnxOcr::new(&self.paths, &cfg.extraction);

        let mut frame_sets: Vec<(String, Vec<FrameSample>)> = Vec::new();
        for v in videos {
            let frames_dir = self.media_root().join(&v.id).join("frames");
            if frames_dir.is_dir() {
                let mut paths: Vec<_> = std::fs::read_dir(&frames_dir)?
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jpg"))
                    .collect();
                paths.sort();
                let samples: Vec<FrameSample> = paths
                    .into_iter()
                    .enumerate()
                    .map(|(i, image_path)| FrameSample {
                        frame_ms: (i as i64) * (cfg.extraction.keyframe_interval_seconds as i64) * 1000,
                        image_path,
                    })
                    .collect();
                if !samples.is_empty() {
                    frame_sets.push((v.id, samples));
                }
            }
        }

        let total_frames: usize = frame_sets.iter().map(|(_, s)| s.len()).sum();

        let eval_engine = |engine_res: Vec<(usize, f64, usize)>, elapsed_ms: u64, name: &str| -> OcrEngineStats {
            let total_lines: usize = engine_res.iter().map(|x| x.0).sum();
            let cyrillic_lines: usize = engine_res.iter().map(|x| x.2).sum();
            let conf_sum: f64 = engine_res.iter().map(|x| x.1).sum();
            let conf_count = engine_res.iter().filter(|x| x.0 > 0).count().max(1);
            OcrEngineStats {
                engine: name.to_string(),
                clips_tested: frame_sets.len(),
                total_frames,
                total_elapsed_ms: elapsed_ms,
                ms_per_frame: if total_frames > 0 {
                    elapsed_ms as f64 / total_frames as f64
                } else {
                    0.0
                },
                deduped_lines_found: total_lines,
                cyrillic_lines_found: cyrillic_lines,
                mean_confidence: (conf_sum / conf_count as f64 * 1000.0).round() / 1000.0,
            }
        };

        let t0 = Instant::now();
        let mut tess_rows = Vec::new();
        for (_vid, samples) in &frame_sets {
            let lines = tesseract.extract_frames(samples).await.unwrap_or_default();
            let cyr = lines
                .iter()
                .filter(|l| l.text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)))
                .count();
            let mconf = if lines.is_empty() {
                0.0
            } else {
                lines.iter().map(|l| l.confidence).sum::<f64>() / lines.len() as f64
            };
            tess_rows.push((lines.len(), mconf, cyr));
        }
        let tess_stats = eval_engine(tess_rows, t0.elapsed().as_millis() as u64, "tesseract (bul+eng)");

        let p0 = Instant::now();
        let mut paddle_rows = Vec::new();
        for (_vid, samples) in &frame_sets {
            let lines = paddle.extract_frames(samples).await.unwrap_or_default();
            let cyr = lines
                .iter()
                .filter(|l| l.text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)))
                .count();
            let mconf = if lines.is_empty() {
                0.0
            } else {
                lines.iter().map(|l| l.confidence).sum::<f64>() / lines.len() as f64
            };
            paddle_rows.push((lines.len(), mconf, cyr));
        }
        let paddle_stats = eval_engine(paddle_rows, p0.elapsed().as_millis() as u64, "paddle_onnx (RapidOCR)");

        let chosen = if tess_stats.cyrillic_lines_found >= paddle_stats.cyrillic_lines_found {
            "tesseract".to_string()
        } else {
            "paddle_onnx".to_string()
        };

        Ok(OcrBenchmarkSummary {
            engines: vec![tess_stats, paddle_stats],
            chosen_engine: chosen,
        })
    }

    pub async fn compute_wer_report(&self, references: &HashMap<String, String>) -> Result<WerReport> {
        let mut items = Vec::new();
        for (video_id, ref_text) in references {
            if let Some(tr) = self.db.get_transcript(video_id).await? {
                let wer = compute_wer(ref_text, &tr.text);
                items.push(WerClipEntry {
                    video_id: video_id.clone(),
                    source: tr.source,
                    engine: tr.engine.unwrap_or_default(),
                    model: tr.model.unwrap_or_default(),
                    quality: tr.quality.unwrap_or(0.0),
                    wer: (wer * 1000.0).round() / 1000.0,
                    reference_words: ref_text.split_whitespace().count(),
                    hypothesis_words: tr.text.split_whitespace().count(),
                });
            }
        }
        items.sort_by(|a, b| a.video_id.cmp(&b.video_id));
        let mean_wer = if items.is_empty() {
            0.0
        } else {
            items.iter().map(|x| x.wer).sum::<f64>() / items.len() as f64
        };
        Ok(WerReport {
            clips_evaluated: items.len(),
            mean_wer: (mean_wer * 1000.0).round() / 1000.0,
            clips: items,
        })
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

        let creators = self.db.list_creators().await.unwrap_or_default();
        let watchlist_creators: Vec<_> = creators.into_iter().filter(|c| c.watchlist).collect();

        for creator in watchlist_creators {
            summary.seeds_attempted += 1;
            let query = DiscoveryQuery::Creator {
                handle: creator.handle.clone(),
                max_results: cfg.expansion.max_videos_per_creator.min(max_per_seed).max(3),
                expansion_depth: creator.expansion_depth,
            };
            match ytdlp_source.discover(&query).await {
                Ok(refs) => {
                    if let Ok(stored) = self.ingest_video_refs(refs, creator.expansion_depth, true).await {
                        summary.videos_discovered += stored.len();
                    }
                }
                Err(err) => {
                    summary.errors.push(format!("@{}: {err}", creator.handle));
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

        let seeds = self.db.list_seeds(true).await.unwrap_or_default();
        for seed in seeds.into_iter().filter(|s| s.kind == "creator") {
            summary.seeds_attempted += 1;
            let query = DiscoveryQuery::Creator {
                handle: seed.value.clone(),
                max_results: max_per_seed.max(3),
                expansion_depth: 0,
            };
            if let Ok(refs) = ytdlp_source.discover(&query).await {
                if let Ok(stored) = self.ingest_video_refs(refs, 0, true).await {
                    summary.videos_discovered += stored.len();
                }
                let _ = self.db.mark_seed_ran(seed.id).await;
            }
        }

        Ok(summary)
    }

    pub async fn scan_tiktok_custom(&self, cfg: &CustomScanConfig) -> Result<CustomScanResult> {
        let mut result = CustomScanResult {
            targets_scanned: 0,
            videos_found: 0,
            videos_ingested: 0,
            videos_skipped_existing: 0,
            videos_skipped_filter: 0,
            ingested_videos: Vec::new(),
            errors: Vec::new(),
        };

        let ytdlp_source = YtDlpDiscoverySource::new(self.ytdlp.clone());
        let mut target_handles: Vec<String> = Vec::new();
        let is_keyword_mode = cfg.target_mode == "keywords" || cfg.target_mode == "hashtags";

        if cfg.target_mode == "watchlist" || is_keyword_mode {
            let creators = self.db.list_creators().await.unwrap_or_default();
            let mut wl: Vec<String> = creators
                .into_iter()
                .filter(|c| c.watchlist)
                .map(|c| c.handle.trim().trim_start_matches('@').to_lowercase())
                .filter(|h| !h.is_empty())
                .collect();
            if wl.is_empty() {
                let seeds = self.db.list_seeds(true).await.unwrap_or_default();
                wl = seeds
                    .into_iter()
                    .filter(|s| s.kind == "creator")
                    .map(|s| s.value.trim().trim_start_matches('@').to_lowercase())
                    .filter(|h| !h.is_empty())
                    .collect();
            }
            target_handles.extend(wl);
        }

        if cfg.target_mode == "creators" {
            for t in &cfg.targets {
                for part in t.split([',', ' ', ';', '\n', '\r', '\t']) {
                    let clean = part.trim().trim_start_matches('@').to_lowercase();
                    if !clean.is_empty() && !target_handles.contains(&clean) {
                        target_handles.push(clean);
                    }
                }
            }
        }

        if let Some(max_t) = cfg.max_targets {
            if max_t > 0 && target_handles.len() > max_t {
                target_handles.truncate(max_t);
            }
        }

        let search_terms: Vec<String> = if is_keyword_mode {
            cfg.targets
                .iter()
                .flat_map(|t| t.split([',', ';', '\n', '\r', '\t']))
                .map(|s| s.trim().trim_start_matches('#').to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        let max_v = cfg.max_per_target.max(1).min(30);

        for handle in target_handles {
            result.targets_scanned += 1;
            let query = DiscoveryQuery::Creator {
                handle: handle.clone(),
                max_results: max_v,
                expansion_depth: 0,
            };

            let refs = match ytdlp_source.discover(&query).await {
                Ok(r) => r,
                Err(e) => {
                    result.errors.push(format!("@{handle}: {e}"));
                    continue;
                }
            };

            for vref in refs {
                result.videos_found += 1;

                if let Some(mv) = cfg.min_views {
                    if vref.views.unwrap_or(0) < mv {
                        result.videos_skipped_filter += 1;
                        continue;
                    }
                }

                if let Some(ml) = cfg.min_likes {
                    if vref.likes.unwrap_or(0) < ml {
                        result.videos_skipped_filter += 1;
                        continue;
                    }
                }

                if let Some(max_days) = cfg.max_age_days {
                    if let Some(p) = &vref.posted_at {
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(p) {
                            let age = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
                            if age.num_days() > max_days {
                                result.videos_skipped_filter += 1;
                                continue;
                            }
                        }
                    }
                }

                if !search_terms.is_empty() {
                    let caption_lower = vref.caption.as_deref().unwrap_or("").to_lowercase();
                    let matches_term = search_terms.iter().any(|term| {
                        caption_lower.contains(term)
                            || vref.hashtags.iter().any(|h| h.to_lowercase().contains(term))
                    });
                    if !matches_term {
                        result.videos_skipped_filter += 1;
                        continue;
                    }
                }

                let exists = match self.db.get_video(&vref.id).await {
                    Ok(Some(_)) => true,
                    _ => false,
                };

                if exists {
                    result.videos_skipped_existing += 1;
                    continue;
                }

                match self.db.upsert_video_discovered(&vref, 0).await {
                    Ok((video, created)) => {
                        if created {
                            if cfg.auto_process {
                                let _ = self.db.set_video_lane(&video.id, "pasted").await;
                                let _ = self.db.enqueue_job(Some(&video.id), Stage::ScreenCheap).await;
                            } else {
                                let _ = self.db.set_video_lane(&video.id, "pasted").await;
                            }
                            self.emit(CoreEvent::VideoUpdated {
                                video_id: video.id.clone(),
                                state: video.state.clone(),
                                lane: Some("pasted".to_string()),
                            });
                            result.ingested_videos.push(VideoRefSummary {
                                id: video.id.clone(),
                                url: video.url.clone(),
                                creator_handle: video.creator_handle.clone(),
                                creator_display_name: vref.creator_display_name.clone(),
                                caption: video.caption.clone(),
                                views: video.views,
                                likes: video.likes,
                                posted_at: video.posted_at.clone(),
                                duration_s: video.duration_s,
                            });
                            result.videos_ingested += 1;
                        } else {
                            result.videos_skipped_existing += 1;
                        }
                    }
                    Err(e) => {
                        result.errors.push(format!("video {}: {e}", vref.id));
                    }
                }
            }
        }

        Ok(result)
    }

    pub fn spawn_worker_loop(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let maintenance_ctx = Arc::clone(&self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let _ = maintenance_ctx.db.recover_stalled_jobs(900).await;
                let cfg = maintenance_ctx.config.read().await.clone();
                let _ = run_media_retention_janitor(
                    &maintenance_ctx.db,
                    &maintenance_ctx.media_root(),
                    cfg.funnel.retention_days,
                )
                .await;
            }
        });

        let discovery_ctx = Arc::clone(&self);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(15)).await;
            loop {
                let is_enabled = {
                    let c = discovery_ctx.config.read().await;
                    c.discovery.automated_discovery_enabled
                };
                if is_enabled {
                    let _ = discovery_ctx.run_automated_discovery(5).await;
                }
                tokio::time::sleep(Duration::from_secs(1800)).await;
            }
        });

        tokio::spawn(async move {
            loop {
                let counts = self
                    .db
                    .queue_counts()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({}));
                let queued_count = counts.get("queued").and_then(|v| v.as_i64()).unwrap_or(0);
                self.concurrency.update_backpressure(queued_count as usize).await;

                match self.db.claim_next_job(None).await {
                    Ok(Some(job)) => {
                        let st_opt = Stage::from_db(&job.stage);
                        let aimd_stage = match st_opt {
                            Some(Stage::ScreenCheap) => AimdStage::ScreenCheap,
                            Some(Stage::Acquire) | Some(Stage::Extract) => AimdStage::Acquire,
                            Some(Stage::Claims) | Some(Stage::Checkworthy) => AimdStage::Claims,
                            Some(Stage::Verify) | Some(Stage::ResearchMore) => AimdStage::Verify,
                            _ => AimdStage::Extract,
                        };

                        let permit = self.concurrency.acquire_permit(aimd_stage).await;

                        let res = match st_opt {
                            Some(Stage::ScreenCheap) => {
                                if let Some(vid) = &job.video_id {
                                    self.run_screen_cheap_job(vid).await
                                } else {
                                    Ok(())
                                }
                            }
                            Some(Stage::Acquire) | Some(Stage::Extract) => {
                                if let Some(vid) = &job.video_id {
                                    self.process_single_video(vid).await.map(|_| ())
                                } else {
                                    Ok(())
                                }
                            }
                            Some(Stage::Claims) | Some(Stage::Checkworthy) => {
                                if let Some(vid) = &job.video_id {
                                    self.run_claims_job(vid).await
                                } else {
                                    Ok(())
                                }
                            }
                            Some(Stage::Verify) => {
                                if let Some(vid) = &job.video_id {
                                    self.run_verify_job(vid).await
                                } else {
                                    Ok(())
                                }
                            }
                            Some(Stage::ResearchMore) => {
                                if let Some(vid) = &job.video_id {
                                    self.run_research_more_job(vid).await
                                } else {
                                    Ok(())
                                }
                            }
                            _ => Ok(()),
                        };

                        match res {
                            Ok(()) => {
                                permit.finish(true).await;
                                let _ = self.db.complete_job(job.id).await;
                                self.emit(CoreEvent::JobUpdated {
                                    job_id: job.id,
                                    video_id: job.video_id.clone(),
                                    stage: job.stage,
                                    status: "succeeded".to_string(),
                                });
                                if let Some(vid) = &job.video_id {
                                    if let Ok(Some(v)) = self.db.get_video(vid).await {
                                        self.emit(CoreEvent::VideoUpdated {
                                            video_id: vid.clone(),
                                            state: v.state,
                                            lane: v.lane,
                                        });
                                    }
                                }
                            }
                            Err(err) => {
                                permit.finish(false).await;
                                let err_str = err.to_string();
                                let is_quota = err_str.contains("429")
                                    || err_str.contains("quota")
                                    || err_str.contains("RateLimited")
                                    || err_str.contains("QuotaExhausted");

                                if is_quota {
                                    let provider_err = ProviderError::new(
                                        ProviderErrorKind::QuotaExhausted,
                                        "llm",
                                        &err_str,
                                        false,
                                    );
                                    let chain = vec![
                                        "local".to_string(),
                                        "gemini".to_string(),
                                        "heuristics".to_string(),
                                    ];
                                    let _ = self
                                        .quota_guard
                                        .handle_error(
                                            "local",
                                            "BgGPT",
                                            &provider_err,
                                            &chain,
                                            &self.db,
                                            &job,
                                        )
                                        .await;
                                } else {
                                    let _ = self
                                        .db
                                        .fail_or_retry_job(&job, &err_str, 3, true)
                                        .await;
                                }

                                self.emit(CoreEvent::JobUpdated {
                                    job_id: job.id,
                                    video_id: job.video_id.clone(),
                                    stage: job.stage,
                                    status: "failed".to_string(),
                                });
                            }
                        }
                    }
                    _ => {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoveryRunSummary {
    pub seeds_attempted: usize,
    pub videos_discovered: usize,
    pub paused_by_breaker: bool,
    pub skipped_reason: Option<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrEngineStats {
    pub engine: String,
    pub clips_tested: usize,
    pub total_frames: usize,
    pub total_elapsed_ms: u64,
    pub ms_per_frame: f64,
    pub deduped_lines_found: usize,
    pub cyrillic_lines_found: usize,
    pub mean_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBenchmarkSummary {
    pub engines: Vec<OcrEngineStats>,
    pub chosen_engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WerClipEntry {
    pub video_id: String,
    pub source: String,
    pub engine: String,
    pub model: String,
    pub quality: f64,
    pub wer: f64,
    pub reference_words: usize,
    pub hypothesis_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WerReport {
    pub clips_evaluated: usize,
    pub mean_wer: f64,
    pub clips: Vec<WerClipEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomScanConfig {
    #[serde(default = "default_target_mode")]
    pub target_mode: String,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default = "default_max_per_target")]
    pub max_per_target: usize,
    #[serde(default)]
    pub min_views: Option<i64>,
    #[serde(default)]
    pub min_likes: Option<i64>,
    #[serde(default)]
    pub max_age_days: Option<i64>,
    #[serde(default = "default_auto_process")]
    pub auto_process: bool,
    #[serde(default)]
    pub max_targets: Option<usize>,
}

fn default_target_mode() -> String {
    "watchlist".to_string()
}

fn default_max_per_target() -> usize {
    5
}

fn default_auto_process() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomScanResult {
    pub targets_scanned: usize,
    pub videos_found: usize,
    pub videos_ingested: usize,
    pub videos_skipped_existing: usize,
    pub videos_skipped_filter: usize,
    pub ingested_videos: Vec<VideoRefSummary>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoRefSummary {
    pub id: String,
    pub url: String,
    pub creator_handle: Option<String>,
    pub creator_display_name: Option<String>,
    pub caption: Option<String>,
    pub views: Option<i64>,
    pub likes: Option<i64>,
    pub posted_at: Option<String>,
    pub duration_s: Option<f64>,
}
