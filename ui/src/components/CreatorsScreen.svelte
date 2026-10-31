<script lang="ts">
  import { onMount } from 'svelte';
  import { assessCreatorRisk, fetchCreators, setCreatorWatchlist } from '../lib/api';
  import { startAiJob, finishAiJob } from '../lib/aiTasks.svelte';
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

  let assessingHandles = $state<Set<string>>(new Set());

  async function assessRisk(c: Creator) {
    if (assessingHandles.has(c.handle)) return;
    assessingHandles.add(c.handle);
    assessingHandles = new Set(assessingHandles);
    startAiJob('creator-risk', c.handle, currentLang === 'bg' ? `Проверка на автор: @${c.handle}` : `Creator check: @${c.handle}`);
    try {
      const res = await assessCreatorRisk(c.handle, c.display_name);
      c.risk_assessment = res;
      c.risk_assessment_json = JSON.stringify(res);
      creators = [...creators];
      finishAiJob('creator-risk', c.handle, currentLang === 'bg' ? `✓ Проверката на @${c.handle} завърши` : `✓ @${c.handle} check complete`, true);
    } catch {
      finishAiJob('creator-risk', c.handle, currentLang === 'bg' ? `Грешка при проверка на @${c.handle}` : `Error checking @${c.handle}`, false);
    } finally {
      assessingHandles.delete(c.handle);
      assessingHandles = new Set(assessingHandles);
    }
  }

  function getCreatorRiskInfo(c: Creator) {
    if (c.risk_assessment) {
      return formatCreatorRisk(c.risk_assessment);
    }
    if (c.risk_assessment_json) {
      try {
        const parsed = JSON.parse(c.risk_assessment_json);
        return formatCreatorRisk(parsed);
      } catch (e) {
      }
    }
    const flagged = c.flagged_count ?? 0;
    if (flagged >= 3) {
      return {
        label: currentLang === 'bg' ? '🔴 Висок' : '🔴 High',
        badgeClass: 'risk-high',
        confidencePercent: 70,
        confidenceClass: 'conf-high',
        confidenceText: currentLang === 'bg' ? 'Висока' : 'High',
        webSourcesCount: 0,
      };
    }
    if (flagged > 0) {
      return {
        label: currentLang === 'bg' ? '🟡 Умерен' : '🟡 Moderate',
        badgeClass: 'risk-moderate',
        confidencePercent: 40,
        confidenceClass: 'conf-moderate',
        confidenceText: currentLang === 'bg' ? 'Умерена' : 'Moderate',
        webSourcesCount: 0,
      };
    }
    return {
      label: currentLang === 'bg' ? '⚪ Непроверен' : '⚪ Unverified',
      badgeClass: 'risk-unverified',
      confidencePercent: 15,
      confidenceClass: 'conf-low',
      confidenceText: currentLang === 'bg' ? 'Нисък' : 'Low',
      webSourcesCount: 0,
    };
  }

  function formatCreatorRisk(ra: any) {
    const level = ra.risk_level || 'unverified';
    const conf = ra.confidence ?? 0.2;
    const confPercent = Math.round(conf * 100);
    const confLevel = ra.confidence_level || (conf >= 0.7 ? 'high' : conf >= 0.35 ? 'moderate' : 'low');

    let label = currentLang === 'bg' ? '⚪ Непроверен' : '⚪ Unverified';
    let badgeClass = 'risk-unverified';
    if (level === 'high') {
      label = currentLang === 'bg' ? '🔴 Висок' : '🔴 High';
      badgeClass = 'risk-high';
    } else if (level === 'moderate') {
      label = currentLang === 'bg' ? '🟡 Умерен' : '🟡 Moderate';
      badgeClass = 'risk-moderate';
    } else if (level === 'low') {
      label = currentLang === 'bg' ? '🟢 Нисък' : '🟢 Low';
      badgeClass = 'risk-low';
    }

    return {
      label,
      badgeClass,
      confidencePercent: confPercent,
      confidenceClass: `conf-${confLevel}`,
      confidenceText: confLevel === 'high' ? (currentLang === 'bg' ? 'Висока' : 'High') : confLevel === 'moderate' ? (currentLang === 'bg' ? 'Умерена' : 'Moderate') : (currentLang === 'bg' ? 'Нисък' : 'Low'),
      webSourcesCount: ra.web_sources?.length ?? 0,
    };
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
            <th>{currentLang === 'bg' ? 'Маркирани видеа' : 'Flagged videos'}</th>
            <th>{currentLang === 'bg' ? 'Риск и надеждност' : 'Risk & Confidence'}</th>
            <th>{currentLang === 'bg' ? 'Ниво' : 'Depth'}</th>
            <th>{currentLang === 'bg' ? 'Наблюдение' : 'Watchlist'}</th>
          </tr>
        </thead>
        <tbody>
          {#each creators as c}
            {@const riskInfo = getCreatorRiskInfo(c)}
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
              <td>
                <div class="risk-cell">
                  <span class="author-risk-badge {riskInfo.badgeClass}">{riskInfo.label}</span>
                  <span class="author-confidence-pill {riskInfo.confidenceClass}" title="{riskInfo.confidenceText} увереност">
                    {riskInfo.confidencePercent}% ({riskInfo.confidenceText})
                  </span>
                  <button
                    class="btn-mini-web"
                    onclick={() => assessRisk(c)}
                    disabled={assessingHandles.has(c.handle)}
                    title={currentLang === 'bg' ? 'Провери автора в уеб източници' : 'Check creator in web sources'}
                  >
                    {#if assessingHandles.has(c.handle)}
                      ...
                    {:else}
                      🌐 {currentLang === 'bg' ? 'Провери' : 'Check'}
                    {/if}
                  </button>
                </div>
              </td>
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

  .risk-cell {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .author-risk-badge {
    font-size: 0.72rem;
    font-weight: 800;
    padding: 2px 8px;
    border-radius: 999px;
  }

  .risk-high {
    background: rgba(225, 29, 72, 0.18);
    color: var(--verdict-contradicted);
    border: 1px solid rgba(225, 29, 72, 0.35);
  }

  .risk-moderate {
    background: rgba(217, 119, 6, 0.18);
    color: var(--verdict-unverifiable);
    border: 1px solid rgba(217, 119, 6, 0.35);
  }

  .risk-low {
    background: rgba(16, 185, 129, 0.18);
    color: var(--verdict-supported);
    border: 1px solid rgba(16, 185, 129, 0.35);
  }

  .risk-unverified {
    background: rgba(148, 163, 184, 0.15);
    color: #94a3b8;
    border: 1px solid rgba(148, 163, 184, 0.3);
  }

  .author-confidence-pill {
    font-size: 0.7rem;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 999px;
  }

  .conf-high {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.3);
  }

  .conf-moderate {
    background: rgba(217, 119, 6, 0.15);
    color: #d97706;
    border: 1px solid rgba(217, 119, 6, 0.3);
  }

  .conf-low {
    background: rgba(225, 29, 72, 0.15);
    color: #e11d48;
    border: 1px solid rgba(225, 29, 72, 0.3);
  }

  .btn-mini-web {
    background: transparent;
    border: 1px solid rgba(99, 102, 241, 0.4);
    color: #818cf8;
    font-size: 0.7rem;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .btn-mini-web:hover:not(:disabled) {
    background: rgba(99, 102, 241, 0.2);
    border-color: #818cf8;
  }
</style>
