<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchCreators, setCreatorWatchlist } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { Creator } from '../lib/types';

  let { currentLang }: { currentLang: Language } = $props();
  const t = $derived(translations[currentLang]);

  let creators = $state<Creator[]>([]);
  let isLoading = $state(true);

  onMount(async () => {
    isLoading = true;
    try {
      creators = await fetchCreators();
    } finally {
      isLoading = false;
    }
  });

  async function toggleWatchlist(c: Creator) {
    const updated = !c.watchlist;
    await setCreatorWatchlist(c.handle, updated);
    c.watchlist = updated;
    creators = [...creators];
  }
</script>

<div class="screen-container">
  <div class="screen-header">
    <h2>{t.tabCreators}</h2>
    <span class="count-pill">{creators.length} {currentLang === 'bg' ? 'автора' : 'creators'}</span>
  </div>

  {#if isLoading}
    <div class="loading-state">{currentLang === 'bg' ? 'Зареждане на авторите...' : 'Loading creators...'}</div>
  {:else if creators.length === 0}
    <div class="empty-state">{currentLang === 'bg' ? 'Все още няма записани автори.' : 'No creators recorded yet.'}</div>
  {:else}
    <div class="table-card">
      <table class="data-table">
        <thead>
          <tr>
            <th>{currentLang === 'bg' ? 'Автор' : 'Handle'}</th>
            <th>{currentLang === 'bg' ? 'Последователи' : 'Followers'}</th>
            <th>{currentLang === 'bg' ? 'Флагнати видеа' : 'Flagged videos'}</th>
            <th>{currentLang === 'bg' ? 'Дълбочина' : 'Depth'}</th>
            <th>{currentLang === 'bg' ? 'Наблюдение' : 'Watchlist'}</th>
          </tr>
        </thead>
        <tbody>
          {#each creators as c}
            <tr>
              <td>
                <div class="creator-cell">
                  <strong>@{c.handle}</strong>
                  {#if c.display_name}
                    <span class="creator-sub">({c.display_name})</span>
                  {/if}
                </div>
              </td>
              <td>{c.follower_count ? c.follower_count.toLocaleString() : '-'}</td>
              <td><span class="flagged-count">{c.flagged_count}</span></td>
              <td>{c.expansion_depth}</td>
              <td>
                <button
                  class="toggle-btn"
                  class:active={c.watchlist}
                  onclick={() => toggleWatchlist(c)}
                  aria-label="Превключи списък за наблюдение"
                >
                  {c.watchlist ? (currentLang === 'bg' ? 'Активен' : 'Active') : (currentLang === 'bg' ? 'Неактивен' : 'Inactive')}
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
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
    margin-bottom: 1rem;
  }

  .screen-header h2 {
    margin: 0;
    font-size: 1.2rem;
    color: var(--text-heading);
  }

  .count-pill {
    font-size: 0.78rem;
    font-weight: 600;
    padding: 2px 8px;
    background: var(--pill-bg);
    border-radius: 999px;
    color: var(--text-muted);
  }

  .table-card {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    overflow: hidden;
  }

  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .data-table th,
  .data-table td {
    padding: 0.65rem 1rem;
    text-align: left;
    border-bottom: 1px solid var(--border-color);
  }

  .data-table th {
    background: var(--card-bg);
    font-weight: 700;
    color: var(--text-muted);
    font-size: 0.78rem;
  }

  .creator-cell {
    display: flex;
    flex-direction: column;
  }

  .creator-sub {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .flagged-count {
    font-weight: 700;
    color: var(--verdict-contradicted);
  }

  .toggle-btn {
    padding: 0.25rem 0.65rem;
    font-size: 0.75rem;
    font-weight: 600;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--pill-bg);
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .toggle-btn.active {
    background: rgba(16, 185, 129, 0.15);
    color: var(--verdict-supported);
    border-color: var(--verdict-supported);
  }

  .loading-state,
  .empty-state {
    padding: 2rem;
    text-align: center;
    color: var(--text-muted);
  }
</style>
