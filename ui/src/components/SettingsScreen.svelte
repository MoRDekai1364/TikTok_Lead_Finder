<script lang="ts">
  import { onMount } from 'svelte';
  import {
    fetchSettings,
    getApiToken,
    getCoreUrl,
    setApiToken,
    setCoreUrl,
    updateSettings,
  } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';

  let { currentLang }: { currentLang: Language } = $props();
  const t = $derived(translations[currentLang]);

  let coreUrl = $state(getCoreUrl());
  let apiToken = $state(getApiToken());
  let cloudEnabled = $state(false);
  let retentionDays = $state(7);
  let isSaving = $state(false);
  let saveSuccess = $state(false);

  onMount(async () => {
    try {
      const settings = await fetchSettings();
      if (typeof settings.cloud_enabled === 'boolean') {
        cloudEnabled = settings.cloud_enabled;
      }
      if (typeof settings.retention_days === 'number') {
        retentionDays = settings.retention_days;
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

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    font-size: 0.85rem;
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
</style>
