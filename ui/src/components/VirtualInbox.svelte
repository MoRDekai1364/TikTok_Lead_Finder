<script lang="ts">
  import { onMount } from 'svelte';
  import { aiCheckTranscript, getCoreUrl } from '../lib/api';
  import {
    registerPlaybackElement,
    unregisterPlaybackElement,
    updatePlaybackProgress,
    setPlaybackPlaying,
  } from '../lib/playback.svelte';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { Lane, LeadCard } from '../lib/types';
  import { widescreen } from '../lib/widescreen.svelte';
  import LeadCardComponent from './LeadCard.svelte';

  let {
    cards,
    activeLane,
    selectedId,
    checkedIds,
    currentLang,
    isVideoFeedOpen = true,
    isDrawerOpen = false,
    onToggleVideoFeed,
    onOpenDetail,
    onSelectCard,
    onToggleCheck,
    onChangeLane,
    onPullMore,
    onKeep,
    onDiscard,
    onResearchMore,
    onBatchKeep,
    onBatchDiscard,
    laneCounts,
  }: {
    cards: LeadCard[];
    activeLane: Lane;
    selectedId: string | null;
    checkedIds: Set<string>;
    currentLang: Language;
    isVideoFeedOpen?: boolean;
    isDrawerOpen?: boolean;
    onToggleVideoFeed?: () => void;
    onOpenDetail?: (id: string) => void;
    laneCounts?: Record<string, number>;
    onSelectCard: (id: string) => void;
    onToggleCheck: (id: string) => void;
    onChangeLane: (lane: Lane) => void;
    onPullMore: () => Promise<{ promoted: number } | void>;
    onKeep: (id: string) => void;
    onDiscard: (id: string) => void;
    onResearchMore: (id: string) => void;
    onBatchKeep: () => void;
    onBatchDiscard: () => void;
  } = $props();

  const t = $derived(translations[currentLang]);
  const coreUrl = getCoreUrl();

  let feedToast = $state<string | null>(null);
  let isAiCheckingFeed = $state(false);
  let pullToast = $state<string | null>(null);
  let isPullingMore = $state(false);
  let inboxVideoElement = $state<HTMLVideoElement | null>(null);

  $effect(() => {
    return () => {
      if (inboxVideoElement) {
        unregisterPlaybackElement(inboxVideoElement);
      }
    };
  });

  const queuedBehindCap = $derived(laneCounts?.leads_queued ?? 0);

  async function handlePullMoreClick() {
    if (isPullingMore) return;
    if (queuedBehindCap === 0) {
      pullToast = currentLang === 'bg'
        ? `Всички налични ${cards.length} лийда са заредени. Поставете нови TikTok връзки горе!`
        : `All ${cards.length} leads are loaded. Paste new TikTok links above!`;
      setTimeout(() => (pullToast = null), 4000);
      return;
    }
    isPullingMore = true;
    try {
      const res = await onPullMore();
      if (res && typeof res.promoted === 'number') {
        if (res.promoted > 0) {
          pullToast = currentLang === 'bg' ? `📥 +${res.promoted} нови лийда` : `📥 +${res.promoted} new leads`;
        } else {
          pullToast = currentLang === 'bg' ? 'Няма чакащи лийдове зад лимита' : 'No queued leads behind cap';
        }
      } else {
        pullToast = currentLang === 'bg' ? 'Готово' : 'Done';
      }
    } catch {
      pullToast = currentLang === 'bg' ? 'Грешка при изтегляне' : 'Pull error';
    } finally {
      isPullingMore = false;
    }
    setTimeout(() => {
      pullToast = null;
    }, 3500);
  }

  let searchQuery = $state('');
  let verdictFilter = $state('all');
  let sortBy = $state<'score' | 'views' | 'quality'>('score');
  let searchInputRef = $state<HTMLInputElement | null>(null);

  let containerRef = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let containerHeight = $state(600);

  const itemHeight = $derived(widescreen.inboxRowHeight);
  const BUFFER_COUNT = 7;

  export function focusSearch() {
    if (searchInputRef) {
      searchInputRef.focus();
    }
  }

  async function handleCopyFeedText() {
    if (!selectedCard) return;
    const txt = selectedCard.top_claim_text || '';
    await navigator.clipboard.writeText(txt);
    feedToast = currentLang === 'bg' ? '✓ Копирано!' : '✓ Copied!';
    setTimeout(() => (feedToast = null), 2000);
  }

  async function handleAiCheckFeed() {
    if (!selectedCard || isAiCheckingFeed) return;
    isAiCheckingFeed = true;
    feedToast = currentLang === 'bg' ? 'AI проверка...' : 'AI check...';
    try {
      const res = await aiCheckTranscript(selectedCard.id);
      if (res && res.corrected_text) {
        selectedCard.top_claim_text = res.corrected_text;
        await navigator.clipboard.writeText(res.corrected_text);
        feedToast = currentLang === 'bg' ? '✓ AI коригирано и копирано!' : '✓ AI corrected & copied!';
      }
    } catch {
      feedToast = currentLang === 'bg' ? 'Грешка' : 'Error';
    } finally {
      isAiCheckingFeed = false;
      setTimeout(() => (feedToast = null), 2500);
    }
  }

  const filteredCards = $derived.by(() => {
    let list = cards.filter((c: LeadCard) => {
      if (activeLane === 'pasted') {
        return c.lane === 'pasted' || (c.reason_flagged && c.reason_flagged.includes('paste'));
      }
      if (activeLane === 'leads') {
        return c.lane === 'leads' || c.lane === 'pasted' || c.lane === 'leads_queued' || !c.lane;
      }
      return c.lane === activeLane;
    });

    if (verdictFilter !== 'all') {
      list = list.filter((c: LeadCard) => c.top_claim_verdict?.toUpperCase() === verdictFilter.toUpperCase());
    }

    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase().trim();
      list = list.filter((c: LeadCard) => {
        const claim = (c.top_claim_text || '').toLowerCase();
        const creator = (c.creator_handle || '').toLowerCase();
        const reason = (c.reason_flagged || '').toLowerCase();
        const person = (c.aegis_target_person || '').toLowerCase();
        const verdict = (c.aegis_verdict || '').toLowerCase();
        return claim.includes(q) || creator.includes(q) || reason.includes(q) || person.includes(q) || verdict.includes(q);
      });
    }

    if (sortBy === 'score') {
      list = [...list].sort((a, b) => b.lead_score - a.lead_score);
    } else if (sortBy === 'views') {
      list = [...list].sort((a, b) => b.views - a.views);
    } else if (sortBy === 'quality') {
      list = [...list].sort((a, b) => (b.transcript_quality || 0) - (a.transcript_quality || 0));
    }

    return list;
  });

  const selectedCard = $derived(
    filteredCards.find((c: LeadCard) => c.id === selectedId) || filteredCards[0] || null
  );

  const totalHeight = $derived(filteredCards.length * itemHeight);

  const visibleRange = $derived.by(() => {
    const startIdx = Math.max(0, Math.floor(scrollTop / itemHeight) - BUFFER_COUNT);
    const visibleCount = Math.ceil(containerHeight / itemHeight) + 2 * BUFFER_COUNT;
    const endIdx = Math.min(filteredCards.length, startIdx + visibleCount);
    return { startIdx, endIdx };
  });

  const visibleItems = $derived.by(() => {
    const { startIdx, endIdx } = visibleRange;
    return filteredCards.slice(startIdx, endIdx).map((item: LeadCard, idx: number) => ({
      item,
      top: (startIdx + idx) * itemHeight,
    }));
  });

  function handleScroll(e: Event) {
    const el = e.target as HTMLDivElement;
    scrollTop = el.scrollTop;
  }

  onMount(() => {
    if (containerRef) {
      containerHeight = containerRef.clientHeight;
    }
  });
</script>

<div class="inbox-container" class:has-right-column={Boolean((isVideoFeedOpen && !isDrawerOpen) || isDrawerOpen)}>
  <div class="inbox-subbar">
    <div class="lane-tabs" role="tablist">
      <button
        class="lane-tab"
        class:active={activeLane === 'pasted'}
        onclick={() => onChangeLane('pasted')}
        role="tab"
        aria-selected={activeLane === 'pasted'}
      >
        {t.lanePasted || (currentLang === 'bg' ? '📌 Ръчно добавени' : '📌 Pasted Links')}
        {#if laneCounts?.pasted}
          <span class="tab-badge">{laneCounts.pasted}</span>
        {/if}
      </button>
      <button
        class="lane-tab"
        class:active={activeLane === 'leads'}
        onclick={() => onChangeLane('leads')}
        role="tab"
        aria-selected={activeLane === 'leads'}
      >
        {t.laneLeads}
        {#if laneCounts?.leads}
          <span class="tab-badge">{laneCounts.leads}</span>
        {/if}
      </button>
      <button
        class="lane-tab"
        class:active={activeLane === 'already_checked'}
        onclick={() => onChangeLane('already_checked')}
        role="tab"
        aria-selected={activeLane === 'already_checked'}
      >
        {t.laneAlreadyChecked}
        {#if laneCounts?.already_checked}
          <span class="tab-badge">{laneCounts.already_checked}</span>
        {/if}
      </button>
      <button
        class="lane-tab"
        class:active={activeLane === 'low_quality'}
        onclick={() => onChangeLane('low_quality')}
        role="tab"
        aria-selected={activeLane === 'low_quality'}
      >
        {t.laneLowQuality}
        {#if laneCounts?.low_quality}
          <span class="tab-badge">{laneCounts.low_quality}</span>
        {/if}
      </button>
      <button
        class="lane-tab"
        class:active={activeLane === 'screened_out'}
        onclick={() => onChangeLane('screened_out')}
        role="tab"
        aria-selected={activeLane === 'screened_out'}
      >
        {currentLang === 'bg' ? 'Отхвърлени' : 'Screened Out'}
        {#if laneCounts?.screened_out}
          <span class="tab-badge">{laneCounts.screened_out}</span>
        {/if}
      </button>
      <button
        class="lane-tab"
        class:active={activeLane === 'failed'}
        onclick={() => onChangeLane('failed')}
        role="tab"
        aria-selected={activeLane === 'failed'}
      >
        {t.laneFailed}
        {#if laneCounts?.failed}
          <span class="tab-badge">{laneCounts.failed}</span>
        {/if}
      </button>
    </div>

    <div class="filter-controls">
      <div class="search-box">
        <input
          bind:this={searchInputRef}
          type="text"
          placeholder={t.searchPlaceholder}
          bind:value={searchQuery}
          aria-label={t.searchPlaceholder}
        />
      </div>

      <select bind:value={verdictFilter} class="select-filter" aria-label={t.filterVerdict}>
        <option value="all">{t.filterVerdict}</option>
        <option value="PROCESSING">{currentLang === 'bg' ? 'В обработка' : 'Processing'}</option>
        <option value="CONTRADICTED">CONTRADICTED</option>
        <option value="SUPPORTED">SUPPORTED</option>
        <option value="UNVERIFIABLE">UNVERIFIABLE</option>
        <option value="NOT_CHECKABLE">NOT_CHECKABLE</option>
        <option value="ALREADY_CHECKED">ALREADY_CHECKED</option>
      </select>

      <select bind:value={sortBy} class="select-filter" aria-label="Сортиране">
        <option value="score">{t.sortScore}</option>
        <option value="views">{t.sortViews}</option>
        <option value="quality">{t.sortQuality}</option>
      </select>

      <button
        class="btn btn-secondary btn-sm feed-toggle-btn"
        class:active={isVideoFeedOpen}
        onclick={onToggleVideoFeed}
        title={currentLang === 'bg' ? 'Включи/изключи видео фийд и досие' : 'Toggle video feed & dossier'}
      >
        🎬 {t.toggleFeed}
      </button>

      {#if activeLane === 'leads'}
        {#if queuedBehindCap > 0}
          <button
            class="btn btn-secondary btn-sm"
            onclick={handlePullMoreClick}
            disabled={isPullingMore}
            title={t.pullMoreTooltip}
          >
            📥 {t.pullMoreButton} ({queuedBehindCap})
          </button>
        {:else}
          <button
            class="btn btn-secondary btn-sm btn-cap-all"
            onclick={handlePullMoreClick}
            title={currentLang === 'bg'
              ? 'Всички проверени лийдове са показани в списъка. За да добавите нови видеа, поставете връзка в полето горе.'
              : 'All verified leads are already displayed. Paste new TikTok links in the input bar above.'}
          >
            ✓ {currentLang === 'bg' ? 'Всички лийдове са заредени' : 'All leads loaded'} ({cards.length})
          </button>
        {/if}
        {#if pullToast}
          <span class="pull-toast">{pullToast}</span>
        {/if}
      {/if}
    </div>
  </div>

  {#if checkedIds.size > 0}
    <div class="batch-bar">
      <span>{checkedIds.size} {currentLang === 'bg' ? 'избрани видеа' : 'selected videos'}</span>
      <div class="batch-actions">
        <button class="btn btn-primary btn-sm" onclick={onBatchKeep}>
          ✓ {currentLang === 'bg' ? 'Запази избраните' : 'Keep all'}
        </button>
        <button class="btn btn-danger btn-sm" onclick={onBatchDiscard}>
          ✕ {currentLang === 'bg' ? 'Отхвърли избраните' : 'Discard all'}
        </button>
      </div>
    </div>
  {/if}

  <div class="inbox-body-layout" class:with-feed={isVideoFeedOpen}>
    <div
      class="virtual-scroll-viewport"
      bind:this={containerRef}
      onscroll={handleScroll}
      role="listbox"
      aria-label="Списък с видеа"
    >
      {#if filteredCards.length === 0}
        <div class="empty-inbox">
          <p>{currentLang === 'bg' ? 'Няма видеа в този раздел.' : 'No videos in this lane.'}</p>
        </div>
      {:else}
        <div class="virtual-scroll-spacer" style="height: {totalHeight}px;">
          {#each visibleItems as { item, top } (item.id)}
            <div class="virtual-item-wrapper" style="transform: translateY({top}px);">
              <LeadCardComponent
                card={item}
                isSelected={selectedId === item.id}
                isChecked={checkedIds.has(item.id)}
                {currentLang}
                isCompact={Boolean((isVideoFeedOpen && !isDrawerOpen) || isDrawerOpen)}
                onSelect={onSelectCard}
                onToggleCheck={onToggleCheck}
                onKeep={onKeep}
                onDiscard={onDiscard}
                onResearchMore={onResearchMore}
              />
            </div>
          {/each}
        </div>
      {/if}
    </div>

    {#if isVideoFeedOpen && !isDrawerOpen}
      <aside class="feed-sidebar">
        {#if selectedCard}
          <div class="feed-card">
            <div class="feed-header">
              <span class="feed-author">@{selectedCard.creator_handle || 'tiktok_user'}</span>
              <span class="feed-views">{selectedCard.views.toLocaleString()} {currentLang === 'bg' ? 'гледания' : 'views'}</span>
            </div>
            <div class="feed-media-box">
              {#key selectedCard.id}
                <video
                  bind:this={inboxVideoElement}
                  src="{coreUrl}/api/media/{selectedCard.id}/video.mp4"
                  poster={selectedCard.thumbnail_url || undefined}
                  controls
                  class="feed-video-player"
                  onplay={() => {
                    if (inboxVideoElement) {
                      registerPlaybackElement(inboxVideoElement, {
                        videoId: selectedCard.id,
                        creatorHandle: selectedCard.creator_handle,
                        caption: selectedCard.top_claim_text,
                        thumbnailUrl: selectedCard.thumbnail_url,
                      });
                      setPlaybackPlaying(true);
                    }
                  }}
                  onpause={() => setPlaybackPlaying(false)}
                  ontimeupdate={() => {
                    if (inboxVideoElement) {
                      updatePlaybackProgress(inboxVideoElement.currentTime, inboxVideoElement.duration);
                    }
                  }}
                >
                  <track kind="captions" />
                </video>
              {/key}
            </div>
            <div class="feed-details">
              <div class="feed-details-header">
                <span class="feed-claim-label">{currentLang === 'bg' ? 'Основно твърдение:' : 'Key Claim:'}</span>
                {#if feedToast}
                  <span class="feed-toast">{feedToast}</span>
                {/if}
              </div>
              <p class="feed-claim-text">{selectedCard.top_claim_text || (currentLang === 'bg' ? 'Видеото се обработва...' : 'Processing...')}</p>
              <div class="feed-text-actions">
                <button class="btn btn-secondary btn-sm" onclick={handleCopyFeedText} title="Копирай текста">
                  📋 {currentLang === 'bg' ? 'Копирай' : 'Copy'}
                </button>
                <button class="btn btn-secondary btn-sm" onclick={handleAiCheckFeed} disabled={isAiCheckingFeed} title="AI проверка и копиране">
                  {isAiCheckingFeed ? '...' : '🤖 AI проверка'}
                </button>
                {#if onOpenDetail}
                  <button class="btn btn-secondary btn-sm btn-dossier" onclick={() => onOpenDetail?.(selectedCard!.id)} title="Отвори пълно досие и доказателства">
                    📖 {currentLang === 'bg' ? 'Досие' : 'Dossier'}
                  </button>
                {/if}
              </div>
            </div>
            <div class="feed-quick-actions">
              <button class="btn btn-primary btn-sm" onclick={() => onKeep(selectedCard!.id)}>
                ✓ {currentLang === 'bg' ? 'Запази' : 'Keep'}
              </button>
              <button class="btn btn-danger btn-sm" onclick={() => onDiscard(selectedCard!.id)}>
                ✕ {currentLang === 'bg' ? 'Отхвърли' : 'Discard'}
              </button>
              <button
                class="btn btn-secondary btn-sm btn-research-action"
                class:active-researching={selectedCard?.research_running}
                disabled={Boolean(selectedCard?.research_running)}
                onclick={() => onResearchMore(selectedCard!.id)}
                title={selectedCard?.research_running
                  ? (currentLang === 'bg' ? 'В момента тече задълбочено проучване...' : 'Deep research currently running...')
                  : t.researchAction}
              >
                {#if selectedCard?.research_running}
                  <span class="spin-icon">⚙</span> {currentLang === 'bg' ? 'Проучва се...' : 'Researching...'}
                {:else}
                  ⚡ {currentLang === 'bg' ? 'Проучи' : 'Research'}
                {/if}
              </button>
            </div>
          </div>
        {:else}
          <div class="feed-empty">
            <p>{currentLang === 'bg' ? 'Няма избрано видео.' : 'No video selected.'}</p>
          </div>
        {/if}
      </aside>
    {/if}
  </div>

  <footer class="keyboard-guide">
    <span>{t.keyboardHelp}</span>
  </footer>
</div>

<style>
  .inbox-container {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-canvas);
  }

  .has-right-column .virtual-scroll-viewport {
    padding: 0.5rem 0.65rem;
  }

  .has-right-column .inbox-subbar {
    padding: 0.5rem 0.75rem;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .has-right-column .filter-controls {
    flex-wrap: wrap;
  }

  .has-right-column .search-box input {
    width: 140px;
  }

  .inbox-subbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.85rem;
    padding: var(--subbar-pad-y, 0.6rem) 1.15rem;
    background: var(--surface-bg);
    border-bottom: 1px solid var(--border-color);
  }

  :global([data-display-profile='18']) .has-right-column .inbox-subbar {
    padding: 0.42rem 0.7rem;
    gap: 0.35rem;
  }

  :global([data-display-profile='18']) .has-right-column .lane-tab {
    padding: 0.3rem 0.58rem;
    font-size: 0.76rem;
  }

  :global([data-display-profile='13']) .lane-tab,
  :global([data-display-profile='14']) .lane-tab {
    padding: 0.32rem 0.6rem;
    font-size: 0.77rem;
  }

  .lane-tabs {
    display: flex;
    gap: 0.35rem;
  }

  .lane-tab {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.4rem 0.85rem;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .lane-tab:hover {
    color: var(--text-body);
    background: var(--hover-bg);
  }

  .lane-tab.active {
    background: var(--primary-accent);
    color: #fff;
    border-color: var(--primary-accent);
  }

  .tab-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 0.7rem;
    font-weight: 700;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--pill-bg);
    border: 1px solid var(--border-color);
    margin-left: 0.35rem;
    color: var(--text-muted);
  }

  .lane-tab.active .tab-badge {
    background: rgba(255, 255, 255, 0.25);
    color: #fff;
    border-color: transparent;
  }

  .filter-controls {
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }

  .search-box input {
    width: 200px;
    padding: 0.35rem 0.65rem;
    font-size: 0.82rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
  }

  :global([data-widescreen='true']) .search-box input {
    width: 240px;
  }

  :global([data-widescreen='true']) .has-right-column .search-box input {
    width: 190px;
  }

  .select-filter {
    padding: 0.35rem 0.6rem;
    font-size: 0.82rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
  }

  .batch-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--primary-accent-alpha);
    border-bottom: 1px solid var(--primary-accent);
    padding: 0.4rem 1.25rem;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--primary-accent);
  }

  .batch-actions {
    display: flex;
    gap: 0.5rem;
  }

  .virtual-scroll-viewport {
    flex: 1;
    overflow-y: auto;
    position: relative;
    padding: 0.65rem 1.25rem;
  }

  .virtual-scroll-spacer {
    position: relative;
    width: 100%;
  }

  .virtual-item-wrapper {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 86px;
  }

  .empty-inbox {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 300px;
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  .keyboard-guide {
    padding: 0.35rem 1.25rem;
    background: var(--surface-bg);
    border-top: 1px solid var(--border-color);
    font-size: 0.74rem;
    color: var(--text-muted);
  }

  .inbox-body-layout {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  .feed-toggle-btn.active {
    background: var(--primary-accent);
    color: #fff;
    border-color: var(--primary-accent);
  }

  .pull-toast {
    font-size: 0.8rem;
    color: var(--primary-accent);
    padding: 3px 8px;
    background: rgba(37, 99, 235, 0.12);
    border: 1px solid rgba(37, 99, 235, 0.25);
    border-radius: 4px;
    font-weight: 500;
  }

  .btn-cap-all {
    opacity: 0.85;
    color: var(--text-muted);
    border-style: dashed;
  }

  .btn-cap-all:hover {
    opacity: 1;
    color: var(--text-color);
  }

  .feed-sidebar {
    width: var(--inbox-feed-width, 380px);
    min-width: 300px;
    max-width: 450px;
    border-left: 1px solid var(--border-color);
    background: var(--surface-bg);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 0.85rem;
    flex-shrink: 0;
  }

  .feed-card {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .feed-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .feed-author {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--primary-accent);
  }

  .feed-views {
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  .feed-media-box {
    width: 100%;
    aspect-ratio: 9 / 16;
    max-height: var(--video-stage-height, 420px);
    background: #000;
    border-radius: 8px;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .feed-video-player {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .feed-details {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.65rem 0.85rem;
  }

  .feed-claim-label {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .feed-claim-text {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-heading);
    line-height: 1.4;
  }

  .feed-details-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .feed-toast {
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--verdict-supported);
    background: rgba(16, 185, 129, 0.15);
    border: 1px solid rgba(16, 185, 129, 0.3);
    padding: 1px 5px;
    border-radius: 4px;
  }

  .feed-text-actions {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.4rem;
    flex-wrap: wrap;
  }

  .feed-text-actions .btn {
    font-size: 0.74rem;
    padding: 0.25rem 0.5rem;
  }

  .btn-dossier {
    background: var(--primary-accent);
    color: #fff;
    border: none;
  }

  .btn-dossier:hover {
    opacity: 0.9;
  }

  .feed-quick-actions {
    display: flex;
    gap: 0.5rem;
  }

  .feed-quick-actions .btn {
    flex: 1;
    justify-content: center;
    padding: 0.45rem;
    font-size: 0.85rem;
    font-weight: 700;
  }

  .btn-research-action.active-researching {
    background: rgba(59, 130, 246, 0.25);
    color: #60a5fa;
    border-color: rgba(59, 130, 246, 0.5);
    cursor: wait;
  }

  .spin-icon {
    display: inline-block;
    animation: iconSpin 1.2s linear infinite;
  }

  @keyframes iconSpin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .feed-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--text-muted);
    font-size: 0.85rem;
  }
</style>
