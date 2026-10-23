use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use leadfinder_core::VideoFilter;
use leadfinder_pipeline::{
    build_router, render_spike_markdown, run_discovery_spike, PipelineContext,
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
            ensure_webview2: _,
            with_models: _,
            calibrate: _,
        } => {
            let status = serde_json::json!({
                "yt_dlp": ctx.paths.resolve_binary("yt-dlp"),
                "ffmpeg": ctx.paths.resolve_binary("ffmpeg"),
                "ffprobe": ctx.paths.resolve_binary("ffprobe"),
                "whisper_cli": ctx.paths.resolve_binary("whisper-cli"),
                "tesseract": ctx.paths.resolve_binary("tesseract"),
                "llama_server": ctx.paths.resolve_binary("llama-server"),
            });
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
    }

    Ok(())
}
