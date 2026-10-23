<script lang="ts">
  import { onMount } from 'svelte';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { Lane, LeadCard } from '../lib/types';
  import LeadCardComponent from './LeadCard.svelte';

  let {
    cards,
    activeLane,
    selectedId,
    checkedIds,
    currentLang,
    isVideoFeedOpen = true,
    onToggleVideoFeed,
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
    onToggleVideoFeed?: () => void;
    laneCounts?: Record<string, number>;
    onSelectCard: (id: string) => void;
    onToggleCheck: (id: string) => void;
    onChangeLane: (lane: Lane) => void;
    onPullMore: () => Promise<void>;
    onKeep: (id: string) => void;
    onDiscard: (id: string) => void;
    onResearchMore: (id: string) => void;
    onBatchKeep: () => void;
    onBatchDiscard: () => void;
  } = $props();

  const t = $derived(translations[currentLang]);

  let searchQuery = $state('');
  let verdictFilter = $state('all');
  let sortBy = $state<'score' | 'views' | 'quality'>('score');
  let searchInputRef = $state<HTMLInputElement | null>(null);

  let containerRef = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let containerHeight = $state(600);

  const ITEM_HEIGHT = 92;
  const BUFFER_COUNT = 6;

  export function focusSearch() {
    if (searchInputRef) {
      searchInputRef.focus();
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
        return claim.includes(q) || creator.includes(q) || reason.includes(q);
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

  const totalHeight = $derived(filteredCards.length * ITEM_HEIGHT);

  const visibleRange = $derived.by(() => {
    const startIdx = Math.max(0, Math.floor(scrollTop / ITEM_HEIGHT) - BUFFER_COUNT);
    const visibleCount = Math.ceil(containerHeight / ITEM_HEIGHT) + 2 * BUFFER_COUNT;
    const endIdx = Math.min(filteredCards.length, startIdx + visibleCount);
    return { startIdx, endIdx };
  });

  const visibleItems = $derived.by(() => {
    const { startIdx, endIdx } = visibleRange;
    return filteredCards.slice(startIdx, endIdx).map((item: LeadCard, idx: number) => ({
      item,
      top: (startIdx + idx) * ITEM_HEIGHT,
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

<div class="inbox-container">
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
        <button class="btn btn-secondary btn-sm" onclick={onPullMore} title={t.pullMoreButton}>
          +15 {t.pullMoreButton}
        </button>
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

  .inbox-subbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.65rem 1.25rem;
    background: var(--surface-bg);
    border-bottom: 1px solid var(--border-color);
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
</style>
