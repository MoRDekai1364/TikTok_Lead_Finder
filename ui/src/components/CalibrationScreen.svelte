<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchProfile, triggerCalibration, updateOverrides } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { HardwareProfile } from '../lib/types';

  let { currentLang }: { currentLang: Language } = $props();
  const t = $derived(translations[currentLang]);

  let profile = $state<HardwareProfile | null>(null);
  let isCalibrating = $state(false);
  let isSavingOverrides = $state(false);

  let asrOverride = $state('');
  let llmProviderOverride = $state('');
  let llmModelOverride = $state('');
  let ocrOverride = $state('');

  onMount(async () => {
    profile = await fetchProfile();
    populateOverrides();
  });

  function populateOverrides() {
    if (profile?.chosen.effective) {
      asrOverride = profile.chosen.overrides?.asr_model || profile.chosen.effective.asr_model || '';
      llmProviderOverride = profile.chosen.overrides?.llm_provider || profile.chosen.effective.llm_provider || '';
      llmModelOverride = profile.chosen.overrides?.llm_model || profile.chosen.effective.llm_model || '';
      ocrOverride = profile.chosen.overrides?.ocr_engine || profile.chosen.effective.ocr_engine || '';
    }
  }

  async function handleRecalibrate() {
    isCalibrating = true;
    try {
      profile = await triggerCalibration();
      populateOverrides();
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

      profile = await updateOverrides(overrides);
    } finally {
      isSavingOverrides = false;
    }
  }
</script>

<div class="screen-container">
  <div class="screen-header">
    <h2>{t.tabCalibration}</h2>
    {#if profile}
      <span class="capacity-pill">
        {Math.round(profile.capacity_per_day).toLocaleString()} {currentLang === 'bg' ? 'видеа / ден' : 'videos / day'}
      </span>
    {/if}
  </div>

  {#if profile}
    <div class="grid-layout">
      <div class="section-card">
        <h3>{currentLang === 'bg' ? 'Открит хардуер' : 'Detected Hardware'}</h3>
        <div class="hw-spec-list">
          <div class="spec-row">
            <span class="spec-label">CPU:</span>
            <span class="spec-val">{profile.hardware.cpu.brand} ({profile.hardware.cpu.physical_cores}C / {profile.hardware.cpu.logical_cores}T)</span>
          </div>
          <div class="spec-row">
            <span class="spec-label">RAM:</span>
            <span class="spec-val">{(profile.hardware.ram_mb / 1024).toFixed(1)} GB</span>
          </div>
          {#each profile.hardware.gpus as gpu}
            <div class="spec-row">
              <span class="spec-label">GPU:</span>
              <span class="spec-val">{gpu.vendor} {gpu.name} ({(gpu.vram_mb / 1024).toFixed(1)} GB VRAM)</span>
            </div>
          {/each}
          <div class="spec-row">
            <span class="spec-label">{currentLang === 'bg' ? 'Бекенди:' : 'Backends:'}</span>
            <span class="spec-val">{profile.hardware.compute_backends.join(', ')}</span>
          </div>
        </div>

        <button class="btn btn-secondary mt-3" onclick={handleRecalibrate} disabled={isCalibrating}>
          {isCalibrating ? t.benchmarking : t.recalibrateBtn}
        </button>
      </div>

      <div class="section-card">
        <h3>{currentLang === 'bg' ? 'Модели и конфигурация' : 'Effective Models & Overrides'}</h3>
        <div class="overrides-form">
          <label>
            <span class="input-label">Whisper ASR Model:</span>
            <input type="text" bind:value={asrOverride} placeholder="small, base, tiny..." />
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
            </select>
          </label>

          <button class="btn btn-primary mt-2" onclick={handleSaveOverrides} disabled={isSavingOverrides}>
            {isSavingOverrides ? '...' : (currentLang === 'bg' ? 'Запази промените' : 'Save Overrides')}
          </button>
        </div>
      </div>
    </div>
  {:else}
    <div class="loading-state">{t.benchmarking}</div>
  {/if}
</div>

<style>
  .screen-container {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem;
    background: var(--bg-canvas);
  }

  .screen-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1.25rem;
  }

  .screen-header h2 {
    margin: 0;
    font-size: 1.2rem;
    color: var(--text-heading);
  }

  .capacity-pill {
    font-size: 0.85rem;
    font-weight: 700;
    padding: 3px 10px;
    background: rgba(16, 185, 129, 0.15);
    color: var(--verdict-supported);
    border: 1px solid var(--verdict-supported);
    border-radius: 999px;
  }

  .grid-layout {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
    gap: 1rem;
  }

  .section-card {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1.15rem;
  }

  .section-card h3 {
    margin: 0 0 0.85rem 0;
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-heading);
  }

  .hw-spec-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    font-size: 0.85rem;
  }

  .spec-row {
    display: flex;
    justify-content: space-between;
    padding-bottom: 0.35rem;
    border-bottom: 1px solid var(--border-color);
  }

  .spec-label {
    font-weight: 700;
    color: var(--text-muted);
  }

  .spec-val {
    color: var(--text-heading);
    font-weight: 600;
  }

  .overrides-form {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .overrides-form label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .input-label {
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .overrides-form input,
  .overrides-form select {
    padding: 0.45rem 0.65rem;
    font-size: 0.85rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
  }

  .mt-2 {
    margin-top: 0.5rem;
  }

  .mt-3 {
    margin-top: 0.85rem;
  }

  .loading-state {
    padding: 2rem;
    text-align: center;
    color: var(--text-muted);
  }
</style>
