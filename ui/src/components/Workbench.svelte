<script lang="ts">
  import { onMount } from 'svelte';
  import { cases, type Verdict, type Suggestion, type Check, type Claim, type Segment, type Case } from './workbenchData';
  import {
    buildHits,
    buildQueries,
    buildReport,
    buildReposts,
    defaultTaxonomy,
    entityMap,
    makeSource,
    type Entity,
    type Hit,
    type Query,
    type ReportSet,
    type Repost,
    type Source,
    type Taxonomy
  } from './workbenchModel';
  import {
    fetchVideos,
    fetchVideoDetail,
    fetchPeople,
    fetchCreators,
    pasteLinks,
    fetchQueueStatus,
    initDesktopConnection,
    subscribeSse,
    getCoreUrl,
    openExternalUrl
  } from '../lib/api';
  import type { LeadCard, Person, Creator, VideoDetail } from '../lib/types';
  import WbSpread from './WbSpread.svelte';
  import WbEvidence from './WbEvidence.svelte';
  import WbReport from './WbReport.svelte';
  import WbTaxonomyModal from './WbTaxonomyModal.svelte';

  let realCards = $state<LeadCard[]>([]);
  let realPeople = $state<Person[]>([]);
  let realCreators = $state<Creator[]>([]);
  let currentDetail = $state<VideoDetail | null>(null);
  let currentCase = $state<Case>(cases[0]);
  let runningTasksCount = $state(0);
  let pasteModalOpen = $state(false);
  let pasteUrls = $state('');

  let searchQuery = $state('');
  let selectedLane = $state('all');
  let selectedRating = $state('All ratings');

  function parseStatementChecks(reasoning: string, defaultVerdict: Verdict): Check[] {
    if (!reasoning) return [{ v: defaultVerdict, s: 'Оценка', c: 'Няма допълнителни мотиви' }];
    const lines = reasoning.split('\n').map((l) => l.trim()).filter(Boolean);
    const checks: Check[] = [];
    for (const line of lines) {
      const match = line.match(/^\[(ПОТВЪРДЕНО|ОПРОВЕРГАНО|НЕПРОВЕРИМО)\]\s*(.*)$/);
      if (match) {
        const cv: Verdict = match[1] === 'ПОТВЪРДЕНО' ? 'SUPPORTED' : match[1] === 'ОПРОВЕРГАНО' ? 'CONTRADICTED' : 'UNVERIFIABLE';
        const body = match[2].trim();
        const sepIdx = body.indexOf('::');
        if (sepIdx !== -1) {
          checks.push({
            v: cv,
            s: body.slice(0, sepIdx).trim(),
            c: body.slice(sepIdx + 2).trim()
          });
        } else {
          checks.push({
            v: cv,
            s: body,
            c: body
          });
        }
      }
    }
    if (checks.length === 0) {
      checks.push({ v: defaultVerdict, s: 'Оценка на твърдението', c: reasoning });
    }
    return checks;
  }

  function parseReasoningSummary(reasoning: string): string {
    if (!reasoning) return '';
    const lines = reasoning.split('\n').map((l) => l.trim()).filter(Boolean);
    for (const line of lines) {
      const match = line.match(/^\[ОБОБЩЕНИЕ\]\s*(.*)$/);
      if (match) return match[1].trim();
    }
    return '';
  }

  function videoDetailToCase(detail: VideoDetail, tax: Taxonomy): Case {
    const v = detail.video;
    const topClaim = detail.claims[0]?.claim;
    const topVerdict: Verdict =
      topClaim?.verdict === 'SUPPORTED'
        ? 'SUPPORTED'
        : topClaim?.verdict === 'CONTRADICTED'
        ? 'CONTRADICTED'
        : 'UNVERIFIABLE';

    const mappedRating =
      topVerdict === 'SUPPORTED'
        ? 'True'
        : topVerdict === 'CONTRADICTED'
        ? 'False'
        : 'Unsubstantiated';

    const durationSec = Math.max(1, Math.round(v.duration_s || 60));

    const ocrList = (detail.ocr_texts || []).map((o) => {
      const sec = Math.floor(o.frame_ms / 1000);
      const m = Math.floor(sec / 60);
      const sc = sec % 60;
      return {
        t: `${m}:${String(sc).padStart(2, '0')}`,
        text: o.text
      };
    });

    const claimList: Claim[] = (detail.claims || []).map((cwith) => {
      const c = cwith.claim;
      const cv: Verdict =
        c.verdict === 'SUPPORTED'
          ? 'SUPPORTED'
          : c.verdict === 'CONTRADICTED'
          ? 'CONTRADICTED'
          : 'UNVERIFIABLE';
      const fromSec = Math.floor((c.start_ms || 0) / 1000);
      const toSec = Math.max(fromSec + 2, Math.floor((c.end_ms || (c.start_ms + 10000)) / 1000));
      const rawReason = c.reasoning_bg || '';
      const checks = parseStatementChecks(rawReason, cv);
      const summary = parseReasoningSummary(rawReason) || rawReason.slice(0, 160) || 'Проверка на твърдението';
      const evList = (cwith.evidence || []).map((e) => e.url || e.source_name || '').filter(Boolean);
      return {
        type: c.claim_type || tax.claimTypes[0] || 'твърдение',
        at: `${fromSec}s - ${toSec}s`,
        v: cv,
        text: c.text_bg,
        summary,
        evidence: evList,
        checks
      };
    });

    let segmentList: Segment[] = [];
    if (detail.transcript?.segments && detail.transcript.segments.length > 0) {
      segmentList = detail.transcript.segments.map((s) => {
        const sec = Math.floor(s.start_ms / 1000);
        const m = Math.floor(sec / 60);
        const sc = sec % 60;
        const matchedIdx = detail.claims.findIndex((c) => {
          const cs = (c.claim.start_ms || 0) / 1000;
          const ce = (c.claim.end_ms || cs + 10) / 1000;
          return sec >= cs && sec <= ce;
        });
        return {
          t: `${m}:${String(sc).padStart(2, '0')}`,
          sec,
          text: s.text,
          claim: matchedIdx >= 0 ? matchedIdx : null
        };
      });
    } else if (detail.transcript?.text) {
      const parts = detail.transcript.text.split(/(?<=[.!?])\s+/).filter(Boolean);
      const stepSec = Math.max(3, Math.floor(durationSec / Math.max(1, parts.length)));
      segmentList = parts.map((pt, idx) => {
        const sec = Math.min(durationSec, idx * stepSec);
        const m = Math.floor(sec / 60);
        const sc = sec % 60;
        return {
          t: `${m}:${String(sc).padStart(2, '0')}`,
          sec,
          text: pt,
          claim: claimList.length > 0 ? (idx < claimList.length ? idx : null) : null
        };
      });
    }

    const suggestions: Suggestion[] = [];
    let sId = 1;
    for (const cwith of detail.claims || []) {
      for (const ev of cwith.evidence || []) {
        if (ev.url) {
          suggestions.push({
            id: sId++,
            kind: 'Source',
            text: `${ev.source_name || 'Източник'}: ${ev.snippet || ev.title || ev.url}`,
            why: 'Свързано доказателство за твърдение',
            state: 'new'
          });
        }
      }
    }
    for (const p of detail.people || []) {
      suggestions.push({
        id: sId++,
        kind: 'Person',
        text: `${p.canonical_name}: ${p.role || p.role_hint || p.summary || 'Лице от видеото'}`,
        why: 'Открито публично лице / субект',
        state: 'new'
      });
    }

    const notesBg = claimList.map((c, i) => `Т${i + 1} (${c.type}): ${c.text}\nОценка: ${c.v === 'SUPPORTED' ? 'Потвърдено' : c.v === 'CONTRADICTED' ? 'Опровергано' : 'Непроверимо'}\n${c.summary}`).join('\n\n');
    const notesEn = claimList.map((c, i) => `Claim ${i + 1} (${c.type}): ${c.text}\nVerdict: ${c.v}\n${c.summary}`).join('\n\n');

    const title = v.caption ? (v.caption.slice(0, 80) + (v.caption.length > 80 ? '…' : '')) : (topClaim?.text_bg || `Видео ${v.id}`);

    return {
      id: v.id,
      date: v.posted_at ? new Date(v.posted_at).toLocaleDateString('bg-BG') : 'Днес',
      taskUrl: v.url || `https://www.tiktok.com/@${v.creator_handle || 'user'}/video/${v.id}`,
      title,
      creator: v.creator_handle ? (v.creator_handle.startsWith('@') ? v.creator_handle : '@' + v.creator_handle) : '@видео',
      views: v.views ? Number(v.views).toLocaleString('bg-BG') : '0',
      duration: durationSec,
      status: v.state || v.lane || 'В процес',
      rating: mappedRating,
      topic: tax.topics[0] || 'Политика',
      kind: tax.claimKinds[0] || 'Known Claim',
      harm: 'Medium',
      total: Math.max(1, (v.shares || 0) + ((v.likes || 0) > 100 ? 5 : 1)),
      platforms: {
        TikTok: 1,
        Facebook: detail.claims.some((c) => c.evidence.some((e) => e.url?.includes('facebook.com'))) ? 2 : 1,
        Telegram: detail.claims.some((c) => c.evidence.some((e) => e.url?.includes('t.me'))) ? 1 : 0,
        X: 0
      },
      ocr: ocrList,
      segments: segmentList,
      claims: claimList,
      suggestions,
      notesEn,
      notesBg,
      mediaPath: v.media_path,
      videoUrl: v.url
    };
  }

  async function loadAllData() {
    try {
      const [cards, ppl, creats, q] = await Promise.all([
        fetchVideos('all', 2000).catch(() => fetchVideos('leads', 2000)),
        fetchPeople().catch(() => []),
        fetchCreators().catch(() => []),
        fetchQueueStatus().catch(() => null)
      ]);
      if (cards && cards.length > 0) realCards = cards;
      if (ppl && ppl.length > 0) realPeople = ppl;
      if (creats && creats.length > 0) realCreators = creats;
      if (q) runningTasksCount = (q.running || 0) + (q.queued || 0);

      if (cards && cards.length > 0) {
        openRealVideo(cards[0].id, false);
      }
    } catch {}
  }

  onMount(() => {
    initDesktopConnection().then(() => {
      loadAllData();
      subscribeSse((event: any) => {
        if (event && (event.type === 'queue_update' || event.type === 'job_progress')) {
          fetchQueueStatus().then((q) => {
            if (q) runningTasksCount = (q.running || 0) + (q.queued || 0);
          }).catch(() => {});
        }
      });
    }).catch(() => {
      loadAllData();
    });
  });

  let sourceCatalog = $state([
    { name: 'Facebook (site search)', type: 'platform', on: true },
    { name: 'Telegram t.me/s', type: 'platform', on: true },
    { name: 'Bluesky API', type: 'platform', on: true },
    { name: 'Търговски регистър', type: 'registry', on: true },
    { name: 'NASA / WMO / Copernicus', type: 'official', on: true },
    { name: 'factcheck.bg', type: 'fact-check', on: true }
  ]);

  let view = $state<'library' | 'case' | 'sources'>('library');
  let libTab = $state<'Videos' | 'People & Orgs' | 'Creators' | 'Sources' | 'Категории'>('Videos');
  let step = $state(0);
  let assist = $state<'Quiet' | 'Suggest' | 'Draft'>('Suggest');
  let playhead = $state(0);
  let playing = $state(false);
  let videoPaused = $state(true);
  let videoElement = $state<HTMLVideoElement | null>(null);
  let wizard = $state(0);
  let wizardUrl = $state('');
  let history = $state<string[]>(['Системата е готова']);

  let leftColWidth = $state(
    typeof localStorage !== 'undefined'
      ? Math.max(180, Math.min(520, Number(localStorage.getItem('leadfinder_left_col_w') || 240)))
      : 240
  );
  let isResizing = $state(false);

  function startResize(e: MouseEvent) {
    e.preventDefault();
    isResizing = true;
    const startX = e.clientX;
    const startW = leftColWidth;

    const onMouseMove = (ev: MouseEvent) => {
      const delta = ev.clientX - startX;
      leftColWidth = Math.max(180, Math.min(520, startW + delta));
    };

    const onMouseUp = () => {
      isResizing = false;
      localStorage.setItem('leadfinder_left_col_w', String(leftColWidth));
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }

  let suggestions = $state<Suggestion[]>(cases[0].suggestions);
  let hits = $state<Hit[]>(buildHits(cases[0]));
  let reposts = $state<Repost[]>(buildReposts(cases[0]));
  let queries = $state<Query[]>(buildQueries(cases[0]));
  let sources = $state<Source[]>(cases[0].suggestions.filter((x) => x.kind === 'Source').slice(0, 2).map((x, k) => makeSource(cases[0], x.text, x.why, k + 1)));
  let taxonomy = $state<Taxonomy>(structuredClone(defaultTaxonomy));
  let taxModalOpen = $state(false);
  let entities = $state<Entity[]>(structuredClone(entityMap[1] ?? []));
  let report = $state<ReportSet>(buildReport(cases[0], buildHits(cases[0]), sources, buildQueries(cases[0]), defaultTaxonomy));

  const cur = $derived(currentCase);
  const claims = $derived(cur.claims);
  const segments = $derived(cur.segments);

  const filteredVideos = $derived(
    realCards.filter((c) => {
      if (selectedLane !== 'all' && c.lane !== selectedLane) return false;
      if (selectedRating !== 'All ratings') {
        if (selectedRating === 'Supported' && c.top_claim_verdict !== 'SUPPORTED') return false;
        if (selectedRating === 'Contradicted' && c.top_claim_verdict !== 'CONTRADICTED') return false;
        if (selectedRating === 'Unverifiable' && c.top_claim_verdict !== 'UNVERIFIABLE') return false;
      }
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase();
        const matchHandle = c.creator_handle?.toLowerCase().includes(q);
        const matchClaim = c.top_claim_text?.toLowerCase().includes(q);
        const matchReason = c.reason_flagged?.toLowerCase().includes(q);
        const matchId = c.id.includes(q);
        return matchHandle || matchClaim || matchReason || matchId;
      }
      return true;
    })
  );

  const activeClaimHits = $derived(hits.filter((h) => h.include && h.claim === activeClaim));
  const totalIncludedHits = $derived(hits.filter((h) => h.include).length);

  function loadCaseFromCase(c: Case, detail?: VideoDetail) {
    suggestions = c.suggestions;
    hits = buildHits(c);
    reposts = buildReposts(c);
    queries = buildQueries(c);
    sources = c.suggestions.filter((x) => x.kind === 'Source').slice(0, 3).map((x, k) => makeSource(c, x.text, x.why, k + 1));

    if (detail && detail.people && detail.people.length > 0) {
      entities = detail.people.map((p, idx) => ({
        id: idx + 1,
        name: p.canonical_name,
        type: p.entity_type || 'лице',
        summary: p.summary || p.role || 'Субект от видеото',
        found: true,
        checks: [{ src: 'Регистри и публикации', result: p.tags?.join(', ') || 'Няма данни за нарушения' }],
        sanctions: 'Няма съвпадение'
      }));
    } else {
      entities = structuredClone(entityMap[typeof c.id === 'number' ? c.id : 1] ?? []);
    }

    report = buildReport(c, hits, sources, queries, taxonomy);
  }

  async function openRealVideo(id: string, switchView = true) {
    if (switchView) {
      view = 'case';
      step = 0;
      activeClaim = 0;
      playhead = 0;
      playing = false;
    }
    try {
      const detail = await fetchVideoDetail(id);
      currentDetail = detail;
      currentCase = videoDetailToCase(detail, taxonomy);
      loadCaseFromCase(currentCase, detail);
      history = [`Отворен казус #${id} (${currentCase.creator})`, ...history];
    } catch {
      const fallbackCard = realCards.find((c) => c.id === id);
      if (fallbackCard) {
        history = [`Заредено резюме на видео #${id}`, ...history];
      }
    }
  }

  function pushLog(s: string) {
    history = [s, ...history];
  }

  function insertNote(t: string) {
    const en = report.TCS.find((b) => b.key === 'notesEn');
    if (en) en.value = en.value ? en.value + '\n\n' + t : t;
  }

  async function submitPaste() {
    const urls = pasteUrls.split('\n').map((u) => u.trim()).filter((u) => u.startsWith('http'));
    if (urls.length > 0) {
      await pasteLinks(urls.join('\n')).catch(() => {});
      pasteUrls = '';
      pasteModalOpen = false;
      pushLog(`Внесени за анализ ${urls.length} TikTok връзки`);
      loadAllData();
    }
  }

  const field = (k: string) => report.TCS.find((b) => b.key === k);
  let tool = $state('');
  let toolsOpen = $state(false);
  let activeClaim = $state(0);
  let showOcr = $state(true);
  const vk = (v: Verdict) => v.toLowerCase();

  const steps = ['Statements', 'Spread', 'Evidence', 'Report'];

  const verdictBg: Record<Verdict, string> = { SUPPORTED: 'ПОТВЪРДЕНО', CONTRADICTED: 'ОПРОВЕРГАНО', UNVERIFIABLE: 'НЕПРОВЕРИМО' };

  const chip = (v: Verdict) => (v === 'SUPPORTED' ? 'ok' : v === 'CONTRADICTED' ? 'bad' : 'warn');

  function resolve(s: Suggestion, state: 'accepted' | 'dismissed') {
    s.state = state;
    if (state === 'accepted') {
      if (s.kind === 'Source') {
        const id = Math.max(0, ...sources.map((x) => x.id)) + 1;
        sources = [...sources, makeSource(cur, s.text, s.why, id)];
      } else if (s.kind === 'Person') {
        const id = Math.max(0, ...entities.map((x) => x.id)) + 1;
        const name = s.text.split(':')[0];
        entities = [...entities, { id, name, type: 'лице', summary: s.text.split(':').slice(1).join(':').trim(), found: false, checks: [{ src: s.why, result: 'Няма резултати' }], sanctions: 'Няма съвпадение' }];
      }
    }
    history = [`${state === 'accepted' ? 'Прието' : 'Отхвърлено'}: ${s.kind}: ${s.text.slice(0, 40)}`, ...history];
  }

  function undo() {
    if (history.length > 1) history = history.slice(1);
  }

  function runTool(name: string) {
    tool = name;
    history = [`Стартиран инструмент: ${name}`, ...history];
  }

  function seek(sec: number) {
    playhead = sec;
    if (videoElement) {
      videoElement.currentTime = sec;
    }
  }

  const checklist = $derived([
    { label: 'Твърденията са разделени', done: true },
    { label: 'Оценката е зададена', done: !!field('rating')?.value },
    { label: 'Всяко твърдение има източник', done: claims.length > 0 && claims.every((_, i) => sources.some((x) => x.include && x.claims.includes(i))) },
    { label: 'Публикации намерени', done: hits.length > 0 },
    { label: 'Бележки EN', done: !!field('notesEn')?.value },
    { label: 'Бележки BG', done: !!field('notesBg')?.value }
  ]);
  const accepted = $derived(suggestions.filter((s) => s.state === 'accepted'));
</script>

<div class="app">
  <header>
    <b class="brand">Lead Finder</b>
    <nav>
      <button class:on={view === 'library'} onclick={() => (view = 'library')}>Library</button>
      <button class:on={view === 'case'} onclick={() => (view = 'case')}>Case</button>
      <button class:on={view === 'sources'} onclick={() => (view = 'sources')}>Source Library</button>
      <button onclick={() => (taxModalOpen = true)} title="Управление на теми, оценки и типове твърдения">🏷️ Таксономия</button>
      <a href="?legacy=true" style="color: #8b93a8; font-size: 11px; text-decoration: none; padding: 4px 8px; border: 1px solid #353b50; border-radius: 6px; margin-left: 6px;">Режим Inbox</a>
    </nav>
    <span class="grow"></span>
    <label>Assist
      <select bind:value={assist}><option>Quiet</option><option>Suggest</option><option>Draft</option></select>
    </label>
    <span class="pill">{runningTasksCount > 0 ? `${runningTasksCount} фонови задачи` : `${realCards.length} видеа в базата`}</span>
    <button class="ghost">Ctrl+K</button>
    <button class="ghost">Settings</button>
  </header>

  {#if view === 'library'}
    <main class="lib">
      <div class="tabs">
        {#each ['Videos', 'People & Orgs', 'Creators', 'Sources', 'Категории'] as t}
          <button class:on={libTab === t} onclick={() => (libTab = t as typeof libTab)}>
            {t}
            {#if t === 'Videos'} ({realCards.length})
            {:else if t === 'People & Orgs'} ({realPeople.length})
            {:else if t === 'Creators'} ({realCreators.length})
            {/if}
          </button>
        {/each}
        <span class="grow"></span>
        <button class="primary" onclick={() => (pasteModalOpen = true)}>+ Paste TikTok URL</button>
      </div>
      <div class="filters">
        <input placeholder="Търси по автор, твърдение или ID…" bind:value={searchQuery} />
        <select bind:value={selectedLane}>
          <option value="all">Всички ленти ({realCards.length})</option>
          <option value="leads">Лийдове (leads)</option>
          <option value="pasted">Внесени (pasted)</option>
          <option value="kept">Запазени (kept)</option>
          <option value="low_quality">Ниско качество</option>
          <option value="screened_out">Филтрирани</option>
        </select>
        <select bind:value={selectedRating}>
          <option value="All ratings">Всички оценки</option>
          <option value="Supported">Потвърдени (Supported)</option>
          <option value="Contradicted">Опровергани (Contradicted)</option>
          <option value="Unverifiable">Непроверими (Unverifiable)</option>
        </select>
        <span class="hint" style="margin-left: auto;">{filteredVideos.length} резултата</span>
      </div>
      {#if libTab === 'Videos'}
        <table>
          <thead><tr><th>Видео / Твърдение</th><th>Автор</th><th>Лента</th><th>Оценка</th><th>Гледания</th><th>Lead Score</th></tr></thead>
          <tbody>
            {#each filteredVideos as v}
              <tr onclick={() => openRealVideo(v.id)}>
                <td>
                  <div style="font-weight: 600;">{v.top_claim_text || v.reason_flagged || v.id}</div>
                  <div class="hint" style="font-size: 11px;">ID: {v.id} {v.duration_s ? `• ${Math.round(v.duration_s)}s` : ''}</div>
                </td>
                <td><span class="tag">@{v.creator_handle || 'неизвестен'}</span></td>
                <td><span class="pill" style="font-size: 11px;">{v.lane}</span></td>
                <td>
                  {#if v.top_claim_verdict === 'SUPPORTED'}
                    <span class="chip ok">✓ Потвърдено</span>
                  {:else if v.top_claim_verdict === 'CONTRADICTED'}
                    <span class="chip bad">✕ Опровергано</span>
                  {:else}
                    <span class="chip warn">? {v.top_claim_verdict || 'Непроверено'}</span>
                  {/if}
                </td>
                <td><b>{v.views.toLocaleString('bg-BG')}</b></td>
                <td><span class="tag">{Math.round(v.lead_score)}</span></td>
              </tr>
            {/each}
            {#if filteredVideos.length === 0}
              <tr>
                <td colspan="6" style="text-align: center; padding: 24px;" class="hint">
                  {realCards.length === 0 ? 'Зареждане на видеа от базата данни...' : 'Няма намерени видеа по избраните критерии.'}
                </td>
              </tr>
            {/if}
          </tbody>
        </table>
      {:else if libTab === 'People & Orgs'}
        <table>
          <thead><tr><th>Име</th><th>Тип</th><th>Роля / Описание</th><th>Тагове / Категория</th><th>Споменавания</th></tr></thead>
          <tbody>
            {#each realPeople as p}
              <tr>
                <td><b>{p.canonical_name}</b></td>
                <td><span class="tag">{p.entity_type || 'лице'}</span></td>
                <td>{p.role || p.summary || p.role_hint || '-'}</td>
                <td>{p.tags?.join(', ') || p.category || '-'}</td>
                <td><span class="pill">{p.mentions || 1}</span></td>
              </tr>
            {/each}
            {#if realPeople.length === 0}
              <tr><td colspan="5" style="text-align: center; padding: 24px;" class="hint">Няма открити субекти в базата.</td></tr>
            {/if}
          </tbody>
        </table>
      {:else if libTab === 'Creators'}
        <table>
          <thead><tr><th>Автор (@handle)</th><th>Име</th><th>Последователи</th><th>Флагнати видеа</th><th>Наблюдение</th></tr></thead>
          <tbody>
            {#each realCreators as c}
              <tr>
                <td><b>@{c.handle}</b></td>
                <td>{c.display_name || '-'}</td>
                <td>{c.follower_count ? c.follower_count.toLocaleString('bg-BG') : '-'}</td>
                <td><span class="pill">{c.flagged_count}</span></td>
                <td>{c.watchlist ? '★ Да' : 'Не'}</td>
              </tr>
            {/each}
            {#if realCreators.length === 0}
              <tr><td colspan="5" style="text-align: center; padding: 24px;" class="hint">Няма открити автори в базата.</td></tr>
            {/if}
          </tbody>
        </table>
        <p class="hint">Sentence-like names are blocked on entry. Duplicates attach to the existing entity. 3 borderline merges wait in Review.</p>
      {:else if libTab === 'Категории'}
        <div class="panel" style="max-width: 680px; margin-top: 10px;">
          <h3>🏷️ Активна таксономия</h3>
          <p class="hint">Всички теми, оценки, видове и типове твърдения са напълно динамични и се редактират от потребителя.</p>
          <div class="grid g2" style="margin: 14px 0;">
            <div class="card">
              <b>Теми ({taxonomy.topics.length})</b>
              <div class="hint" style="margin-top: 4px;">{taxonomy.topics.join(', ')}</div>
            </div>
            <div class="card">
              <b>Оценки / Присъди ({taxonomy.ratings.length})</b>
              <div class="hint" style="margin-top: 4px;">{taxonomy.ratings.join(', ')}</div>
            </div>
            <div class="card">
              <b>Типове твърдения ({taxonomy.claimTypes.length})</b>
              <div class="hint" style="margin-top: 4px;">{taxonomy.claimTypes.join(', ')}</div>
            </div>
            <div class="card">
              <b>Вид на твърдението ({taxonomy.claimKinds.length})</b>
              <div class="hint" style="margin-top: 4px;">{taxonomy.claimKinds.join(', ')}</div>
            </div>
          </div>
          <button class="primary" onclick={() => (taxModalOpen = true)}>⚙️ Отвори редактора на категориите</button>
        </div>
      {:else}
        <p class="hint">{libTab}: same table layout, same filters.</p>
      {/if}
    </main>
  {/if}

  {#if view === 'case'}
    <div class="case" style="grid-template-columns: {leftColWidth}px 1fr 290px;">
      <aside class="stepper" style="width: {leftColWidth}px;">
        <div
          class="stepper-resizer"
          class:active={isResizing}
          onmousedown={startResize}
          role="separator"
          tabindex="0"
          aria-valuenow={leftColWidth}
          aria-label="Промени ширината на лявата колона"
        ></div>
        <div class="crumb"><button class="ghost" onclick={() => (view = 'library')}>‹ Library</button></div>
        {#each steps as s, i}
          <button class="step" class:on={step === i} onclick={() => (step = i)}>{i + 1}. {s}</button>
        {/each}
        <div class="dock">
          <div class="player dockvid">
            {#if currentDetail?.video?.id}
              <video
                bind:this={videoElement}
                src={`${getCoreUrl()}/api/videos/${encodeURIComponent(currentDetail.video.id)}/media`}
                poster={currentDetail.video.thumbnail_url || undefined}
                bind:currentTime={playhead}
                bind:paused={videoPaused}
                controls
                playsinline
                class="docked-video-elem"
              ></video>
            {:else}
              <div class="dock-placeholder">
                {playing ? '❚❚' : '▶'}
                <span class="dockts">{Math.floor(playhead / 60)}:{String(Math.floor(playhead % 60)).padStart(2, '0')} / {Math.floor(cur.duration / 60)}:{String(cur.duration % 60).padStart(2, '0')}</span>
              </div>
            {/if}
          </div>
          <div class="marks">
            {#each claims as c, i}<i class="mk {chip(c.v)}" style="left:{(Number(c.at.split('s')[0]) / Math.max(1, cur.duration)) * 100}%" title="Т{i + 1}" onclick={() => { activeClaim = i; seek(Number(c.at.split('s')[0])); }}></i>{/each}
            <input type="range" min="0" max={cur.duration} bind:value={playhead} oninput={(e) => seek(Number((e.target as HTMLInputElement).value))} />
          </div>
          <div class="row">
            <button class="ghost" onclick={() => {
              if (videoElement) {
                if (videoElement.paused) videoElement.play(); else videoElement.pause();
              } else {
                playing = !playing;
              }
            }}>{(videoElement ? !videoPaused : playing) ? 'Пауза' : 'Пусни'}</button>
            <button class="ghost" onclick={() => seek(Math.max(0, playhead - 5))}>-5s</button>
            <button class="ghost" onclick={() => seek(Math.min(cur.duration, playhead + 5))}>+5s</button>
            {#if cur.videoUrl}
              <button class="ghost" onclick={() => openExternalUrl(cur.videoUrl || '')} title="Отвори видеото в TikTok">🔗 TikTok</button>
            {/if}
          </div>
          <div class="hint">{cur.creator} • {cur.views} гледания • {cur.duration}s</div>
          {#if realCards.length > 1}
            <div style="margin-top: 6px;">
              <select
                style="width: 100%; font-size: 11px;"
                value={cur.id}
                onchange={(e) => openRealVideo((e.target as HTMLSelectElement).value)}
              >
                {#each realCards as rc}
                  <option value={rc.id}>
                    @{rc.creator_handle || 'user'}: {(rc.top_claim_text || rc.reason_flagged || rc.id).slice(0, 36)}...
                  </option>
                {/each}
              </select>
            </div>
          {/if}
        </div>
        <h4>Checklist</h4>
        {#each checklist as c}<div class="chk" class:done={c.done}>{c.done ? '✓' : '○'} {c.label}</div>{/each}
        <h4>History</h4>
        <div class="hist">{#each history.slice(0, 6) as h}<div>{h}</div>{/each}</div>
        <button onclick={undo}>Undo</button>
      </aside>

      <section class="canvas">
        <div class="title">
          {cur.title}
          <span class="grow"></span>
          <div class="menu">
            <button onclick={() => (toolsOpen = !toolsOpen)}>Инструменти ▾</button>
            {#if toolsOpen}
              <div class="dropdown">
                {#each ['Раздели твърдения', 'Търси в платформите', 'Провери лице', 'Провери фирма', 'Намери източници', 'Обратно търсене на снимка', 'Архив', 'Санкции', 'Deepfake анализ'] as t}
                  <button class="ghost" onclick={() => { runTool(t); toolsOpen = false; }}>{t}</button>
                {/each}
                <hr />
                <button class="ghost">Пресет: Здравно твърдение ▾</button>
              </div>
            {/if}
          </div>
          <button>Експорт ▾</button>
        </div>

        {#if step === 0}
          <div class="two">
            <div class="section-card transcript-section">
              <div class="section-header">
                <div>
                  <h3>Текст от видеото</h3>
                  <span class="section-subhint">Цветове: Червено = Опровергано, Зелено = Потвърдено, Жълто = Непроверимо</span>
                </div>
                <div class="transcript-btn-group">
                  <button class="btn btn-secondary btn-sm">🔄 Whisper</button>
                  <button class="btn btn-primary btn-sm btn-ai-fix">✨ AI Поправка</button>
                  <button class="btn btn-secondary btn-sm btn-google-audio">🎙️ Google AI</button>
                  <button class="btn btn-secondary btn-sm">✏️ Редактирай</button>
                  <button class="btn btn-secondary btn-sm">📋 Копирай</button>
                </div>
              </div>

              <div class="ocr-overview">
                <div class="ocr-overview-head">
                  <button class="ocr-toggle" class:on={showOcr} onclick={() => (showOcr = !showOcr)}>🖥️ Текст на екрана ({cur.ocr.length})</button>
                  {#if showOcr}<span class="ocr-legend"><i class="lg-speech"></i>Реч <i class="lg-ocr"></i>Текст на екрана</span>{/if}
                </div>
                {#if showOcr}
                  {#each cur.ocr as o}<div class="ocr-chip"><span class="ocr-chip-tag">🖥️ {o.t}</span><span class="ocr-chip-text">{o.text}</span></div>{/each}
                {/if}
              </div>

              <div class="transcript-segments">
                {#each segments as sg}
                  {@const ci = sg.claim}
                  <span
                    class="transcript-segment"
                    class:highlighted-pulse={activeClaim === ci && ci !== null}
                    class:active={playhead >= sg.sec && playhead < sg.sec + 8}
                    class:claim-mark-contradicted={ci !== null && claims[ci] && vk(claims[ci].v) === 'contradicted'}
                    class:claim-mark-supported={ci !== null && claims[ci] && vk(claims[ci].v) === 'supported'}
                    class:claim-mark-unverifiable={ci !== null && claims[ci] && vk(claims[ci].v) === 'unverifiable'}
                    onclick={() => { seek(sg.sec); if (ci !== null) activeClaim = ci; }}
                    role="button"
                    tabindex="0"
                    onkeydown={(e) => e.key === 'Enter' && seek(sg.sec)}
                  >
                    {#if ci !== null && claims[ci]}<span class="claim-badge-inline {vk(claims[ci].v)}">[Т{ci + 1}]</span>{/if}
                    {sg.text}{' '}
                  </span>
                {/each}
                {#if segments.length === 0}
                  <p class="empty-hint" style="padding: 12px;">Няма налична стенограма за това видео.</p>
                {/if}
              </div>
            </div>

            <div class="section-card claims-section">
              <div class="section-header">
                <div>
                  <h3>Твърдения и преценка</h3>
                  <span class="section-subhint">Бърза проверка в български и международни източници</span>
                </div>
              </div>

              {#each claims as c, i}
                <div class="claim-block claim-verdict-{vk(c.v)}" class:claim-pulse={activeClaim === i} onclick={() => (activeClaim = i)} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && (activeClaim = i)}>
                  <div class="claim-header">
                    <span class="claim-index-pill">Т{i + 1}</span>
                    <select
                      class="claim-type-select"
                      bind:value={c.type}
                      onclick={(e) => e.stopPropagation()}
                      onchange={(e) => {
                        const val = (e.target as HTMLSelectElement).value;
                        if (val === '__manage__') {
                          c.type = taxonomy.claimTypes[0] || 'твърдение';
                          taxModalOpen = true;
                        } else {
                          pushLog(`Променен тип на Т${i + 1}: ${c.type}`);
                        }
                      }}
                      title="Промени типа на твърдението"
                    >
                      {#each taxonomy.claimTypes as ct}
                        <option value={ct}>{ct}</option>
                      {/each}
                      <option value="__manage__">+ Управление на типовете...</option>
                    </select>
                    <span class="claim-source-tag speech">🎙️ Реч</span>
                    <span class="verdict-tag {vk(c.v)}">{verdictBg[c.v]}</span>
                    <button class="btn btn-shortcut btn-sm">📍 Към цитата в текста</button>
                    <button class="btn btn-text btn-sm" onclick={(e) => { e.stopPropagation(); seek(Number(c.at.split('s')[0])); }}>⏱ {c.at}</button>
                    <button class="btn btn-text btn-sm">📋 Цитат</button>
                    {#if c.evidence.length}<button class="btn btn-shortcut btn-sm">🎯 Към доказателството</button>{/if}
                  </div>

                  <div class="claim-text">{c.text}</div>

                  <div class="claim-reasoning reasoning-verdict-{vk(c.v)}">
                    <div class="reasoning-head">
                      <span class="reasoning-title">⚖️ Мотиви и оценка</span>
                      <span class="reasoning-count-pill">{c.checks.length} елемент</span>
                    </div>
                    <div class="reasoning-statements-list">
                      {#each c.checks as ck, k}
                        <div class="reasoning-stmt-card stmt-{vk(ck.v)}">
                          <div class="stmt-top-row">
                            <span class="stmt-index">#{k + 1}</span>
                            <span class="stmt-status-pill {vk(ck.v)}">{ck.v === 'SUPPORTED' ? '✓ Потвърдено' : ck.v === 'CONTRADICTED' ? '✕ Опровергано' : '? Непроверимо'}</span>
                            <strong class="stmt-subject">{ck.s}</strong>
                            {#each c.evidence as e, ei}<span class="stmt-refs"><span class="stmt-ref-tag">[p{ei + 1}]</span></span>{/each}
                          </div>
                          <div class="stmt-detail">{ck.c}</div>
                          <button class="btn-feedback">✏️ Редактирай изявлението</button>
                        </div>
                      {/each}
                    </div>
                    <div class="reasoning-summary-footer summary-{vk(c.v)}">
                      <span class="summary-badge">📌 Обобщение:</span>
                      <span class="summary-text">{c.summary}</span>
                    </div>
                  </div>

                  <button class="btn-feedback">✏️ Редактирай и провери отново</button>
                  <div class="claim-feedback-bar">
                    <span class="feedback-hint">Оценка на резултата:</span>
                    <button class="btn-feedback correct">👍 Вярна преценка</button>
                    <button class="btn-feedback wrong">👎 Грешна преценка</button>
                    <button class="btn-feedback">⚠️ Несвързано доказателство</button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if step === 1}
          <WbSpread {cur} bind:hits bind:reposts bind:queries log={pushLog} />
        {/if}

        {#if step === 2}
          <WbEvidence {cur} bind:suggestions bind:sources bind:entities insertNote={insertNote} log={pushLog} />
        {/if}

        {#if step === 3}
          <WbReport {cur} bind:report bind:taxonomy {hits} {sources} {queries} {assist} log={pushLog} />
        {/if}
      </section>

      <aside class="rail">
        {#if claims.length > 0 && claims[activeClaim]}
          <h4>Избрано твърдение Т{activeClaim + 1}</h4>
          <div class="st {chip(claims[activeClaim].v)}">{claims[activeClaim].text}</div>
          <div class="spread">
            <b>{activeClaimHits.length} намерени</b> <span class="hint">за Т{activeClaim + 1} ({totalIncludedHits} общо)</span>
            {#each Object.entries(cur.platforms) as [p, _]}
              {@const pn = activeClaimHits.filter((h) => h.platform === p).length}
              {#if pn > 0}
                <div class="bar"><span>{p}</span><i style="width:{pn * 18}px"></i>{pn}</div>
              {/if}
            {/each}
          </div>
        {:else}
          <h4>Детайли</h4>
          <p class="hint">Няма избрано твърдение.</p>
        {/if}
        {#if assist !== 'Quiet'}
          <h4>Предложения</h4>
          {#each suggestions.filter((s) => s.state === 'new') as s}
            <div class="sug">
              <span class="tag">{s.kind}</span> {s.text}
              <div class="hint">Защо: {s.why}</div>
              <div class="row">
                <button class="primary" onclick={() => resolve(s, 'accepted')}>Приеми</button>
                <button class="ghost" onclick={() => resolve(s, 'dismissed')}>Отхвърли</button>
              </div>
            </div>
          {:else}<p class="hint">Няма чакащи предложения.</p>{/each}
        {/if}
        <button class="ghost">📎 Прикачи файл (по избор)</button>
      </aside>
    </div>
  {/if}

  {#if view === 'sources'}
    <main class="lib">
      <div class="tabs"><b>Source Library</b><span class="grow"></span><button class="primary" onclick={() => (wizard = 1)}>+ Add source</button><button>Import pack</button><button>Export pack</button></div>
      <table>
        <thead><tr><th>On</th><th>Name</th><th>Type</th><th></th></tr></thead>
        <tbody>{#each sourceCatalog as s}<tr><td><input type="checkbox" bind:checked={s.on} /></td><td>{s.name}</td><td>{s.type}</td><td><button class="ghost">Test query</button></td></tr>{/each}</tbody>
      </table>
      {#if wizard > 0}
        <div class="wizard">
          <b>Add source: step {wizard} of 4</b>
          {#if wizard === 1}<input placeholder="Paste a URL" bind:value={wizardUrl} />
          {:else if wizard === 2}<p>Detected: site search form at {wizardUrl || 'example.org'}/search?q=</p>
          {:else if wizard === 3}<input placeholder="Type a test query" /><p class="hint">3 results found. Title, URL, date mapped.</p>
          {:else}<p>Name, type, topic tags, reliability.</p>{/if}
          <button onclick={() => (wizard = wizard < 4 ? wizard + 1 : 0)}>{wizard < 4 ? 'Next' : 'Save'}</button>
          <button class="ghost" onclick={() => (wizard = 0)}>Cancel</button>
        </div>
      {/if}
    </main>
  {/if}

  <WbTaxonomyModal bind:taxonomy bind:open={taxModalOpen} />

  {#if pasteModalOpen}
    <div class="tax-backdrop" onclick={() => (pasteModalOpen = false)} role="presentation">
      <div class="tax-dialog" style="max-width: 520px;" onclick={(e) => e.stopPropagation()} role="dialog">
        <div class="tax-header">
          <div>
            <h3>+ Внеси TikTok видео линкове</h3>
            <p class="hint">Постави един или повече TikTok URL адреса (по един на ред). Те ще бъдат изтеглени, транскрибирани и анализирани.</p>
          </div>
          <button class="ghost" onclick={() => (pasteModalOpen = false)}>✕</button>
        </div>
        <div class="tax-body">
          <textarea
            style="min-height: 140px;"
            placeholder="https://www.tiktok.com/@creator/video/123456789..."
            bind:value={pasteUrls}
          ></textarea>
        </div>
        <div class="tax-footer">
          <button class="ghost" onclick={() => (pasteModalOpen = false)}>Отказ</button>
          <button class="primary" onclick={submitPaste}>Внеси и сканирай</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .app { font: 13px system-ui, sans-serif; background: #12141a; color: #dde1ea; min-height: 100vh; }
  header { display: flex; gap: 10px; align-items: center; padding: 8px 14px; background: #1a1d26; border-bottom: 1px solid #2b3040; }
  .brand { margin-right: 10px; }
  .grow { flex: 1; }
  button { background: #262b3a; color: inherit; border: 1px solid #353b50; padding: 5px 10px; border-radius: 6px; cursor: pointer; }
  button.on { background: #3b4fa0; }
  button.primary { background: #3b6fe0; border-color: #3b6fe0; }
  button.ghost { background: transparent; }
  input, select, textarea { background: #0e1016; color: inherit; border: 1px solid #353b50; border-radius: 6px; padding: 5px; }
  textarea { width: 100%; min-height: 110px; box-sizing: border-box; }
  .pill { background: #2b3a66; padding: 3px 8px; border-radius: 12px; }
  .lib { padding: 14px; }
  .tabs, .filters { display: flex; gap: 8px; align-items: center; margin-bottom: 10px; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 8px; border-bottom: 1px solid #2b3040; }
  tbody tr { cursor: pointer; }
  tbody tr:hover { background: #1c2030; }
  .tag { background: #262b3a; border-radius: 4px; padding: 1px 6px; margin-right: 4px; font-size: 11px; }
  .claim-type-select { background: #262b3a; color: #dde1ea; border: 1px solid #353b50; border-radius: 4px; padding: 2px 6px; font-size: 11px; cursor: pointer; }
  .claim-type-select:focus { outline: 1px solid #3b6fe0; }
  .chip { padding: 2px 8px; border-radius: 10px; }
  .ok { background: #1f5a3a; } .bad { background: #6a2530; } .warn { background: #6a5520; }
  .hint { color: #8b93a8; font-size: 12px; }
  .case { display: grid; height: calc(100vh - 46px); }
  .stepper { position: relative; border-right: 1px solid #2b3040; display: flex; flex-direction: column; gap: 6px; padding: 12px; background: #161922; overflow-y: auto; box-sizing: border-box; }
  .stepper-resizer { position: absolute; top: 0; right: -4px; bottom: 0; width: 8px; cursor: col-resize; z-index: 20; transition: background 0.15s ease; }
  .stepper-resizer:hover, .stepper-resizer.active { background: #3b6fe0; }
  .dock { position: sticky; top: 0; background: #0e1016; border: 1px solid #2b3040; border-radius: 10px; padding: 8px; margin-bottom: 8px; }
  .dockvid { height: 230px; position: relative; margin-bottom: 6px; overflow: hidden; background: #000; border-radius: 6px; }
  .docked-video-elem { width: 100%; height: 100%; object-fit: contain; background: #000; }
  .dock-placeholder { width: 100%; height: 100%; display: grid; place-items: center; font-size: 28px; }
  .dockts { position: absolute; bottom: 6px; right: 8px; font-size: 11px; color: #c8cde0; }
  .marks { position: relative; padding-top: 10px; }
  .marks input { width: 100%; }
  .mk { position: absolute; top: 0; width: 8px; height: 8px; border-radius: 50%; cursor: pointer; transform: translateX(-4px); }
  .rail { padding: 12px; background: #161922; overflow: auto; border-left: 1px solid #2b3040; }
  .step { text-align: left; }
  .chk { color: #8b93a8; } .chk.done { color: #6fd39a; }
  .hist { font-size: 11px; color: #8b93a8; }
  .canvas { padding: 14px; overflow: auto; }
  .title { display: flex; gap: 8px; align-items: center; font-size: 16px; font-weight: 600; margin-bottom: 10px; }
  .palette { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 12px; }
  .video { display: grid; grid-template-columns: 220px 1fr; gap: 12px; margin-bottom: 12px; }
  .player { background: #000; height: 140px; display: grid; place-items: center; border-radius: 8px; }
  .st { padding: 6px 8px; border-radius: 6px; margin-bottom: 4px; }
  .form { display: flex; gap: 10px; flex-wrap: wrap; }
  .form label { display: flex; flex-direction: column; gap: 3px; }
  .notes { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-bottom: 8px; }
  .src, .sug { background: #1c2030; border-radius: 6px; padding: 8px; margin-bottom: 6px; }
  .spread .bar { display: flex; align-items: center; gap: 6px; margin: 2px 0; }
  .spread .bar span { width: 70px; }
  .spread .bar i { display: block; height: 8px; background: #3b6fe0; border-radius: 4px; }
  .wizard { margin-top: 14px; padding: 12px; background: #1c2030; border-radius: 8px; display: flex; flex-direction: column; gap: 8px; max-width: 480px; }
  .menu { position: relative; }
  .dropdown { position: absolute; right: 0; top: 34px; z-index: 5; background: #1c2030; border: 1px solid #353b50; border-radius: 8px; padding: 6px; display: flex; flex-direction: column; min-width: 230px; }
  .dropdown button { text-align: left; border: 0; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .panel { background: #161922; border: 1px solid #2b3040; border-radius: 10px; padding: 14px; }
  .panel h3 { margin: 0 0 2px; font-size: 15px; }
  .phead { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 10px; }
  .row { display: flex; gap: 6px; flex-wrap: wrap; margin: 8px 0; }
  .player.big { height: 260px; }
  .segs { margin-top: 10px; display: flex; flex-direction: column; gap: 4px; }
  .seg { padding: 6px 8px; border-radius: 6px; border-left: 3px solid transparent; line-height: 1.45; }
  .seg.ok { background: #1f5a3a55; border-color: #3fbf7f; } .seg.bad { background: #6a253055; border-color: #e0525f; } .seg.warn { background: #6a552055; border-color: #e0b341; }
  .seg.pulse { outline: 2px solid #3b6fe0; }
  .ts { color: #8b93a8; font-variant-numeric: tabular-nums; margin-right: 6px; }
  .ocr { margin-top: 10px; }
  .claim { border: 1px solid #2b3040; border-left: 4px solid #555; border-radius: 8px; padding: 8px 10px; margin-bottom: 8px; cursor: pointer; }
  .claim.ok { border-left-color: #3fbf7f; } .claim.bad { border-left-color: #e0525f; } .claim.warn { border-left-color: #e0b341; }
  .claim.active { background: #1c2030; }
  .chead { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .idx { background: #3b4fa0; border-radius: 10px; padding: 1px 8px; font-weight: 600; }
  .vt { border-radius: 10px; padding: 1px 8px; font-size: 11px; }
  .ctext { margin: 6px 0; }
  .checks { border-top: 1px dashed #353b50; padding-top: 6px; }
  .check { padding: 4px 6px; border-radius: 4px; margin-bottom: 3px; }
  .summary { margin: 6px 0; color: #c8cde0; }
  .ev { margin: 4px 0; }
@keyframes activePulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.65; }
  }
.media-section {
    width: 100%;
    background: #000;
    border-radius: 8px;
    overflow: hidden;
  }
.media-section.feed-mode {
    width: 100%;
    background: #000;
    border-radius: 8px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-color);
    margin-top: 0.75rem;
    margin-bottom: 0.85rem;
  }
.feed-top-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.45rem 0.85rem;
    background: rgba(18, 18, 24, 0.9);
    border-bottom: 1px solid var(--border-color);
  }
.feed-creator-badge {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
.creator-name {
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--primary-accent);
  }
.creator-views {
    font-size: 0.75rem;
    color: var(--text-muted);
  }
.feed-duration-pill {
    font-size: 0.72rem;
    font-weight: 700;
    color: #fff;
    background: rgba(0, 0, 0, 0.6);
    padding: 1px 6px;
    border-radius: 4px;
  }
.feed-video-container {
    width: 100%;
    height: var(--video-stage-height, 420px);
    background: #000;
    display: flex;
    align-items: center;
    justify-content: center;
  }
.feed-vertical-player {
    max-width: 100%;
    max-height: 100%;
    height: 100%;
    aspect-ratio: 9 / 16;
    object-fit: contain;
    display: block;
    background: #000;
  }
.btn-ai-fix {
    background: var(--primary-accent);
    color: #fff;
    font-weight: 600;
  }
.btn-google-audio {
    background: rgba(37, 99, 235, 0.15);
    color: #3b82f6;
    border: 1px solid rgba(37, 99, 235, 0.35);
    font-weight: 600;
  }
.btn-google-audio:hover {
    background: rgba(37, 99, 235, 0.25);
  }
.editing-banner {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--primary-accent);
    background: var(--primary-accent-alpha);
    border: 1px dashed var(--primary-accent);
    padding: 0.35rem 0.65rem;
    border-radius: 6px;
    margin-bottom: 0.5rem;
    width: 100%;
  }
.transcript-segment-editable {
    display: inline;
    padding: 1px 3px;
    border-radius: 3px;
    cursor: text;
    background: rgba(59, 130, 246, 0.08);
    border-bottom: 1px dashed #3b82f6;
    outline: none;
  }
.transcript-segment-editable:focus {
    background: rgba(59, 130, 246, 0.2);
    border-bottom: 1px solid #3b82f6;
  }
.section-card {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.85rem;
  }
.section-card h3 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-heading);
  }
.section-subhint {
    display: block;
    font-size: 0.72rem;
    color: var(--text-muted);
    margin-top: 0.2rem;
  }
.section-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 0.65rem;
    gap: 0.5rem;
  }
.transcript-btn-group {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.35rem;
  }
.copy-toast {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--verdict-supported);
    background: rgba(16, 185, 129, 0.15);
    border: 1px solid rgba(16, 185, 129, 0.3);
    padding: 2px 6px;
    border-radius: 4px;
    white-space: nowrap;
  }
.transcript-editor {
    width: 100%;
    padding: 0.5rem;
    font-size: 0.85rem;
    background: var(--input-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-body);
    box-sizing: border-box;
    font-family: inherit;
  }
.transcript-segments {
    font-size: 0.88rem;
    line-height: 1.6;
    background: var(--surface-bg);
    padding: 0.75rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
  }
.transcript-segment {
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
    transition: background 0.15s ease, border-color 0.15s ease;
    display: inline;
  }
.transcript-segment:hover {
    background: var(--hover-bg);
  }
.transcript-segment.active {
    background: var(--primary-accent-alpha);
    color: var(--primary-accent);
    font-weight: 600;
  }
.transcript-segment.claim-mark-contradicted {
    background: rgba(225, 29, 72, 0.15);
    border-bottom: 2px solid var(--verdict-contradicted);
    color: var(--text-heading);
  }
.transcript-segment.claim-mark-supported {
    background: rgba(16, 185, 129, 0.15);
    border-bottom: 2px solid var(--verdict-supported);
    color: var(--text-heading);
  }
.transcript-segment.claim-mark-unverifiable {
    background: rgba(217, 119, 6, 0.15);
    border-bottom: 2px solid var(--verdict-unverifiable);
    color: var(--text-heading);
  }
.ocr-overview {
    margin-bottom: 0.6rem;
    padding: 0.55rem 0.75rem;
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
  }
.ocr-overview-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
.ocr-toggle {
    font-size: 0.78rem;
    font-weight: 600;
    padding: 3px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--pill-bg);
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
  }
.ocr-toggle:hover {
    background: var(--hover-bg);
  }
.ocr-toggle.on {
    background: var(--primary-accent-alpha);
    color: var(--primary-accent);
    border-color: var(--primary-accent);
  }
.ocr-legend {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.72rem;
    color: var(--text-muted);
  }
.ocr-legend i {
    display: inline-block;
    border-radius: 3px;
  }
.ocr-legend .lg-speech {
    width: 16px;
    height: 6px;
    background: var(--text-muted);
    opacity: 0.4;
  }
.ocr-legend .lg-ocr {
    width: 4px;
    height: 12px;
    margin-left: 0.5rem;
    background: var(--primary-accent);
  }
.ocr-track {
    position: relative;
    height: 22px;
    margin-top: 0.55rem;
    background: var(--input-bg);
    border-radius: 4px;
  }
.ocr-track-speech {
    position: absolute;
    top: 8px;
    height: 6px;
    border-radius: 3px;
    background: var(--text-muted);
    opacity: 0.4;
  }
.ocr-track-mark {
    position: absolute;
    top: 2px;
    width: 4px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 2px;
    transform: translateX(-50%);
    background: var(--primary-accent);
    cursor: pointer;
    transition: transform 0.12s ease;
  }
.ocr-track-mark:hover {
    transform: translateX(-50%) scaleX(1.8);
  }
.ocr-track-playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--text-heading);
    opacity: 0.55;
    pointer-events: none;
  }
.ocr-chip {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    margin: 0.35rem 0;
    padding: 0.3rem 0.6rem;
    border-left: 3px solid var(--primary-accent);
    border-radius: 4px;
    background: var(--primary-accent-alpha);
    color: var(--text-body);
    font-size: 0.82rem;
    line-height: 1.45;
    cursor: pointer;
    transition: background 0.15s ease, box-shadow 0.15s ease;
  }
.ocr-chip:hover {
    background: var(--hover-bg);
  }
.ocr-chip.active {
    box-shadow: 0 0 0 1px var(--primary-accent);
    color: var(--text-heading);
  }
.ocr-chip-tag {
    flex-shrink: 0;
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: var(--primary-accent);
    white-space: nowrap;
  }
.ocr-chip-text {
    font-style: italic;
    word-break: break-word;
  }
.ocr-tail {
    margin-top: -1px;
    padding: 0.4rem 0.75rem;
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-top: 1px dashed var(--border-color);
    border-radius: 0 0 6px 6px;
  }
.claim-badge-inline {
    font-size: 0.68rem;
    font-weight: 700;
    padding: 1px 4px;
    border-radius: 3px;
    margin-right: 2px;
    cursor: pointer;
    background: var(--primary-accent-alpha);
    color: var(--primary-accent);
  }
.claim-badge-inline.contradicted {
    background: rgba(225, 29, 72, 0.25);
    color: var(--verdict-contradicted);
  }
.claim-badge-inline.supported {
    background: rgba(16, 185, 129, 0.25);
    color: var(--verdict-supported);
  }
.claim-badge-inline.unverifiable {
    background: rgba(217, 119, 6, 0.25);
    color: var(--verdict-unverifiable);
  }
.highlighted-pulse {
    animation: segPulse 1.8s ease-in-out;
  }
@keyframes segPulse {
    0% {
      box-shadow: 0 0 0 0 rgba(37, 99, 235, 0.8);
      background: rgba(37, 99, 235, 0.35);
    }
    50% {
      box-shadow: 0 0 0 6px rgba(37, 99, 235, 0);
      background: rgba(37, 99, 235, 0.45);
    }
    100% {
      box-shadow: 0 0 0 0 rgba(37, 99, 235, 0);
    }
  }
.claim-block {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-left: 4px solid var(--border-color);
    border-radius: 6px;
    padding: 0.85rem;
    margin-bottom: 0.85rem;
    transition: box-shadow 0.2s ease, border-color 0.2s ease;
  }
.claim-block.claim-verdict-supported {
    border-left-color: #10b981;
    background: linear-gradient(90deg, rgba(16, 185, 129, 0.045) 0%, var(--surface-bg) 32%);
  }
.claim-block.claim-verdict-contradicted {
    border-left-color: #ef4444;
    background: linear-gradient(90deg, rgba(239, 68, 68, 0.05) 0%, var(--surface-bg) 32%);
  }
.claim-block.claim-verdict-unverifiable {
    border-left-color: #eab308;
    background: linear-gradient(90deg, rgba(234, 179, 8, 0.045) 0%, var(--surface-bg) 32%);
  }
.claim-pulse {
    border-color: var(--primary-accent) !important;
    box-shadow: 0 0 14px rgba(37, 99, 235, 0.3) !important;
  }
.claim-header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-bottom: 0.5rem;
  }
.claim-index-pill {
    font-size: 0.7rem;
    font-weight: 800;
    background: var(--pill-bg);
    color: var(--text-heading);
    padding: 2px 6px;
    border-radius: 4px;
    border: 1px solid var(--border-color);
  }
.claim-source-tag {
    font-size: 0.68rem;
    font-weight: 700;
    background: var(--pill-bg);
    color: var(--text-muted);
    padding: 1px 6px;
    border-radius: 4px;
    white-space: nowrap;
  }
.claim-source-tag.screen,
  .claim-source-tag.both {
    background: var(--primary-accent-alpha);
    color: var(--primary-accent);
  }
.claim-type-tag {
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    background: var(--pill-bg);
    color: var(--text-muted);
    padding: 1px 5px;
    border-radius: 4px;
  }
.verdict-tag {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 999px;
  }
.verdict-tag.contradicted {
    background: rgba(225, 29, 72, 0.15);
    color: var(--verdict-contradicted);
  }
.verdict-tag.supported {
    background: rgba(16, 185, 129, 0.15);
    color: var(--verdict-supported);
  }
.verdict-tag.unverifiable,
  .verdict-tag.already_checked {
    background: rgba(217, 119, 6, 0.15);
    color: var(--verdict-unverifiable);
  }
.btn-shortcut {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 4px;
    background: var(--primary-accent-alpha);
    color: var(--primary-accent);
    border: 1px solid rgba(37, 99, 235, 0.25);
    cursor: pointer;
  }
.btn-shortcut:hover {
    background: var(--primary-accent);
    color: #fff;
  }
.claim-text {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-heading);
    margin-bottom: 0.55rem;
    line-height: 1.45;
  }
.attribution-box {
    background: var(--card-bg);
    border-left: 3px solid var(--primary-accent);
    padding: 0.45rem 0.65rem;
    border-radius: 0 4px 4px 0;
    margin-bottom: 0.55rem;
    font-size: 0.8rem;
  }
.attr-row {
    display: flex;
    gap: 0.35rem;
    margin-bottom: 0.2rem;
  }
.attr-label {
    font-weight: 700;
    color: var(--text-muted);
  }
.attr-val {
    color: var(--text-body);
  }
.claim-reasoning {
    font-size: 0.81rem;
    color: var(--text-body);
    margin-bottom: 0.7rem;
    line-height: 1.45;
    background: var(--card-bg);
    padding: 0.6rem 0.75rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    border-left: 3px solid var(--border-color);
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
.claim-reasoning.reasoning-verdict-supported {
    border-left-color: #10b981;
    border-color: rgba(16, 185, 129, 0.28);
    background: rgba(16, 185, 129, 0.04);
  }
.claim-reasoning.reasoning-verdict-contradicted {
    border-left-color: #ef4444;
    border-color: rgba(239, 68, 68, 0.3);
    background: rgba(239, 68, 68, 0.045);
  }
.claim-reasoning.reasoning-verdict-unverifiable {
    border-left-color: #eab308;
    border-color: rgba(234, 179, 8, 0.3);
    background: rgba(234, 179, 8, 0.04);
  }
.reasoning-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding-bottom: 0.3rem;
    border-bottom: 1px solid rgba(148, 163, 184, 0.16);
  }
.reasoning-title {
    font-size: 0.78rem;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    color: var(--text-heading);
  }
.reasoning-count-pill {
    font-size: 0.68rem;
    font-weight: 700;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--pill-bg);
    color: var(--text-muted);
    border: 1px solid var(--border-color);
  }
.reasoning-statements-list {
    display: flex;
    flex-direction: column;
    gap: 0.42rem;
  }
.reasoning-stmt-card {
    padding: 0.45rem 0.6rem;
    border-radius: 5px;
    border: 1px solid var(--border-color);
    border-left: 3px solid var(--border-color);
    background: var(--surface-bg);
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
.reasoning-stmt-card.stmt-supported {
    border-left-color: #10b981;
    border-color: rgba(16, 185, 129, 0.32);
    background: rgba(16, 185, 129, 0.075);
  }
.reasoning-stmt-card.stmt-contradicted {
    border-left-color: #ef4444;
    border-color: rgba(239, 68, 68, 0.35);
    background: rgba(239, 68, 68, 0.08);
  }
.reasoning-stmt-card.stmt-unverifiable {
    border-left-color: #eab308;
    border-color: rgba(234, 179, 8, 0.35);
    background: rgba(234, 179, 8, 0.075);
  }
.stmt-top-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.35rem;
  }
.stmt-index {
    font-size: 0.68rem;
    font-weight: 800;
    color: var(--text-muted);
  }
.stmt-status-pill {
    font-size: 0.68rem;
    font-weight: 800;
    padding: 1px 6px;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 0.01em;
  }
.stmt-status-pill.supported {
    background: rgba(16, 185, 129, 0.2);
    color: #10b981;
  }
.stmt-status-pill.contradicted {
    background: rgba(239, 68, 68, 0.2);
    color: #ef4444;
  }
.stmt-status-pill.unverifiable {
    background: rgba(234, 179, 8, 0.22);
    color: #d97706;
  }
.stmt-subject {
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--text-heading);
  }
.stmt-refs {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    margin-left: auto;
  }
.stmt-ref-btn {
    font-size: 0.68rem;
    font-weight: 700;
    padding: 0 5px;
    border-radius: 3px;
    border: 1px solid rgba(59, 130, 246, 0.35);
    background: rgba(59, 130, 246, 0.12);
    color: var(--primary-accent);
    cursor: pointer;
  }
.stmt-ref-btn:hover {
    background: var(--primary-accent);
    color: #fff;
  }
.stmt-ref-tag {
    font-size: 0.68rem;
    font-weight: 700;
    color: var(--text-muted);
  }
.stmt-detail {
    font-size: 0.79rem;
    color: var(--text-body);
    line-height: 1.42;
  }
.reasoning-summary-footer {
    padding: 0.5rem 0.65rem;
    border-radius: 5px;
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-top: 2px solid var(--border-color);
    font-size: 0.81rem;
    line-height: 1.45;
  }
.reasoning-summary-footer.summary-supported {
    border-top-color: #10b981;
    background: rgba(16, 185, 129, 0.09);
  }
.reasoning-summary-footer.summary-contradicted {
    border-top-color: #ef4444;
    background: rgba(239, 68, 68, 0.095);
  }
.reasoning-summary-footer.summary-unverifiable {
    border-top-color: #eab308;
    background: rgba(234, 179, 8, 0.09);
  }
.reasoning-extra-notes {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.76rem;
    color: var(--text-muted);
  }
.reasoning-extra-note {
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    background: rgba(148, 163, 184, 0.08);
    border: 1px dashed var(--border-color);
  }
.claim-feedback-bar {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0.5rem 0;
    padding: 0.35rem 0.6rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-color, rgba(255, 255, 255, 0.1));
    border-radius: 6px;
    font-size: 0.8rem;
    flex-wrap: wrap;
  }
.feedback-hint {
    color: var(--text-secondary, #94a3b8);
    font-weight: 500;
  }
.btn-feedback {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 4px;
    padding: 2px 7px;
    font-size: 0.76rem;
    cursor: pointer;
    color: inherit;
    transition: all 0.15s ease;
  }
.btn-feedback:hover {
    background: rgba(255, 255, 255, 0.08);
  }
.btn-feedback.selected.correct {
    background: rgba(16, 185, 129, 0.2);
    border-color: #10b981;
    color: #34d399;
  }
.btn-feedback.selected.wrong {
    background: rgba(239, 68, 68, 0.2);
    border-color: #ef4444;
    color: #f87171;
  }
.btn-feedback.selected.unrelated {
    background: rgba(245, 158, 11, 0.2);
    border-color: #f59e0b;
    color: #fbbf24;
  }
.feedback-saved-badge {
    color: #34d399;
    font-size: 0.76rem;
    font-weight: 600;
    margin-left: 0.2rem;
  }
.feedback-verdict-picker {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin: 0.3rem 0 0.5rem 0.5rem;
    font-size: 0.78rem;
  }
.creator-box {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    font-size: 0.85rem;
    background: var(--surface-bg);
    padding: 0.75rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    margin-bottom: 0.75rem;
  }
.creator-main-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
.creator-name-link {
    font-weight: 800;
    font-size: 0.95rem;
    color: var(--primary-accent);
    text-decoration: none;
  }
.creator-name-link:hover {
    text-decoration: underline;
  }
.creator-sub {
    color: var(--text-muted);
    font-size: 0.8rem;
    margin-left: 0.3rem;
  }
.creator-stat-row {
    font-size: 0.78rem;
    color: var(--text-muted);
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
.empty-hint {
    color: var(--text-muted);
    font-size: 0.8rem;
    margin: 0;
  }
.creator-empty-box {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 0.25rem;
  }
.creator-empty-actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
.claim-redact-box { display: flex; flex-direction: column; gap: 6px; margin: 8px 0; }
.claim-redact-box textarea { width: 100%; padding: 6px; border-radius: 6px; background: var(--bg-elevated, #1e1e1e); color: inherit; border: 1px solid #444; }
</style>