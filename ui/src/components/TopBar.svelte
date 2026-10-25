<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchGeminiStatus, fetchVideoDetail, getCoreUrl } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { BenchmarkProgress, PowerInfo, QueueCounts, ScaleStatus, Screen } from '../lib/types';
  import { getCapacityTierInfo } from '../lib/types';

  let {
    activeScreen,
    currentLang,
    theme,
    queueCounts,
    capacityPerDay,
    scaleStatus,
    powerInfo,
    benchmarkProgress,
    onNavigate,
    onPasteUrls,
    onJumpToVideo,
    onOpenScanModal,
    onOpenGoogleAiWizard,
    onOpenBenchmarkModal,
    onRequestExit,
    onToggleTheme,
    onToggleLang,
  }: {
    activeScreen: Screen;
    currentLang: Language;
    theme: 'light' | 'dark';
    queueCounts: QueueCounts;
    capacityPerDay: number | null;
    scaleStatus: ScaleStatus | null;
    powerInfo?: PowerInfo | null;
    benchmarkProgress?: BenchmarkProgress | null;
    onNavigate: (s: Screen) => void;
    onPasteUrls: (text: string) => Promise<any>;
    onJumpToVideo?: (id: string) => void;
    onOpenScanModal?: () => void;
    onOpenGoogleAiWizard?: () => void;
    onOpenBenchmarkModal?: () => void;
    onRequestExit?: () => void;
    onToggleTheme: () => void;
    onToggleLang: () => void;
  } = $props();

  let geminiConfigured = $state(false);

  let pasteText = $state('');
  let isPasting = $state(false);
  let toastMessage = $state<string | null>(null);
  let toastIsError = $state(false);
  let lastPasted = $state<{
    id: string;
    url?: string;
    creator_handle?: string | null;
    creator_display_name?: string | null;
    caption?: string | null;
    duration_s?: number | null;
    views?: number | null;
    thumbnail_url?: string | null;
    state?: string | null;
    progress_pct?: number | null;
    etr_seconds?: number | null;
    stage_desc?: string | null;
    done?: boolean;
    failed?: boolean;
  } | null>(null);

  const t = $derived(translations[currentLang]);

  const topTier = $derived(
    capacityPerDay ? getCapacityTierInfo(capacityPerDay, currentLang) : null
  );
  const capacityTooltip = $derived(
    topTier
      ? `${t.capacityPerDay} ${capacityPerDay} (${topTier.tierName} • ${topTier.percentOfTarget}% ${currentLang === 'bg' ? 'от таргета' : 'of target'} • ~${topTier.secondsPerVideo}s/${currentLang === 'bg' ? 'видео' : 'vid'})`
      : t.capacityPerDay
  );

  const TERMINAL_STATES = [
    'VERIFIED',
    'READY',
    'KEPT',
    'FAILED',
    'SCREENED_OUT',
    'DISCARDED',
    'CLAIMS_DONE',
  ];
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    fetchGeminiStatus().then((s) => {
      geminiConfigured = s.configured;
    }).catch(() => {});
    return () => stopPolling();
  });

  function stopPolling() {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  function describeState(state: string): { pct: number; desc: string } {
    const bg = currentLang === 'bg';
    switch (state) {
      case 'ACQUIRED':
        return { pct: 50, desc: bg ? 'Свалено съдържание...' : 'Media downloaded...' };
      case 'EXTRACTED':
        return { pct: 70, desc: bg ? 'Извлечен говор...' : 'Speech extracted...' };
      case 'CLAIMS_DONE':
        return { pct: 85, desc: bg ? 'Проверка на твърдения...' : 'Verifying claims...' };
      default:
        return { pct: 25, desc: bg ? 'В опашка...' : 'Queued...' };
    }
  }

  function startPolling(id: string) {
    stopPolling();
    let ticks = 0;
    const run = async () => {
      ticks += 1;
      if (!lastPasted || lastPasted.id !== id || lastPasted.done) {
        stopPolling();
        return;
      }
      try {
        const detail = await fetchVideoDetail(id);
        const v = detail?.video;
        if (!v || v.id !== id || !lastPasted || lastPasted.id !== id) return;
        const state = String(v.state || '');
        const claimsDone = Boolean(
          detail?.claims &&
            detail.claims.length > 0 &&
            detail.claims.every((c: any) => c.claim?.verdict)
        );
        const finished = TERMINAL_STATES.includes(state) || claimsDone;
        const info = describeState(state);
        lastPasted = {
          ...lastPasted,
          creator_handle: v.creator_handle || lastPasted.creator_handle,
          creator_display_name: v.creator_display_name || lastPasted.creator_display_name,
          caption: v.caption || lastPasted.caption,
          duration_s: v.duration_s || lastPasted.duration_s,
          views: v.views || lastPasted.views,
          state,
          progress_pct: finished ? 100 : info.pct,
          etr_seconds: null,
          stage_desc: finished
            ? currentLang === 'bg'
              ? 'Анализът е завършен'
              : 'Analysis complete'
            : info.desc,
          done: finished,
          failed: state === 'FAILED',
        };
        if (finished) stopPolling();
      } catch {}
      if (ticks > 450) stopPolling();
    };
    pollTimer = setInterval(run, 2000);
    run();
  }

  function dismissPreview() {
    stopPolling();
    lastPasted = null;
  }

  function inspectUrlImmediately(text: string) {
    const raw = text.trim();
    if (!raw) return;
    const tiktokMatch = raw.match(/tiktok\.com\/@([a-zA-Z0-9_.-]+)\/video\/([0-9]+)/i);
    if (tiktokMatch) {
      lastPasted = {
        id: tiktokMatch[2],
        creator_handle: tiktokMatch[1],
        caption: currentLang === 'bg' ? 'Добавяне...' : 'Adding...',
        progress_pct: 10,
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
      };
      return;
    }
    if (raw.includes('tiktok.com')) {
      lastPasted = {
        id: 'tiktok_link',
        creator_handle: 'tiktok_user',
        caption: raw,
        progress_pct: 10,
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
    stopPolling();
    inspectUrlImmediately(pasteText);
    isPasting = true;
    toastMessage = currentLang === 'bg' ? 'Добавяне на линк...' : 'Adding link...';
    toastIsError = false;
    try {
      const res = await onPasteUrls(pasteText);
      pasteText = '';
      if (res) {
        const finished = TERMINAL_STATES.includes(String(res.state || ''));
        lastPasted = {
          ...lastPasted,
          ...res,
          creator_handle: res.creator_handle || lastPasted?.creator_handle,
          creator_display_name: res.creator_display_name || lastPasted?.creator_display_name,
          caption: res.caption || lastPasted?.caption,
          duration_s: res.duration_s || lastPasted?.duration_s,
          views: res.views || lastPasted?.views,
          thumbnail_url: res.id ? `${getCoreUrl()}/api/media/${res.id}/thumb.jpg` : null,
          progress_pct: finished ? 100 : 25,
          etr_seconds: null,
          stage_desc: currentLang === 'bg' ? 'В опашка...' : 'Queued...',
          done: finished,
          failed: res.state === 'FAILED',
        };
        if (!finished && res.id) startPolling(String(res.id));
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
      {#if onOpenScanModal}
        <button
          type="button"
          class="btn scan-trigger-btn"
          onclick={onOpenScanModal}
          title={currentLang === 'bg' ? 'Фин скенер на TikTok профили и теми' : 'Fine-tuned TikTok scanner'}
        >
          🎯 {currentLang === 'bg' ? 'Фин скенер' : 'Fine Scanner'}
        </button>
      {/if}
      {#if toastMessage}
        <span class="paste-toast" class:toast-error={toastIsError}>{toastMessage}</span>
      {/if}
    </div>

    <div class="indicators">
      {#if benchmarkProgress && benchmarkProgress.is_running}
        <button
          type="button"
          class="indicator-pill benchmark-pulsing-pill"
          onclick={onOpenBenchmarkModal}
          title={currentLang === 'bg' ? 'Бенчмаркът се изпълнява - кликнете за преглед' : 'Benchmark running - click to view'}
        >
          <span class="pulsing-circle"></span>
          <span>⚡ {benchmarkProgress.progress_pct}%</span>
        </button>
      {/if}

      {#if powerInfo}
        <button
          type="button"
          class="indicator-pill power-pill"
          onclick={onOpenBenchmarkModal}
          title={powerInfo.power_source === 'battery'
            ? (currentLang === 'bg' ? 'Работа на батерия (енергоспестяващ профил)' : 'Battery Mode (power-saver profile)')
            : (currentLang === 'bg' ? 'Мрежово захранване (максимален капацитет)' : 'AC Plugged In (high performance)')}
        >
          <span>{powerInfo.power_source === 'battery' ? '🔋' : '⚡'}</span>
          <span>
            {powerInfo.power_source === 'battery'
              ? (powerInfo.battery_percent !== null ? `${powerInfo.battery_percent}%` : 'Batt')
              : 'AC'}
          </span>
        </button>
      {/if}

      {#if capacityPerDay !== null}
        <button
          type="button"
          class="indicator-pill capacity-pill clickable"
          title={capacityTooltip}
          onclick={onOpenBenchmarkModal}
        >
          <span class="dot green"></span>
          {#if topTier}
            <span class="topbar-tier-icon">{topTier.icon}</span>
          {/if}
          <span>{capacityPerDay.toLocaleString()} / ден</span>
        </button>
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

      {#if onOpenGoogleAiWizard}
        <button
          type="button"
          class="indicator-pill google-ai-pill"
          class:ai-active={geminiConfigured}
          onclick={onOpenGoogleAiWizard}
          title={geminiConfigured
            ? (currentLang === 'bg' ? 'Google AI е активен (98% точност)' : 'Google AI active (98% accuracy)')
            : (currentLang === 'bg' ? 'Свържи безплатен Google AI ключ' : 'Connect free Google AI key')}
        >
          <span class="dot" class:green={geminiConfigured} class:amber={!geminiConfigured}></span>
          <span>✨ Google AI</span>
        </button>
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

      {#if onRequestExit}
        <button
          class="icon-btn exit-btn"
          onclick={onRequestExit}
          title={currentLang === 'bg' ? 'Затваряне на приложението' : 'Exit application'}
          aria-label="Изход"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
            <polyline points="16 17 21 12 16 7"></polyline>
            <line x1="21" y1="12" x2="9" y2="12"></line>
          </svg>
        </button>
      {/if}
    </div>
  </div>

  {#if lastPasted}
    <div class="pasted-preview-card">
      <div class="preview-thumb-col">
        {#if lastPasted.id && lastPasted.id !== 'tiktok_link'}
          <img
            src={`${getCoreUrl()}/api/media/${lastPasted.id}/thumb.jpg`}
            alt=""
            class="preview-thumb-img"
            onerror={(e) => {
              (e.currentTarget as HTMLElement).style.display = 'none';
            }}
          />
        {/if}
        <div class="preview-thumb-fallback" aria-hidden="true">🎬</div>
      </div>

      <div class="preview-content-col">
        <div class="preview-header-line">
          <span class="preview-badge">📌 {t.previewCardTitle}</span>
          <span class="preview-author-handle">@{lastPasted.creator_handle || 'tiktok_creator'}</span>
          {#if lastPasted.creator_display_name}
            <span class="preview-author-name">({lastPasted.creator_display_name})</span>
          {/if}
          {#if lastPasted.duration_s}
            <span class="preview-pill">⏱ {Math.round(lastPasted.duration_s)}s</span>
          {/if}
          {#if lastPasted.views}
            <span class="preview-pill">👁 {lastPasted.views.toLocaleString()}</span>
          {/if}
        </div>

        <div class="preview-caption-text" title={lastPasted.caption || lastPasted.id}>
          {lastPasted.caption || lastPasted.id}
        </div>

        <div class="preview-status-line">
          {#if lastPasted.done}
            <span class="preview-done-label" class:preview-failed-label={lastPasted.failed}>
              {lastPasted.failed ? '✕' : '✓'}
              {lastPasted.failed
                ? currentLang === 'bg' ? 'Анализът не успя' : 'Analysis failed'
                : currentLang === 'bg' ? 'Анализът е завършен' : 'Analysis complete'}
            </span>
          {:else}
            <div class="preview-progress-track">
              <div
                class="preview-progress-fill"
                style="width: {Math.max(12, lastPasted.progress_pct ?? 25)}%"
              ></div>
            </div>
            <span class="preview-progress-label">
              ⚙️ {lastPasted.stage_desc || (currentLang === 'bg' ? 'В обработка...' : 'Processing...')} ({lastPasted.progress_pct ?? 25}%)
              {#if lastPasted.etr_seconds}
                • ~{lastPasted.etr_seconds}s
              {/if}
            </span>
          {/if}
        </div>
      </div>

      <div class="preview-actions-col">
        {#if onJumpToVideo && lastPasted.id && lastPasted.id !== 'tiktok_link'}
          <button class="btn btn-primary btn-sm btn-jump" onclick={() => onJumpToVideo?.(lastPasted!.id)}>
            👁 {t.jumpToVideo}
          </button>
        {/if}
        <button class="preview-dismiss" onclick={dismissPreview} aria-label="Close">✕</button>
      </div>

    </div>
  {/if}

  <nav class="nav-tabs" aria-label="Основна навигация">
    <button class="nav-tab" class:active={activeScreen === 'inbox'} onclick={() => onNavigate('inbox')}>
      {t.tabInbox}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'creators'} onclick={() => onNavigate('creators')}>
      {t.tabCreators}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'people'} onclick={() => onNavigate('people')}>
      {t.tabPeople}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'seeds'} onclick={() => onNavigate('seeds')}>
      {t.tabSeeds}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'calibration'} onclick={() => onNavigate('calibration')}>
      {t.tabCalibration}
    </button>
    <button class="nav-tab" class:active={activeScreen === 'models'} onclick={() => onNavigate('models')}>
      🤖 {t.tabModels}
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
    flex-wrap: wrap;
    gap: 0.5rem 1rem;
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
    min-width: 260px;
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

  .scan-trigger-btn {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    color: var(--text-body);
    font-size: 0.8rem;
    font-weight: 700;
    padding: 0.45rem 0.75rem;
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  .scan-trigger-btn:hover {
    border-color: var(--primary-accent);
    color: var(--primary-accent);
    background: var(--hover-bg);
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

  .dot.amber {
    background-color: #f59e0b;
  }

  .google-ai-pill {
    cursor: pointer;
    transition: filter 0.15s, transform 0.1s, border-color 0.15s;
  }

  .google-ai-pill:hover {
    filter: brightness(1.15);
    transform: translateY(-1px);
    border-color: #3b82f6;
  }

  .google-ai-pill.ai-active {
    background-color: rgba(16, 185, 129, 0.15);
    border-color: rgba(16, 185, 129, 0.4);
    color: #34d399;
  }

  .benchmark-pulsing-pill {
    cursor: pointer;
    background: rgba(59, 130, 246, 0.18);
    border-color: rgba(59, 130, 246, 0.45);
    color: #60a5fa;
    transition: filter 0.15s, transform 0.1s;
  }

  .benchmark-pulsing-pill:hover {
    filter: brightness(1.2);
    transform: translateY(-1px);
  }

  .pulsing-circle {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #60a5fa;
    box-shadow: 0 0 6px #60a5fa;
    animation: topbarPulse 1.4s infinite;
  }

  @keyframes topbarPulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.3; transform: scale(0.7); }
  }

  .power-pill {
    cursor: pointer;
    transition: filter 0.15s, transform 0.1s;
  }

  .power-pill:hover {
    filter: brightness(1.15);
    transform: translateY(-1px);
  }

  .capacity-pill.clickable {
    cursor: pointer;
    transition: filter 0.15s, transform 0.1s;
    background: rgba(16, 185, 129, 0.12);
    border: 1px solid rgba(16, 185, 129, 0.3);
    color: #34d399;
  }

  .capacity-pill.clickable:hover {
    filter: brightness(1.2);
    transform: translateY(-1px);
  }

  .topbar-tier-icon {
    font-size: 0.85rem;
    margin-right: 0.2rem;
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
    transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
  }

  .icon-btn.exit-btn:hover {
    color: #ef4444;
    border-color: rgba(239, 68, 68, 0.4);
    background-color: rgba(239, 68, 68, 0.12);
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

  .pasted-preview-card {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    padding: 0.55rem 0.85rem;
    margin: 0.45rem 0 0.25rem 0;
    border-radius: 8px;
    background: var(--surface-bg);
    border: 1px solid rgba(99, 102, 241, 0.35);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  }

  .preview-thumb-col {
    width: 48px;
    height: 64px;
    border-radius: 6px;
    overflow: hidden;
    position: relative;
    background: #000;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border-color);
  }

  .preview-thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    position: relative;
    z-index: 1;
  }

  .preview-thumb-fallback {
    position: absolute;
    font-size: 1.3rem;
    color: var(--text-muted);
  }

  .preview-content-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
  }

  .preview-header-line {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    flex-wrap: wrap;
    font-size: 0.78rem;
  }

  .preview-badge {
    font-weight: 700;
    color: var(--primary-accent);
  }

  .preview-author-handle {
    font-weight: 700;
    color: var(--text-heading);
  }

  .preview-author-name {
    color: var(--text-muted);
    font-size: 0.72rem;
  }

  .preview-pill {
    font-size: 0.7rem;
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--pill-bg);
    color: var(--text-muted);
  }

  .preview-caption-text {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-heading);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .preview-status-line {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .preview-progress-track {
    width: 140px;
    height: 10px;
    background: var(--border-color);
    border-radius: 999px;
    overflow: hidden;
    position: relative;
  }

  .preview-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #6366f1, #06b6d4);
    border-radius: 999px;
    transition: width 0.3s ease;
  }

  .preview-progress-label {
    font-size: 0.72rem;
    color: var(--text-muted);
    font-weight: 600;
  }

  .preview-done-label {
    font-size: 0.74rem;
    font-weight: 700;
    color: var(--verdict-supported);
  }

  .preview-failed-label {
    color: var(--verdict-contradicted);
  }

  .preview-actions-col {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
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
