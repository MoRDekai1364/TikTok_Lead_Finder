<script lang="ts">
  import './wb.css';
  import type { Case } from './workbenchData';
  import { makeHit, platformsAll, queryFor, type Hit, type Query, type Repost } from './workbenchModel';

  let { cur, hits = $bindable(), reposts = $bindable(), queries = $bindable(), log }: { cur: Case; hits: Hit[]; reposts: Repost[]; queries: Query[]; log: (s: string) => void } = $props();

  let claimFilter = $state(-1);
  let platformFilter = $state('');
  let kindFilter = $state('');
  let sortBy = $state<'date' | 'sim'>('date');
  let threshold = $state(10);
  let newUrl = $state('');
  let newPlatform = $state('Facebook');
  let newKind = $state<Hit['kind']>('exact');
  let newDate = $state('2026-11-10');
  let repostUrl = $state('');
  let copied = $state('');

  const included = $derived(hits.filter((h) => h.include));
  const total = $derived(included.length);
  const perPlatform = $derived(platformsAll.map((p) => ({ p, n: included.filter((h) => h.include && h.platform === p).length, all: hits.filter((h) => h.platform === p).length })));
  const maxN = $derived(Math.max(1, ...perPlatform.map((x) => x.n)));
  const perClaim = $derived(cur.claims.map((_, i) => included.filter((h) => h.claim === i).length));
  const first = $derived([...included].sort((a, b) => a.date.localeCompare(b.date))[0]);
  const exactCount = $derived(included.filter((h) => h.kind === 'exact').length);
  const quantity = $derived(total >= threshold ? 'High' : 'Low');
  const months = $derived.by(() => {
    const m: Record<string, number> = {};
    for (const h of included) m[h.date.slice(0, 7)] = (m[h.date.slice(0, 7)] ?? 0) + 1;
    return Object.entries(m).sort((a, b) => a[0].localeCompare(b[0]));
  });
  const maxM = $derived(Math.max(1, ...months.map((x) => x[1])));
  const shown = $derived(
    hits
      .filter((h) => (claimFilter < 0 || h.claim === claimFilter) && (!platformFilter || h.platform === platformFilter) && (!kindFilter || h.kind === kindFilter))
      .sort((a, b) => (sortBy === 'date' ? b.date.localeCompare(a.date) : b.sim - a.sim))
  );
  const repostsIncluded = $derived(reposts.filter((r) => r.include).length);

  const kindLabel = { exact: 'точно', near: 'почти точно', paraphrase: 'перифраза' };
  const kindClass = { exact: 'ok', near: 'info', paraphrase: 'warn' };

  function rescan(p: string) {
    const nextId = Math.max(0, ...hits.map((h) => h.id)) + 1;
    const add = 1 + (nextId % 3);
    const fresh = Array.from({ length: add }, (_, i) => makeHit(cur, p, typeof cur.id === 'number' ? cur.id + nextId : `${cur.id}_${nextId}`, nextId + i, claimFilter < 0 ? undefined : claimFilter));
    hits = [...hits, ...fresh];
    log(`Сканиране: ${p} (+${add})`);
  }

  function addHit() {
    if (!newUrl.trim()) return;
    const nextId = Math.max(0, ...hits.map((h) => h.id)) + 1;
    hits = [...hits, { id: nextId, platform: newPlatform, url: newUrl.trim(), author: 'ръчно добавен', date: newDate, kind: newKind, sim: newKind === 'exact' ? 1 : 0.9, snippet: cur.claims[claimFilter < 0 ? 0 : claimFilter].text, include: true, claim: claimFilter < 0 ? 0 : claimFilter }];
    log(`Ръчно добавена публикация: ${newPlatform}`);
    newUrl = '';
  }

  function findReposts() {
    const nextId = Math.max(0, ...reposts.map((r) => r.id)) + 1;
    const methods = ['Същ звук', 'Същ надпис', 'Визуален отпечатък', 'Stitch / Duet'];
    reposts = [...reposts, { id: nextId, url: `https://www.tiktok.com/@repost_${nextId}/video/${7300000000000000000 + nextId * 7}`, author: `@repost_${nextId}`, date: '2026-11-09', method: methods[nextId % 4], conf: 0.7 + (nextId % 3) / 10, include: true }];
    log('Търсене на препубликации (+1)');
  }

  function addRepost() {
    if (!repostUrl.trim()) return;
    const nextId = Math.max(0, ...reposts.map((r) => r.id)) + 1;
    reposts = [...reposts, { id: nextId, url: repostUrl.trim(), author: 'ръчно добавен', date: '2026-11-10', method: 'Ръчно', conf: 1, include: true }];
    repostUrl = '';
  }

  function copy(text: string, label: string) {
    navigator.clipboard?.writeText(text);
    copied = label;
    setTimeout(() => (copied = ''), 1500);
  }

  function resetQuery(q: Query) {
    q.q = queryFor(cur, q.platform);
  }
</script>

<div class="wb">
  <div class="panel">
    <div class="row">
      <h3>Разпространение</h3>
      <span class="grow"></span>
      <button onclick={() => platformsAll.filter((p) => queries.find((q) => q.platform === p)?.on).forEach(rescan)}>🔍 Сканирай всички платформи</button>
      <button onclick={findReposts}>🔁 Намери препубликации</button>
      <button onclick={() => copy(included.map((h) => h.url).join('\n'), 'examples')}>{copied === 'examples' ? '✓ Копирано' : '📋 Копирай Examples'}</button>
    </div>
    <div class="grid g3">
      <div class="card"><div class="big">{total}</div><div class="hint">общо намерени (долна граница)</div><div class="hint">точни: {exactCount} • препубликации в TikTok: {repostsIncluded}</div></div>
      <div class="card">
        <div class="hint">Количество на платформата</div>
        <div class="big">{quantity === 'High' ? 'Високо' : 'Ниско'}</div>
        <label class="hint">Праг за „Високо“: <input type="number" min="1" style="width:56px" bind:value={threshold} /></label>
      </div>
      <div class="card"><div class="hint">Първо появяване</div><div class="big" style="font-size:20px">{first?.date ?? '—'}</div><div class="hint">{first ? `${first.platform} • ${first.author}` : 'няма данни'}</div></div>
    </div>
  </div>

  <div class="two">
    <div class="panel">
      <h3>По платформи</h3>
      {#each perPlatform as x}
        {@const q = queries.find((qq) => qq.platform === x.p)}
        <div class="bar">
          <label style="width:110px"><input type="checkbox" checked={q?.on} onchange={(e) => q && (q.on = (e.target as HTMLInputElement).checked)} /> {x.p}</label>
          <i style="width:{(x.n / maxN) * 160}px"></i>
          <b>{x.n}</b><span class="hint" style="width:auto">/ {x.all}</span>
          <span class="grow"></span>
          <button class="ghost" onclick={() => rescan(x.p)}>Сканирай</button>
        </div>
      {/each}
      <p class="hint">Платформите без публичен достъп са долна граница. Добави източник от „Source Library“.</p>
    </div>
    <div class="panel">
      <h3>Във времето</h3>
      <div class="tl">{#each months as [m, n]}<div style="height:{(n / maxM) * 100}%" title="{m}: {n}"><small>{m.slice(2)}</small></div>{/each}</div>
      <div style="height:18px"></div>
      <h4>По твърдения</h4>
      {#each cur.claims as c, i}<div class="bar"><span style="width:34px">Т{i + 1}</span><i style="width:{(perClaim[i] / Math.max(1, ...perClaim)) * 160}px"></i><b>{perClaim[i]}</b></div>{/each}
    </div>
  </div>

  <div class="panel">
    <div class="row">
      <h3>Публикации ({shown.length})</h3>
      <span class="grow"></span>
      <select bind:value={claimFilter}><option value={-1}>Всички твърдения</option>{#each cur.claims as _, i}<option value={i}>Т{i + 1}</option>{/each}</select>
      <select bind:value={platformFilter}><option value="">Всички платформи</option>{#each platformsAll as p}<option>{p}</option>{/each}</select>
      <select bind:value={kindFilter}><option value="">Всяко съвпадение</option><option value="exact">точно</option><option value="near">почти точно</option><option value="paraphrase">перифраза</option></select>
      <select bind:value={sortBy}><option value="date">По дата</option><option value="sim">По сходство</option></select>
    </div>
    <table>
      <thead><tr><th></th><th>Платформа</th><th>Автор</th><th>Дата</th><th>Съвпадение</th><th>Т</th><th>Откъс</th><th></th></tr></thead>
      <tbody>
        {#each shown as h (h.id)}
          <tr style={h.include ? '' : 'opacity:.45'}>
            <td><input type="checkbox" bind:checked={h.include} /></td>
            <td>{h.platform}</td><td>{h.author}</td><td>{h.date}</td>
            <td><span class="chip {kindClass[h.kind]}">{kindLabel[h.kind]} {Math.round(h.sim * 100)}%</span></td>
            <td>Т{h.claim + 1}</td>
            <td>{h.snippet}<div><a href={h.url} target="_blank" rel="noreferrer">{h.url}</a></div></td>
            <td><button class="ghost danger" onclick={() => (hits = hits.filter((x) => x.id !== h.id))}>✕</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="row">
      <input style="flex:1;min-width:260px" placeholder="Добави публикация ръчно (URL)" bind:value={newUrl} />
      <select bind:value={newPlatform}>{#each platformsAll as p}<option>{p}</option>{/each}</select>
      <select bind:value={newKind}><option value="exact">точно</option><option value="near">почти точно</option><option value="paraphrase">перифраза</option></select>
      <input type="date" bind:value={newDate} />
      <button class="primary" onclick={addHit}>+ Добави</button>
    </div>
  </div>

  <div class="panel">
    <div class="row"><h3>Препубликации в TikTok ({repostsIncluded})</h3><span class="grow"></span><button onclick={findReposts}>🔁 Търси още</button></div>
    <p class="hint">TikTok не показва препубликациите публично. Те се откриват по: същ звук, същ надпис/хаштагове, визуален отпечатък на кадрите, Stitch/Duet и ръчно добавени връзки. Всеки резултат има метод и достоверност.</p>
    <table>
      <thead><tr><th></th><th>Връзка</th><th>Автор</th><th>Дата</th><th>Метод</th><th>Достоверност</th><th></th></tr></thead>
      <tbody>
        {#each reposts as r (r.id)}
          <tr style={r.include ? '' : 'opacity:.45'}>
            <td><input type="checkbox" bind:checked={r.include} /></td>
            <td><a href={r.url} target="_blank" rel="noreferrer">{r.url}</a></td><td>{r.author}</td><td>{r.date}</td>
            <td><span class="tag">{r.method}</span></td>
            <td><span class="chip {r.conf >= 0.85 ? 'ok' : r.conf >= 0.7 ? 'info' : 'warn'}">{Math.round(r.conf * 100)}%</span></td>
            <td><button class="ghost danger" onclick={() => (reposts = reposts.filter((x) => x.id !== r.id))}>✕</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="row"><input style="flex:1" placeholder="Добави препубликация ръчно (TikTok URL)" bind:value={repostUrl} /><button class="primary" onclick={addRepost}>+ Добави</button></div>
  </div>

  <div class="panel">
    <h3>Булеви заявки</h3>
    <p class="hint">Редактирай заявките за всяка платформа. Те се записват в доклада (Boolean queries).</p>
    {#each queries as q (q.platform)}
      <div class="listrow">
        <label style="width:110px"><input type="checkbox" bind:checked={q.on} /> {q.platform}</label>
        <input bind:value={q.q} />
        <button class="ghost" title="Генерирай отново" onclick={() => resetQuery(q)}>↻</button>
        <button class="ghost" onclick={() => copy(q.q, q.platform)}>{copied === q.platform ? '✓' : '📋'}</button>
        <button class="ghost">↗</button>
      </div>
    {/each}
  </div>
</div>
