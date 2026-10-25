<script lang="ts">
  import { onMount } from 'svelte';
  import { deleteGeminiKey, fetchGeminiStatus, openInBrowser, rotateGeminiKey, saveGeminiKey } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import type { GeminiStatus } from '../lib/types';

  let {
    isOpen,
    currentLang,
    onClose,
    onStatusChange,
  }: {
    isOpen: boolean;
    currentLang: Language;
    onClose: () => void;
    onStatusChange?: (status: GeminiStatus) => void;
  } = $props();

  let step = $state<1 | 2 | 3 | 4>(1);
  let status = $state<GeminiStatus | null>(null);
  let apiKeyInput = $state('');
  let isValidating = $state(false);
  let isRotating = $state(false);
  let errorMessage = $state<string | null>(null);
  let successNotice = $state<string | null>(null);

  $effect(() => {
    if (isOpen) {
      loadStatus();
    }
  });

  async function loadStatus() {
    try {
      const res = await fetchGeminiStatus();
      status = res;
      if (res.configured) {
        step = 4;
      } else {
        step = 1;
      }
    } catch {
      status = {
        configured: false,
        masked_key: null,
        cloud_enabled: false,
        model: 'gemini-2.5-flash',
      };
      step = 1;
    }
  }

  function handleOpenStudio() {
    openInBrowser('https://aistudio.google.com/app/apikey');
  }

  async function handleValidateAndSave() {
    const trimmed = apiKeyInput.trim();
    if (!trimmed) {
      errorMessage = currentLang === 'bg'
        ? 'Моля, въведете вашия Google AI API ключ.'
        : 'Please enter your Google AI API key.';
      return;
    }

    isValidating = true;
    errorMessage = null;
    successNotice = null;

    try {
      const res = await saveGeminiKey(trimmed);
      if (res.ok) {
        apiKeyInput = '';
        await loadStatus();
        step = 4;
        const total = res.total_keys || 1;
        successNotice = currentLang === 'bg'
          ? (total > 1 ? `Успешно запазени и проверени ${total} ключа в пула!` : 'Ключът беше потвърден успешно от Google AI!')
          : (total > 1 ? `Successfully saved and verified ${total} keys in pool!` : 'API key successfully verified by Google AI!');
        if (onStatusChange && status) {
          onStatusChange(status);
        }
      } else {
        errorMessage = res.error || (currentLang === 'bg' ? 'Грешка при проверка на ключа' : 'Error verifying key');
      }
    } catch (e: any) {
      errorMessage = e?.message || (currentLang === 'bg' ? 'Неуспешна връзка с Google AI' : 'Connection failed');
    } finally {
      isValidating = false;
    }
  }

  async function handleRotateKey() {
    isRotating = true;
    errorMessage = null;
    try {
      const res = await rotateGeminiKey();
      if (res.ok) {
        await loadStatus();
        successNotice = currentLang === 'bg'
          ? `Успешна ротация: Активен ключ #${(res.active_index ?? 0) + 1} (${res.masked_key || ''})`
          : `Rotated key: Active key #${(res.active_index ?? 0) + 1} (${res.masked_key || ''})`;
      } else {
        errorMessage = res.error || (currentLang === 'bg' ? 'Грешка при ротация на ключа' : 'Error rotating key');
      }
    } catch (e: any) {
      errorMessage = e?.message || (currentLang === 'bg' ? 'Неуспешна ротация' : 'Rotation failed');
    } finally {
      isRotating = false;
    }
  }

  async function handleDisconnect() {
    try {
      await deleteGeminiKey();
      status = {
        configured: false,
        masked_key: null,
        cloud_enabled: false,
        model: 'gemini-2.5-flash',
      };
      step = 1;
      apiKeyInput = '';
      if (onStatusChange && status) {
        onStatusChange(status);
      }
    } catch {}
  }
</script>

{#if isOpen}
  <div
    class="wizard-backdrop"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    onkeydown={(e) => { if (e.key === 'Escape') onClose(); }}
  >
    <div class="wizard-dialog" role="dialog" aria-modal="true">
      <header class="wizard-header">
        <div class="header-title-wrap">
          <div class="header-icon">✨</div>
          <div>
            <h3 class="wizard-title">
              {currentLang === 'bg' ? 'Магьосник за Google AI (Gemini Studio)' : 'Google AI Setup Wizard (Gemini Studio)'}
            </h3>
            <p class="wizard-sub">
              {currentLang === 'bg'
                ? 'Настройка за >95% точност на българска транскрипция и мулти-ключов пул'
                : 'Setup for >95% Bulgarian speech accuracy & multi-key rotation pool'}
            </p>
          </div>
        </div>
        <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>
      </header>

      <div class="steps-progress">
        <div class="step-pill" class:active={step === 1} class:done={step > 1}>
          <span class="step-num">1</span>
          <span class="step-text">{currentLang === 'bg' ? 'Предимства' : 'Overview'}</span>
        </div>
        <div class="step-sep"></div>
        <div class="step-pill" class:active={step === 2} class:done={step > 2}>
          <span class="step-num">2</span>
          <span class="step-text">{currentLang === 'bg' ? 'Вземане на ключ' : 'Get Key'}</span>
        </div>
        <div class="step-sep"></div>
        <div class="step-pill" class:active={step === 3} class:done={step > 3}>
          <span class="step-num">3</span>
          <span class="step-text">{currentLang === 'bg' ? 'Свързване' : 'Connect'}</span>
        </div>
        <div class="step-sep"></div>
        <div class="step-pill" class:active={step === 4}>
          <span class="step-num">✓</span>
          <span class="step-text">{currentLang === 'bg' ? 'Готово' : 'Ready'}</span>
        </div>
      </div>

      <div class="wizard-body">
        {#if step === 1}
          <div class="slide-content">
            <h4 class="slide-title">
              {currentLang === 'bg' ? 'Защо ви е нужен Google AI ключ?' : 'Why use Google AI with LeadFinder?'}
            </h4>
            <p class="slide-desc">
              {currentLang === 'bg'
                ? 'LeadFinder разполага с локален модел Whisper large-v3-turbo, но при видеа със силна музика или говор в шум, Google AI осигурява ненадминато качество.'
                : 'LeadFinder includes offline Whisper large-v3-turbo, but Google AI provides best-in-class results even over loud TikTok background music.'}
            </p>

            <div class="feature-grid">
              <div class="feature-card">
                <div class="f-icon">🎯</div>
                <div class="f-title">{currentLang === 'bg' ? 'Над 95% точност на БГ' : '>95% Accuracy in Bulgarian'}</div>
                <div class="f-desc">
                  {currentLang === 'bg'
                    ? 'Чиста пунктуация, точни числа, имена на български политици и институции без грешки.'
                    : 'Clean punctuation, accurate numbers, Bulgarian names and institutions without misspellings.'}
                </div>
              </div>

              <div class="feature-card">
                <div class="f-icon">🎁</div>
                <div class="f-title">{currentLang === 'bg' ? '100% Безплатен персонален достъп' : '100% Free Personal Tier'}</div>
                <div class="f-desc">
                  {currentLang === 'bg'
                    ? 'Google AI Studio предлага безплатен план с до 1500 заявки дневно без нужда от банкова карта.'
                    : 'Google AI Studio provides a free quota of up to 1,500 daily requests without requiring a credit card.'}
                </div>
              </div>

              <div class="feature-card">
                <div class="f-icon">🔄</div>
                <div class="f-title">{currentLang === 'bg' ? 'Автоматична ротация на ключове' : 'Automatic Key Pool Rotation'}</div>
                <div class="f-desc">
                  {currentLang === 'bg'
                    ? 'Можете да въведете няколко безплатни ключа - системата се прехвърля автоматично при лимит (429).'
                    : 'Add multiple free keys to auto-rotate seamlessly when a daily quota limit (429) is hit.'}
                </div>
              </div>

              <div class="feature-card">
                <div class="f-icon">🛡️</div>
                <div class="f-title">{currentLang === 'bg' ? 'Сигурен офлайн резерв' : 'Local Fallback'}</div>
                <div class="f-desc">
                  {currentLang === 'bg'
                    ? 'Ако нямате интернет връзка, системата автоматично използва локалния модел Whisper large-v3-turbo.'
                    : 'If offline, LeadFinder seamlessly falls back to your local Whisper large-v3-turbo.'}
                </div>
              </div>
            </div>
          </div>

        {:else if step === 2}
          <div class="slide-content">
            <h4 class="slide-title">
              {currentLang === 'bg' ? 'Как да вземете безплатен ключ в 3 прости стъпки' : 'How to get your free key in 3 simple steps'}
            </h4>

            <div class="steps-guide">
              <div class="guide-item">
                <div class="guide-badge">1</div>
                <div class="guide-info">
                  <div class="guide-heading">
                    {currentLang === 'bg' ? 'Отворете портала на Google AI Studio' : 'Open Google AI Studio'}
                  </div>
                  <div class="guide-detail">
                    {currentLang === 'bg'
                      ? 'Влезте с вашия личен Google (Gmail) профил. Линкът се отваря в браузъра ви.'
                      : 'Sign in with your Google / Gmail account in your default web browser.'}
                  </div>
                  <button class="btn btn-studio" onclick={handleOpenStudio}>
                    🔗 {currentLang === 'bg' ? 'Отвори Google AI Studio в браузъра' : 'Open Google AI Studio in browser'}
                  </button>
                </div>
              </div>

              <div class="guide-item">
                <div class="guide-badge">2</div>
                <div class="guide-info">
                  <div class="guide-heading">
                    {currentLang === 'bg' ? 'Натиснете синия бутон "Create API key"' : 'Click the blue "Create API key" button'}
                  </div>
                  <div class="guide-detail">
                    {currentLang === 'bg'
                      ? 'Изберете "Create key in new project" (или съществуващ проект).'
                      : 'Choose "Create key in new project" (or an existing Google Cloud project).'}
                  </div>
                </div>
              </div>

              <div class="guide-item">
                <div class="guide-badge">3</div>
                <div class="guide-info">
                  <div class="guide-heading">
                    {currentLang === 'bg' ? 'Копирайте ключа' : 'Copy your key'}
                  </div>
                  <div class="guide-detail">
                    {currentLang === 'bg'
                      ? 'Ключът започва с "AIzaSy...". Можете да създадете и втори ключ от друг акаунт за резерв!'
                      : 'Your key starts with "AIzaSy...". You can also create keys in other accounts for rotation backup!'}
                  </div>
                </div>
              </div>
            </div>
          </div>

        {:else if step === 3}
          <div class="slide-content">
            <h4 class="slide-title">
              {currentLang === 'bg' ? 'Въведете вашия Google AI ключ (или списък от ключове)' : 'Enter your Google AI key(s)'}
            </h4>
            <p class="slide-desc">
              {currentLang === 'bg'
                ? 'Ключовете ще бъдат проверени веднага със сървърите на Google и запазени в Windows Credential Store.'
                : 'Keys will be verified directly against Google AI servers and stored in Windows Credential Store.'}
            </p>

            <div class="input-card">
              <label for="gemini-key-input" class="input-title">
                {currentLang === 'bg' ? 'API ключове (по един на ред или запетая):' : 'API Key(s) (one per line or comma-separated):'}
              </label>

              <textarea
                id="gemini-key-input"
                class="key-textarea-input"
                bind:value={apiKeyInput}
                placeholder={currentLang === 'bg'
                  ? "Въведете един или няколко ключа (по един на ред):\nAIzaSyA...\nAIzaSyB..."
                  : "Enter one or multiple keys (one per line):\nAIzaSyA...\nAIzaSyB..."}
                disabled={isValidating}
                rows="4"
              ></textarea>

              {#if errorMessage}
                <div class="error-box">
                  ⚠️ {errorMessage}
                </div>
              {/if}

              <div class="hint-text">
                💡 {currentLang === 'bg'
                  ? 'Мулти-ключов пул: Ако добавите повече от 1 ключ, LeadFinder автоматично ще се прехвърли към следващия свободен ключ, ако дневната безплатна квота (1500 заявки) бъде достигната!'
                  : 'Multi-key pool: Adding multiple keys allows LeadFinder to automatically switch to the next backup key when the 1,500 daily quota limit is reached!'}
              </div>
            </div>
          </div>

        {:else if step === 4}
          <div class="slide-content active-status-view">
            <div class="success-banner">
              <div class="check-circle">✓</div>
              <div class="success-info">
                <h4 class="success-title">
                  {currentLang === 'bg' ? 'Google AI е свързан и напълно активен!' : 'Google AI is Connected & Active!'}
                </h4>
                <p class="success-sub">
                  {currentLang === 'bg'
                    ? 'Автоматичната обработка вече използва Google AI с 95%+ точност.'
                    : 'Automatic ingestion pipeline is now running with Google AI for >95% accuracy.'}
                </p>
              </div>
            </div>

            {#if successNotice}
              <div class="notice-box">
                {successNotice}
              </div>
            {/if}

            <div class="status-summary-card">
              <div class="summary-row">
                <span class="s-label">{currentLang === 'bg' ? 'Статус:' : 'Status:'}</span>
                <span class="s-val badge-green">🟢 {currentLang === 'bg' ? 'Активен (Cloud Enabled)' : 'Active (Cloud Enabled)'}</span>
              </div>
              <div class="summary-row">
                <span class="s-label">{currentLang === 'bg' ? 'Активен ключ:' : 'Active Key:'}</span>
                <span class="s-val mono">{status?.masked_key || 'AIzaSy...****'}</span>
              </div>
              <div class="summary-row">
                <span class="s-label">{currentLang === 'bg' ? 'Общо ключове в пула:' : 'Keys in Pool:'}</span>
                <span class="s-val bold">{status?.total_keys || 1}</span>
              </div>
              <div class="summary-row">
                <span class="s-label">{currentLang === 'bg' ? 'Облачен модел:' : 'Cloud Model:'}</span>
                <span class="s-val bold">{status?.model || 'gemini-2.5-flash'}</span>
              </div>
              <div class="summary-row">
                <span class="s-label">{currentLang === 'bg' ? 'Основен ASR:' : 'Primary ASR:'}</span>
                <span class="s-val bold">Google AI Speech + Vocal Bandpass Filter</span>
              </div>
              <div class="summary-row">
                <span class="s-label">{currentLang === 'bg' ? 'Офлайн резерв:' : 'Offline Fallback:'}</span>
                <span class="s-val">Whisper large-v3-turbo (574 MB)</span>
              </div>
            </div>

            {#if status?.keys && status.keys.length > 0}
              <div class="keys-pool-card">
                <div class="keys-pool-header">
                  <span class="kph-title">
                    {currentLang === 'bg' ? 'Пул от налични ключове:' : 'Key Pool Status:'}
                  </span>
                  <span class="kph-count">
                    ({status.keys.length} {currentLang === 'bg' ? 'ключа' : 'keys'})
                  </span>
                </div>
                <div class="keys-list">
                  {#each status.keys as k}
                    <div class="key-item-row" class:active-key={k.is_active}>
                      <div class="key-item-left">
                        <span class="key-dot" class:active={k.is_active}></span>
                        <span class="key-idx">#{k.index + 1}</span>
                        <span class="key-code mono">{k.masked}</span>
                      </div>
                      <span class="key-status-badge" class:badge-active={k.is_active}>
                        {k.is_active
                          ? (currentLang === 'bg' ? '🟢 Активен' : '🟢 Active')
                          : (currentLang === 'bg' ? '⚪ Резервен' : '⚪ Standby')}
                      </span>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}

            <div class="management-zone">
              <span class="mgmt-title">{currentLang === 'bg' ? 'Управление на пула:' : 'Pool Management:'}</span>
              <div class="mgmt-actions">
                {#if (status?.total_keys || 0) > 1}
                  <button class="btn btn-secondary" onclick={handleRotateKey} disabled={isRotating}>
                    {isRotating ? '⏳ ...' : (currentLang === 'bg' ? '🔄 Ротация на ключ' : '🔄 Rotate Key')}
                  </button>
                {/if}
                <button class="btn btn-secondary" onclick={() => { step = 3; apiKeyInput = ''; }}>
                  ➕ {currentLang === 'bg' ? 'Добави / Промени' : 'Add / Edit Keys'}
                </button>
                <button class="btn btn-danger" onclick={handleDisconnect}>
                  🗑️ {currentLang === 'bg' ? 'Премахни всички' : 'Disconnect All'}
                </button>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <footer class="wizard-footer">
        {#if step === 1}
          <div class="footer-left">
            <button class="btn btn-secondary" onclick={onClose}>
              {currentLang === 'bg' ? 'Затвори' : 'Close'}
            </button>
          </div>
          <div class="footer-right">
            <button class="btn btn-primary" onclick={() => (step = 2)}>
              {currentLang === 'bg' ? 'Напред към вземане на ключ →' : 'Next: Get Free Key →'}
            </button>
          </div>

        {:else if step === 2}
          <div class="footer-left">
            <button class="btn btn-secondary" onclick={() => (step = 1)}>
              ← {currentLang === 'bg' ? 'Назад' : 'Back'}
            </button>
          </div>
          <div class="footer-right">
            <button class="btn btn-primary" onclick={() => (step = 3)}>
              {currentLang === 'bg' ? 'Имам ключ, напред →' : 'I have a key, Next →'}
            </button>
          </div>

        {:else if step === 3}
          <div class="footer-left">
            <button class="btn btn-secondary" onclick={() => (step = 2)} disabled={isValidating}>
              ← {currentLang === 'bg' ? 'Назад' : 'Back'}
            </button>
          </div>
          <div class="footer-right">
            <button
              class="btn btn-primary btn-validate"
              onclick={handleValidateAndSave}
              disabled={isValidating || !apiKeyInput.trim()}
            >
              {#if isValidating}
                <span class="spin">⏳</span> {currentLang === 'bg' ? 'Проверка с Google AI...' : 'Validating with Google...'}
              {:else}
                ⚡ {currentLang === 'bg' ? 'Тествай и свържи' : 'Test & Save Key(s)'}
              {/if}
            </button>
          </div>

        {:else if step === 4}
          <div class="footer-left"></div>
          <div class="footer-right">
            <button class="btn btn-primary" onclick={onClose}>
              ✓ {currentLang === 'bg' ? 'Готово' : 'Done'}
            </button>
          </div>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .wizard-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background-color: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    animation: fadeIn 0.2s ease-out;
  }

  .wizard-dialog {
    width: 90%;
    max-width: 660px;
    max-height: 90vh;
    background-color: var(--bg-surface, #1e2024);
    border: 1px solid var(--border-color, #333842);
    border-radius: 12px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: scaleIn 0.25s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .wizard-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 24px;
    background-color: var(--bg-surface-elevated, #25282f);
    border-bottom: 1px solid var(--border-color, #333842);
  }

  .header-title-wrap {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .header-icon {
    font-size: 1.6rem;
  }

  .wizard-title {
    margin: 0;
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--text-primary, #f3f4f6);
  }

  .wizard-sub {
    margin: 2px 0 0 0;
    font-size: 0.8rem;
    color: var(--text-secondary, #9ca3af);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-secondary, #9ca3af);
    font-size: 1.2rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    transition: background-color 0.15s, color 0.15s;
  }

  .close-btn:hover {
    background-color: rgba(255, 255, 255, 0.08);
    color: var(--text-primary, #ffffff);
  }

  .steps-progress {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 24px;
    background-color: rgba(0, 0, 0, 0.18);
    border-bottom: 1px solid var(--border-color, #333842);
  }

  .step-pill {
    display: flex;
    align-items: center;
    gap: 8px;
    opacity: 0.5;
    transition: opacity 0.2s, color 0.2s;
  }

  .step-pill.active {
    opacity: 1;
    font-weight: 600;
  }

  .step-pill.done {
    opacity: 0.85;
  }

  .step-num {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    font-size: 0.75rem;
    border-radius: 50%;
    background-color: var(--border-color, #4b5563);
    color: #ffffff;
  }

  .step-pill.active .step-num {
    background: #3b82f6;
    box-shadow: 0 0 8px rgba(59, 130, 246, 0.5);
  }

  .step-pill.done .step-num {
    background: #10b981;
  }

  .step-text {
    font-size: 0.8rem;
    color: var(--text-primary, #f3f4f6);
  }

  .step-sep {
    flex: 1;
    height: 2px;
    margin: 0 10px;
    background-color: var(--border-color, #333842);
  }

  .wizard-body {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }

  .slide-title {
    margin: 0 0 8px 0;
    font-size: 1.1rem;
    font-weight: 600;
    color: var(--text-primary, #f3f4f6);
  }

  .slide-desc {
    margin: 0 0 20px 0;
    font-size: 0.86rem;
    color: var(--text-secondary, #9ca3af);
    line-height: 1.45;
  }

  .feature-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }

  .feature-card {
    padding: 14px;
    background-color: var(--bg-surface-elevated, #25282f);
    border: 1px solid var(--border-color, #333842);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .f-icon {
    font-size: 1.3rem;
    margin-bottom: 2px;
  }

  .f-title {
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text-primary, #f3f4f6);
  }

  .f-desc {
    font-size: 0.78rem;
    color: var(--text-secondary, #9ca3af);
    line-height: 1.35;
  }

  .steps-guide {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .guide-item {
    display: flex;
    gap: 14px;
    padding: 14px;
    background-color: var(--bg-surface-elevated, #25282f);
    border: 1px solid var(--border-color, #333842);
    border-radius: 8px;
  }

  .guide-badge {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background-color: #3b82f6;
    color: #ffffff;
    font-size: 0.88rem;
    font-weight: 700;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .guide-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .guide-heading {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-primary, #f3f4f6);
  }

  .guide-detail {
    font-size: 0.82rem;
    color: var(--text-secondary, #9ca3af);
    line-height: 1.4;
  }

  .btn-studio {
    align-self: flex-start;
    margin-top: 6px;
    padding: 7px 14px;
    font-size: 0.82rem;
    font-weight: 600;
    background: linear-gradient(135deg, #2563eb, #1d4ed8);
    color: #ffffff;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(37, 99, 235, 0.4);
    transition: filter 0.15s, transform 0.1s;
  }

  .btn-studio:hover {
    filter: brightness(1.1);
    transform: translateY(-1px);
  }

  .input-card {
    padding: 18px;
    background-color: var(--bg-surface-elevated, #25282f);
    border: 1px solid var(--border-color, #333842);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .input-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-primary, #f3f4f6);
  }

  .key-textarea-input {
    width: 100%;
    padding: 10px 14px;
    font-size: 0.88rem;
    font-family: monospace;
    background-color: var(--bg-surface, #1e2024);
    border: 1px solid var(--border-color, #4b5563);
    border-radius: 6px;
    color: #ffffff;
    outline: none;
    box-sizing: border-box;
    resize: vertical;
    line-height: 1.5;
    transition: border-color 0.15s;
  }

  .key-textarea-input:focus {
    border-color: #3b82f6;
    box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.3);
  }

  .error-box {
    padding: 10px 14px;
    background-color: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.4);
    border-radius: 6px;
    color: #f87171;
    font-size: 0.82rem;
    line-height: 1.4;
  }

  .hint-text {
    font-size: 0.78rem;
    color: var(--text-secondary, #9ca3af);
    line-height: 1.4;
  }

  .success-banner {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px;
    background: linear-gradient(135deg, rgba(16, 185, 129, 0.15), rgba(5, 150, 105, 0.08));
    border: 1px solid rgba(16, 185, 129, 0.35);
    border-radius: 8px;
    margin-bottom: 16px;
  }

  .check-circle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    flex-shrink: 0;
    border-radius: 50%;
    background-color: #10b981;
    color: #ffffff;
    font-size: 1.2rem;
    font-weight: 700;
  }

  .success-title {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 700;
    color: #34d399;
  }

  .success-sub {
    margin: 3px 0 0 0;
    font-size: 0.82rem;
    color: var(--text-secondary, #9ca3af);
  }

  .notice-box {
    padding: 10px 14px;
    background-color: rgba(16, 185, 129, 0.12);
    border: 1px solid rgba(16, 185, 129, 0.3);
    border-radius: 6px;
    color: #34d399;
    font-size: 0.82rem;
    margin-bottom: 16px;
  }

  .status-summary-card {
    padding: 16px;
    background-color: var(--bg-surface-elevated, #25282f);
    border: 1px solid var(--border-color, #333842);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 16px;
  }

  .summary-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.84rem;
  }

  .s-label {
    color: var(--text-secondary, #9ca3af);
  }

  .s-val {
    color: var(--text-primary, #f3f4f6);
  }

  .badge-green {
    padding: 2px 8px;
    background-color: rgba(16, 185, 129, 0.2);
    color: #34d399;
    border-radius: 4px;
    font-weight: 600;
    font-size: 0.8rem;
  }

  .mono {
    font-family: monospace;
    letter-spacing: 0.5px;
  }

  .bold {
    font-weight: 600;
  }

  .keys-pool-card {
    background-color: var(--bg-surface-elevated, #25282f);
    border: 1px solid var(--border-color, #333842);
    border-radius: 8px;
    padding: 14px;
    margin-bottom: 16px;
  }

  .keys-pool-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 10px;
  }

  .kph-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-primary, #f3f4f6);
  }

  .kph-count {
    font-size: 0.8rem;
    color: var(--text-secondary, #9ca3af);
  }

  .keys-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .key-item-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.04);
  }

  .key-item-row.active-key {
    background: rgba(16, 185, 129, 0.08);
    border-color: rgba(16, 185, 129, 0.25);
  }

  .key-item-left {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.82rem;
  }

  .key-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #6b7280;
  }

  .key-dot.active {
    background: #10b981;
    box-shadow: 0 0 6px #10b981;
  }

  .key-idx {
    color: var(--text-secondary, #9ca3af);
    font-weight: 600;
  }

  .key-code {
    color: var(--text-primary, #f3f4f6);
  }

  .key-status-badge {
    font-size: 0.75rem;
    padding: 2px 6px;
    border-radius: 4px;
    color: var(--text-secondary, #9ca3af);
    background: rgba(255, 255, 255, 0.05);
  }

  .key-status-badge.badge-active {
    color: #34d399;
    background: rgba(16, 185, 129, 0.15);
    font-weight: 600;
  }

  .management-zone {
    padding-top: 14px;
    border-top: 1px solid var(--border-color, #333842);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .mgmt-title {
    font-size: 0.82rem;
    color: var(--text-secondary, #9ca3af);
  }

  .mgmt-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .wizard-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    background-color: var(--bg-surface-elevated, #25282f);
    border-top: 1px solid var(--border-color, #333842);
  }

  .footer-right {
    display: flex;
    gap: 10px;
  }

  .btn {
    padding: 8px 16px;
    font-size: 0.86rem;
    font-weight: 600;
    border-radius: 6px;
    cursor: pointer;
    border: none;
    transition: filter 0.15s, transform 0.1s;
  }

  .btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #3b82f6;
    color: #ffffff;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary, #f3f4f6);
    border: 1px solid var(--border-color, #4b5563);
  }

  .btn-danger {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.35);
  }

  .spin {
    display: inline-block;
    animation: rotate 1.5s linear infinite;
  }

  @keyframes rotate {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes scaleIn {
    from { transform: scale(0.96); opacity: 0; }
    to { transform: scale(1); opacity: 1; }
  }
</style>
