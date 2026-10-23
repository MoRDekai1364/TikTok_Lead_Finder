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
    onJumpToVideo,
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
    onPasteUrls: (text: string) => Promise<any>;
    onJumpToVideo?: (id: string) => void;
    onToggleTheme: () => void;
    onToggleLang: () => void;
  } = $props();

  let pasteText = $state('');
  let isPasting = $state(false);
  let toastMessage = $state<string | null>(null);
  let toastIsError = $state(false);
  let lastPasted = $state<{
    id: string;
    url?: string;
    creator_handle?: string | null;
    caption?: string | null;
    state?: string | null;
    progress_pct?: number | null;
    etr_seconds?: number | null;
  } | null>(null);

  const t = $derived(translations[currentLang]);

  function inspectUrlImmediately(text: string) {
    const raw = text.trim();
    if (!raw) return;
    const tiktokMatch = raw.match(/tiktok\.com\/@([a-zA-Z0-9_.-]+)\/video\/([0-9]+)/i);
    if (tiktokMatch) {
      lastPasted = {
        id: tiktokMatch[2],
        creator_handle: tiktokMatch[1],
        caption: currentLang === 'bg' ? 'Зареждане на видео данни...' : 'Loading video details...',
        progress_pct: 15,
        etr_seconds: 25,
      };
      return;
    }
    const shortMatch = raw.match(/(?:vm|vt)\.tiktok\.com\/([a-zA-Z0-9_-]+)/i);
    if (shortMatch) {
      lastPasted = {
        id: shortMatch[1],
        creator_handle: 'tiktok_user',
        caption: currentLang === 'bg' ? 'Разрешаване на кратък линк...' : 'Resolving shortlink...',
        progress_pct: 10,
        etr_seconds: 30,
      };
      return;
    }
    if (raw.includes('tiktok.com')) {
      lastPasted = {
        id: 'tiktok_link',
        creator_handle: 'tiktok_user',
        caption: raw,
        progress_pct: 10,
        etr_seconds: 30,
      };
    }
  }

  function handlePasteEvent(e: ClipboardEvent) {
    const pasted = e.clipboardData?.getData('text') || '';
    if (pasted) {
      inspectUrlImmediately(pasted);
    }
  }

  function handleInput() {
    if (pasteText.includes('tiktok.com')) {
      inspectUrlImmediately(pasteText);
    }
  }

  async function handlePaste() {
    if (!pasteText.trim() || isPasting) return;
    inspectUrlImmediately(pasteText);
    isPasting = true;
    toastMessage = currentLang === 'bg' ? 'Добавяне на линк...' : 'Adding link...';
    toastIsError = false;
    try {
      const res = await onPasteUrls(pasteText);
      pasteText = '';
      if (res) {
        lastPasted = res;
      }
      toastMessage = currentLang === 'bg' ? '✓ Линкът е добавен в опашката!' : '✓ Added to processing queue!';
      setTimeout(() => { toastMessage = null; }, 3500);
    } catch (e: any) {
      toastIsError = true;
      toastMessage = currentLang === 'bg' ? '✕ Невалиден линк или грешка' : '✕ Invalid link or error';
      setTimeout(() => { toastMessage = null; }, 4000);
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
        oninput={handleInput}
        onpaste={handlePasteEvent}
        onkeydown={handleKeyDown}
        aria-label={t.pastePlaceholder}
      />
      <button class="btn btn-primary" onclick={handlePaste} disabled={isPasting || !pasteText.trim()}>
        {isPasting ? '...' : t.pasteButton}
      </button>
      {#if toastMessage}
        <span class="paste-toast" class:toast-error={toastIsError}>{toastMessage}</span>
      {/if}
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

  {#if lastPasted}
    <div class="pasted-preview-bar">
      <div class="preview-badge">📌 {t.previewCardTitle}</div>
      <div class="preview-author">@{lastPasted.creator_handle || 'tiktok_user'}</div>
      <div class="preview-title" title={lastPasted.caption || lastPasted.id}>{lastPasted.caption || lastPasted.id}</div>
      <div class="preview-status">
        {#if lastPasted.progress_pct !== undefined && lastPasted.progress_pct !== null && lastPasted.progress_pct < 100}
          <div class="preview-progress-box">
            <div class="preview-progress-fill" style="width: {lastPasted.progress_pct}%"></div>
            <span class="preview-progress-text">⏳ {lastPasted.progress_pct}% {#if lastPasted.etr_seconds}(~{lastPasted.etr_seconds}s ETR){/if}</span>
          </div>
        {:else}
          <span class="preview-ready-badge">✓ {t.processingBadge}</span>
        {/if}
      </div>
      {#if onJumpToVideo}
        <button class="btn-jump" onclick={() => onJumpToVideo?.(lastPasted!.id)}>
          🔍 {t.jumpToVideo}
        </button>
      {/if}
      <button class="preview-dismiss" onclick={() => (lastPasted = null)} aria-label="Close">✕</button>
    </div>
  {/if}

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
    <button class="nav-tab" class:active={activeScreen === 'help'} onclick={() => onNavigate('help')}>
      ❓ {t.tabHelp}
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

  .paste-toast {
    display: inline-flex;
    align-items: center;
    font-size: 0.78rem;
    font-weight: 600;
    padding: 0.35rem 0.65rem;
    border-radius: 6px;
    background: rgba(16, 185, 129, 0.2);
    color: var(--verdict-supported);
    border: 1px solid rgba(16, 185, 129, 0.4);
    white-space: nowrap;
  }

  .paste-toast.toast-error {
    background: rgba(225, 29, 72, 0.2);
    color: var(--verdict-contradicted);
    border-color: rgba(225, 29, 72, 0.4);
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

  .pasted-preview-bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.4rem 0.75rem;
    margin: 0.4rem 0 0.2rem 0;
    border-radius: 6px;
    background: rgba(99, 102, 241, 0.08);
    border: 1px solid rgba(99, 102, 241, 0.25);
    font-size: 0.82rem;
  }

  .preview-badge {
    font-weight: 700;
    color: var(--primary-accent);
    white-space: nowrap;
  }

  .preview-author {
    font-weight: 600;
    color: var(--text-heading);
    white-space: nowrap;
  }

  .preview-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
  }

  .preview-status {
    display: flex;
    align-items: center;
    white-space: nowrap;
  }

  .preview-progress-box {
    position: relative;
    width: 140px;
    height: 18px;
    background: rgba(0, 0, 0, 0.1);
    border-radius: 999px;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .preview-progress-fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    background: linear-gradient(90deg, #6366f1, #10b981);
    transition: width 0.3s ease;
  }

  .preview-progress-text {
    position: relative;
    font-size: 0.7rem;
    font-weight: 700;
    color: #fff;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
  }

  .preview-ready-badge {
    font-weight: 600;
    color: var(--verdict-supported);
  }

  .btn-jump {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.25rem 0.6rem;
    font-size: 0.78rem;
    font-weight: 600;
    border-radius: 4px;
    background: var(--primary-accent);
    color: #fff;
    border: none;
    cursor: pointer;
    white-space: nowrap;
    transition: opacity 0.15s ease;
  }

  .btn-jump:hover {
    opacity: 0.9;
  }

  .preview-dismiss {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.9rem;
    color: var(--text-muted);
    padding: 0 0.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .preview-dismiss:hover {
    color: var(--text-heading);
  }
</style>
