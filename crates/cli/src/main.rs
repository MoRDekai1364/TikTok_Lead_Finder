use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use leadfinder_core::{ClaimsOutput, LlmJsonRequest, VideoFilter};
use leadfinder_pipeline::{
    build_router, claims_output_json_schema, render_spike_markdown, run_discovery_spike,
    run_media_retention_janitor, split_and_normalize_attribution, PipelineContext,
};
use leadfinder_probe::{
    apply_calibration_overrides, run_calibration, CalibrationOverrides,
};
use leadfinder_providers::{
    global_lexicon, inspect_dependencies, install_missing_dependencies, stop_managed_llama_server,
    LlmChainRunner,
};

#[derive(Parser)]
#[command(name = "leadfinder", version, about = "TikTok Lead Finder Core Service & CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    AddUrl {
        #[arg(required = true)]
        urls: Vec<String>,
    },
    ImportFile {
        #[arg(required = true)]
        path: PathBuf,
    },
    Discover {
        #[arg(long, default_value_t = 10)]
        max_per_seed: usize,
    },
    Process {
        #[arg(required = true)]
        video_id: String,
    },
    ProcessBatch {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    BenchmarkOcr {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    WerReport {
        #[arg(required = true)]
        references_json: PathBuf,
    },
    RunJanitor {
        #[arg(long)]
        retention_days: Option<u64>,
    },
    SpikeDiscovery {
        #[arg(long, default_value_t = 3)]
        runs: usize,
        #[arg(long, default_value_t = 10)]
        max_per_seed: usize,
        #[arg(long, default_value = "docs/SPIKE_DISCOVERY.md")]
        out_report: PathBuf,
    },
    ListVideos {
        #[arg(long)]
        lane: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    SetupDeps {
        #[arg(long)]
        non_interactive: bool,
        #[arg(long)]
        ensure_webview2: bool,
        #[arg(long)]
        with_models: bool,
        #[arg(long)]
        calibrate: bool,
        #[arg(long)]
        check_only: bool,
    },
    LexiconInspect {
        #[arg(long)]
        text: Option<String>,
    },
    Calibrate {
        #[arg(long)]
        force_cpu: bool,
        #[arg(long)]
        override_asr: Option<String>,
        #[arg(long)]
        override_llm_provider: Option<String>,
        #[arg(long)]
        override_llm_model: Option<String>,
        #[arg(long)]
        override_ocr: Option<String>,
        #[arg(long)]
        clear_overrides: bool,
    },
    ExtractClaims {
        #[arg(required = true)]
        video_id: String,
    },
    ExtractClaimsBatch {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    EvalAttributionTraps {
        #[arg(required = true)]
        fixtures_json: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = Arc::new(PipelineContext::init().await?);

    match cli.command {
        Commands::Serve { host, port } => {
            let cfg = ctx.config.read().await.clone();
            let bind_host = host.unwrap_or(cfg.server.host);
            let bind_port = port.unwrap_or(cfg.server.port);
            let addr = format!("{bind_host}:{bind_port}");

            let _worker = ctx.clone().spawn_worker_loop();
            let router = build_router(ctx.clone());
            let listener = TcpListener::bind(&addr).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "listening": format!("http://{addr}"),
                    "token_file": ctx.paths.data_dir.join("api_token"),
                }))?
            );
            axum::serve(listener, router).await?;
        }
        Commands::AddUrl { urls } => {
            let joined = urls.join("\n");
            let stored = ctx.paste_urls(&joined).await?;
            println!("{}", serde_json::to_string_pretty(&stored)?);
        }
        Commands::ImportFile { path } => {
            let stored = ctx.import_partner_file(&path).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "imported_count": stored.len(),
                    "videos": stored,
                }))?
            );
        }
        Commands::Discover { max_per_seed } => {
            let summary = ctx.run_automated_discovery(max_per_seed).await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Commands::Process { video_id } => {
            let detail = ctx.process_single_video(&video_id).await?;
            println!("{}", serde_json::to_string_pretty(&detail)?);
        }
        Commands::ProcessBatch { limit } => {
            let batch_started = std::time::Instant::now();
            let videos = ctx
                .db
                .list_videos(&VideoFilter {
                    limit: Some(limit as i64),
                    ..Default::default()
                })
                .await?;
            let mut processed = Vec::new();
            let mut errors = Vec::new();
            for v in videos {
                match ctx.process_single_video(&v.id).await {
                    Ok(detail) => processed.push(serde_json::json!({
                        "video_id": detail.video.id,
                        "state": detail.video.state,
                        "transcript_source": detail.transcript.as_ref().map(|t| t.source.clone()),
                        "transcript_language": detail.transcript.as_ref().and_then(|t| t.language.clone()),
                        "transcript_quality": detail.transcript.as_ref().and_then(|t| t.quality),
                        "ocr_lines": detail.ocr_texts.len(),
                    })),
                    Err(e) => errors.push(serde_json::json!({
                        "video_id": v.id,
                        "error": e.to_string(),
                    })),
                }
            }
            let elapsed_s = batch_started.elapsed().as_secs_f64();
            let sec_per_video = if processed.is_empty() {
                0.0
            } else {
                elapsed_s / processed.len() as f64
            };
            let window_s = ctx.config.read().await.capacity.processing_window_hours * 3600.0;
            let extrapolated_per_day = if sec_per_video > 0.0 {
                (window_s / sec_per_video * 10.0).round() / 10.0
            } else {
                0.0
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "processed_count": processed.len(),
                    "error_count": errors.len(),
                    "elapsed_seconds": (elapsed_s * 100.0).round() / 100.0,
                    "mean_seconds_per_video": (sec_per_video * 100.0).round() / 100.0,
                    "measured_capacity_per_day": extrapolated_per_day,
                    "results": processed,
                    "errors": errors,
                }))?
            );
        }
        Commands::BenchmarkOcr { limit } => {
            let summary = ctx.benchmark_ocr_on_acquired(limit).await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Commands::WerReport { references_json } => {
            let raw = std::fs::read_to_string(&references_json)?;
            let refs: HashMap<String, String> = serde_json::from_str(&raw)?;
            let report = ctx.compute_wer_report(&refs).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::RunJanitor { retention_days } => {
            let days = match retention_days {
                Some(d) => d,
                None => ctx.config.read().await.funnel.retention_days,
            };
            let rep = run_media_retention_janitor(&ctx.db, &ctx.media_root(), days).await?;
            println!("{}", serde_json::to_string_pretty(&rep)?);
        }
        Commands::SpikeDiscovery {
            runs,
            max_per_seed,
            out_report,
        } => {
            let seeds = ctx.db.list_seeds(true).await?;
            let report = run_discovery_spike(&ctx.paths, &seeds, runs, max_per_seed).await?;
            let md = render_spike_markdown(&report);
            if let Some(parent) = out_report.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&out_report, &md)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::ListVideos { lane, limit } => {
            let filter = VideoFilter {
                lane,
                limit: Some(limit),
                ..Default::default()
            };
            let videos = ctx.db.list_videos(&filter).await?;
            println!("{}", serde_json::to_string_pretty(&videos)?);
        }
        Commands::SetupDeps {
            non_interactive: _,
            ensure_webview2,
            with_models,
            calibrate,
            check_only,
        } => {
            let status = if check_only {
                inspect_dependencies(&ctx.paths)
            } else {
                install_missing_dependencies(&ctx.paths, ensure_webview2, with_models).await?
            };
            if calibrate && !check_only {
                let cfg = ctx.config.read().await.clone();
                let _ = run_calibration(&ctx.paths, &cfg, &ctx.db, false).await?;
            }
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Commands::LexiconInspect { text } => {
            let lex_file = ctx.config.read().await.normalisation.lexicon_config_file.clone();
            let lex = global_lexicon(&ctx.paths, &lex_file);
            let sample = text.unwrap_or_default();
            let normalized = if sample.is_empty() {
                String::new()
            } else {
                lex.normalize_text(&sample)
            };
            let analysis = lex.analyze_for_llm(if normalized.is_empty() { &sample } else { &normalized });
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "stats": lex.stats(),
                    "input": sample,
                    "normalized": normalized,
                    "llm_context": analysis,
                }))?
            );
        }
        Commands::Calibrate {
            force_cpu,
            override_asr,
            override_llm_provider,
            override_llm_model,
            override_ocr,
            clear_overrides,
        } => {
            let cfg = ctx.config.read().await.clone();
            let has_override_flags = clear_overrides
                || override_asr.is_some()
                || override_llm_provider.is_some()
                || override_llm_model.is_some()
                || override_ocr.is_some();

            if has_override_flags {
                let ov = if clear_overrides {
                    CalibrationOverrides::default()
                } else {
                    CalibrationOverrides {
                        asr_model: override_asr,
                        llm_provider: override_llm_provider,
                        llm_model: override_llm_model,
                        ocr_engine: override_ocr,
                        asr_concurrency: None,
                        llm_concurrency: None,
                    }
                };
                if let Some(updated) = apply_calibration_overrides(&ctx.db, &cfg, ov).await? {
                    println!("{}", serde_json::to_string_pretty(&updated)?);
                    return Ok(());
                }
            }

            let profile = run_calibration(&ctx.paths, &cfg, &ctx.db, force_cpu).await?;
            println!("{}", serde_json::to_string_pretty(&profile)?);
        }
        Commands::ExtractClaims { video_id } => {
            let report = ctx.extract_claims_for_video(&video_id).await;
            stop_managed_llama_server().await;
            let report = report?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::ExtractClaimsBatch { limit } => {
            let videos = ctx
                .db
                .list_videos(&VideoFilter {
                    limit: Some(limit as i64),
                    ..Default::default()
                })
                .await?;
            let mut reports = Vec::new();
            let mut errors = Vec::new();
            let mut first_try_ok = 0usize;
            let mut repaired_ok = 0usize;
            let mut total_claims = 0usize;
            let mut total_checkworthy = 0usize;
            let mut total_people = 0usize;
            let mut total_public_figures = 0usize;

            for v in &videos {
                match ctx.extract_claims_for_video(&v.id).await {
                    Ok(rep) => {
                        if rep.first_try_valid {
                            first_try_ok += 1;
                        }
                        if rep.repair_used {
                            repaired_ok += 1;
                        }
                        total_claims += rep.extracted_claims_count;
                        total_checkworthy += rep.checkworthy_claims_count;
                        total_people += rep.resolved_people_count;
                        total_public_figures += rep.public_figures_count;
                        reports.push(rep);
                    }
                    Err(e) => {
                        errors.push(serde_json::json!({
                            "video_id": v.id,
                            "error": e.to_string(),
                        }));
                    }
                }
            }
            stop_managed_llama_server().await;

            let total_attempted = videos.len().max(1);
            let first_try_rate = (first_try_ok as f64 / total_attempted as f64 * 1000.0).round() / 1000.0;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "videos_attempted": videos.len(),
                    "videos_succeeded": reports.len(),
                    "error_count": errors.len(),
                    "first_try_valid_count": first_try_ok,
                    "first_try_validity_rate": first_try_rate,
                    "repaired_count": repaired_ok,
                    "total_claims_extracted": total_claims,
                    "total_checkworthy_claims": total_checkworthy,
                    "total_people_resolved": total_people,
                    "total_public_figures": total_public_figures,
                    "reports": reports,
                    "errors": errors,
                }))?
            );
        }
        Commands::EvalAttributionTraps { fixtures_json } => {
            #[derive(serde::Deserialize)]
            struct TrapCase {
                id: String,
                speaker: String,
                transcript: String,
                expected_actor: String,
                expected_act: String,
            }

            let raw = std::fs::read_to_string(&fixtures_json)?;
            let cases: Vec<TrapCase> = serde_json::from_str(&raw)?;
            let cfg = ctx.config.read().await.clone();
            let lex = global_lexicon(&ctx.paths, &cfg.normalisation.lexicon_config_file);
            let verbs = lex.attribution_verbs();
            let runner = LlmChainRunner::from_runtime(&ctx.paths, &cfg, &ctx.db).await;
            let prompt_path = ctx.paths.prompts_dir.join("claims_v1.txt");
            let system_prompt = std::fs::read_to_string(&prompt_path)?;

            let mut results = Vec::new();
            let mut split_pass_count = 0usize;
            let mut first_try_count = 0usize;

            for case in &cases {
                let ling = lex.analyze_for_llm(&case.transcript);
                let user_prompt = format!(
                    "VIDEO_METADATA:\n- id: {}\n- creator: @{}\n\nTRANSCRIPT_SEGMENTS:\n[0-8000 ms] {}\n\nLEXICON_NOTES:\n{}",
                    case.id, case.speaker, case.transcript, ling.prompt_notes
                );
                let validated = runner
                    .complete_validated(
                        &LlmJsonRequest {
                            system_prompt: system_prompt.clone(),
                            user_prompt,
                            json_schema: Some(claims_output_json_schema()),
                            temperature: 0.0,
                            max_tokens: 768,
                        },
                        &ctx.db,
                    )
                    .await?;
                if validated.first_try_valid {
                    first_try_count += 1;
                }
                let mut parsed: ClaimsOutput = serde_json::from_value(validated.response.parsed_json)?;
                for cl in &mut parsed.claims {
                    split_and_normalize_attribution(cl, Some(&case.speaker), &verbs);
                }

                let split_ok = !parsed.claims.is_empty()
                    && parsed.claims.iter().any(|cl| {
                        !cl.attribution.is_empty()
                            && cl.attribution.iter().all(|a| {
                                a.asserted_part.as_deref().map(|s| !s.is_empty()).unwrap_or(false)
                                    && a.attributed_part.as_deref().map(|s| !s.is_empty()).unwrap_or(false)
                            })
                    });
                if split_ok {
                    split_pass_count += 1;
                }
                results.push(serde_json::json!({
                    "id": case.id,
                    "expected_actor": case.expected_actor,
                    "expected_act": case.expected_act,
                    "first_try_valid": validated.first_try_valid,
                    "split_ok": split_ok,
                    "claims": parsed.claims,
                }));
            }
            stop_managed_llama_server().await;

            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "total_traps": cases.len(),
                    "split_pass_count": split_pass_count,
                    "split_pass_rate": if cases.is_empty() { 1.0 } else { split_pass_count as f64 / cases.len() as f64 },
                    "first_try_valid_count": first_try_count,
                    "cases": results,
                }))?
            );
        }
    }

    Ok(())
}
