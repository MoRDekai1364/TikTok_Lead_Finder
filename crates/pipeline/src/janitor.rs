use std::path::Path;
use std::time::{Duration, SystemTime};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use leadfinder_core::{Database, VideoFilter};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JanitorReport {
    pub inspected_videos: usize,
    pub cleaned_videos: usize,
    pub bytes_reclaimed: u64,
}

pub async fn run_media_retention_janitor(
    db: &Database,
    media_root: &Path,
    retention_days: u64,
) -> Result<JanitorReport> {
    let mut report = JanitorReport::default();
    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);

    let videos = db
        .list_videos(&VideoFilter {
            limit: Some(5000),
            ..Default::default()
        })
        .await?;

    for video in videos {
        report.inspected_videos += 1;
        let vdir = media_root.join(&video.id);

        let expired_by_db = DateTime::parse_from_rfc3339(&video.discovered_at)
            .map(|dt| dt.with_timezone(&Utc) < cutoff)
            .unwrap_or(false);

        let expired_by_mtime = std::fs::metadata(&vdir)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|mt| SystemTime::now().duration_since(mt).ok())
            .map(|age| age > Duration::from_secs(retention_days * 86400))
            .unwrap_or(false);

        if (expired_by_db || expired_by_mtime) && (vdir.is_dir() || video.media_path.is_some()) {
            let reclaimed = purge_heavy_media_keep_thumb(&vdir);
            if reclaimed > 0 || video.media_path.is_some() {
                db.clear_video_media_path(&video.id).await?;
                report.cleaned_videos += 1;
                report.bytes_reclaimed += reclaimed;
            }
        }
    }

    Ok(report)
}

fn purge_heavy_media_keep_thumb(video_dir: &Path) -> u64 {
    let mut bytes = 0u64;
    if !video_dir.is_dir() {
        return 0;
    }

    if let Ok(entries) = std::fs::read_dir(video_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if path.is_dir() && name == "frames" {
                bytes += dir_size(&path);
                let _ = std::fs::remove_dir_all(&path);
            } else if path.is_file() && name != "thumb.jpg" {
                if let Ok(meta) = std::fs::metadata(&path) {
                    bytes += meta.len();
                }
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    bytes
}

fn dir_size(dir: &Path) -> u64 {
    let mut sum = 0u64;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                sum += dir_size(&p);
            } else if let Ok(m) = std::fs::metadata(&p) {
                sum += m.len();
            }
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use leadfinder_core::{Transcript, TranscriptSegment, VideoRef};

    #[tokio::test]
    async fn janitor_deletes_video_and_frames_after_retention_while_keeping_text_and_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::connect(&dir.path().join("janitor.db")).await.unwrap();
        let media_root = dir.path().join("media");

        let vref = VideoRef {
            id: "vid_old_1".to_string(),
            url: "https://www.tiktok.com/@bg/video/7100000000000000001".to_string(),
            creator_id: Some("bg".to_string()),
            creator_handle: Some("bg".to_string()),
            creator_display_name: None,
            creator_follower_count: None,
            creator_region: Some("BG".to_string()),
            caption: Some("Важен текст, който остава".to_string()),
            hashtags: vec![],
            posted_at: None,
            views: Some(5000),
            likes: None,
            shares: None,
            comments: None,
            duration_s: Some(15.0),
            sound_title: None,
            subtitle_urls: vec![],
            discovered_via: "paste".to_string(),
        };
        db.upsert_video_discovered(&vref, 0).await.unwrap();

        let vdir = media_root.join("vid_old_1");
        std::fs::create_dir_all(vdir.join("frames")).unwrap();
        let video_file = vdir.join("video.mp4");
        std::fs::write(&video_file, vec![0u8; 4096]).unwrap();
        std::fs::write(vdir.join("audio_16k.wav"), vec![0u8; 2048]).unwrap();
        std::fs::write(vdir.join("frames").join("frame_001.jpg"), vec![0u8; 1024]).unwrap();
        std::fs::write(vdir.join("thumb.jpg"), vec![1u8; 512]).unwrap();

        db.set_video_media_and_phash("vid_old_1", Some(&video_file.to_string_lossy()), Some("0123456789abcdef"))
            .await
            .unwrap();
        db.save_transcript(&Transcript {
            video_id: "vid_old_1".to_string(),
            source: "generated".to_string(),
            engine: Some("whisper_cpp".to_string()),
            model: Some("small".to_string()),
            language: Some("bg".to_string()),
            quality: Some(0.85),
            text: "Запазен транскрипт след изтриване на видео файла.".to_string(),
            segments: vec![TranscriptSegment {
                start_ms: 0,
                end_ms: 2000,
                text: "Запазен транскрипт след изтриване на видео файла.".to_string(),
                confidence: Some(0.9),
            }],
            edited_text: None,
        })
        .await
        .unwrap();

        sqlx::query("UPDATE videos SET discovered_at = '2020-01-01T00:00:00Z' WHERE id = 'vid_old_1'")
            .execute(db.pool())
            .await
            .unwrap();

        let rep = run_media_retention_janitor(&db, &media_root, 7).await.unwrap();
        assert_eq!(rep.cleaned_videos, 1);
        assert!(rep.bytes_reclaimed >= 7168);
        assert!(!video_file.exists());
        assert!(!vdir.join("frames").exists());
        assert!(vdir.join("thumb.jpg").exists());

        let tr = db.get_transcript("vid_old_1").await.unwrap().unwrap();
        assert!(tr.text.contains("Запазен транскрипт"));
        let v_after = db.get_video("vid_old_1").await.unwrap().unwrap();
        assert!(v_after.media_path.is_none());
    }
}
