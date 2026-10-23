use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use leadfinder_core::VideoFilter;
use leadfinder_pipeline::{
    build_router, render_spike_markdown, run_discovery_spike, run_media_retention_janitor,
    PipelineContext,
};
use leadfinder_providers::{
    global_lexicon, inspect_dependencies, install_missing_dependencies,
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
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "processed_count": processed.len(),
                    "error_count": errors.len(),
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
            calibrate: _,
            check_only,
        } => {
            let status = if check_only {
                inspect_dependencies(&ctx.paths)
            } else {
                install_missing_dependencies(&ctx.paths, ensure_webview2, with_models).await?
            };
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
    }

    Ok(())
}
