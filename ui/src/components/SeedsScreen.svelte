<script lang="ts">
  import { onMount } from 'svelte';
  import { deleteSeed, fetchSeeds, upsertSeed } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { Seed } from '../lib/types';

  let { currentLang }: { currentLang: Language } = $props();
  const t = $derived(translations[currentLang]);

  let seeds = $state<Seed[]>([]);
  let isLoading = $state(true);

  let newKind = $state<'hashtag' | 'keyword' | 'creator'>('hashtag');
  let newValue = $state('');
  let newPriority = $state<number>(5);

  onMount(async () => {
    isLoading = true;
    try {
      seeds = await fetchSeeds();
    } finally {
      isLoading = false;
    }
  });

  async function handleAddSeed() {
    if (!newValue.trim()) return;
    const added = await upsertSeed({
      kind: newKind,
      value: newValue.trim(),
      active: true,
      priority: newPriority,
    });
    seeds = [...seeds, added];
    newValue = '';
  }

  async function handleDelete(id: number) {
    await deleteSeed(id);
    seeds = seeds.filter((s) => s.id !== id);
  }

  async function handleToggleActive(s: Seed) {
    const updated = !s.active;
    await upsertSeed({ ...s, active: updated });
    s.active = updated;
    seeds = [...seeds];
  }

  async function handleSetPriority(s: Seed, priority: number) {
    s.priority = priority;
    await upsertSeed({ ...s, priority });
    seeds = [...seeds];
  }
</script>

<div class="screen-container">
  <div class="screen-header">
    <h2>{t.tabSeeds}</h2>
    <span class="count-pill">{seeds.length} {currentLang === 'bg' ? 'записи' : 'seeds'}</span>
  </div>

  <div class="add-seed-card">
    <select bind:value={newKind} class="select-kind" aria-label="Тип на ключова дума">
      <option value="hashtag">Hashtag (#)</option>
      <option value="keyword">Keyword</option>
      <option value="creator">Creator (@)</option>
    </select>
    <input
      type="text"
      placeholder={currentLang === 'bg' ? 'Въведете ключова дума или хаштаг...' : 'Enter seed value...'}
      bind:value={newValue}
      class="seed-input"
      aria-label="Стойност на ключова дума"
      onkeydown={(e) => e.key === 'Enter' && handleAddSeed()}
    />

    <div class="priority-picker" title={currentLang === 'bg' ? 'Приоритет (1-5 звезди: повече звезди = по-висок приоритет)' : 'Priority (1-5 stars: more stars = higher priority)'}>
      <span class="priority-picker-label">{currentLang === 'bg' ? 'Приоритет:' : 'Priority:'}</span>
      <div class="stars-group">
        {#each [1, 2, 3, 4, 5] as star}
          <button
            type="button"
            class="star-btn"
            class:active={star <= newPriority}
            onclick={() => (newPriority = star)}
            aria-label={`${star} звезди`}
            title={`${star} / 5`}
          >
            ★
          </button>
        {/each}
      </div>
    </div>

    <button class="btn btn-primary" onclick={handleAddSeed} disabled={!newValue.trim()}>
      {currentLang === 'bg' ? 'Добави' : 'Add Seed'}
    </button>
  </div>

  {#if isLoading}
    <div class="loading-state">{currentLang === 'bg' ? 'Зареждане на думите...' : 'Loading seeds...'}</div>
  {:else if seeds.length === 0}
    <div class="empty-state">{currentLang === 'bg' ? 'Няма конфигурирани ключови думи.' : 'No seeds configured.'}</div>
  {:else}
    <div class="table-card">
      <table class="data-table">
        <thead>
          <tr>
            <th>{currentLang === 'bg' ? 'Тип' : 'Kind'}</th>
            <th>{currentLang === 'bg' ? 'Стойност' : 'Value'}</th>
            <th>{currentLang === 'bg' ? 'Приоритет (1-5 звезди)' : 'Priority (1-5 stars)'}</th>
            <th>{currentLang === 'bg' ? 'Последно пускане' : 'Last run'}</th>
            <th>{currentLang === 'bg' ? 'Статус' : 'Status'}</th>
            <th>{currentLang === 'bg' ? 'Действия' : 'Actions'}</th>
          </tr>
        </thead>
        <tbody>
          {#each seeds as s}
            <tr>
              <td><span class="kind-tag {s.kind}">{s.kind}</span></td>
              <td><strong>{s.value}</strong></td>
              <td>
                <div class="stars-group table-stars" title={`${s.priority ?? 5} / 5 звезди`}>
                  {#each [1, 2, 3, 4, 5] as star}
                    <button
                      type="button"
                      class="star-btn"
                      class:active={star <= (s.priority ?? 5)}
                      onclick={() => handleSetPriority(s, star)}
                      aria-label={`${star} звезди`}
                      title={`${star} звезди`}
                    >
                      ★
                    </button>
                  {/each}
                  <span class="stars-count-text">({s.priority ?? 5}/5)</span>
                </div>
              </td>
              <td>{s.last_ran_at ? s.last_ran_at.substring(0, 16) : '-'}</td>
              <td>
                <button
                  class="toggle-btn"
                  class:active={s.active}
                  onclick={() => handleToggleActive(s)}
                  aria-label="Превключи статус"
                >
                  {s.active ? (currentLang === 'bg' ? 'Активен' : 'Active') : (currentLang === 'bg' ? 'Паузиран' : 'Paused')}
                </button>
              </td>
              <td>
                <button class="btn btn-text btn-danger btn-sm" onclick={() => handleDelete(s.id)}>
                  ✕
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

  .add-seed-card {
    display: flex;
    gap: 0.5rem;
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.75rem;
    margin-bottom: 1rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .select-kind {
    padding: 0.45rem 0.65rem;
    font-size: 0.85rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
  }

  .seed-input {
    flex: 1;
    min-width: 200px;
    padding: 0.45rem 0.85rem;
    font-size: 0.85rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
  }

  .priority-picker {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0.65rem;
    background: var(--input-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
  }

  .priority-picker-label {
    font-size: 0.76rem;
    color: var(--text-muted);
    font-weight: 600;
  }

  .stars-group {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }

  .table-stars {
    gap: 3px;
  }

  .star-btn {
    background: none;
    border: none;
    font-size: 1.15rem;
    line-height: 1;
    cursor: pointer;
    color: #64748b;
    padding: 0 1px;
    transition: color 0.12s ease, transform 0.12s ease;
  }

  .star-btn:hover {
    transform: scale(1.25);
    color: #fbbf24;
  }

  .star-btn.active {
    color: #f59e0b;
    text-shadow: 0 0 4px rgba(245, 158, 11, 0.4);
  }

  .stars-count-text {
    font-size: 0.72rem;
    color: var(--text-muted);
    margin-left: 0.3rem;
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

  .kind-tag {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .kind-tag.hashtag {
    background: rgba(59, 130, 246, 0.15);
    color: #3b82f6;
  }

  .kind-tag.keyword {
    background: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
  }

  .kind-tag.creator {
    background: rgba(168, 85, 247, 0.15);
    color: #a855f7;
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