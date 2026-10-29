<script lang="ts">
  import { onMount } from 'svelte';
  import {
    fetchCalibrationStatus,
    triggerBenchmark,
    updateOverrides,
  } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { BenchmarkProgress, BenchmarkStatusResponse, HardwareProfile, PowerInfo } from '../lib/types';
  import { getCapacityTierInfo } from '../lib/types';

  let {
    currentLang,
    onOpenBenchmarkModal,
  }: {
    currentLang: Language;
    onOpenBenchmarkModal?: () => void;
  } = $props();

  const t = $derived(translations[currentLang]);

  let status = $state<BenchmarkStatusResponse | null>(null);
  let isCalibrating = $state(false);
  let isSavingOverrides = $state(false);

  let asrOverride = $state('');
  let llmProviderOverride = $state('');
  let llmModelOverride = $state('');
  let ocrOverride = $state('');

  const activeProfile = $derived(
    status?.active_profile || status?.ac_profile || status?.battery_profile || null
  );

  const power = $derived<PowerInfo | null>(status?.power ?? null);
  const progress = $derived<BenchmarkProgress | null>(status?.progress ?? null);

  const activeTier = $derived(
    activeProfile?.capacity_per_day
      ? getCapacityTierInfo(activeProfile.capacity_per_day, currentLang)
      : null
  );

  const acTier = $derived(
    status?.ac_profile?.capacity_per_day
      ? getCapacityTierInfo(status.ac_profile.capacity_per_day, currentLang)
      : null
  );

  const batteryTier = $derived(
    status?.battery_profile?.capacity_per_day
      ? getCapacityTierInfo(status.battery_profile.capacity_per_day, currentLang)
      : null
  );

  onMount(async () => {
    await loadStatus();
  });

  $effect(() => {
    let timer: ReturnType<typeof setInterval> | null = null;
    if (status?.progress?.is_running) {
      timer = setInterval(async () => {
        try {
          status = await fetchCalibrationStatus();
          populateOverrides();
        } catch {}
      }, 1000);
    }
    return () => {
      if (timer) clearInterval(timer);
    };
  });

  async function loadStatus() {
    try {
      status = await fetchCalibrationStatus();
      populateOverrides();
    } catch {}
  }

  function populateOverrides() {
    const prof = activeProfile;
    if (prof?.chosen.effective) {
      asrOverride = prof.chosen.overrides?.asr_model || prof.chosen.effective.asr_model || '';
      llmProviderOverride = prof.chosen.overrides?.llm_provider || prof.chosen.effective.llm_provider || '';
      llmModelOverride = prof.chosen.overrides?.llm_model || prof.chosen.effective.llm_model || '';
      ocrOverride = prof.chosen.overrides?.ocr_engine || prof.chosen.effective.ocr_engine || '';
    }
  }

  async function handleStartBenchmark(targetPower: 'ac' | 'battery') {
    isCalibrating = true;
    try {
      const res = await triggerBenchmark({ power_source: targetPower, force_rerun: true });
      if (status && res?.progress) {
        status.progress = res.progress;
      }
      await loadStatus();
      if (onOpenBenchmarkModal) {
        onOpenBenchmarkModal();
      }
    } finally {
      isCalibrating = false;
    }
  }

  async function handleSaveOverrides() {
    isSavingOverrides = true;
    try {
      const overrides: Record<string, string> = {};
      if (asrOverride) overrides.asr_model = asrOverride;
      if (llmProviderOverride) overrides.llm_provider = llmProviderOverride;
      if (llmModelOverride) overrides.llm_model = llmModelOverride;
      if (ocrOverride) overrides.ocr_engine = ocrOverride;

      const updated = await updateOverrides(overrides);
      if (status) {
        status.active_profile = updated;
      }
    } finally {
      isSavingOverrides = false;
    }
  }
</script>

<div class="screen-container">
  <div class="screen-header">
    <div class="header-left">
      <h2>{t.tabCalibration}</h2>
      <span class="sub-text">
        {currentLang === 'bg'
          ? 'Двупрофилно хардуерно калибриране: Мрежово захранване (AC) и Батерия'
          : 'Dual-Profile Hardware Calibration: AC Power vs. Battery Mode'}
      </span>
    </div>
    {#if activeProfile}
      <div class="header-cap-group">
        {#if activeTier}
          <span class="tier-pill tier-{activeTier.badgeClass}">
            {activeTier.icon} {activeTier.tierName}
          </span>
        {/if}
        <span class="capacity-pill">
          {Math.round(activeProfile.capacity_per_day).toLocaleString()} {currentLang === 'bg' ? 'видеа / ден' : 'videos / day'}
        </span>
      </div>
    {/if}
  </div>

  <div class="power-status-banner">
    <div class="psb-icon">
      {power?.power_source === 'battery' ? '🔋' : '⚡'}
    </div>
    <div class="psb-content">
      <div class="psb-title-row">
        <strong class="psb-title">
          {power?.power_source === 'battery'
            ? (currentLang === 'bg' ? 'Текущ режим: Работа на батерия' : 'Current Mode: Battery Power')
            : (currentLang === 'bg' ? 'Текущ режим: Мрежово захранване (AC Plugged In)' : 'Current Mode: AC Power')}
        </strong>
        {#if power?.battery_percent !== null && power?.battery_percent !== undefined}
          <span class="battery-pct-pill">{power.battery_percent}%</span>
        {/if}
      </div>
      <p class="psb-desc">
        {currentLang === 'bg'
          ? 'LeadFinder съхранява 2 отделни калибрационни профила. При включване на зарядното системата автоматично задейства високоскоростния AC профил, а на батерия ограничава фоновата конкурентност за пестене на енергия.'
          : 'LeadFinder maintains 2 distinct profiles. When plugged into AC power, high-throughput settings run; on battery, concurrency is tuned down to preserve battery life.'}
      </p>
    </div>
  </div>

  {#if progress?.is_running}
    <div class="active-progress-card">
      <div class="apc-top">
        <div class="apc-left">
          <span class="pulsing-dot"></span>
          <span class="apc-title">
            {currentLang === 'bg' ? 'В процес на хардуерен бенчмарк...' : 'Hardware benchmark running...'}
          </span>
          <span class="apc-stage mono">
            ({progress.stage_index}/{progress.total_stages}) {currentLang === 'bg' ? progress.status_text : (progress.status_text_en || progress.status_text)}
          </span>
        </div>
        <div class="apc-right">
          <span class="apc-pct">{progress.progress_pct}%</span>
          {#if onOpenBenchmarkModal}
            <button class="btn btn-secondary btn-sm" onclick={onOpenBenchmarkModal}>
              🔍 {currentLang === 'bg' ? 'Подробен изглед' : 'Open Details'}
            </button>
          {/if}
        </div>
      </div>
      <div class="progress-track">
        <div class="progress-bar-fill" style="width: {Math.max(5, Math.min(100, progress.progress_pct))}%"></div>
      </div>
    </div>
  {/if}

  <div class="dual-profiles-grid">
    <div class="profile-card" class:active-mode={power?.power_source !== 'battery'}>
      <div class="pc-header">
        <div class="pc-title-group">
          <span class="pc-icon">⚡</span>
          <div>
            <h4 class="pc-name">{currentLang === 'bg' ? 'Профил: Мрежово захранване (AC)' : 'Profile: AC Power'}</h4>
            <span class="pc-sub">{currentLang === 'bg' ? 'Максимална скорост и паралелизъм' : 'High throughput & concurrency'}</span>
          </div>
        </div>
        {#if power?.power_source !== 'battery'}
          <span class="badge-active-now">🟢 {currentLang === 'bg' ? 'АКТИВЕН В МОМЕНТА' : 'ACTIVE NOW'}</span>
        {/if}
      </div>

      {#if status?.ac_profile}
        <div class="pc-metrics">
          <div class="metric-row">
            <span class="m-label">{currentLang === 'bg' ? 'Дневен капацитет:' : 'Daily Capacity:'}</span>
            <span class="m-val highlight">
              {Math.round(status.ac_profile.capacity_per_day).toLocaleString()} {currentLang === 'bg' ? 'видеа/24ч' : 'vids/24h'}
            </span>
          </div>
          <div class="metric-row">
            <span class="m-label">Whisper ASR:</span>
            <span class="m-val mono">{status.ac_profile.chosen.effective.asr_model}</span>
          </div>
          <div class="metric-row">
            <span class="m-label">LLM Анализ:</span>
            <span class="m-val mono">{status.ac_profile.chosen.effective.llm_model}</span>
          </div>
          <div class="metric-row">
            <span class="m-label">{currentLang === 'bg' ? 'Паралелни нишки:' : 'Concurrency:'}</span>
            <span class="m-val">
              ASR: {status.ac_profile.chosen.effective.concurrency.extract} | Claims: {status.ac_profile.chosen.effective.concurrency.claims} | Verify: {status.ac_profile.chosen.effective.concurrency.verify}
            </span>
          </div>
          <div class="metric-row">
            <span class="m-label">{currentLang === 'bg' ? 'Измерен на:' : 'Calibrated at:'}</span>
            <span class="m-val text-muted">{new Date(status.ac_profile.measured_at).toLocaleString()}</span>
          </div>
        </div>
        {#if acTier}
          <div class="tier-card-snippet tier-{acTier.badgeClass}">
            <div class="tcs-top">
              <span class="tcs-badge">{acTier.icon} {acTier.tierName}</span>
              <span class="tcs-target">{acTier.percentOfTarget}% {currentLang === 'bg' ? 'от таргета' : 'of target'}</span>
            </div>
            <p class="tcs-desc">{acTier.explanation}</p>
            <div class="tcs-stats">
              <span>⏱️ ~{acTier.secondsPerVideo}s / {currentLang === 'bg' ? 'видео' : 'video'}</span>
              <span>📺 ~{acTier.equivHoursPerDay}h {currentLang === 'bg' ? 'говор/24ч' : 'audio/24h'}</span>
              <span>👥 ~{acTier.equivChannels} {currentLang === 'bg' ? 'канала' : 'creators'}</span>
            </div>
          </div>
        {/if}
        <button
          class="btn btn-secondary mt-3"
          onclick={() => handleStartBenchmark('ac')}
          disabled={isCalibrating || progress?.is_running}
        >
          ⚡ {currentLang === 'bg' ? 'Повтори AC бенчмарка' : 'Re-run AC Benchmark'}
        </button>
      {:else}
        <div class="empty-profile-state">
          <p>{currentLang === 'bg' ? 'Все още няма калибриран профил за мрежово захранване.' : 'No AC power profile calibrated yet.'}</p>
          <button
            class="btn btn-primary"
            onclick={() => handleStartBenchmark('ac')}
            disabled={isCalibrating || progress?.is_running}
          >
            ⚡ {currentLang === 'bg' ? 'Стартирай AC бенчмарк' : 'Run AC Benchmark'}
          </button>
        </div>
      {/if}
    </div>

    <div class="profile-card" class:active-mode={power?.power_source === 'battery'}>
      <div class="pc-header">
        <div class="pc-title-group">
          <span class="pc-icon">🔋</span>
          <div>
            <h4 class="pc-name">{currentLang === 'bg' ? 'Профил: Работа на батерия' : 'Profile: Battery Mode'}</h4>
            <span class="pc-sub">{currentLang === 'bg' ? 'Енергоспестяващ режим за лаптопи' : 'Power-saver mode for laptops'}</span>
          </div>
        </div>
        {#if power?.power_source === 'battery'}
          <span class="badge-active-now">🟢 {currentLang === 'bg' ? 'АКТИВЕН В МОМЕНТА' : 'ACTIVE NOW'}</span>
        {/if}
      </div>

      {#if status?.battery_profile}
        <div class="pc-metrics">
          <div class="metric-row">
            <span class="m-label">{currentLang === 'bg' ? 'Дневен капацитет:' : 'Daily Capacity:'}</span>
            <span class="m-val highlight">
              {Math.round(status.battery_profile.capacity_per_day).toLocaleString()} {currentLang === 'bg' ? 'видеа/24ч' : 'vids/24h'}
            </span>
          </div>
          <div class="metric-row">
            <span class="m-label">Whisper ASR:</span>
            <span class="m-val mono">{status.battery_profile.chosen.effective.asr_model}</span>
          </div>
          <div class="metric-row">
            <span class="m-label">LLM Анализ:</span>
            <span class="m-val mono">{status.battery_profile.chosen.effective.llm_model}</span>
          </div>
          <div class="metric-row">
            <span class="m-label">{currentLang === 'bg' ? 'Паралелни нишки:' : 'Concurrency:'}</span>
            <span class="m-val">
              ASR: {status.battery_profile.chosen.effective.concurrency.extract} | Claims: {status.battery_profile.chosen.effective.concurrency.claims} | Verify: {status.battery_profile.chosen.effective.concurrency.verify}
            </span>
          </div>
          <div class="metric-row">
            <span class="m-label">{currentLang === 'bg' ? 'Измерен на:' : 'Calibrated at:'}</span>
            <span class="m-val text-muted">{new Date(status.battery_profile.measured_at).toLocaleString()}</span>
          </div>
        </div>
        {#if batteryTier}
          <div class="tier-card-snippet tier-{batteryTier.badgeClass}">
            <div class="tcs-top">
              <span class="tcs-badge">{batteryTier.icon} {batteryTier.tierName}</span>
              <span class="tcs-target">{batteryTier.percentOfTarget}% {currentLang === 'bg' ? 'от таргета' : 'of target'}</span>
            </div>
            <p class="tcs-desc">{batteryTier.explanation}</p>
            <div class="tcs-stats">
              <span>⏱️ ~{batteryTier.secondsPerVideo}s / {currentLang === 'bg' ? 'видео' : 'video'}</span>
              <span>📺 ~{batteryTier.equivHoursPerDay}h {currentLang === 'bg' ? 'говор/24ч' : 'audio/24h'}</span>
              <span>👥 ~{batteryTier.equivChannels} {currentLang === 'bg' ? 'канала' : 'creators'}</span>
            </div>
          </div>
        {/if}
        <button
          class="btn btn-secondary mt-3"
          onclick={() => handleStartBenchmark('battery')}
          disabled={isCalibrating || progress?.is_running}
        >
          🔋 {currentLang === 'bg' ? 'Повтори бенчмарка за батерия' : 'Re-run Battery Benchmark'}
        </button>
      {:else}
        <div class="empty-profile-state">
          <p>{currentLang === 'bg' ? 'Все още няма калибриран профил за работа на батерия.' : 'No battery profile calibrated yet.'}</p>
          <button
            class="btn btn-primary"
            onclick={() => handleStartBenchmark('battery')}
            disabled={isCalibrating || progress?.is_running}
          >
            🔋 {currentLang === 'bg' ? 'Стартирай бенчмарк за батерия' : 'Run Battery Benchmark'}
          </button>
        </div>
      {/if}
    </div>
  </div>

  <div class="grid-layout">
    <div class="section-card">
      <h3>{currentLang === 'bg' ? 'Открит хардуер на системата' : 'Detected System Hardware'}</h3>
      {#if activeProfile}
        <div class="hw-spec-list">
          <div class="spec-row">
            <span class="spec-label">CPU:</span>
            <span class="spec-val">{activeProfile.hardware.cpu.brand} ({activeProfile.hardware.cpu.physical_cores}C / {activeProfile.hardware.cpu.logical_cores}T)</span>
          </div>
          <div class="spec-row">
            <span class="spec-label">RAM:</span>
            <span class="spec-val">{(activeProfile.hardware.ram_mb / 1024).toFixed(1)} GB</span>
          </div>
          {#each activeProfile.hardware.gpus as gpu}
            <div class="spec-row">
              <span class="spec-label">GPU:</span>
              <span class="spec-val">{gpu.vendor} {gpu.name} ({(gpu.vram_mb / 1024).toFixed(1)} GB VRAM)</span>
            </div>
          {/each}
          <div class="spec-row">
            <span class="spec-label">{currentLang === 'bg' ? 'Бекенди:' : 'Compute Backends:'}</span>
            <span class="spec-val">{activeProfile.hardware.compute_backends.join(', ')}</span>
          </div>
        </div>
      {:else}
        <p class="text-muted">{currentLang === 'bg' ? 'Хардуерът ще бъде идентифициран при първото калибриране.' : 'Hardware specs will be probed upon initial calibration.'}</p>
      {/if}
    </div>

    <div class="section-card">
      <h3>{currentLang === 'bg' ? 'Ръчни настройки и презаписване' : 'Effective Models & Overrides'}</h3>
      <div class="overrides-form">
        <label>
          <span class="input-label">Whisper ASR Model:</span>
          <input type="text" bind:value={asrOverride} placeholder="large-v3-turbo, small, base..." />
        </label>

        <label>
          <span class="input-label">LLM Provider:</span>
          <select bind:value={llmProviderOverride}>
            <option value="local">local (llama.cpp)</option>
            <option value="gemini">gemini (Google Gemini)</option>
            <option value="heuristics">heuristics (offline)</option>
          </select>
        </label>

        <label>
          <span class="input-label">LLM Model (GGUF):</span>
          <input type="text" bind:value={llmModelOverride} placeholder="BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf" />
        </label>

        <label>
          <span class="input-label">OCR Engine:</span>
          <select bind:value={ocrOverride}>
            <option value="tesseract">Tesseract (bul+eng)</option>
            <option value="paddle_onnx">PaddleOCR ONNX (RapidOCR)</option>
            <option value="combined">Tesseract + RapidOCR (combined)</option>
          </select>
        </label>

        <button class="btn btn-primary mt-2" onclick={handleSaveOverrides} disabled={isSavingOverrides}>
          {isSavingOverrides ? '...' : (currentLang === 'bg' ? 'Запази промените' : 'Save Overrides')}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .screen-container {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem;
    background: var(--bg-canvas);
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .screen-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .header-left h2 {
    margin: 0 0 0.25rem 0;
    font-size: 1.35rem;
    color: var(--text-heading);
  }

  .sub-text {
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .header-cap-group {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .tier-pill {
    padding: 0.35rem 0.8rem;
    border-radius: 999px;
    font-size: 0.82rem;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }

  .tier-pill.tier-ultra {
    background: rgba(168, 85, 247, 0.18);
    border: 1px solid rgba(168, 85, 247, 0.4);
    color: #c084fc;
  }

  .tier-pill.tier-high {
    background: rgba(59, 130, 246, 0.18);
    border: 1px solid rgba(59, 130, 246, 0.4);
    color: #60a5fa;
  }

  .tier-pill.tier-recommended {
    background: rgba(16, 185, 129, 0.18);
    border: 1px solid rgba(16, 185, 129, 0.4);
    color: #34d399;
  }

  .tier-pill.tier-moderate {
    background: rgba(245, 158, 11, 0.18);
    border: 1px solid rgba(245, 158, 11, 0.4);
    color: #fbbf24;
  }

  .tier-pill.tier-basic {
    background: rgba(107, 114, 128, 0.18);
    border: 1px solid rgba(107, 114, 128, 0.4);
    color: #9ca3af;
  }

  .capacity-pill {
    padding: 0.4rem 0.9rem;
    background: rgba(16, 185, 129, 0.15);
    border: 1px solid rgba(16, 185, 129, 0.35);
    border-radius: 999px;
    font-size: 0.88rem;
    font-weight: 600;
    color: #34d399;
  }

  .tier-card-snippet {
    margin-top: 0.75rem;
    padding: 0.85rem;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color);
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .tier-card-snippet.tier-ultra {
    background: rgba(168, 85, 247, 0.08);
    border-color: rgba(168, 85, 247, 0.3);
  }

  .tier-card-snippet.tier-high {
    background: rgba(59, 130, 246, 0.08);
    border-color: rgba(59, 130, 246, 0.3);
  }

  .tier-card-snippet.tier-recommended {
    background: rgba(16, 185, 129, 0.08);
    border-color: rgba(16, 185, 129, 0.3);
  }

  .tier-card-snippet.tier-moderate {
    background: rgba(245, 158, 11, 0.08);
    border-color: rgba(245, 158, 11, 0.3);
  }

  .tier-card-snippet.tier-basic {
    background: rgba(107, 114, 128, 0.08);
    border-color: rgba(107, 114, 128, 0.3);
  }

  .tcs-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .tcs-badge {
    font-weight: 700;
    font-size: 0.85rem;
    color: var(--text-heading);
  }

  .tcs-target {
    font-size: 0.75rem;
    font-weight: 600;
    color: #34d399;
  }

  .tcs-desc {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.35;
    color: var(--text-muted);
  }

  .tcs-stats {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    font-size: 0.75rem;
    color: var(--text-body);
    font-weight: 500;
    padding-top: 0.25rem;
    border-top: 1px dashed rgba(255, 255, 255, 0.08);
  }

  .power-status-banner {
    display: flex;
    align-items: flex-start;
    gap: 1rem;
    padding: 1rem 1.25rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color);
    border-radius: 12px;
  }

  .psb-icon {
    font-size: 1.75rem;
    width: 44px;
    height: 44px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.06);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .psb-content {
    flex: 1;
  }

  .psb-title-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .psb-title {
    color: var(--text-heading);
    font-size: 0.95rem;
  }

  .battery-pct-pill {
    font-size: 0.78rem;
    padding: 0.15rem 0.5rem;
    border-radius: 999px;
    background: rgba(16, 185, 129, 0.18);
    color: #34d399;
    font-weight: 600;
  }

  .psb-desc {
    margin: 0.3rem 0 0 0;
    font-size: 0.82rem;
    color: var(--text-muted);
    line-height: 1.45;
  }

  .active-progress-card {
    background: rgba(59, 130, 246, 0.08);
    border: 1px solid rgba(59, 130, 246, 0.3);
    border-radius: 10px;
    padding: 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .apc-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .apc-left {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .pulsing-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #60a5fa;
    box-shadow: 0 0 8px #60a5fa;
    animation: pulse 1.5s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.4; transform: scale(0.8); }
  }

  .apc-title {
    font-weight: 600;
    color: #93c5fd;
    font-size: 0.88rem;
  }

  .apc-stage {
    font-size: 0.82rem;
    color: var(--text-muted);
  }

  .apc-right {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .apc-pct {
    font-weight: 700;
    color: #60a5fa;
    font-size: 0.92rem;
  }

  .progress-track {
    width: 100%;
    height: 8px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 999px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #3b82f6, #10b981);
    transition: width 0.3s ease;
  }

  .dual-profiles-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.25rem;
  }

  @media (max-width: 860px) {
    .dual-profiles-grid {
      grid-template-columns: 1fr;
    }
  }

  .profile-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 1rem;
    transition: border-color 0.2s;
  }

  .profile-card.active-mode {
    border-color: #3b82f6;
    box-shadow: 0 0 16px rgba(59, 130, 246, 0.15);
  }

  .pc-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .pc-title-group {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .pc-icon {
    font-size: 1.5rem;
  }

  .pc-name {
    margin: 0;
    font-size: 1rem;
    color: var(--text-heading);
  }

  .pc-sub {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .badge-active-now {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 0.2rem 0.55rem;
    border-radius: 999px;
    background: rgba(16, 185, 129, 0.15);
    color: #34d399;
    border: 1px solid rgba(16, 185, 129, 0.3);
  }

  .pc-metrics {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    background: rgba(0, 0, 0, 0.15);
    border-radius: 8px;
    padding: 0.85rem;
  }

  .metric-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.82rem;
  }

  .m-label {
    color: var(--text-muted);
  }

  .m-val {
    color: var(--text-body);
    font-weight: 500;
  }

  .m-val.highlight {
    color: #34d399;
    font-weight: 700;
    font-size: 0.9rem;
  }

  .empty-profile-state {
    padding: 1.5rem;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .grid-layout {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.25rem;
  }

  @media (max-width: 860px) {
    .grid-layout {
      grid-template-columns: 1fr;
    }
  }

  .section-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.25rem;
  }

  .section-card h3 {
    margin: 0 0 1rem 0;
    font-size: 1rem;
    color: var(--text-heading);
  }

  .hw-spec-list {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .spec-row {
    display: flex;
    justify-content: space-between;
    font-size: 0.84rem;
    padding-bottom: 0.4rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .spec-label {
    color: var(--text-muted);
  }

  .spec-val {
    color: var(--text-body);
    font-weight: 500;
  }

  .overrides-form {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .overrides-form label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .input-label {
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .overrides-form input,
  .overrides-form select {
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    background: var(--bg-canvas);
    border: 1px solid var(--border-color);
    color: var(--text-body);
    font-size: 0.84rem;
    outline: none;
  }

  .overrides-form input:focus,
  .overrides-form select:focus {
    border-color: #3b82f6;
  }

  .mono {
    font-family: monospace;
  }

  .text-muted {
    color: var(--text-muted);
  }

  .mt-2 { margin-top: 0.5rem; }
  .mt-3 { margin-top: 0.75rem; }

  .btn {
    padding: 0.5rem 1rem;
    border-radius: 8px;
    font-size: 0.84rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: background 0.15s, opacity 0.15s;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-sm {
    padding: 0.3rem 0.7rem;
    font-size: 0.78rem;
  }

  .btn-primary {
    background: #3b82f6;
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #2563eb;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-body);
    border: 1px solid var(--border-color);
  }

  .btn-secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.14);
  }
</style>
