use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sysinfo::{Disks, System};
use tokio::process::{Child, Command};
use leadfinder_core::{
    resolve_secret, AppConfig, Database, HardwareProfile, RuntimePaths, TranscribeInput,
    Transcriber,
};
use leadfinder_providers::{compute_wer, WhisperCppTranscriber};

pub fn crate_ready() -> &'static str {
    leadfinder_providers::crate_ready()
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SystemPowerStatus {
    pub ac_line_status: u8,
    pub battery_flag: u8,
    pub battery_life_percent: u8,
    pub system_status_flag: u8,
    pub battery_life_time: u32,
    pub battery_full_life_time: u32,
}

#[cfg(windows)]
extern "system" {
    fn GetSystemPowerStatus(lpSystemPowerStatus: *mut SystemPowerStatus) -> i32;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerInfo {
    pub power_source: String,
    pub ac_online: bool,
    pub battery_percent: Option<u8>,
    pub has_battery: bool,
    pub battery_life_time_seconds: Option<u32>,
}

pub fn detect_power_info() -> PowerInfo {
    #[cfg(windows)]
    {
        let mut status = SystemPowerStatus {
            ac_line_status: 255,
            battery_flag: 255,
            battery_life_percent: 255,
            system_status_flag: 0,
            battery_life_time: 0,
            battery_full_life_time: 0,
        };
        let ok = unsafe { GetSystemPowerStatus(&mut status) };
        if ok != 0 {
            let ac_online = status.ac_line_status != 0;
            let has_battery = (status.battery_flag & 128) == 0 && status.battery_flag != 255;
            let battery_percent = if status.battery_life_percent <= 100 {
                Some(status.battery_life_percent)
            } else {
                None
            };
            let power_source = if !ac_online && has_battery {
                "battery".to_string()
            } else {
                "ac".to_string()
            };
            let battery_life_time_seconds = if status.battery_life_time != u32::MAX {
                Some(status.battery_life_time)
            } else {
                None
            };
            return PowerInfo {
                power_source,
                ac_online,
                battery_percent,
                has_battery,
                battery_life_time_seconds,
            };
        }
    }

    PowerInfo {
        power_source: "ac".to_string(),
        ac_online: true,
        battery_percent: None,
        has_battery: false,
        battery_life_time_seconds: None,
    }
}

pub fn default_power_source() -> String {
    "ac".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProgressUpdate {
    pub stage: String,
    pub stage_index: usize,
    pub total_stages: usize,
    pub percent: usize,
    pub message_bg: String,
    pub message_en: String,
    pub current_model: Option<String>,
    pub asr_benchmark: Option<AsrCandidateBenchmark>,
    pub llm_benchmark: Option<LlmCandidateBenchmark>,
    pub power_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkState {
    pub is_running: bool,
    pub power_source: String,
    pub current_stage: String,
    pub stage_index: usize,
    pub total_stages: usize,
    pub progress_pct: usize,
    pub status_text: String,
    pub status_text_en: String,
    pub current_model: Option<String>,
    pub asr_results: Vec<AsrCandidateBenchmark>,
    pub llm_results: Vec<LlmCandidateBenchmark>,
    pub final_capacity: Option<f64>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl Default for BenchmarkState {
    fn default() -> Self {
        Self {
            is_running: false,
            power_source: "ac".to_string(),
            current_stage: "idle".to_string(),
            stage_index: 0,
            total_stages: 4,
            progress_pct: 0,
            status_text: "Готовност за хардуерно калибриране".to_string(),
            status_text_en: "Ready for calibration".to_string(),
            current_model: None,
            asr_results: Vec::new(),
            llm_results: Vec::new(),
            final_capacity: None,
            error: None,
            started_at: None,
            completed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSpec {
    pub vendor: String,
    pub name: String,
    pub vram_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSpec {
    pub cpu_model: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub total_ram_mb: u64,
    pub available_ram_mb: u64,
    pub free_disk_mb: u64,
    pub gpus: Vec<GpuSpec>,
    pub compute_backends: Vec<String>,
    pub force_cpu: bool,
    #[serde(default = "default_power_source")]
    pub power_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrCandidateBenchmark {
    pub model: String,
    pub elapsed_ms: u64,
    pub rtf: f64,
    pub wer: f64,
    pub raw_capacity_per_day: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCandidateBenchmark {
    pub model: String,
    pub param_billions: f64,
    pub backend: String,
    pub runs: usize,
    pub valid_json_runs: usize,
    pub json_validity_rate: f64,
    pub tokens_per_second: f64,
    pub mean_call_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalibrationOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asr_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asr_concurrency: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_concurrency: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageSelection {
    pub compute_backend: String,
    pub asr_engine: String,
    pub asr_model: String,
    pub llm_provider: String,
    pub llm_model: String,
    pub ocr_engine: String,
    pub asr_concurrency: usize,
    pub ocr_concurrency: usize,
    pub llm_concurrency: usize,
    pub heuristics_only_banner: bool,
    pub recommend_cloud_assist: bool,
    pub capacity_shortfall_per_day: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChosenProfilePayload {
    pub recommended: StageSelection,
    pub overrides: CalibrationOverrides,
    pub effective: StageSelection,
    pub asr_benchmarks: Vec<AsrCandidateBenchmark>,
    pub llm_benchmarks: Vec<LlmCandidateBenchmark>,
    pub effective_overhead_seconds: f64,
    pub estimated_seconds_per_video: f64,
    #[serde(default = "default_power_source")]
    pub power_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationCase {
    pub id: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub expected: Value,
}

pub async fn detect_hardware(paths: &RuntimePaths, force_cpu: bool) -> HardwareSpec {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let physical_cores = sys.physical_core_count().unwrap_or(1).max(1);
    let logical_cores = sys.cpus().len().max(physical_cores);
    let total_ram_mb = sys.total_memory() / (1024 * 1024);
    let available_ram_mb = sys.available_memory() / (1024 * 1024);

    let disks = Disks::new_with_refreshed_list();
    let target_canon = std::fs::canonicalize(&paths.data_dir).unwrap_or_else(|_| paths.data_dir.clone());
    let mut best_mount_len = 0usize;
    let mut free_disk_mb = 0u64;
    for d in disks.list() {
        let mp = d.mount_point();
        let m_str = mp.to_string_lossy().to_lowercase();
        let t_str = target_canon
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase();
        if t_str.starts_with(&m_str) && m_str.len() >= best_mount_len {
            best_mount_len = m_str.len();
            free_disk_mb = d.available_space() / (1024 * 1024);
        }
    }
    if free_disk_mb == 0 {
        free_disk_mb = disks
            .list()
            .first()
            .map(|d| d.available_space() / (1024 * 1024))
            .unwrap_or(0);
    }

    let (gpus, has_nvidia_cuda) = probe_gpus().await;
    let has_vulkan_dll = Path::new(r"C:\Windows\System32\vulkan-1.dll").is_file();

    let mut compute_backends = Vec::new();
    if !force_cpu && !gpus.is_empty() {
        if has_vulkan_dll {
            compute_backends.push("vulkan".to_string());
        }
        if has_nvidia_cuda {
            compute_backends.push("cuda".to_string());
        }
    }
    compute_backends.push("cpu".to_string());

    let power = detect_power_info();

    HardwareSpec {
        cpu_model,
        physical_cores,
        logical_cores,
        total_ram_mb,
        available_ram_mb,
        free_disk_mb,
        gpus,
        compute_backends,
        force_cpu,
        power_source: power.power_source,
    }
}

async fn probe_gpus() -> (Vec<GpuSpec>, bool) {
    let mut gpus = Vec::new();
    let mut has_cuda = false;

    let mut nv_cmd = Command::new("nvidia-smi");
    nv_cmd
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        nv_cmd.creation_flags(0x08000000);
    }

    if let Ok(Ok(out)) = tokio::time::timeout(Duration::from_secs(5), nv_cmd.output()).await {
        if out.status.success() {
            let txt = String::from_utf8_lossy(&out.stdout);
            for line in txt.lines() {
                if let Some((name, mem)) = line.split_once(',') {
                    let vram_mb = mem.trim().parse::<u64>().unwrap_or(0);
                    let clean_name = name.trim().to_string();
                    if !clean_name.is_empty() {
                        has_cuda = true;
                        gpus.push(GpuSpec {
                            vendor: "NVIDIA".to_string(),
                            name: clean_name,
                            vram_mb,
                        });
                    }
                }
            }
        }
    }

    if gpus.is_empty() && cfg!(windows) {
        let mut cim_cmd = Command::new("powershell");
        cim_cmd
            .args([
                "-NoProfile",
                "-Command",
                "Get-CimInstance Win32_VideoController | Select-Object Name, AdapterRAM | ConvertTo-Json -Compress",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            cim_cmd.creation_flags(0x08000000);
        }
        if let Ok(Ok(out)) = tokio::time::timeout(Duration::from_secs(6), cim_cmd.output()).await {
            if let Ok(val) = serde_json::from_slice::<Value>(&out.stdout) {
                let items = match val {
                    Value::Array(a) => a,
                    obj @ Value::Object(_) => vec![obj],
                    _ => Vec::new(),
                };
                for item in items {
                    let name = item
                        .get("Name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if name.is_empty() || name.to_lowercase().contains("basic display") {
                        continue;
                    }
                    let ram_bytes = item.get("AdapterRAM").and_then(|v| v.as_u64()).unwrap_or(0);
                    let vendor = if name.to_lowercase().contains("nvidia") {
                        "NVIDIA"
                    } else if name.to_lowercase().contains("amd") || name.to_lowercase().contains("radeon") {
                        "AMD"
                    } else if name.to_lowercase().contains("intel") {
                        "Intel"
                    } else {
                        "Unknown"
                    };
                    gpus.push(GpuSpec {
                        vendor: vendor.to_string(),
                        name,
                        vram_mb: ram_bytes / (1024 * 1024),
                    });
                }
            }
        }
    }

    (gpus, has_cuda)
}

pub type BenchmarkProgressCallback = std::sync::Arc<dyn Fn(BenchmarkProgressUpdate) + Send + Sync>;

pub async fn run_calibration(
    paths: &RuntimePaths,
    cfg: &AppConfig,
    db: &Database,
    force_cpu: bool,
) -> Result<HardwareProfile> {
    run_calibration_for_power(paths, cfg, db, force_cpu, None, None).await
}

pub async fn run_calibration_for_power(
    paths: &RuntimePaths,
    cfg: &AppConfig,
    db: &Database,
    force_cpu: bool,
    requested_power: Option<&str>,
    progress_cb: Option<BenchmarkProgressCallback>,
) -> Result<HardwareProfile> {
    let power = detect_power_info();
    let power_source = requested_power
        .unwrap_or(&power.power_source)
        .to_lowercase();

    if let Some(ref cb) = progress_cb {
        cb(BenchmarkProgressUpdate {
            stage: "detect_hw".to_string(),
            stage_index: 1,
            total_stages: 4,
            percent: 10,
            message_bg: "Откриване на хардуерни спецификации, CPU, RAM и GPU...".to_string(),
            message_en: "Detecting CPU, RAM, GPUs and compute backends...".to_string(),
            current_model: None,
            asr_benchmark: None,
            llm_benchmark: None,
            power_source: power_source.clone(),
        });
    }

    let mut hw = detect_hardware(paths, force_cpu).await;
    hw.power_source = power_source.clone();

    let sample_wav = paths.assets_dir.join("calibration").join("sample_bg_30s.wav");
    let sample_ref_path = paths.assets_dir.join("calibration").join("sample_bg_30s.txt");
    let llm_cases_path = paths.assets_dir.join("calibration").join("llm_cases.json");

    let reference_text = std::fs::read_to_string(&sample_ref_path).unwrap_or_default();
    let cases: Vec<CalibrationCase> = std::fs::read_to_string(&llm_cases_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    if let Some(ref cb) = progress_cb {
        cb(BenchmarkProgressUpdate {
            stage: "whisper_bench".to_string(),
            stage_index: 2,
            total_stages: 4,
            percent: 25,
            message_bg: "Стартиране на бенчмарк за разпознаване на реч (Whisper ASR)...".to_string(),
            message_en: "Starting Whisper ASR speech recognition benchmark...".to_string(),
            current_model: None,
            asr_benchmark: None,
            llm_benchmark: None,
            power_source: power_source.clone(),
        });
    }

    let asr_benchmarks = benchmark_whisper_candidates(
        paths,
        cfg,
        &sample_wav,
        &reference_text,
        force_cpu,
        &power_source,
        &progress_cb,
    )
    .await;

    if let Some(ref cb) = progress_cb {
        cb(BenchmarkProgressUpdate {
            stage: "llm_bench".to_string(),
            stage_index: 3,
            total_stages: 4,
            percent: 65,
            message_bg: "Стартиране на бенчмарк за LLM анализ на твърдения...".to_string(),
            message_en: "Starting LLM claim extraction benchmark...".to_string(),
            current_model: None,
            asr_benchmark: None,
            llm_benchmark: None,
            power_source: power_source.clone(),
        });
    }

    let llm_benchmarks = benchmark_llm_candidates(
        paths,
        cfg,
        &hw,
        &cases,
        &power_source,
        &progress_cb,
    )
    .await;

    if let Some(ref cb) = progress_cb {
        cb(BenchmarkProgressUpdate {
            stage: "capacity_calc".to_string(),
            stage_index: 4,
            total_stages: 4,
            percent: 90,
            message_bg: "Изчисляване на максимален дневен капацитет и паралелизъм...".to_string(),
            message_en: "Calculating daily processing capacity and concurrency...".to_string(),
            current_model: None,
            asr_benchmark: None,
            llm_benchmark: None,
            power_source: power_source.clone(),
        });
    }

    let cloud_key_available = cfg.providers.cloud_enabled && resolve_secret("GEMINI_API_KEY").is_some();

    let (recommended, base_capacity, eff_overhead, est_sec_per_vid) = select_pipeline_configuration(
        &hw,
        cfg,
        &asr_benchmarks,
        &llm_benchmarks,
        cloud_key_available,
    );

    let overrides: CalibrationOverrides = db
        .get_setting("calibration_overrides")
        .await?
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let effective = merge_overrides(&recommended, &overrides);

    let effective_asr_rtf = asr_benchmarks
        .iter()
        .find(|b| b.model == effective.asr_model)
        .map(|b| b.rtf)
        .unwrap_or_else(|| {
            asr_benchmarks
                .first()
                .map(|b| b.rtf)
                .unwrap_or(0.25)
        });
    let effective_llm_s = llm_benchmarks
        .iter()
        .find(|b| b.model == effective.llm_model)
        .map(|b| b.mean_call_seconds)
        .unwrap_or(0.0);

    let final_overhead = (cfg.capacity.overhead_seconds + effective_llm_s).max(cfg.capacity.overhead_seconds);
    let final_sec_per_vid = cfg.capacity.avg_video_seconds * effective_asr_rtf + final_overhead;
    let window_s = cfg.capacity.processing_window_hours * 3600.0;
    let final_capacity = if asr_benchmarks.is_empty() {
        base_capacity
    } else {
        ((window_s / final_sec_per_vid.max(1.0)) * 10.0).round() / 10.0
    };

    let payload = ChosenProfilePayload {
        recommended,
        overrides,
        effective,
        asr_benchmarks,
        llm_benchmarks,
        effective_overhead_seconds: (eff_overhead * 100.0).round() / 100.0,
        estimated_seconds_per_video: (est_sec_per_vid * 100.0).round() / 100.0,
        power_source: power_source.clone(),
    };

    let profile = HardwareProfile {
        hardware_json: serde_json::to_value(&hw)?,
        chosen_json: serde_json::to_value(&payload)?,
        capacity_per_day: Some(final_capacity),
        measured_at: Utc::now().to_rfc3339(),
    };

    db.save_profile_for_power(&profile, &power_source).await?;

    if let Some(ref cb) = progress_cb {
        cb(BenchmarkProgressUpdate {
            stage: "done".to_string(),
            stage_index: 4,
            total_stages: 4,
            percent: 100,
            message_bg: format!("Готово! Измерен капацитет: {:.0} видеа/ден ({})", final_capacity, power_source.to_uppercase()),
            message_en: format!("Complete! Measured capacity: {:.0} videos/day ({})", final_capacity, power_source.to_uppercase()),
            current_model: None,
            asr_benchmark: None,
            llm_benchmark: None,
            power_source: power_source.clone(),
        });
    }

    Ok(profile)
}

pub async fn apply_calibration_overrides(
    db: &Database,
    cfg: &AppConfig,
    overrides: CalibrationOverrides,
) -> Result<Option<HardwareProfile>> {
    db.set_setting("calibration_overrides", &serde_json::to_value(&overrides)?)
        .await?;
    let Some(mut existing) = db.get_profile().await? else {
        return Ok(None);
    };
    if let Ok(mut payload) = serde_json::from_value::<ChosenProfilePayload>(existing.chosen_json.clone()) {
        payload.effective = merge_overrides(&payload.recommended, &overrides);
        payload.overrides = overrides;

        let rtf = payload
            .asr_benchmarks
            .iter()
            .find(|b| b.model == payload.effective.asr_model)
            .map(|b| b.rtf)
            .unwrap_or(0.25);
        let llm_s = payload
            .llm_benchmarks
            .iter()
            .find(|b| b.model == payload.effective.llm_model)
            .map(|b| b.mean_call_seconds)
            .unwrap_or(0.0);
        let over = cfg.capacity.overhead_seconds + llm_s;
        let sec_per_vid = cfg.capacity.avg_video_seconds * rtf + over;
        let cap = ((cfg.capacity.processing_window_hours * 3600.0 / sec_per_vid.max(1.0)) * 10.0).round() / 10.0;
        payload.effective_overhead_seconds = (over * 100.0).round() / 100.0;
        payload.estimated_seconds_per_video = (sec_per_vid * 100.0).round() / 100.0;
        existing.chosen_json = serde_json::to_value(&payload)?;
        existing.capacity_per_day = Some(cap);
        db.save_profile(&existing).await?;
    }
    Ok(Some(existing))
}

pub fn merge_overrides(base: &StageSelection, overrides: &CalibrationOverrides) -> StageSelection {
    let mut out = base.clone();
    if let Some(ref m) = overrides.asr_model {
        if !m.trim().is_empty() {
            out.asr_model = m.trim().to_string();
        }
    }
    if let Some(ref p) = overrides.llm_provider {
        if !p.trim().is_empty() {
            out.llm_provider = p.trim().to_string();
            out.heuristics_only_banner = out.llm_provider == "heuristics";
        }
    }
    if let Some(ref m) = overrides.llm_model {
        if !m.trim().is_empty() {
            out.llm_model = m.trim().to_string();
        }
    }
    if let Some(ref o) = overrides.ocr_engine {
        if !o.trim().is_empty() {
            out.ocr_engine = o.trim().to_string();
        }
    }
    if let Some(c) = overrides.asr_concurrency {
        out.asr_concurrency = c.max(1);
    }
    if let Some(c) = overrides.llm_concurrency {
        out.llm_concurrency = c.max(1);
    }
    out
}

pub fn select_pipeline_configuration(
    hw: &HardwareSpec,
    cfg: &AppConfig,
    asr_benchmarks: &[AsrCandidateBenchmark],
    llm_benchmarks: &[LlmCandidateBenchmark],
    cloud_key_available: bool,
) -> (StageSelection, f64, f64, f64) {
    let compute_backend = hw
        .compute_backends
        .first()
        .cloned()
        .unwrap_or_else(|| "cpu".to_string());

    let mut sorted_llm = llm_benchmarks.to_vec();
    sorted_llm.sort_by(|a, b| {
        b.param_billions
            .partial_cmp(&a.param_billions)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.tokens_per_second
                    .partial_cmp(&a.tokens_per_second)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let qualifying_llm = sorted_llm.iter().find(|cand| {
        cand.tokens_per_second >= cfg.capacity.min_llm_tokens_per_second
            && cand.json_validity_rate >= cfg.capacity.min_llm_json_validity
    });

    let (llm_provider, llm_model, llm_call_s, heuristics_only_banner) = if let Some(win_llm) = qualifying_llm {
        (
            "local".to_string(),
            win_llm.model.clone(),
            win_llm.mean_call_seconds,
            false,
        )
    } else if cloud_key_available {
        (
            "gemini".to_string(),
            cfg.providers.gemini_model.clone(),
            2.5,
            false,
        )
    } else {
        ("heuristics".to_string(), "rule_based".to_string(), 0.05, true)
    };

    let effective_overhead = cfg.capacity.overhead_seconds + llm_call_s;
    let window_s = cfg.capacity.processing_window_hours * 3600.0;
    let target = cfg.capacity.target_videos_per_day as f64;

    let cap_for_rtf = |rtf: f64| -> f64 {
        let per_vid = cfg.capacity.avg_video_seconds * rtf + effective_overhead;
        window_s / per_vid.max(1.0)
    };

    let mut meeting_target: Vec<&AsrCandidateBenchmark> = asr_benchmarks
        .iter()
        .filter(|b| b.wer <= cfg.capacity.max_wer_ceiling && cap_for_rtf(b.rtf) >= target)
        .collect();
    meeting_target.sort_by(|a, b| {
        a.wer
            .partial_cmp(&b.wer)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| asr_quality_rank(&b.model).cmp(&asr_quality_rank(&a.model)))
            .then_with(|| a.rtf.partial_cmp(&b.rtf).unwrap_or(std::cmp::Ordering::Equal))
    });

    let (chosen_asr, chosen_rtf, recommend_cloud_assist, shortfall) = if let Some(best) = meeting_target.first() {
        (best.model.clone(), best.rtf, false, 0.0)
    } else {
        let mut under_ceiling: Vec<&AsrCandidateBenchmark> = asr_benchmarks
            .iter()
            .filter(|b| b.wer <= cfg.capacity.max_wer_ceiling)
            .collect();
        if under_ceiling.is_empty() {
            under_ceiling = asr_benchmarks.iter().collect();
        }
        under_ceiling.sort_by(|a, b| {
            cap_for_rtf(b.rtf)
                .partial_cmp(&cap_for_rtf(a.rtf))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if let Some(best_cap) = under_ceiling.first() {
            let cap = cap_for_rtf(best_cap.rtf);
            let miss = (target - cap).max(0.0);
            (best_cap.model.clone(), best_cap.rtf, miss > 0.0, (miss * 10.0).round() / 10.0)
        } else {
            (cfg.providers.whisper_model.clone(), 0.25, true, target)
        }
    };

    let est_sec_per_vid = cfg.capacity.avg_video_seconds * chosen_rtf + effective_overhead;
    let final_cap = ((window_s / est_sec_per_vid.max(1.0)) * 10.0).round() / 10.0;

    let (asr_concurrency, ocr_concurrency) = if hw.power_source.eq_ignore_ascii_case("battery") {
        (1, 1)
    } else if compute_backend != "cpu" {
        (2, (hw.physical_cores / 4).clamp(1, 4))
    } else {
        ((hw.physical_cores / 6).clamp(1, 2), (hw.physical_cores / 4).clamp(1, 4))
    };
    let llm_concurrency = 1;

    let selection = StageSelection {
        compute_backend,
        asr_engine: "whisper_cpp".to_string(),
        asr_model: chosen_asr,
        llm_provider,
        llm_model,
        ocr_engine: cfg.providers.ocr_engine.clone(),
        asr_concurrency,
        ocr_concurrency,
        llm_concurrency,
        heuristics_only_banner,
        recommend_cloud_assist,
        capacity_shortfall_per_day: shortfall,
    };

    (selection, final_cap, effective_overhead, est_sec_per_vid)
}

async fn benchmark_whisper_candidates(
    paths: &RuntimePaths,
    cfg: &AppConfig,
    sample_wav: &Path,
    reference_text: &str,
    force_cpu: bool,
    power_source: &str,
    progress_cb: &Option<BenchmarkProgressCallback>,
) -> Vec<AsrCandidateBenchmark> {
    if !sample_wav.is_file() {
        return Vec::new();
    }
    let candidate_names = ["tiny", "base", "small-q5_1", "small", "medium", "large-v3"];
    let mut results = Vec::new();
    let window_s = cfg.capacity.processing_window_hours * 3600.0;
    let total_candidates = candidate_names.len();

    for (idx, name) in candidate_names.iter().enumerate() {
        let rel = format!("whisper/ggml-{name}.bin");
        if paths.resolve_model_path(&rel).is_none() {
            continue;
        }
        let cur_pct = 25 + (idx * 35 / total_candidates);
        if let Some(ref cb) = progress_cb {
            cb(BenchmarkProgressUpdate {
                stage: "whisper_bench".to_string(),
                stage_index: 2,
                total_stages: 4,
                percent: cur_pct,
                message_bg: format!("Тестване на Whisper ASR: {}...", name),
                message_en: format!("Benchmarking Whisper ASR: {}...", name),
                current_model: Some(name.to_string()),
                asr_benchmark: None,
                llm_benchmark: None,
                power_source: power_source.to_string(),
            });
        }

        let whisper = WhisperCppTranscriber::new(
            paths,
            name.to_string(),
            Duration::from_secs(cfg.extraction.whisper_timeout_seconds),
        )
        .with_force_cpu(force_cpu);

        let started = Instant::now();
        let res = whisper
            .transcribe(&TranscribeInput {
                video_id: format!("calib_{name}"),
                audio_wav_path: Some(sample_wav.to_path_buf()),
                caption_tracks: Vec::new(),
                expected_languages: cfg.discovery.languages.clone(),
            })
            .await;
        let elapsed = started.elapsed();

        if let Ok(rec) = res {
            let rtf = (elapsed.as_secs_f64() / 30.0 * 1000.0).round() / 1000.0;
            let wer = (compute_wer(reference_text, &rec.text) * 1000.0).round() / 1000.0;
            let raw_cap = window_s / (cfg.capacity.avg_video_seconds * rtf + cfg.capacity.overhead_seconds);
            let bench = AsrCandidateBenchmark {
                model: name.to_string(),
                elapsed_ms: elapsed.as_millis() as u64,
                rtf,
                wer,
                raw_capacity_per_day: (raw_cap * 10.0).round() / 10.0,
            };
            if let Some(ref cb) = progress_cb {
                cb(BenchmarkProgressUpdate {
                    stage: "whisper_bench".to_string(),
                    stage_index: 2,
                    total_stages: 4,
                    percent: cur_pct + (35 / total_candidates).max(1),
                    message_bg: format!("Завършен Whisper {}: RTF {:.3}, WER {:.1}%", name, rtf, wer * 100.0),
                    message_en: format!("Completed Whisper {}: RTF {:.3}, WER {:.1}%", name, rtf, wer * 100.0),
                    current_model: Some(name.to_string()),
                    asr_benchmark: Some(bench.clone()),
                    llm_benchmark: None,
                    power_source: power_source.to_string(),
                });
            }
            results.push(bench);
        }
    }
    results
}

async fn benchmark_llm_candidates(
    paths: &RuntimePaths,
    _cfg: &AppConfig,
    hw: &HardwareSpec,
    cases: &[CalibrationCase],
    power_source: &str,
    progress_cb: &Option<BenchmarkProgressCallback>,
) -> Vec<LlmCandidateBenchmark> {
    let Some(llama_bin) = paths.resolve_binary("llama-server") else {
        if let Some(ref cb) = progress_cb {
            cb(BenchmarkProgressUpdate {
                stage: "llm_bench".to_string(),
                stage_index: 3,
                total_stages: 4,
                percent: 85,
                message_bg: "Локален LLM сървър не е наличен; активни са евристики и облак".to_string(),
                message_en: "Local LLM server not available; heuristics and cloud active".to_string(),
                current_model: None,
                asr_benchmark: None,
                llm_benchmark: None,
                power_source: power_source.to_string(),
            });
        }
        return Vec::new();
    };
    if cases.is_empty() {
        return Vec::new();
    }

    let mut gguf_files: Vec<PathBuf> = Vec::new();
    for dir in [
        paths.models_dir.join("llm"),
        paths.static_root.join("models").join("llm"),
    ] {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("gguf") {
                    if !gguf_files.iter().any(|x| x.file_name() == p.file_name()) {
                        gguf_files.push(p);
                    }
                }
            }
        }
    }

    if gguf_files.is_empty() {
        if let Some(ref cb) = progress_cb {
            cb(BenchmarkProgressUpdate {
                stage: "llm_bench".to_string(),
                stage_index: 3,
                total_stages: 4,
                percent: 85,
                message_bg: "Няма намерени локални GGUF модели; активен е облачен/евристичен бекенд".to_string(),
                message_en: "No local GGUF models found; cloud/heuristics backend active".to_string(),
                current_model: None,
                asr_benchmark: None,
                llm_benchmark: None,
                power_source: power_source.to_string(),
            });
        }
        return Vec::new();
    }

    gguf_files.sort_by(|a, b| {
        let pa = infer_param_billions(a);
        let pb = infer_param_billions(b);
        pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
    });

    let max_vram_mb = hw.gpus.iter().map(|g| g.vram_mb).max().unwrap_or(0);
    let gpu_active = !hw.force_cpu && hw.compute_backends.iter().any(|b| b != "cpu");
    let total_candidates = gguf_files.len();

    let mut results = Vec::new();
    for (idx, gguf) in gguf_files.into_iter().enumerate() {
        let file_mb = std::fs::metadata(&gguf).map(|m| m.len() / (1024 * 1024)).unwrap_or(0);
        if file_mb == 0 {
            continue;
        }
        let use_gpu = gpu_active && max_vram_mb >= file_mb + 512;
        if !use_gpu && hw.total_ram_mb < file_mb + 2048 {
            continue;
        }
        if hw.force_cpu && file_mb > 3500 {
            continue;
        }

        let model_name = gguf
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("local.gguf")
            .to_string();
        let params_b = infer_param_billions(&gguf);
        let backend_label = if use_gpu {
            hw.compute_backends
                .first()
                .cloned()
                .unwrap_or_else(|| "vulkan".to_string())
        } else {
            "cpu".to_string()
        };

        let cur_pct = 65 + (idx * 20 / total_candidates.max(1));
        if let Some(ref cb) = progress_cb {
            cb(BenchmarkProgressUpdate {
                stage: "llm_bench".to_string(),
                stage_index: 3,
                total_stages: 4,
                percent: cur_pct,
                message_bg: format!("Тестване на LLM модел: {}...", model_name),
                message_en: format!("Benchmarking LLM model: {}...", model_name),
                current_model: Some(model_name.clone()),
                asr_benchmark: None,
                llm_benchmark: None,
                power_source: power_source.to_string(),
            });
        }

        if let Some(bench) = run_single_llama_server_benchmark(
            &llama_bin,
            &gguf,
            &model_name,
            params_b,
            &backend_label,
            use_gpu,
            cases,
        )
        .await
        {
            if let Some(ref cb) = progress_cb {
                cb(BenchmarkProgressUpdate {
                    stage: "llm_bench".to_string(),
                    stage_index: 3,
                    total_stages: 4,
                    percent: cur_pct + (20 / total_candidates.max(1)),
                    message_bg: format!("Завършен LLM {}: {:.1} токена/сек", model_name, bench.tokens_per_second),
                    message_en: format!("Completed LLM {}: {:.1} tokens/sec", model_name, bench.tokens_per_second),
                    current_model: Some(model_name.clone()),
                    asr_benchmark: None,
                    llm_benchmark: Some(bench.clone()),
                    power_source: power_source.to_string(),
                });
            }
            results.push(bench);
        }
    }

    results
}

fn infer_param_billions(path: &Path) -> f64 {
    let fname = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if fname.contains("27b") {
        27.0
    } else if fname.contains("12b") {
        12.0
    } else if fname.contains("9b") {
        9.0
    } else if fname.contains("7b") {
        7.0
    } else if fname.contains("4b") {
        4.0
    } else if fname.contains("2.6b") || fname.contains("2b") {
        2.6
    } else {
        let mb = std::fs::metadata(path).map(|m| m.len() / (1024 * 1024)).unwrap_or(1500);
        (mb as f64 / 650.0 * 10.0).round() / 10.0
    }
}

fn asr_quality_rank(model: &str) -> u32 {
    match model {
        "large-v3" => 60,
        "large-v3-q5_0" => 55,
        "medium" => 50,
        "medium-q5_0" => 45,
        "small" => 40,
        "small-q5_1" => 35,
        "base" => 20,
        "base-q5_1" => 18,
        "tiny" => 10,
        "tiny-q5_1" => 8,
        _ => 15,
    }
}

fn pick_ephemeral_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
        .unwrap_or(18089)
}

async fn run_single_llama_server_benchmark(
    llama_bin: &Path,
    gguf_path: &Path,
    model_name: &str,
    param_billions: f64,
    backend_label: &str,
    use_gpu: bool,
    cases: &[CalibrationCase],
) -> Option<LlmCandidateBenchmark> {
    let port = pick_ephemeral_port();
    let ngl = if use_gpu { "99" } else { "0" };
    let threads = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(4, 16))
        .unwrap_or(8)
        .to_string();

    let mut cmd = Command::new(llama_bin);
    cmd.args([
        "-m",
        &gguf_path.to_string_lossy(),
        "--host",
        "127.0.0.1",
        "--port",
        &port.to_string(),
        "-ngl",
        ngl,
        "-c",
        "2048",
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

    let mut child: Child = cmd.spawn().ok()?;
    let base_url = format!("http://127.0.0.1:{port}");
    let client = Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .ok()?;

    let mut ready = false;
    for _ in 0..90 {
        if let Ok(resp) = client.get(format!("{base_url}/health")).send().await {
            if resp.status().is_success() {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    if !ready {
        let _ = child.kill().await;
        return None;
    }

    let schema = calibration_claims_json_schema();
    let runs_per_case = 3usize;
    let mut total_runs = 0usize;
    let mut valid_runs = 0usize;
    let mut tps_samples: Vec<f64> = Vec::new();
    let mut call_durations: Vec<f64> = Vec::new();

    for case in cases {
        for _ in 0..runs_per_case {
            total_runs += 1;
            let body = json!({
                "model": model_name,
                "temperature": 0.0,
                "max_tokens": 384,
                "messages": [
                    { "role": "system", "content": case.system_prompt },
                    { "role": "user", "content": case.user_prompt }
                ],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "claims_output",
                        "strict": true,
                        "schema": schema
                    }
                }
            });

            let t0 = Instant::now();
            let resp_opt = client
                .post(format!("{base_url}/v1/chat/completions"))
                .json(&body)
                .send()
                .await
                .ok();
            let call_s = t0.elapsed().as_secs_f64();
            call_durations.push(call_s);

            if let Some(resp) = resp_opt {
                if let Ok(val) = resp.json::<Value>().await {
                    let content = val
                        .get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|a| a.first())
                        .and_then(|x| x.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("");

                    if validate_claims_schema_output(content) {
                        valid_runs += 1;
                    }

                    let predicted_tps = val
                        .get("timings")
                        .and_then(|t| t.get("predicted_per_second"))
                        .and_then(|v| v.as_f64())
                        .or_else(|| {
                            let comp_toks = val
                                .get("usage")
                                .and_then(|u| u.get("completion_tokens"))
                                .and_then(|v| v.as_f64())?;
                            Some(comp_toks / call_s.max(0.05))
                        });
                    if let Some(tps) = predicted_tps {
                        if tps > 0.0 {
                            tps_samples.push(tps);
                        }
                    }
                }
            }
        }
    }

    let _ = child.kill().await;

    if total_runs == 0 {
        return None;
    }

    let mean_tps = if tps_samples.is_empty() {
        0.0
    } else {
        tps_samples.iter().sum::<f64>() / tps_samples.len() as f64
    };
    let mean_call_s = call_durations.iter().sum::<f64>() / call_durations.len().max(1) as f64;
    let validity = valid_runs as f64 / total_runs as f64;

    Some(LlmCandidateBenchmark {
        model: model_name.to_string(),
        param_billions,
        backend: backend_label.to_string(),
        runs: total_runs,
        valid_json_runs: valid_runs,
        json_validity_rate: (validity * 1000.0).round() / 1000.0,
        tokens_per_second: (mean_tps * 100.0).round() / 100.0,
        mean_call_seconds: (mean_call_s * 100.0).round() / 100.0,
    })
}

pub fn calibration_claims_json_schema() -> Value {
    json!({
        "type": "object",
        "required": ["claims", "people"],
        "additionalProperties": false,
        "properties": {
            "claims": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["text_bg", "claim_type", "speaker", "attribution", "start_ms", "end_ms", "checkworthiness"],
                    "additionalProperties": false,
                    "properties": {
                        "text_bg": { "type": "string" },
                        "claim_type": {
                            "type": "string",
                            "enum": ["factual", "attribution", "statistical", "health", "quote", "prediction", "opinion"]
                        },
                        "speaker": { "type": ["string", "null"] },
                        "attribution": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["actor", "act", "target", "about"],
                                "additionalProperties": false,
                                "properties": {
                                    "actor": { "type": "string" },
                                    "act": {
                                        "type": "string",
                                        "enum": ["said", "agreed", "proposed", "voted", "denied", "other"]
                                    },
                                    "target": { "type": ["string", "null"] },
                                    "about": { "type": "string" }
                                }
                            }
                        },
                        "start_ms": { "type": "integer" },
                        "end_ms": { "type": "integer" },
                        "checkworthiness": { "type": "number" }
                    }
                }
            },
            "people": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["name", "variants", "role_hint"],
                    "additionalProperties": false,
                    "properties": {
                        "name": { "type": "string" },
                        "variants": { "type": "array", "items": { "type": "string" } },
                        "role_hint": { "type": ["string", "null"] }
                    }
                }
            }
        }
    })
}

pub fn validate_claims_schema_output(raw: &str) -> bool {
    let trimmed = raw.trim();
    let json_slice = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };
    let Ok(val) = serde_json::from_str::<Value>(json_slice) else {
        return false;
    };
    let Some(claims) = val.get("claims").and_then(|v| v.as_array()) else {
        return false;
    };
    let Some(_people) = val.get("people").and_then(|v| v.as_array()) else {
        return false;
    };
    let valid_types = [
        "factual",
        "attribution",
        "statistical",
        "health",
        "quote",
        "prediction",
        "opinion",
    ];
    for c in claims {
        let Some(txt) = c.get("text_bg").and_then(|v| v.as_str()) else {
            return false;
        };
        if txt.trim().is_empty() {
            return false;
        }
        let Some(ct) = c.get("claim_type").and_then(|v| v.as_str()) else {
            return false;
        };
        if !valid_types.contains(&ct) {
            return false;
        }
        if c.get("checkworthiness").and_then(|v| v.as_f64()).is_none() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use leadfinder_core::load_app_config;

    #[test]
    fn selects_lowest_wer_asr_meeting_target_and_largest_valid_llm() {
        let paths = RuntimePaths::discover().unwrap();
        let cfg = load_app_config(&paths.config_dir).unwrap();
        let hw = HardwareSpec {
            cpu_model: "Test CPU".to_string(),
            physical_cores: 8,
            logical_cores: 16,
            total_ram_mb: 32768,
            available_ram_mb: 24000,
            free_disk_mb: 100000,
            gpus: vec![GpuSpec {
                vendor: "NVIDIA".to_string(),
                name: "RTX Test".to_string(),
                vram_mb: 12288,
            }],
            compute_backends: vec!["vulkan".to_string(), "cuda".to_string(), "cpu".to_string()],
            force_cpu: false,
            power_source: "ac".to_string(),
        };

        let asr = vec![
            AsrCandidateBenchmark {
                model: "tiny".to_string(),
                elapsed_ms: 900,
                rtf: 0.03,
                wer: 0.34,
                raw_capacity_per_day: 3800.0,
            },
            AsrCandidateBenchmark {
                model: "small-q5_1".to_string(),
                elapsed_ms: 3300,
                rtf: 0.11,
                wer: 0.16,
                raw_capacity_per_day: 2890.0,
            },
            AsrCandidateBenchmark {
                model: "small".to_string(),
                elapsed_ms: 3600,
                rtf: 0.12,
                wer: 0.16,
                raw_capacity_per_day: 2800.0,
            },
        ];

        let llm = vec![
            LlmCandidateBenchmark {
                model: "BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf".to_string(),
                param_billions: 4.0,
                backend: "vulkan".to_string(),
                runs: 9,
                valid_json_runs: 9,
                json_validity_rate: 1.0,
                tokens_per_second: 52.0,
                mean_call_seconds: 3.1,
            },
            LlmCandidateBenchmark {
                model: "BgGPT-Gemma-2-2B-IT-v1.0.Q4_K_M.gguf".to_string(),
                param_billions: 2.6,
                backend: "vulkan".to_string(),
                runs: 9,
                valid_json_runs: 9,
                json_validity_rate: 1.0,
                tokens_per_second: 84.0,
                mean_call_seconds: 2.0,
            },
        ];

        let (sel, cap, _, _) = select_pipeline_configuration(&hw, &cfg, &asr, &llm, false);
        assert_eq!(sel.asr_model, "small");
        assert_eq!(sel.llm_provider, "local");
        assert_eq!(sel.llm_model, "BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf");
        assert!(!sel.heuristics_only_banner);
        assert!(cap >= 300.0);
    }

    #[test]
    fn falls_back_to_heuristics_and_flags_shortfall_on_slow_cpu() {
        let paths = RuntimePaths::discover().unwrap();
        let cfg = load_app_config(&paths.config_dir).unwrap();
        let hw = HardwareSpec {
            cpu_model: "Slow CPU".to_string(),
            physical_cores: 2,
            logical_cores: 4,
            total_ram_mb: 8192,
            available_ram_mb: 4096,
            free_disk_mb: 20000,
            gpus: Vec::new(),
            compute_backends: vec!["cpu".to_string()],
            force_cpu: true,
            power_source: "ac".to_string(),
        };

        let asr = vec![AsrCandidateBenchmark {
            model: "tiny".to_string(),
            elapsed_ms: 120000,
            rtf: 4.0,
            wer: 0.32,
            raw_capacity_per_day: 227.0,
        }];
        let llm = vec![LlmCandidateBenchmark {
            model: "BgGPT-2B".to_string(),
            param_billions: 2.6,
            backend: "cpu".to_string(),
            runs: 9,
            valid_json_runs: 9,
            json_validity_rate: 1.0,
            tokens_per_second: 3.2,
            mean_call_seconds: 40.0,
        }];

        let (sel, cap, _, _) = select_pipeline_configuration(&hw, &cfg, &asr, &llm, false);
        assert_eq!(sel.llm_provider, "heuristics");
        assert!(sel.heuristics_only_banner);
        assert!(sel.recommend_cloud_assist);
        assert!(sel.capacity_shortfall_per_day > 0.0);
        assert!(cap < 300.0);

        let ov = CalibrationOverrides {
            asr_model: Some("base".to_string()),
            ..Default::default()
        };
        let merged = merge_overrides(&sel, &ov);
        assert_eq!(merged.asr_model, "base");
    }
}
