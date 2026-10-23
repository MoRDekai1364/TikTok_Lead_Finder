<script lang="ts">
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { QueueCounts, ScaleStatus, Screen } from '../lib/types';

  let {
    activeScreen,
    currentLang,
    theme,
    queueCounts,
    capacityPerDay,
    scaleStatus,
    onNavigate,
    onPasteUrls,
    onToggleTheme,
    onToggleLang,
  }: {
    activeScreen: Screen;
    currentLang: Language;
    theme: 'light' | 'dark';
    queueCounts: QueueCounts;
    capacityPerDay: number | null;
    scaleStatus: ScaleStatus | null;
    onNavigate: (s: Screen) => void;
    onPasteUrls: (text: string) => Promise<void>;
    onToggleTheme: () => void;
    onToggleLang: () => void;
  } = $props();

  let pasteText = $state('');
  let isPasting = $state(false);

  const t = $derived(translations[currentLang]);

  async function handlePaste() {
    if (!pasteText.trim() || isPasting) return;
    isPasting = true;
    try {
      await onPasteUrls(pasteText);
      pasteText = '';
    } finally {
      isPasting = false;
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.ctrlKey || !e.shiftKey)) {
      e.preventDefault();
      handlePaste();
    }
  }
</script>

<header class="topbar">
  <div class="top-row">
    <div class="brand-group">
      <div class="app-icon" aria-hidden="true">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
      <span class="app-name">{t.appTitle}</span>
    </div>

    <div class="paste-bar">
      <input
        type="text"
        placeholder={t.pastePlaceholder}
        bind:value={pasteText}
        onkeydown={handleKeyDown}
        aria-label={t.pastePlaceholder}
      />
      <button class="btn btn-primary" onclick={handlePaste} disabled={isPasting || !pasteText.trim()}>
        {isPasting ? '...' : t.pasteButton}
      </button>
    </div>

    <div class="indicators">
      {#if capacityPerDay !== null}
        <div class="indicator-pill capacity-pill" title={t.capacityPerDay}>
          <span class="dot green"></span>
          <span>{capacityPerDay.toLocaleString()} / ден</span>
        </div>
      {/if}

      <div class="indicator-pill queue-pill" title="Опашка на задачите">
        <span>Q: {queueCounts.queued || 0}</span>
        <span>R: {queueCounts.running || 0}</span>
      </div>

      {#if scaleStatus && scaleStatus.open_breakers > 0}
        <div class="indicator-pill breaker-pill" title={t.breakersOpen}>
          <span class="dot red"></span>
          <span>Breakers: {scaleStatus.open_breakers}</span>
        </div>
      {/if}

      <button class="icon-btn" onclick={onToggleLang} title="Смяна на езика / Switch language" aria-label="Смяна на език">
        {currentLang.toUpperCase()}
      </button>

      <button class="icon-btn" onclick={onToggleTheme} title={theme === 'dark' ? t.themeLight : t.themeDark} aria-label="Смяна на тема">
        {#if theme === 'dark'}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="5"></circle>
            <line x1="12" y1="1" x2="12" y2="3"></line>
            <line x1="12" y1="21" x2="12" y2="23"></line>
            <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line>
            <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line>
            <line x1="1" y1="12" x2="3" y2="12"></line>
            <line x1="21" y1="12" x2="23" y2="12"></line>
            <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line>
            <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line>
          </svg>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path>
          </svg>
        {/if}
      </button>
    </div>
  </div>

  <nav class="nav-tabs" aria-label="Основна навигация">
    <button class="nav-tab" class:active={activeScreen === 'inbox'} onclick={() => onNavigate('inbox')}>
      {t.tabInbox}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'creators'} onclick={() => onNavigate('creators')}>
      {t.tabCreators}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'seeds'} onclick={() => onNavigate('seeds')}>
      {t.tabSeeds}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'calibration'} onclick={() => onNavigate('calibration')}>
      {t.tabCalibration}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'settings'} onclick={() => onNavigate('settings')}>
      {t.tabSettings}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'export'} onclick={() => onNavigate('export')}>
      {t.tabExport}
    </button>
  </nav>
</header>

<style>
  .topbar {
    background-color: var(--surface-bg);
    border-bottom: 1px solid var(--border-color);
    padding: 0.5rem 1.25rem 0 1.25rem;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
  }

  .top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    min-height: 48px;
  }

  .brand-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 700;
    font-size: 1.05rem;
    color: var(--text-heading);
    white-space: nowrap;
  }

  .app-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--primary-accent);
  }

  .paste-bar {
    display: flex;
    flex: 1;
    max-width: 680px;
    gap: 0.5rem;
  }

  .paste-bar input {
    flex: 1;
    padding: 0.45rem 0.85rem;
    font-size: 0.88rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
    outline: none;
    transition: border-color 0.15s ease;
  }

  .paste-bar input:focus {
    border-color: var(--primary-accent);
  }

  .indicators {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .indicator-pill {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.78rem;
    font-weight: 600;
    padding: 0.25rem 0.6rem;
    border-radius: 999px;
    background: var(--pill-bg);
    color: var(--text-body);
    border: 1px solid var(--border-color);
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .dot.green {
    background-color: var(--verdict-supported);
  }

  .dot.red {
    background-color: var(--verdict-contradicted);
  }

  .icon-btn {
    background: none;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.35rem 0.55rem;
    cursor: pointer;
    color: var(--text-body);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.78rem;
    font-weight: 700;
    transition: background 0.15s ease;
  }

  .icon-btn:hover {
    background: var(--hover-bg);
  }

  .nav-tabs {
    display: flex;
    gap: 0.25rem;
    margin-top: 0.35rem;
    border-top: 1px solid var(--border-color);
    padding-top: 0.25rem;
  }

  .nav-tab {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 0.5rem 0.85rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .nav-tab:hover {
    color: var(--text-body);
  }

  .nav-tab.active {
    color: var(--primary-accent);
    border-bottom-color: var(--primary-accent);
  }
</style>
