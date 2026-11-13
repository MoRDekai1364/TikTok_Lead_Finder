<script lang="ts">
  import './wb.css';
  import type { Case, Suggestion } from './workbenchData';
  import { makeSource, roles, type Entity, type Source } from './workbenchModel';

  let { cur, suggestions = $bindable(), sources = $bindable(), entities = $bindable(), insertNote, log }: { cur: Case; suggestions: Suggestion[]; sources: Source[]; entities: Entity[]; insertNote: (t: string) => void; log: (s: string) => void } = $props();

  let focus = $state(-1);
  let url = $state('');
  let title = $state('');
  let role = $state(roles[2]);
  let linkTo = $state<number[]>([]);
  let entName = $state('');
  let entType = $state('лице');

  const pending = $derived(suggestions.filter((s) => s.state === 'new' && s.kind !== 'Spread'));
  const shown = $derived(sources.filter((s) => focus < 0 || s.claims.includes(focus)));
  const coverage = $derived(cur.claims.map((_, i) => sources.filter((s) => s.include && s.claims.includes(i)).length));
  const uncovered = $derived(coverage.filter((n) => n === 0).length);
  const archived = $derived(sources.filter((s) => s.archive !== 'Не е архивиран').length);

  function accept(s: Suggestion) {
    s.state = 'accepted';
    if (s.kind === 'Source') {
      const id = Math.max(0, ...sources.map((x) => x.id)) + 1;
      sources = [...sources, makeSource(cur, s.text, s.why, id)];
    } else if (s.kind === 'Person') {
      const id = Math.max(0, ...entities.map((x) => x.id)) + 1;
      const name = s.text.split(':')[0];
      entities = [...entities, { id, name, type: 'лице', summary: s.text.split(':').slice(1).join(':').trim(), found: false, checks: [{ src: s.why, result: 'Няма резултати' }], sanctions: 'Няма съвпадение' }];
    }
    log(`Прието: ${s.kind}: ${s.text.slice(0, 40)}`);
  }

  function dismiss(s: Suggestion) {
    s.state = 'dismissed';
    log(`Отхвърлено: ${s.kind}`);
  }

  function addSource() {
    if (!url.trim()) return;
    const id = Math.max(0, ...sources.map((x) => x.id)) + 1;
    sources = [...sources, { id, url: url.trim(), title: title.trim() || url.trim(), role, claims: [...linkTo], why: 'Добавен ръчно', quote: '', score: 1, include: true, archive: 'Не е архивиран' }];
    log(`Ръчно добавен източник: ${url.trim()}`);
    url = '';
    title = '';
    linkTo = [];
  }

  function toggleLink(s: Source, i: number) {
    s.claims = s.claims.includes(i) ? s.claims.filter((x) => x !== i) : [...s.claims, i].sort();
  }

  function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= sources.length) return;
    const a = [...sources];
    [a[i], a[j]] = [a[j], a[i]];
    sources = a;
  }

  function archiveNow(s: Source) {
    s.archive = `Архивиран: web.archive.org, 11.11.2026 (${1 + (s.id % 9)} снимки)`;
    log(`Архивиране: ${s.url}`);
  }

  function addEntity() {
    if (!entName.trim()) return;
    const id = Math.max(0, ...entities.map((x) => x.id)) + 1;
    entities = [...entities, { id, name: entName.trim(), type: entType, summary: '', found: false, checks: [], sanctions: 'Не е проверено' }];
    entName = '';
  }

  function recheck(e: Entity) {
    e.checks = [{ src: 'Wikidata', result: e.found ? 'Намерен запис' : 'Няма запис' }, { src: 'Търговски регистър', result: 'Няма съвпадение' }, { src: 'Новини', result: e.found ? '2 резултата' : '0 резултата' }, ...e.checks.slice(3)];
    e.sanctions = 'Няма съвпадение (0 от 6 списъка)';
    log(`Повторна проверка: ${e.name}`);
  }

  function sentence(e: Entity): string {
    return e.found ? `${e.name}: ${e.summary}` : `There is no information available about ${e.name} in the checked sources (${e.checks.map((c) => c.src).join(', ')}).`;
  }
</script>

<div class="wb">
  <div class="panel">
    <div class="row">
      <h3>Доказателства</h3>
      <span class="grow"></span>
      <span class="chip {uncovered ? 'warn' : 'ok'}">{uncovered ? `${uncovered} твърдения без източник` : 'всички твърдения имат източник'}</span>
      <span class="chip info">архивирани: {archived}/{sources.length}</span>
    </div>
    <div class="grid g3">
      {#each cur.claims as c, i}
        <button class="card" class:on={focus === i} style="text-align:left" onclick={() => (focus = focus === i ? -1 : i)}>
          <b>Т{i + 1}</b> <span class="chip {c.v === 'SUPPORTED' ? 'ok' : c.v === 'CONTRADICTED' ? 'bad' : 'warn'}">{coverage[i]} източника</span>
          <div class="hint">{c.text.slice(0, 80)}{c.text.length > 80 ? '…' : ''}</div>
        </button>
      {/each}
    </div>
  </div>

  <div class="two">
    <div>
      <div class="panel">
        <div class="row"><h3>Източници в доклада ({shown.length})</h3><span class="grow"></span>{#if focus >= 0}<button class="ghost" onclick={() => (focus = -1)}>Покажи всички</button>{/if}</div>
        {#each shown as s (s.id)}
          {@const idx = sources.findIndex((x) => x.id === s.id)}
          <div class="block" style={s.include ? '' : 'opacity:.5'}>
            <div class="row">
              <input type="checkbox" bind:checked={s.include} title="Включи в доклада" />
              <input style="flex:1" bind:value={s.title} />
              <span class="chip info">{Math.round(s.score * 100)}%</span>
              <button class="ghost" onclick={() => move(idx, -1)}>↑</button>
              <button class="ghost" onclick={() => move(idx, 1)}>↓</button>
              <button class="ghost danger" onclick={() => (sources = sources.filter((x) => x.id !== s.id))}>✕</button>
            </div>
            <div class="row"><input style="flex:1" bind:value={s.url} /><select bind:value={s.role}>{#each roles as r}<option>{r}</option>{/each}</select></div>
            <div class="hint">Защо съвпада: {s.why}</div>
            <textarea style="min-height:44px" placeholder="Цитат от източника, който подкрепя твърдението" bind:value={s.quote}></textarea>
            <div class="row"><span class="hint">Използвай за:</span>{#each cur.claims as _, i}<button class:on={s.claims.includes(i)} onclick={() => toggleLink(s, i)}>Т{i + 1}</button>{/each}</div>
            <div class="row"><span class="hint">{s.archive}</span><span class="grow"></span><button class="ghost" onclick={() => archiveNow(s)}>🗄️ Архивирай</button></div>
          </div>
        {:else}<p class="hint">Няма източници за този избор. Приеми предложение или добави ръчно.</p>{/each}

        <h4>Добави източник</h4>
        <div class="row"><input style="flex:1" placeholder="URL" bind:value={url} /><input style="flex:1" placeholder="Заглавие" bind:value={title} /></div>
        <div class="row"><select bind:value={role}>{#each roles as r}<option>{r}</option>{/each}</select><span class="hint">за:</span>{#each cur.claims as _, i}<button class:on={linkTo.includes(i)} onclick={() => (linkTo = linkTo.includes(i) ? linkTo.filter((x) => x !== i) : [...linkTo, i])}>Т{i + 1}</button>{/each}<span class="grow"></span><button class="primary" onclick={addSource}>+ Добави</button><button class="ghost">От Source Library</button></div>
      </div>

      <div class="panel">
        <h3>Предложения ({pending.length})</h3>
        {#each pending as s (s.id)}
          <div class="block">
            <span class="tag">{s.kind}</span> {s.text}
            <div class="hint">Защо: {s.why}</div>
            <div class="row"><button class="primary" onclick={() => accept(s)}>Приеми</button><button class="ghost" onclick={() => dismiss(s)}>Отхвърли</button></div>
          </div>
        {:else}<p class="hint">Няма чакащи предложения.</p>{/each}
      </div>
    </div>

    <div>
      <div class="panel">
        <h3>Лица, организации и продукти ({entities.length})</h3>
        {#each entities as e (e.id)}
          <div class="block">
            <div class="row"><input style="flex:1" bind:value={e.name} /><select bind:value={e.type}><option>лице</option><option>организация</option><option>медия</option><option>продукт</option><option>фирма</option></select><button class="ghost danger" onclick={() => (entities = entities.filter((x) => x.id !== e.id))}>✕</button></div>
            <label><input type="checkbox" bind:checked={e.found} /> намерена информация {#if !e.found}<span class="chip warn">няма информация (това също е резултат)</span>{/if}</label>
            <textarea style="min-height:50px" placeholder="Кратко описание" bind:value={e.summary}></textarea>
            <table><tbody>{#each e.checks as ck}<tr><td>{ck.src}</td><td>{ck.result}</td></tr>{/each}</tbody></table>
            <div class="hint">Санкции: {e.sanctions}</div>
            <div class="row"><button class="ghost" onclick={() => recheck(e)}>↻ Провери отново</button><button onclick={() => { insertNote(sentence(e)); log(`Вмъкнато в бележките: ${e.name}`); }}>↪ Вмъкни в бележките</button></div>
          </div>
        {/each}
        <div class="row"><input style="flex:1" placeholder="Добави име за проверка" bind:value={entName} /><select bind:value={entType}><option>лице</option><option>организация</option><option>медия</option><option>продукт</option><option>фирма</option></select><button class="primary" onclick={addEntity}>+ Провери</button></div>
      </div>

      <div class="panel">
        <h3>Покритие: твърдения × източници</h3>
        <table>
          <thead><tr><th></th>{#each sources.filter((s) => s.include) as s, k}<th title={s.title}>И{k + 1}</th>{/each}</tr></thead>
          <tbody>
            {#each cur.claims as _, i}
              <tr><td>Т{i + 1}</td>{#each sources.filter((s) => s.include) as s}<td>{s.claims.includes(i) ? '●' : '·'}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  </div>
</div>
