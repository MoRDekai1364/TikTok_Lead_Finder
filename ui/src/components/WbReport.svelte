<script lang="ts">
  import './wb.css';
  import type { Case } from './workbenchData';
  import { buildReport, defaultTaxonomy, formatDocText, type Block, type Hit, type Query, type ReportSet, type Source, type Taxonomy } from './workbenchModel';
  import WbTaxonomyModal from './WbTaxonomyModal.svelte';

  let { cur, report = $bindable(), taxonomy = $bindable(defaultTaxonomy), hits, sources, queries, assist, log }: { cur: Case; report: ReportSet; taxonomy?: Taxonomy; hits: Hit[]; sources: Source[]; queries: Query[]; assist: string; log: (s: string) => void } = $props();

  type Tpl = 'TCS' | 'TT' | 'FC';
  let tpl = $state<Tpl>('TCS');
  let newLabel = $state('');
  let newKind = $state<Block['kind']>('text');
  let copied = $state(false);
  let taxModalOpen = $state(false);
  let previewMode = $state<'doc' | 'text'>('doc');

  function syncTaxOptions() {
    for (const t of ['TCS', 'TT', 'FC'] as const) {
      for (const b of report[t]) {
        if (b.key === 'rating') b.options = [...taxonomy.ratings];
        if (b.key === 'topic') b.options = [...taxonomy.topics];
        if (b.key === 'kind') b.options = [...taxonomy.claimKinds];
        if (b.key === 'harm') b.options = [...taxonomy.harms];
      }
    }
  }

  $effect(() => {
    syncTaxOptions();
  });

  const blocks = $derived(report[tpl]);

  const exportLabel: Record<Tpl, string> = { TCS: 'Преглед на твърдение (TCS)', TT: 'Доклад за тенденция (TT)', FC: 'Фактчек статия (FC)' };

  function fresh(key: string): Block | undefined {
    return buildReport(cur, hits, sources, queries)[tpl].find((b) => b.key === key);
  }

  function sync(b: Block) {
    const f = fresh(b.key);
    if (!f) return;
    b.list = [...f.list];
    b.value = b.kind === 'list' ? '' : f.value;
    log(`Синхронизирано: ${b.label}`);
  }

  const syncable = (b: Block) => ['examples', 'sources', 'falsif', 'qty', 'bool', 'claim', 'url'].includes(b.key);

  function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= blocks.length) return;
    const a = [...blocks];
    [a[i], a[j]] = [a[j], a[i]];
    report[tpl] = a;
  }

  function remove(id: number) {
    report[tpl] = blocks.filter((b) => b.id !== id);
    log('Премахнато поле от доклада');
  }

  function addBlock() {
    if (!newLabel.trim()) return;
    const id = Math.max(0, ...blocks.map((b) => b.id)) + 1;
    report[tpl] = [...blocks, { id, key: `custom${id}`, label: newLabel.trim(), kind: newKind, value: '', list: [], options: newKind === 'select' ? ['Да', 'Не'] : [] }];
    newLabel = '';
    log(`Ново поле: ${newLabel}`);
  }

  function draft() {
    const en = report.TCS.find((b) => b.key === 'notesEn');
    const bg = report.TCS.find((b) => b.key === 'notesBg');
    if (en) en.value = cur.notesEn;
    if (bg) bg.value = cur.notesBg;
    const lead = report.FC.find((b) => b.key === 'lead');
    if (lead) lead.value = `Is there evidence? No, that's not true: ${cur.notesEn.split('. ')[0]}.`;
    log('Чернова на бележките (EN+BG)');
  }

  function resetTpl() {
    report[tpl] = buildReport(cur, hits, sources, queries)[tpl];
    log(`Върнат оригиналният образец ${tpl}`);
  }

  const text = $derived(formatDocText(tpl, blocks));
  const bMap = $derived(Object.fromEntries(blocks.map((b) => [b.key, b])));
  const standardKeys = new Set(['date', 'task', 'rating', 'topic', 'kind', 'claim', 'notesEn', 'notesBg', 'sources', 'headEn', 'headBg', 'region', 'harm', 'examples', 'falsif', 'qty', 'keywords', 'false', 'bool', 'explanation', 'url', 'caption', 'headline', 'lead', 'appeared', 'newclaim']);
  const customBlocks = $derived(blocks.filter((b) => !standardKeys.has(b.key)));

  function copy() {
    navigator.clipboard?.writeText(text);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function downloadDocx() {
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `${cur.title.replace(/[^a-zA-Z0-9а-яА-Я_-]/g, '_')}_${tpl}.txt`;
    a.click();
    URL.revokeObjectURL(a.href);
  }
</script>

<div class="wb">
  <div class="panel">
    <div class="row">
      <h3>Доклад</h3>
      <span class="grow"></span>
      {#each ['TCS', 'TT', 'FC'] as t}<button class:on={tpl === t} onclick={() => (tpl = t as Tpl)}>{t}</button>{/each}
      <button onclick={() => (taxModalOpen = true)} title="Управление на теми, оценки и категории">🏷️ Категории и теми</button>
      {#if assist === 'Draft'}<button class="primary" onclick={draft}>✨ Чернова EN+BG</button>{/if}
    </div>
    <div class="hint">{exportLabel[tpl]} • всяко поле се редактира, преименува, премества и премахва; може да добавяш свои полета.</div>
  </div>

  <div class="two">
    <div>
      {#each blocks as b, i (b.id)}
        <div class="block">
          <div class="row">
            <input class="lbl" bind:value={b.label} />
            <span class="grow"></span>
            {#if syncable(b)}<button class="ghost" title="Обнови от Разпространение/Доказателства" onclick={() => sync(b)}>↻ от случая</button>{/if}
            <button class="ghost" onclick={() => move(i, -1)}>↑</button>
            <button class="ghost" onclick={() => move(i, 1)}>↓</button>
            <button class="ghost danger" onclick={() => remove(b.id)}>✕</button>
          </div>
          {#if b.kind === 'text'}
            <input style="width:100%;box-sizing:border-box" bind:value={b.value} />
          {:else if b.kind === 'long'}
            <textarea bind:value={b.value}></textarea>
          {:else if b.kind === 'select'}
            <div class="row">
              <select style="flex:1" bind:value={b.value}>{#each b.options as o}<option>{o}</option>{/each}</select>
              <button class="ghost" onclick={() => (taxModalOpen = true)} title="Управление на теми и категории">🏷️ Опции</button>
            </div>
          {:else}
            {#each b.list as _, k}
              <div class="listrow">
                <input bind:value={b.list[k]} />
                <button class="ghost" onclick={() => { if (k > 0) { const a = [...b.list]; [a[k], a[k - 1]] = [a[k - 1], a[k]]; b.list = a; } }}>↑</button>
                <button class="ghost danger" onclick={() => (b.list = b.list.filter((_, x) => x !== k))}>✕</button>
              </div>
            {/each}
            <button class="ghost" onclick={() => (b.list = [...b.list, ''])}>+ ред</button>
          {/if}
        </div>
      {/each}

      <div class="panel">
        <h4>Ново поле</h4>
        <div class="row">
          <input style="flex:1" placeholder="Име на полето" bind:value={newLabel} />
          <select bind:value={newKind}><option value="text">Кратък текст</option><option value="long">Дълъг текст</option><option value="select">Избор (Да/Не)</option><option value="list">Списък</option></select>
          <button class="primary" onclick={addBlock}>+ Добави</button>
        </div>
        <div class="row"><button class="ghost" onclick={resetTpl}>Върни образеца по подразбиране</button><button class="ghost">Запази оформлението като пресет</button></div>
      </div>
    </div>

    <div>
      <div class="panel sticky">
        <div class="row">
          <h3>Преглед</h3>
          <span class="grow"></span>
          <button class:on={previewMode === 'doc'} onclick={() => (previewMode = 'doc')} title="Преглед като Word документ (.docx)">📄 Word (.docx)</button>
          <button class:on={previewMode === 'text'} onclick={() => (previewMode = 'text')} title="Преглед като чист текст">📝 Текст</button>
          <button class="primary" onclick={copy}>{copied ? '✓ Копирано' : '📋 Копирай'}</button>
          <button onclick={downloadDocx} title="Изтегли текстов документ">.docx / .txt</button>
        </div>

        {#if previewMode === 'doc'}
          {#if tpl === 'TCS'}
            <div class="word-doc-page">
              <div class="word-meta">{bMap['date']?.value || cur.date} </div>
              <div class="word-meta"><a href={bMap['task']?.value || cur.taskUrl} target="_blank" rel="noreferrer">{bMap['task']?.value || cur.taskUrl}</a></div>
              <div class="word-rating">{bMap['rating']?.value || cur.rating} – {bMap['topic']?.value || cur.topic} – {bMap['kind']?.value || cur.kind}</div>
              
              <div class="word-head">CLAIM</div>
              <div class="word-p">{bMap['claim']?.value || ''}</div>
              
              <div class="word-head">NOTES</div>
              {#if bMap['notesEn']?.value}
                <div class="word-p">{bMap['notesEn']?.value}</div>
              {/if}
              {#if bMap['notesBg']?.value}
                <div class="word-p">{bMap['notesBg']?.value}</div>
              {/if}
              
              <div class="word-link-list">
                {#each (bMap['sources']?.list || []).filter(Boolean) as src}
                  <div><a href={src} target="_blank" rel="noreferrer">{src}</a></div>
                {/each}
              </div>

              {#if customBlocks.length > 0}
                <div style="margin-top: 18px; border-top: 1px dashed #d0d7de; padding-top: 12px;">
                  {#each customBlocks as cb}
                    <div class="word-head">{cb.label}</div>
                    <div class="word-p">{cb.kind === 'list' ? cb.list.join('\n') : cb.value}</div>
                  {/each}
                </div>
              {/if}
            </div>
          {:else if tpl === 'TT'}
            <div class="word-doc-page">
              <div class="word-field"><span class="word-field-key">1. Headline:</span> {bMap['headEn']?.value || ''}</div>
              {#if bMap['headBg']?.value && bMap['headBg']?.value !== bMap['headEn']?.value}
                <div class="word-p">{bMap['headBg']?.value}</div>
              {/if}
              <div class="word-field"><span class="word-field-key">2. Region/Country:</span> {bMap['region']?.value || 'Bulgaria'}</div>
              <div class="word-field"><span class="word-field-key">3. Harm (Low/Medium/High):</span> {bMap['harm']?.value || 'Low'}</div>
              <div class="word-field">
                <span class="word-field-key">4. Examples:</span>
                <div class="word-sub-list">
                  {#each (bMap['examples']?.list || []).filter(Boolean) as ex}
                    <div><a href={ex} target="_blank" rel="noreferrer">{ex}</a></div>
                  {/each}
                </div>
              </div>
              <div class="word-field" style="margin-top: 12px;">
                <span class="word-field-key">5. Link to Falsification Source Debunking Claim:</span>
                <div class="word-sub-list">
                  {#each (bMap['falsif']?.list || []).filter(Boolean) as fl}
                    <div><a href={fl} target="_blank" rel="noreferrer">{fl}</a></div>
                  {/each}
                </div>
              </div>
              <div class="word-field"><span class="word-field-key">6.Quantity on Platform (High or Low):</span> {bMap['qty']?.value || 'Low'}</div>
              <div class="word-field"><span class="word-field-key">7.Keywords/Hashtags:</span> {bMap['keywords']?.value || ''}</div>
              <div class="word-field"><span class="word-field-key">8. Verifiably False?</span> {bMap['false']?.value || 'No'}</div>
              <div class="word-field"><span class="word-field-key">9. Boolean Queries used to find the trend:</span> {bMap['bool']?.value || ''}</div>
              <div class="word-field" style="margin-top: 12px;">
                <span class="word-field-key">Please include short explanation:</span>
                <div class="word-p" style="margin-top: 4px;">{bMap['explanation']?.value || ''}</div>
              </div>

              {#if customBlocks.length > 0}
                <div style="margin-top: 18px; border-top: 1px dashed #d0d7de; padding-top: 12px;">
                  {#each customBlocks as cb}
                    <div class="word-field"><span class="word-field-key">{cb.label}:</span> {cb.kind === 'list' ? cb.list.join('\n') : cb.value}</div>
                  {/each}
                </div>
              {/if}
            </div>
          {:else}
            <div class="word-doc-page">
              <div class="word-field"><span class="word-field-key">Headline:</span><br />{bMap['headline']?.value || ''}</div>
              <div class="word-field"><span class="word-field-key">Caption:</span> {bMap['caption']?.value || ''}</div>
              <div class="word-p" style="margin-top: 8px;">{bMap['lead']?.value || ''}</div>
              <div class="word-p">{bMap['appeared']?.value || ''}</div>
              <div class="word-field"><span class="word-field-key">1. New Claim or Trend:</span> {bMap['newclaim']?.value || ''}</div>
              <div class="word-field"><span class="word-field-key">2. Region/Country:</span> {bMap['region']?.value || 'Bulgaria'}</div>
              <div class="word-field"><span class="word-field-key">3. Harm (Low/Medium/High):</span> {bMap['harm']?.value || 'Low'}</div>
              <div class="word-field">
                <span class="word-field-key">4. Examples:</span>
                <div class="word-sub-list">
                  {#each (bMap['examples']?.list || []).filter(Boolean) as ex}
                    <div><a href={ex} target="_blank" rel="noreferrer">{ex}</a></div>
                  {/each}
                </div>
              </div>
              <div class="word-field" style="margin-top: 12px;">
                <span class="word-field-key">5. Link to Falsification Source Debunking Claim:</span>
                <div class="word-sub-list">
                  {#each (bMap['falsif']?.list || []).filter(Boolean) as fl}
                    <div><a href={fl} target="_blank" rel="noreferrer">{fl}</a></div>
                  {/each}
                </div>
              </div>
              <div class="word-field"><span class="word-field-key">6.Quantity on Platform (High or Low):</span> {bMap['qty']?.value || 'Low'}</div>
              <div class="word-field"><span class="word-field-key">7.Keywords/Hashtags:</span> {bMap['keywords']?.value || ''}</div>
              <div class="word-field"><span class="word-field-key">8. Verifiably False?</span> {bMap['false']?.value || 'No'}</div>
              <div class="word-field"><span class="word-field-key">9. Boolean Queries used to find the trend:</span> {bMap['bool']?.value || ''}</div>
              <div class="word-field" style="margin-top: 12px;">
                <span class="word-field-key">Please include short explanation:</span>
                <div class="word-p" style="margin-top: 4px;">{bMap['explanation']?.value || ''}</div>
              </div>
            </div>
          {/if}
        {:else}
          <div class="preview">{text}</div>
        {/if}
      </div>
    </div>
  </div>

  <WbTaxonomyModal bind:taxonomy bind:open={taxModalOpen} onchange={syncTaxOptions} />
</div>
