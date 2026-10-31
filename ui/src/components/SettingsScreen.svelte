<script lang="ts">
  import { onMount } from 'svelte';
  import {
    fetchGeminiStatus,
    fetchSettings,
    getApiToken,
    getCoreUrl,
    setApiToken,
    setCoreUrl,
    updateSettings,
  } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { GeminiStatus } from '../lib/types';
  import { DISPLAY_PRESETS, widescreen } from '../lib/widescreen.svelte';

  let {
    currentLang,
    onOpenGoogleAiWizard,
  }: {
    currentLang: Language;
    onOpenGoogleAiWizard?: () => void;
  } = $props();
  const t = $derived(translations[currentLang]);

  let coreUrl = $state(getCoreUrl());
  let apiToken = $state(getApiToken());
  let cloudEnabled = $state(false);
  let retentionDays = $state(7);
  let isSaving = $state(false);
  let saveSuccess = $state(false);
  let geminiStatus = $state<GeminiStatus | null>(null);
  let exitBehavior = $state<'ask' | 'background' | 'terminate'>('ask');

  onMount(async () => {
    try {
      const neverAsk = localStorage.getItem('leadfinder_never_ask_exit') === 'true';
      const savedAction = localStorage.getItem('leadfinder_exit_action');
      if (neverAsk && savedAction === 'background') {
        exitBehavior = 'background';
      } else if (neverAsk && savedAction === 'terminate') {
        exitBehavior = 'terminate';
      } else {
        exitBehavior = 'ask';
      }

      const [settings, gStatus] = await Promise.all([
        fetchSettings(),
        fetchGeminiStatus().catch(() => null),
      ]);
      if (typeof settings.cloud_enabled === 'boolean') {
        cloudEnabled = settings.cloud_enabled;
      }
      if (typeof settings.retention_days === 'number') {
        retentionDays = settings.retention_days;
      }
      if (gStatus) {
        geminiStatus = gStatus;
      }
    } catch {}
  });

  async function handleSave() {
    isSaving = true;
    saveSuccess = false;
    try {
      setCoreUrl(coreUrl);
      setApiToken(apiToken);
      await updateSettings({
        cloud_enabled: cloudEnabled,
        retention_days: retentionDays,
      });
      if (exitBehavior === 'ask') {
        localStorage.removeItem('leadfinder_never_ask_exit');
        localStorage.removeItem('leadfinder_exit_action');
      } else {
        localStorage.setItem('leadfinder_never_ask_exit', 'true');
        localStorage.setItem('leadfinder_exit_action', exitBehavior);
      }
      saveSuccess = true;
      setTimeout(() => (saveSuccess = false), 3000);
    } finally {
      isSaving = false;
    }
  }
</script>

<div class="screen-container">
  <div class="screen-header">
    <h2>{t.tabSettings}</h2>
  </div>

  <div class="settings-grid">
    <div class="section-card">
      <h3>{currentLang === 'bg' ? 'Свързаност с Core услугата' : 'Core Service Connection'}</h3>
      <div class="form-group">
        <label>
          <span class="input-label">Core API URL:</span>
          <input type="text" bind:value={coreUrl} placeholder="http://127.0.0.1:17420" />
        </label>

        <label>
          <span class="input-label">API Bearer Token:</span>
          <input type="password" bind:value={apiToken} placeholder="Token..." />
        </label>
      </div>
    </div>

    <div class="section-card">
      <h3>{currentLang === 'bg' ? 'Облак и поверителност' : 'Cloud & Privacy'}</h3>
      <div class="cloud-toggle-box">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={cloudEnabled} />
          <strong>{currentLang === 'bg' ? 'Разреши облачни LLM модели (Gemini / OpenAI)' : 'Enable cloud LLM providers'}</strong>
        </label>
        <p class="notice-box">{t.cloudWarningNotice}</p>
      </div>

      <div class="form-group mt-3">
        <label>
          <span class="input-label">
            {currentLang === 'bg' ? 'Запазване на видео файлове (дни):' : 'Media retention days:'} {retentionDays}
          </span>
          <input type="range" min="1" max="30" bind:value={retentionDays} />
        </label>
      </div>
    </div>

    <div class="section-card google-ai-card">
      <div class="card-head-with-action">
        <h3>✨ {currentLang === 'bg' ? 'Google AI (Gemini Studio)' : 'Google AI (Gemini Studio)'}</h3>
        {#if onOpenGoogleAiWizard}
          <button type="button" class="btn-wizard-trigger" onclick={onOpenGoogleAiWizard}>
            🔑 {currentLang === 'bg' ? 'Настройка на ключ' : 'API Key Setup'}
          </button>
        {/if}
      </div>

      <div class="ai-status-row">
        <span class="status-badge" class:badge-active={geminiStatus?.configured}>
          {geminiStatus?.configured
            ? (currentLang === 'bg' ? '🟢 Свързан' : '🟢 Connected')
            : (currentLang === 'bg' ? '⚪ Не е настроен (локален Whisper)' : '⚪ Not configured (Whisper)')}
        </span>
        {#if geminiStatus?.masked_key}
          <span class="masked-key-pill">{geminiStatus.masked_key}</span>
        {/if}
      </div>

      <p class="ai-desc">
        {currentLang === 'bg'
          ? 'Google AI осигурява висока точност при разпознаване на българска реч и изготвяне на аналитични справки. Безплатният план включва до 1500 заявки на ден.'
          : 'Google AI enables high speech-to-text accuracy in Bulgarian and detailed analytical reports. Free tier with up to 1,500 requests per day.'}
      </p>
    </div>

    <div class="section-card">
      <h3>🚪 {currentLang === 'bg' ? 'Поведение при затваряне на прозореца' : 'Window Exit Behavior'}</h3>
      <div class="form-group">
        <label class="radio-label">
          <input type="radio" name="exit_behavior" value="ask" bind:group={exitBehavior} />
          <span>{currentLang === 'bg' ? 'Винаги питай преди излизане (Препоръчително)' : 'Always prompt before closing (Recommended)'}</span>
        </label>
        <label class="radio-label">
          <input type="radio" name="exit_behavior" value="background" bind:group={exitBehavior} />
          <span>{currentLang === 'bg' ? 'Продължи на заден план до приключване на задачите и автоматично изключване' : 'Run in background until tasks finish, then auto-terminate'}</span>
        </label>
        <label class="radio-label">
          <input type="radio" name="exit_behavior" value="terminate" bind:group={exitBehavior} />
          <span>{currentLang === 'bg' ? 'Прекрати напълно и спри всички фонови процеси веднага' : 'Terminate all processes immediately (nothing stays running)'}</span>
        </label>
      </div>
    </div>

    <div class="section-card display-preset-card">
      <div class="display-card-head">
        <h3>
          {currentLang === 'bg'
            ? '🖥️ Профил на дисплея и разположение (13" – 18")'
            : '🖥️ Display Profile & Screen Presets (13" – 18")'}
        </h3>
        <span class="hw-detected-pill">{widescreen.resolutionSummary}</span>
      </div>
      <p class="ai-desc">
        {currentLang === 'bg'
          ? 'Калибрирани профили за мащаб на шрифта, брой колони, височина на видеото и пропорции на страничния панел спрямо диагонала и резолюцията на екрана.'
          : 'Calibrated presets for typography scale, column density, video stage height, and docked workspace proportions by screen diagonal and resolution.'}
      </p>
      <div class="preset-cards-grid">
        {#each DISPLAY_PRESETS as preset (preset.id)}
          <button
            type="button"
            class="preset-option-btn"
            class:selected={widescreen.preset === preset.id}
            class:featured-18={preset.featured}
            onclick={() => widescreen.setPreset(preset.id)}
          >
            <div class="preset-opt-top">
              <span class="preset-diag-badge" class:badge-18={preset.featured}>
                {preset.diagonalBadge}{preset.featured ? ' ★' : ''}
              </span>
              <span class="preset-opt-title">
                {currentLang === 'bg' ? preset.titleBg : preset.titleEn}
              </span>
            </div>
            <span class="preset-opt-desc">
              {currentLang === 'bg' ? preset.descBg : preset.descEn}
            </span>
          </button>
        {/each}
      </div>
    </div>
  </div>

  <div class="save-bar">
    <button class="btn btn-primary" onclick={handleSave} disabled={isSaving}>
      {isSaving ? '...' : t.saveSettings}
    </button>
    {#if saveSuccess}
      <span class="success-hint">✓ {currentLang === 'bg' ? 'Настройките са запазени успешно' : 'Settings saved successfully'}</span>
    {/if}
  </div>
</div>

<style>
  .screen-container {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem;
    background: var(--bg-canvas);
  }

  .screen-header {
    margin-bottom: 1.25rem;
  }

  .screen-header h2 {
    margin: 0;
    font-size: 1.2rem;
    color: var(--text-heading);
  }

  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
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

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .form-group label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .input-label {
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .form-group input[type='text'],
  .form-group input[type='password'] {
    padding: 0.45rem 0.65rem;
    font-size: 0.85rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
  }

  .checkbox-label,
  .radio-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    font-size: 0.85rem;
    color: var(--text-body);
  }

  .radio-label input[type='radio'] {
    accent-color: #3b82f6;
  }

  .notice-box {
    margin: 0.5rem 0 0 0;
    padding: 0.5rem;
    background: rgba(245, 158, 11, 0.1);
    border-left: 3px solid #f59e0b;
    font-size: 0.78rem;
    color: var(--text-body);
    line-height: 1.4;
  }

  .save-bar {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .success-hint {
    color: var(--verdict-supported);
    font-weight: 600;
    font-size: 0.85rem;
  }

  .mt-3 {
    margin-top: 0.85rem;
  }

  .card-head-with-action {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.85rem;
  }

  .card-head-with-action h3 {
    margin: 0;
  }

  .btn-wizard-trigger {
    padding: 0.35rem 0.75rem;
    font-size: 0.8rem;
    font-weight: 600;
    border-radius: 6px;
    background: linear-gradient(135deg, #2563eb, #3b82f6);
    color: #ffffff;
    border: none;
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(37, 99, 235, 0.35);
    transition: filter 0.15s, transform 0.1s;
  }

  .btn-wizard-trigger:hover {
    filter: brightness(1.1);
    transform: translateY(-1px);
  }

  .ai-status-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 0.65rem;
  }

  .status-badge {
    font-size: 0.8rem;
    padding: 0.2rem 0.55rem;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-muted);
  }

  .status-badge.badge-active {
    background: rgba(16, 185, 129, 0.15);
    color: #34d399;
    font-weight: 600;
  }

  .masked-key-pill {
    font-size: 0.75rem;
    font-family: monospace;
    padding: 0.2rem 0.45rem;
    border-radius: 4px;
    background: var(--input-bg);
    border: 1px solid var(--border-color);
    color: var(--text-muted);
  }

  .ai-desc {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--text-muted);
  }

  .display-preset-card {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .display-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .display-card-head h3 {
    margin: 0;
  }

  .hw-detected-pill {
    font-size: 0.75rem;
    font-family: monospace;
    padding: 0.22rem 0.55rem;
    border-radius: 6px;
    background: var(--input-bg);
    border: 1px solid var(--border-color);
    color: var(--text-main);
  }

  .preset-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(235px, 1fr));
    gap: 0.6rem;
    margin-top: 0.2rem;
  }

  .preset-option-btn {
    text-align: left;
    background: var(--input-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.65rem 0.75rem;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    color: var(--text-main);
    transition: border-color 0.15s ease, background 0.15s ease, transform 0.12s ease;
  }

  .preset-option-btn:hover {
    border-color: var(--primary-accent);
  }

  .preset-option-btn.featured-18 {
    border-color: rgba(245, 158, 11, 0.42);
    background: linear-gradient(135deg, rgba(245, 158, 11, 0.08), rgba(59, 130, 246, 0.06));
  }

  .preset-option-btn.selected {
    border-color: var(--primary-accent);
    background: rgba(59, 130, 246, 0.12);
    box-shadow: 0 0 0 1px var(--primary-accent);
  }

  .preset-option-btn.featured-18.selected {
    border-color: #f59e0b;
    background: linear-gradient(135deg, rgba(245, 158, 11, 0.16), rgba(59, 130, 246, 0.12));
    box-shadow: 0 0 0 1px #f59e0b;
  }

  .preset-opt-top {
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }

  .preset-diag-badge {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 0.12rem 0.42rem;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-main);
    white-space: nowrap;
  }

  .preset-diag-badge.badge-18 {
    background: rgba(245, 158, 11, 0.22);
    color: #fbbf24;
    border: 1px solid rgba(245, 158, 11, 0.4);
  }

  .preset-opt-title {
    font-size: 0.82rem;
    font-weight: 700;
    line-height: 1.25;
  }

  .preset-opt-desc {
    font-size: 0.75rem;
    line-height: 1.38;
    color: var(--text-muted);
  }
</style>
