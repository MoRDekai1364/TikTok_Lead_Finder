use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use image::GenericImageView;
use tokio::process::Command;
use leadfinder_core::{
    ExtractionConfig, FrameSample, PolitenessGuard, ProviderError, ProviderErrorKind, RuntimePaths,
};

#[derive(Debug, Clone)]
pub struct AcquiredMedia {
    pub video_id: String,
    pub media_dir: PathBuf,
    pub video_path: PathBuf,
    pub audio_wav_path: PathBuf,
    pub thumbnail_path: Option<PathBuf>,
    pub keyframes: Vec<FrameSample>,
    pub caption_files: Vec<(String, PathBuf)>,
    pub phash: Option<String>,
    pub duration_s: Option<f64>,
}

#[derive(Clone)]
pub struct MediaAcquirer {
    ytdlp_bin: Option<PathBuf>,
    ffmpeg_bin: Option<PathBuf>,
    ffprobe_bin: Option<PathBuf>,
    ytdlp_timeout: Duration,
    ffmpeg_timeout: Duration,
    guard: PolitenessGuard,
}

impl MediaAcquirer {
    pub fn new(
        paths: &RuntimePaths,
        ytdlp_timeout: Duration,
        extraction_cfg: &ExtractionConfig,
        guard: PolitenessGuard,
    ) -> Self {
        Self {
            ytdlp_bin: paths.resolve_binary("yt-dlp"),
            ffmpeg_bin: paths.resolve_binary("ffmpeg"),
            ffprobe_bin: paths.resolve_binary("ffprobe"),
            ytdlp_timeout,
            ffmpeg_timeout: Duration::from_secs(extraction_cfg.ffmpeg_timeout_seconds),
            guard,
        }
    }

    pub async fn acquire(
        &self,
        video_id: &str,
        url: &str,
        media_root: &Path,
        extraction_cfg: &ExtractionConfig,
    ) -> Result<AcquiredMedia, ProviderError> {
        let ytdlp = self.ytdlp_bin.as_deref().ok_or_else(|| {
            ProviderError::new(ProviderErrorKind::BinaryMissing, "yt-dlp", "yt-dlp missing", false)
        })?;
        let ffmpeg = self.ffmpeg_bin.as_deref().ok_or_else(|| {
            ProviderError::new(ProviderErrorKind::BinaryMissing, "ffmpeg", "ffmpeg missing", false)
        })?;

        let media_dir = media_root.join(video_id);
        let frames_dir = media_dir.join("frames");
        std::fs::create_dir_all(&frames_dir).map_err(|e| {
            ProviderError::new(ProviderErrorKind::Internal, "acquire", e.to_string(), false)
        })?;

        let existing_video = find_downloaded_video(&media_dir);
        let video_path = if let Some(v) = existing_video {
            v
        } else {
            let host = PolitenessGuard::extract_host(url);
            self.guard.acquire_slot(&host, "yt-dlp-download").await?;

            let out_tmpl = media_dir.join("video.%(ext)s").to_string_lossy().to_string();
            let mut cmd = Command::new(ytdlp);
            cmd.args([
                "--no-warnings",
                "--no-playlist",
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs",
                "bg.*,bul.*,en.*,eng.*,ru.*,rus.*,tr.*,tur.*,orig.*",
                "--convert-subs",
                "vtt",
                "-o",
                &out_tmpl,
                url,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

            #[cfg(windows)]
            {
                cmd.creation_flags(0x08000000);
            }

            let out = match tokio::time::timeout(self.ytdlp_timeout, cmd.output()).await {
                Ok(Ok(o)) => o,
                Ok(Err(e)) => {
                    self.guard.record_failure(&host, &e.to_string()).await;
                    return Err(ProviderError::new(
                        ProviderErrorKind::Internal,
                        "yt-dlp-download",
                        e.to_string(),
                        true,
                    ));
                }
                Err(_) => {
                    self.guard.record_failure(&host, "download timeout").await;
                    return Err(ProviderError::new(
                        ProviderErrorKind::Timeout,
                        "yt-dlp-download",
                        format!("video download timed out for {url}"),
                        true,
                    ));
                }
            };

            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let (kind, retryable) = crate::ytdlp::YtDlpClient::classify_stderr(&stderr);
                self.guard.record_failure(&host, &stderr).await;
                return Err(ProviderError::new(kind, "yt-dlp-download", stderr, retryable));
            }
            self.guard.record_success(&host).await;

            find_downloaded_video(&media_dir).ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::NotFoundOrPrivate,
                    "yt-dlp-download",
                    "yt-dlp exited 0 but no video file was produced",
                    false,
                )
            })?
        };

        let audio_wav_path = media_dir.join("audio_16k.wav");
        if !audio_wav_path.is_file() {
            let mut acmd = Command::new(ffmpeg);
            acmd.args([
                "-y",
                "-i",
                &video_path.to_string_lossy(),
                "-vn",
                "-acodec",
                "pcm_s16le",
                "-ar",
                "16000",
                "-ac",
                "1",
                &audio_wav_path.to_string_lossy(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

            #[cfg(windows)]
            {
                acmd.creation_flags(0x08000000);
            }

            let aout = tokio::time::timeout(self.ffmpeg_timeout, acmd.output())
                .await
                .map_err(|_| {
                    ProviderError::new(
                        ProviderErrorKind::Timeout,
                        "ffmpeg-audio",
                        "ffmpeg audio extraction timed out",
                        true,
                    )
                })?
                .map_err(|e| {
                    ProviderError::new(ProviderErrorKind::Internal, "ffmpeg-audio", e.to_string(), false)
                })?;

            if !aout.status.success() {
                let err_txt = String::from_utf8_lossy(&aout.stderr).trim().to_string();
                return Err(ProviderError::new(
                    ProviderErrorKind::Internal,
                    "ffmpeg-audio",
                    err_txt,
                    false,
                ));
            }
        }

        let interval = extraction_cfg.keyframe_interval_seconds.max(1);
        let max_frames = extraction_cfg.max_keyframes.max(1);
        let frame_pattern = frames_dir.join("frame_%03d.jpg");
        if collect_keyframes(&frames_dir, interval).is_empty() {
            let vf_filter = format!("fps=1/{interval},scale='min(720,iw)':-2");
            let max_str = max_frames.to_string();
            let mut fcmd = Command::new(ffmpeg);
            fcmd.args([
                "-y",
                "-i",
                &video_path.to_string_lossy(),
                "-vf",
                &vf_filter,
                "-frames:v",
                &max_str,
                "-q:v",
                "3",
                &frame_pattern.to_string_lossy(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

            #[cfg(windows)]
            {
                fcmd.creation_flags(0x08000000);
            }

            let _ = tokio::time::timeout(self.ffmpeg_timeout, fcmd.output()).await;
        }

        let keyframes = collect_keyframes(&frames_dir, interval);
        let thumbnail_path = if let Some(first) = keyframes.first() {
            let thumb = media_dir.join("thumb.jpg");
            if !thumb.is_file() {
                let _ = std::fs::copy(&first.image_path, &thumb);
            }
            Some(thumb)
        } else {
            None
        };

        let phash = keyframes
            .get(keyframes.len() / 2)
            .or_else(|| keyframes.first())
            .and_then(|f| compute_image_dhash(&f.image_path));

        let caption_files = collect_caption_files(&media_dir);
        let duration_s = self.probe_duration(&video_path).await;

        Ok(AcquiredMedia {
            video_id: video_id.to_string(),
            media_dir,
            video_path,
            audio_wav_path,
            thumbnail_path,
            keyframes,
            caption_files,
            phash,
            duration_s,
        })
    }

    async fn probe_duration(&self, video_path: &Path) -> Option<f64> {
        let ffprobe = self.ffprobe_bin.as_deref()?;
        let mut cmd = Command::new(ffprobe);
        cmd.args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            &video_path.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let out = tokio::time::timeout(Duration::from_secs(10), cmd.output()).await.ok()?.ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().ok()
    }
}

fn find_downloaded_video(media_dir: &Path) -> Option<PathBuf> {
    let rd = std::fs::read_dir(media_dir).ok()?;
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_file() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if name.starts_with("video.") && matches!(ext.as_str(), "mp4" | "webm" | "mkv" | "mov") {
                return Some(p);
            }
        }
    }
    None
}

fn collect_keyframes(frames_dir: &Path, interval_s: u64) -> Vec<FrameSample> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(frames_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jpg"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .enumerate()
        .map(|(idx, image_path)| FrameSample {
            frame_ms: (idx as i64) * (interval_s as i64) * 1000,
            image_path,
        })
        .collect()
}

fn collect_caption_files(media_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(media_dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with("video.") && (name.ends_with(".vtt") || name.ends_with(".srt")) {
            let parts: Vec<&str> = name.split('.').collect();
            let lang = if parts.len() >= 3 {
                parts[parts.len() - 2].to_string()
            } else {
                "unknown".to_string()
            };
            out.push((lang, p));
        }
    }
    out.sort_by(|a, b| {
        let rank = |l: &str| {
            let low = l.to_lowercase();
            if low.starts_with("bg") || low.starts_with("bul") {
                0
            } else if low == "orig" {
                1
            } else if low.starts_with("ru") || low.starts_with("rus") || low.starts_with("tr") || low.starts_with("tur") {
                2
            } else {
                3
            }
        };
        rank(&a.0).cmp(&rank(&b.0))
    });
    out
}

pub fn compute_image_dhash(image_path: &Path) -> Option<String> {
    let img = image::open(image_path).ok()?;
    let small = img.resize_exact(9, 8, image::imageops::FilterType::Triangle).grayscale();
    let mut hash: u64 = 0;
    let mut bit = 0;
    for y in 0..8 {
        for x in 0..8 {
            let left = small.get_pixel(x, y).0[0];
            let right = small.get_pixel(x + 1, y).0[0];
            if left > right {
                hash |= 1u64 << bit;
            }
            bit += 1;
        }
    }
    Some(format!("{hash:016x}"))
}

pub fn phash_hamming_distance(a_hex: &str, b_hex: &str) -> Option<u32> {
    let a = u64::from_str_radix(a_hex, 16).ok()?;
    let b = u64::from_str_radix(b_hex, 16).ok()?;
    Some((a ^ b).count_ones())
}
