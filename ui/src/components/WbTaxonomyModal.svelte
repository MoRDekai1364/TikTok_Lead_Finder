<script lang="ts">
  import './wb.css';
  import type { Taxonomy } from './workbenchModel';

  let { taxonomy = $bindable(), open = $bindable(), onchange }: { taxonomy: Taxonomy; open: boolean; onchange?: () => void } = $props();

  type Tab = 'topics' | 'ratings' | 'claimTypes' | 'claimKinds' | 'harms';
  let activeTab = $state<Tab>('topics');
  let newItem = $state('');

  const tabLabels: Record<Tab, { title: string; hint: string }> = {
    topics: { title: 'Теми и сфери (Topics)', hint: 'Тематични области (напр. Climate, Health, Civic, Politics, Economy, Elections).' },
    ratings: { title: 'Оценки и присъди (Ratings)', hint: 'Стандартизирани оценки на твърденията (напр. Misleading, False, Unsubstantiated, Out of Scope).' },
    claimTypes: { title: 'Типове твърдения (Claim Types)', hint: 'Естество на изказването във видеото (напр. твърдение, число, цитат, прогноза, причинна връзка).' },
    claimKinds: { title: 'Вид на твърдението (Claim Status)', hint: 'Статус на твърдението в доклада (напр. New Claim, Known Claim, Trend).' },
    harms: { title: 'Ниво на вреда (Harm Levels)', hint: 'Оценка на потенциалната вреда от дезинформацията (напр. Low, Medium, High).' }
  };

  function addItem() {
    const val = newItem.trim();
    if (!val) return;
    if (!taxonomy[activeTab].includes(val)) {
      taxonomy[activeTab] = [...taxonomy[activeTab], val];
      newItem = '';
      onchange?.();
    }
  }

  function removeItem(idx: number) {
    taxonomy[activeTab] = taxonomy[activeTab].filter((_, i) => i !== idx);
    onchange?.();
  }

  function move(idx: number, delta: number) {
    const target = idx + delta;
    if (target < 0 || target >= taxonomy[activeTab].length) return;
    const copy = [...taxonomy[activeTab]];
    [copy[idx], copy[target]] = [copy[target], copy[idx]];
    taxonomy[activeTab] = copy;
    onchange?.();
  }
</script>

{#if open}
  <div class="tax-backdrop" onclick={() => (open = false)} role="presentation">
    <div class="tax-dialog" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-labelledby="tax-title">
      <div class="tax-header">
        <div>
          <h3 id="tax-title">🏷️ Управление на таксономията и категориите</h3>
          <p class="hint">Добавяй, редактирай и премахвай теми, оценки и типове твърдения. Промените веднага се отразяват в падащите менюта на целия инструмент.</p>
        </div>
        <button class="ghost" onclick={() => (open = false)} aria-label="Затвори">✕</button>
      </div>

      <div class="tax-tabs">
        {#each (['topics', 'ratings', 'claimTypes', 'claimKinds', 'harms'] as Tab[]) as t}
          <button class:on={activeTab === t} onclick={() => { activeTab = t; newItem = ''; }}>
            {tabLabels[t].title.split(' ')[0]} ({taxonomy[t].length})
          </button>
        {/each}
      </div>

      <div class="tax-body">
        <div class="tax-desc">
          <b>{tabLabels[activeTab].title}</b>
          <div class="hint">{tabLabels[activeTab].hint}</div>
        </div>

        <div class="tax-list">
          {#each taxonomy[activeTab] as item, idx (idx)}
            <div class="tax-item">
              <span class="tax-idx">#{idx + 1}</span>
              <input
                class="tax-input"
                bind:value={taxonomy[activeTab][idx]}
                onchange={() => onchange?.()}
              />
              <button class="ghost" onclick={() => move(idx, -1)} disabled={idx === 0} title="Нагоре">↑</button>
              <button class="ghost" onclick={() => move(idx, 1)} disabled={idx === taxonomy[activeTab].length - 1} title="Надолу">↓</button>
              <button class="ghost danger" onclick={() => removeItem(idx)} title="Премахни">✕</button>
            </div>
          {/each}
        </div>

        <div class="tax-add">
          <input
            placeholder="Име на новата категория / тема..."
            bind:value={newItem}
            onkeydown={(e) => e.key === 'Enter' && addItem()}
          />
          <button class="primary" onclick={addItem}>+ Добави</button>
        </div>
      </div>

      <div class="tax-footer">
        <span class="hint">Всички промени се запазват автоматично.</span>
        <button class="primary" onclick={() => (open = false)}>Готово</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .tax-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 15, 0.75);
    backdrop-filter: blur(4px);
    z-index: 999;
    display: grid;
    place-items: center;
    padding: 16px;
  }
  .tax-dialog {
    background: #161922;
    border: 1px solid #353b50;
    border-radius: 12px;
    width: 100%;
    max-width: 620px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.55);
    display: flex;
    flex-direction: column;
    max-height: 85vh;
  }
  .tax-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding: 16px 18px 12px;
    border-bottom: 1px solid #2b3040;
  }
  .tax-header h3 {
    margin: 0 0 4px;
    font-size: 16px;
  }
  .tax-tabs {
    display: flex;
    gap: 6px;
    padding: 10px 18px;
    background: #11141c;
    border-bottom: 1px solid #2b3040;
    overflow-x: auto;
  }
  .tax-tabs button {
    background: transparent;
    border: 1px solid transparent;
    padding: 4px 10px;
    border-radius: 6px;
    font-size: 12px;
    cursor: pointer;
    color: #8b93a8;
    white-space: nowrap;
  }
  .tax-tabs button.on {
    background: #3b4fa0;
    color: #fff;
    border-color: #3b6fe0;
  }
  .tax-body {
    padding: 16px 18px;
    overflow-y: auto;
    flex: 1;
  }
  .tax-desc {
    margin-bottom: 12px;
  }
  .tax-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }
  .tax-item {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #1c2030;
    border: 1px solid #2b3040;
    border-radius: 6px;
    padding: 4px 8px;
  }
  .tax-idx {
    color: #8b93a8;
    font-size: 11px;
    width: 22px;
  }
  .tax-input {
    flex: 1;
    background: transparent;
    border: 0;
    color: inherit;
    font: inherit;
    padding: 4px;
  }
  .tax-input:focus {
    background: #0e1016;
    outline: 1px solid #3b6fe0;
    border-radius: 4px;
  }
  .tax-add {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .tax-add input {
    flex: 1;
    background: #0e1016;
    border: 1px solid #353b50;
    border-radius: 6px;
    padding: 6px 10px;
    color: inherit;
    font: inherit;
  }
  .tax-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 18px;
    border-top: 1px solid #2b3040;
    background: #11141c;
    border-radius: 0 0 12px 12px;
  }
  .hint {
    color: #8b93a8;
    font-size: 12px;
    margin: 0;
  }
</style>
