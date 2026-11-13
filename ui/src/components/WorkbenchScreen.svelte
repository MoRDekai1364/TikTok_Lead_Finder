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
    openExternalUrl,
    deleteVideo,
    fetchDeepfakeResult,
    triggerAegisScan,
    logDecision,
    redactClaim,
    submitClaimFeedback
  } from '../lib/api';
  import type { LeadCard, Person, Creator, VideoDetail, AegisDeepfakeResult } from '../lib/types';
  import type { Language } from '../lib/i18n';
  import WbSpread from './WbSpread.svelte';
  import WbEvidence from './WbEvidence.svelte';
  import WbReport from './WbReport.svelte';
  import WbTaxonomyModal from './WbTaxonomyModal.svelte';
  import PersonDossierModal from './PersonDossierModal.svelte';

  let {
    currentLang = 'en',
    initialCards = [],
    onOpenSubject,
    onKeep,
    onDiscard,
    onResearchMore,
    onDeleteVideo
  }: {
    currentLang?: Language;
    initialCards?: LeadCard[];
    onOpenSubject?: (p: Person) => void;
    onKeep?: (id: string) => Promise<void>;
    onDiscard?: (id: string) => Promise<void>;
    onResearchMore?: (id: string) => Promise<void>;
    onDeleteVideo?: (id: string) => Promise<void>;
  } = $props();

  let realCards = $state<LeadCard[]>(initialCards && initialCards.length > 0 ? initialCards : []);
  let realPeople = $state<Person[]>([]);
  let realCreators = $state<Creator[]>([]);
  let currentDetail = $state<VideoDetail | null>(null);
  let currentCase = $state<Case>(cases[0]);
  let runningTasksCount = $state(0);
  let pasteModalOpen = $state(false);
  let pasteUrls = $state('');
  let isLoading = $state(false);

  let aegisResult = $state<AegisDeepfakeResult | null>(null);
  let isAegisScanning = $state(false);
  let selectedDossierPerson = $state<Person | null>(null);

  let editingClaimIdx = $state<number | null>(null);
  let editingClaimText = $state('');
  let feedbackMap = $state<Record<number, string>>({});

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

    const title = v.caption ? (v.caption.slice(0, 80) + (v.caption.length > 80 ? '…' : '')) : (topClaim?.text_bg || `Видео #${v.id}`);

    return {
      id: v.id,
      title,
      creator: v.creator_handle ? `@${v.creator_handle}` : 'Неизвестен автор',
      views: v.views.toLocaleString('bg-BG'),
      duration: durationSec,
      platforms: {
        TikTok: 1,
        Facebook: 0,
        Telegram: 0,
        YouTube: 0,
        Instagram: 0,
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
    if (realCards.length === 0) {
      isLoading = true;
    }
    try {
      const [cards, ppl, creats, q] = await Promise.all([
        fetchVideos('all', 2000).catch(() => fetchVideos('leads', 2000)).catch(() => []),
        fetchPeople().catch(() => []),
        fetchCreators().catch(() => []),
        fetchQueueStatus().catch(() => null)
      ]);
      if (cards && cards.length > 0) {
        realCards = cards;
      } else if (initialCards && initialCards.length > 0 && realCards.length === 0) {
        realCards = initialCards;
      }
      if (ppl) realPeople = ppl;
      if (creats) realCreators = creats;
      if (q) runningTasksCount = (q.running || 0) + (q.queued || 0);

      if (realCards.length > 0 && (!currentDetail || !currentDetail.video?.id)) {
        openRealVideo(realCards[0].id, false);
      }
    } catch {} finally {
      isLoading = false;
    }
  }

  onMount(() => {
    initDesktopConnection().then(() => {
      loadAllData();
      subscribeSse((event: any) => {
        if (event && (event.type === 'queue_update' || event.type === 'job_progress' || event.type === 'video_updated')) {
          fetchQueueStatus().then((q) => {
            if (q) runningTasksCount = (q.running || 0) + (q.queued || 0);
          }).catch(() => {});
          fetchVideos('all', 2000).then((vids) => {
            if (vids && vids.length > 0) realCards = vids;
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
  let history = $state<string[]>([currentLang === 'bg' ? 'Системата е готова' : 'System ready']);

  let leftColWidth = $state(
    typeof localStorage !== 'undefined'
      ? Math.max(180, Math.min(520, Number(localStorage.getItem('leadfinder_left_col_w') || 260)))
      : 260
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
      const [detail, aegis] = await Promise.all([
        fetchVideoDetail(id),
        fetchDeepfakeResult(id).catch(() => null)
      ]);
      currentDetail = detail;
      aegisResult = aegis;
      currentCase = videoDetailToCase(detail, taxonomy);
      loadCaseFromCase(currentCase, detail);
      history = [`${currentLang === 'bg' ? 'Отворен казус' : 'Loaded case'} #${id} (${currentCase.creator})`, ...history];
    } catch {
      const fallbackCard = realCards.find((c) => c.id === id);
      if (fallbackCard) {
        history = [`${currentLang === 'bg' ? 'Заредено резюме на видео' : 'Loaded summary for'} #${id}`, ...history];
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
      pushLog(`${currentLang === 'bg' ? 'Внесени за анализ' : 'Queued for analysis'} ${urls.length} TikTok ${currentLang === 'bg' ? 'връзки' : 'links'}`);
      loadAllData();
    }
  }

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

  async function runTool(name: string) {
    tool = name;
    history = [`Стартиран инструмент: ${name}`, ...history];
    if (name === 'Deepfake анализ' && cur?.id) {
      await handleTriggerAegis();
    } else if (name === 'Намери източници' && cur?.id) {
      await handleResearchCurrent();
    } else if (name === 'Провери лице') {
      if (currentDetail?.people && currentDetail.people.length > 0) {
        handleOpenPerson(currentDetail.people[0]);
      } else if (entities.length > 0) {
        handleOpenPerson({ id: entities[0].id, canonical_name: entities[0].name, entity_type: entities[0].type } as Person);
      }
    }
  }

  function seek(sec: number) {
    playhead = sec;
    if (videoElement) {
      videoElement.currentTime = sec;
    }
  }

  async function handleKeepCurrent() {
    if (!cur?.id) return;
    const vid = String(cur.id);
    if (onKeep) {
      await onKeep(vid);
    } else {
      await logDecision(vid, 'keep');
    }
    pushLog(`✓ ${currentLang === 'bg' ? 'Запазен лийд' : 'Saved lead'} #${vid}`);
    advanceToNextVideo(vid);
  }

  async function handleDiscardCurrent() {
    if (!cur?.id) return;
    const vid = String(cur.id);
    if (onDiscard) {
      await onDiscard(vid);
    } else {
      await logDecision(vid, 'discard');
    }
    pushLog(`✕ ${currentLang === 'bg' ? 'Отхвърлено видео' : 'Discarded video'} #${vid}`);
    realCards = realCards.filter((c) => c.id !== vid);
    advanceToNextVideo(vid);
  }

  async function handleResearchCurrent() {
    if (!cur?.id) return;
    const vid = String(cur.id);
    if (onResearchMore) {
      await onResearchMore(vid);
    } else {
      await logDecision(vid, 'research_more');
    }
    pushLog(`⚡ ${currentLang === 'bg' ? 'Стартирано дълбоко проучване за' : 'Started deep research for'} #${vid}`);
  }

  async function handleDeleteCurrent() {
    if (!cur?.id) return;
    const vid = String(cur.id);
    if (onDeleteVideo) {
      await onDeleteVideo(vid);
    } else {
      await deleteVideo(vid);
    }
    pushLog(`🗑️ ${currentLang === 'bg' ? 'Изтрито видео' : 'Deleted video'} #${vid}`);
    realCards = realCards.filter((c) => c.id !== vid);
    advanceToNextVideo(vid);
  }

  async function handleTriggerAegis() {
    if (!cur?.id) return;
    const vid = String(cur.id);
    isAegisScanning = true;
    pushLog(`🛡️ ${currentLang === 'bg' ? 'Стартиран AEGIS Deepfake анализ...' : 'Running AEGIS scan...'}`);
    try {
      const res = await triggerAegisScan(vid);
      aegisResult = res;
      pushLog(`🛡️ AEGIS: ${res.verdict || 'OK'} (${Math.round((res.audio_score || 0) * 100)}% audio / ${Math.round((res.video_score || 0) * 100)}% video)`);
    } catch {
      pushLog(`⚠️ AEGIS: ${currentLang === 'bg' ? 'Грешка при сканиране' : 'Scan failed'}`);
    } finally {
      isAegisScanning = false;
    }
  }

  function advanceToNextVideo(removedId: string) {
    const remaining = realCards.filter((c) => c.id !== removedId);
    if (remaining.length > 0) {
      openRealVideo(remaining[0].id, false);
    } else {
      view = 'library';
    }
  }

  function handleOpenPerson(p: Person) {
    if (onOpenSubject) {
      onOpenSubject(p);
    } else {
      selectedDossierPerson = p;
    }
  }

  function handleSelectCreator(c: Creator) {
    searchQuery = c.handle;
    libTab = 'Videos';
  }

  function startEditClaim(idx: number, text: string) {
    editingClaimIdx = idx;
    editingClaimText = text;
  }

  async function saveEditClaim(idx: number) {
    if (!currentDetail || !currentDetail.claims[idx]) return;
    const cObj = currentDetail.claims[idx].claim;
    const vid = currentDetail.video.id;
    try {
      await redactClaim(cObj.id, vid, editingClaimText);
      pushLog(`✏️ ${currentLang === 'bg' ? 'Коригирано твърдение' : 'Edited claim'} Т${idx + 1}`);
      editingClaimIdx = null;
      await openRealVideo(vid, false);
    } catch {
      pushLog(`⚠️ ${currentLang === 'bg' ? 'Грешка при редакция на твърдение' : 'Error editing claim'}`);
    }
  }

  async function handleFeedback(claimIdx: number, rating: 'correct' | 'wrong_verdict' | 'unrelated_evidence') {
    if (!currentDetail || !currentDetail.claims[claimIdx]) return;
    const cObj = currentDetail.claims[claimIdx].claim;
    const vid = currentDetail.video.id;
    try {
      await submitClaimFeedback(cObj.id, {
        video_id: vid,
        rating
      });
      feedbackMap = { ...feedbackMap, [cObj.id]: rating };
      pushLog(`👍/👎 ${currentLang === 'bg' ? 'Оценката е записана' : 'Feedback recorded'} (${rating})`);
    } catch {
      pushLog(`⚠️ ${currentLang === 'bg' ? 'Грешка при запис на оценка' : 'Error saving feedback'}`);
    }
  }

  const checklist = $derived([
    { label: currentLang === 'bg' ? 'Твърденията са извлечени' : 'Claims extracted', done: claims.length > 0 },
    { label: currentLang === 'bg' ? 'Оценката е зададена' : 'Rating assigned', done: claims.some((c) => c.v !== 'UNVERIFIABLE') },
    { label: currentLang === 'bg' ? 'Всяко твърдение има източник' : 'All claims sourced', done: claims.length > 0 && claims.every((_, i) => sources.some((x) => x.include && x.claims.includes(i))) },
    { label: currentLang === 'bg' ? 'Публикации намерени' : 'Hits found', done: hits.length > 0 },
    { label: currentLang === 'bg' ? 'Бележки EN' : 'Notes EN', done: !!field('notesEn')?.value },
    { label: currentLang === 'bg' ? 'Бележки BG' : 'Notes BG', done: !!field('notesBg')?.value }
  ]);

  const field = (k: string) => report.TCS.find((b) => b.key === k);
</script>

<div class="wb-screen">
  <div class="wb-subbar">
    <button class:on={view === 'library'} onclick={() => (view = 'library')}>
      📂 {currentLang === 'bg' ? 'Библиотека' : 'Library'}
    </button>
    <button class:on={view === 'case'} onclick={() => (view = 'case')}>
      📋 {currentLang === 'bg' ? 'Казус' : 'Case'}
    </button>
    <button class:on={view === 'sources'} onclick={() => (view = 'sources')}>
      🔗 {currentLang === 'bg' ? 'Източници' : 'Sources'}
    </button>
    <button onclick={() => (taxModalOpen = true)} title="Управление на теми, оценки и типове твърдения">
      🏷️ {currentLang === 'bg' ? 'Таксономия' : 'Taxonomy'}
    </button>
    <span class="grow"></span>
    <label>{currentLang === 'bg' ? 'Асистент:' : 'Assist:'}
      <select bind:value={assist}><option>Quiet</option><option>Suggest</option><option>Draft</option></select>
    </label>
    <span class="pill">{runningTasksCount > 0 ? `${runningTasksCount} ${currentLang === 'bg' ? 'фонови задачи' : 'active tasks'}` : `${realCards.length} ${currentLang === 'bg' ? 'видеа' : 'videos'}`}</span>
  </div>

  <div class="wb-body">
    {#if view === 'library'}
      <main class="lib">
        <div class="tabs">
          {#each ['Videos', 'People & Orgs', 'Creators', 'Sources', 'Категории'] as t}
            <button class:on={libTab === t} onclick={() => (libTab = t as typeof libTab)}>
              {t === 'Videos' ? (currentLang === 'bg' ? 'Видеа' : 'Videos')
                : t === 'People & Orgs' ? (currentLang === 'bg' ? 'Личности и организации' : 'People & Orgs')
                : t === 'Creators' ? (currentLang === 'bg' ? 'Автори' : 'Creators')
                : t === 'Sources' ? (currentLang === 'bg' ? 'Източници' : 'Sources')
                : (currentLang === 'bg' ? 'Категории' : 'Taxonomy')}
              {#if t === 'Videos'} ({realCards.length})
              {:else if t === 'People & Orgs'} ({realPeople.length})
              {:else if t === 'Creators'} ({realCreators.length})
              {/if}
            </button>
          {/each}
          <span class="grow"></span>
          <button class="ghost" onclick={() => loadAllData()} title="Презареди видеата от базата данни">🔄 {currentLang === 'bg' ? 'Обнови' : 'Refresh'}</button>
          <button class="primary" onclick={() => (pasteModalOpen = true)}>+ {currentLang === 'bg' ? 'Внеси TikTok URL' : 'Paste TikTok URL'}</button>
        </div>

        <div class="filters">
          <input placeholder={currentLang === 'bg' ? 'Търси по автор, твърдение или ID…' : 'Search by handle, claim or ID…'} bind:value={searchQuery} />
          <select bind:value={selectedLane}>
            <option value="all">{currentLang === 'bg' ? 'Всички ленти' : 'All lanes'} ({realCards.length})</option>
            <option value="leads">{currentLang === 'bg' ? 'Лийдове' : 'Leads'} (leads)</option>
            <option value="pasted">{currentLang === 'bg' ? 'Внесени' : 'Pasted'} (pasted)</option>
            <option value="kept">{currentLang === 'bg' ? 'Запазени' : 'Kept'} (kept)</option>
            <option value="low_quality">{currentLang === 'bg' ? 'Слаб сигнал' : 'Low quality'}</option>
            <option value="screened_out">{currentLang === 'bg' ? 'Филтрирани' : 'Screened out'}</option>
          </select>
          <select bind:value={selectedRating}>
            <option value="All ratings">{currentLang === 'bg' ? 'Всички оценки' : 'All ratings'}</option>
            <option value="Supported">{currentLang === 'bg' ? 'Потвърдени' : 'Supported'}</option>
            <option value="Contradicted">{currentLang === 'bg' ? 'Опровергани' : 'Contradicted'}</option>
            <option value="Unverifiable">{currentLang === 'bg' ? 'Непроверими' : 'Unverifiable'}</option>
          </select>
          <span class="hint" style="margin-left: auto;">{filteredVideos.length} {currentLang === 'bg' ? 'резултата' : 'results'}</span>
        </div>

        {#if libTab === 'Videos'}
          <table>
            <thead>
              <tr>
                <th>{currentLang === 'bg' ? 'Видео / Твърдение' : 'Video / Claim'}</th>
                <th>{currentLang === 'bg' ? 'Автор' : 'Creator'}</th>
                <th>{currentLang === 'bg' ? 'Лента' : 'Lane'}</th>
                <th>{currentLang === 'bg' ? 'Оценка' : 'Verdict'}</th>
                <th>{currentLang === 'bg' ? 'Гледания' : 'Views'}</th>
                <th>Score</th>
                <th>{currentLang === 'bg' ? 'Действия' : 'Actions'}</th>
              </tr>
            </thead>
            <tbody>
              {#each filteredVideos as v}
                <tr onclick={() => openRealVideo(v.id)}>
                  <td>
                    <div style="font-weight: 600;">{v.top_claim_text || v.reason_flagged || v.id}</div>
                    <div class="hint" style="font-size: 11px;">ID: {v.id} {v.duration_s ? `• ${Math.round(v.duration_s)}s` : ''}</div>
                  </td>
                  <td><span class="tag">@{v.creator_handle || 'user'}</span></td>
                  <td><span class="pill" style="font-size: 11px;">{v.lane}</span></td>
                  <td>
                    {#if v.top_claim_verdict === 'SUPPORTED'}
                      <span class="chip ok">✓ {currentLang === 'bg' ? 'Потвърдено' : 'Supported'}</span>
                    {:else if v.top_claim_verdict === 'CONTRADICTED'}
                      <span class="chip bad">✕ {currentLang === 'bg' ? 'Опровергано' : 'Contradicted'}</span>
                    {:else}
                      <span class="chip warn">? {v.top_claim_verdict || (currentLang === 'bg' ? 'Непроверено' : 'Unchecked')}</span>
                    {/if}
                  </td>
                  <td><b>{v.views.toLocaleString('bg-BG')}</b></td>
                  <td><span class="tag">{Math.round(v.lead_score)}</span></td>
                  <td onclick={(e) => e.stopPropagation()}>
                    <div style="display: flex; gap: 4px;">
                      <button class="ghost btn-sm" onclick={() => { if (onKeep) onKeep(v.id); else logDecision(v.id, 'keep'); pushLog(`✓ Keep #${v.id}`); }} title="Запази лийд">✓</button>
                      <button class="ghost btn-sm" onclick={() => { if (onDiscard) onDiscard(v.id); else logDecision(v.id, 'discard'); realCards = realCards.filter(c => c.id !== v.id); pushLog(`✕ Discard #${v.id}`); }} title="Отхвърли">✕</button>
                      <button class="ghost btn-sm" onclick={() => { if (onResearchMore) onResearchMore(v.id); else logDecision(v.id, 'research_more'); pushLog(`⚡ Research #${v.id}`); }} title="Проучи">⚡</button>
                    </div>
                  </td>
                </tr>
              {/each}
              {#if filteredVideos.length === 0}
                <tr>
                  <td colspan="7" style="text-align: center; padding: 24px;" class="hint">
                    {#if isLoading}
                      {currentLang === 'bg' ? 'Зареждане на видеа от базата данни...' : 'Loading videos from database...'}
                    {:else}
                      {currentLang === 'bg' ? 'Няма намерени видеа по избраните критерии.' : 'No videos found matching the criteria.'}
                      <button class="primary" style="margin-left: 10px;" onclick={() => loadAllData()}>🔄 {currentLang === 'bg' ? 'Презареди' : 'Reload'}</button>
                    {/if}
                  </td>
                </tr>
              {/if}
            </tbody>
          </table>
        {:else if libTab === 'People & Orgs'}
          <table>
            <thead>
              <tr>
                <th>{currentLang === 'bg' ? 'Име' : 'Name'}</th>
                <th>{currentLang === 'bg' ? 'Тип' : 'Type'}</th>
                <th>{currentLang === 'bg' ? 'Роля / Описание' : 'Role / Description'}</th>
                <th>{currentLang === 'bg' ? 'Тагове' : 'Tags'}</th>
                <th>{currentLang === 'bg' ? 'Споменавания' : 'Mentions'}</th>
                <th>{currentLang === 'bg' ? 'Досие' : 'Dossier'}</th>
              </tr>
            </thead>
            <tbody>
              {#each realPeople as p}
                <tr onclick={() => handleOpenPerson(p)}>
                  <td><b>{p.canonical_name}</b></td>
                  <td><span class="tag">{p.entity_type || 'person'}</span></td>
                  <td>{p.role || p.summary || p.role_hint || '-'}</td>
                  <td>{p.tags?.join(', ') || p.category || '-'}</td>
                  <td><span class="pill">{p.mentions || 1}</span></td>
                  <td>
                    <button class="primary btn-sm" onclick={(e) => { e.stopPropagation(); handleOpenPerson(p); }}>
                      📖 {currentLang === 'bg' ? 'Отвори досие' : 'Open Dossier'}
                    </button>
                  </td>
                </tr>
              {/each}
              {#if realPeople.length === 0}
                <tr><td colspan="6" style="text-align: center; padding: 24px;" class="hint">{currentLang === 'bg' ? 'Няма открити субекти в базата.' : 'No entities found in database.'}</td></tr>
              {/if}
            </tbody>
          </table>
        {:else if libTab === 'Creators'}
          <table>
            <thead>
              <tr>
                <th>{currentLang === 'bg' ? 'Автор (@handle)' : 'Creator (@handle)'}</th>
                <th>{currentLang === 'bg' ? 'Име' : 'Name'}</th>
                <th>{currentLang === 'bg' ? 'Последователи' : 'Followers'}</th>
                <th>{currentLang === 'bg' ? 'Флагнати видеа' : 'Flagged Videos'}</th>
                <th>{currentLang === 'bg' ? 'Наблюдение' : 'Watchlist'}</th>
                <th>{currentLang === 'bg' ? 'Действие' : 'Action'}</th>
              </tr>
            </thead>
            <tbody>
              {#each realCreators as c}
                <tr onclick={() => handleSelectCreator(c)}>
                  <td><b>@{c.handle}</b></td>
                  <td>{c.display_name || '-'}</td>
                  <td>{c.follower_count ? c.follower_count.toLocaleString('bg-BG') : '-'}</td>
                  <td><span class="pill">{c.flagged_count}</span></td>
                  <td>{c.watchlist ? '★ Да' : 'Не'}</td>
                  <td>
                    <button class="ghost btn-sm" onclick={(e) => { e.stopPropagation(); handleSelectCreator(c); }}>
                      🔍 {currentLang === 'bg' ? 'Филтрирай видеа' : 'Filter videos'}
                    </button>
                  </td>
                </tr>
              {/each}
              {#if realCreators.length === 0}
                <tr><td colspan="6" style="text-align: center; padding: 24px;" class="hint">{currentLang === 'bg' ? 'Няма открити автори в базата.' : 'No creators found in database.'}</td></tr>
              {/if}
            </tbody>
          </table>
        {:else if libTab === 'Категории'}
          <div class="panel" style="max-width: 680px; margin-top: 10px;">
            <h3>🏷️ {currentLang === 'bg' ? 'Активна таксономия' : 'Active Taxonomy'}</h3>
            <p class="hint">{currentLang === 'bg' ? 'Всички теми, оценки, видове и типове твърдения са напълно динамични и се редактират от потребителя.' : 'Themes, ratings, claim types and kinds are fully customizable.'}</p>
            <div class="grid g2" style="margin: 14px 0;">
              <div class="card">
                <b>{currentLang === 'bg' ? 'Теми' : 'Topics'} ({taxonomy.topics.length})</b>
                <div class="hint" style="margin-top: 4px;">{taxonomy.topics.join(', ')}</div>
              </div>
              <div class="card">
                <b>{currentLang === 'bg' ? 'Оценки' : 'Ratings'} ({taxonomy.ratings.length})</b>
                <div class="hint" style="margin-top: 4px;">{taxonomy.ratings.join(', ')}</div>
              </div>
              <div class="card">
                <b>{currentLang === 'bg' ? 'Типове твърдения' : 'Claim Types'} ({taxonomy.claimTypes.length})</b>
                <div class="hint" style="margin-top: 4px;">{taxonomy.claimTypes.join(', ')}</div>
              </div>
              <div class="card">
                <b>{currentLang === 'bg' ? 'Вид на твърдението' : 'Claim Kinds'} ({taxonomy.claimKinds.length})</b>
                <div class="hint" style="margin-top: 4px;">{taxonomy.claimKinds.join(', ')}</div>
              </div>
            </div>
            <button class="primary" onclick={() => (taxModalOpen = true)}>⚙️ {currentLang === 'bg' ? 'Отвори редактора на категориите' : 'Open Taxonomy Editor'}</button>
          </div>
        {:else}
          <div class="panel" style="margin-top: 10px;">
            <h3>🔗 {currentLang === 'bg' ? 'Каталог на източниците' : 'Source Catalog'}</h3>
            {#each sourceCatalog as s}
              <div style="padding: 8px; border-bottom: 1px solid #2b3040; display: flex; align-items: center; gap: 8px;">
                <b>{s.name}</b>
                <span class="tag">{s.type}</span>
                <span class="grow"></span>
                <input type="checkbox" bind:checked={s.on} />
              </div>
            {/each}
          </div>
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
            aria-label="Resize column"
          ></div>
          <div class="crumb"><button class="ghost" onclick={() => (view = 'library')}>‹ {currentLang === 'bg' ? 'Библиотека' : 'Library'}</button></div>
          {#each steps as s, i}
            <button class="step" class:on={step === i} onclick={() => (step = i)}>
              {i + 1}. {s === 'Statements' ? (currentLang === 'bg' ? 'Твърдения' : 'Statements')
                : s === 'Spread' ? (currentLang === 'bg' ? 'Разпространение' : 'Spread')
                : s === 'Evidence' ? (currentLang === 'bg' ? 'Доказателства' : 'Evidence')
                : (currentLang === 'bg' ? 'Доклад' : 'Report')}
            </button>
          {/each}

          <div class="dock">
            <div class="player dockvid">
              {#if currentDetail?.video?.id && currentDetail.video.media_path}
                <video
                  bind:this={videoElement}
                  src={`${getCoreUrl()}/api/media/${encodeURIComponent(currentDetail.video.id)}/video.mp4`}
                  poster={currentDetail.video.thumbnail_url || undefined}
                  bind:currentTime={playhead}
                  bind:paused={videoPaused}
                  controls
                  playsinline
                  class="docked-video-elem"
                ></video>
              {:else if currentDetail?.video?.id && !currentDetail.video.media_path}
                <div class="dock-placeholder">
                  <span style="font-size:11px;opacity:0.5">{currentLang === 'bg' ? 'Видеото не е изтеглено' : 'Video not downloaded'}</span>
                  {#if currentDetail.video.thumbnail_url}
                    <img src={currentDetail.video.thumbnail_url} alt="thumb" style="width:100%;border-radius:4px;margin-top:4px;opacity:0.7" />
                  {/if}
                </div>
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

            <div class="dock-triage-bar" style="display: flex; gap: 4px; margin-top: 6px; flex-wrap: wrap;">
              <button class="primary btn-sm" onclick={handleKeepCurrent} title="Запази лийд (Enter)">✓ {currentLang === 'bg' ? 'Запази' : 'Keep'}</button>
              <button class="ghost btn-sm" onclick={handleDiscardCurrent} title="Отхвърли (Del)">✕ {currentLang === 'bg' ? 'Отхвърли' : 'Discard'}</button>
              <button class="ghost btn-sm" onclick={handleResearchCurrent} title="Проучи още (R)">⚡ {currentLang === 'bg' ? 'Проучи' : 'Research'}</button>
              <button class="ghost btn-sm" onclick={handleTriggerAegis} title="Сканирай за Deepfake" disabled={isAegisScanning}>
                🛡️ {isAegisScanning ? '...' : 'AEGIS'}
              </button>
              <button class="ghost btn-sm" style="color: #ef4444;" onclick={handleDeleteCurrent} title="Изтрий от базата">🗑️</button>
            </div>

            {#if aegisResult}
              <div class="aegis-chip-badge" style="margin-top: 6px; font-size: 11px; padding: 4px 6px; border-radius: 4px; background: rgba(59, 130, 246, 0.15); border: 1px solid rgba(59, 130, 246, 0.3);">
                🛡️ <b>AEGIS:</b> {aegisResult.verdict || 'OK'}
                {#if aegisResult.audio_score !== undefined}
                  <span>({Math.round((aegisResult.audio_score || 0) * 100)}% audio / {Math.round((aegisResult.video_score || 0) * 100)}% video)</span>
                {/if}
              </div>
            {/if}

            <div class="hint" style="margin-top: 6px;">{cur.creator} • {cur.views} гледания • {cur.duration}s</div>

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

          <h4>{currentLang === 'bg' ? 'Контролен списък' : 'Checklist'}</h4>
          {#each checklist as c}<div class="chk" class:done={c.done}>{c.done ? '✓' : '○'} {c.label}</div>{/each}
          <h4>{currentLang === 'bg' ? 'История' : 'History'}</h4>
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
                </div>
              {/if}
            </div>
            <button onclick={() => (step = 3)}>Експорт ▾</button>
          </div>

          {#if step === 0}
            <div class="two">
              <div class="section-card transcript-section">
                <div class="section-header">
                  <div>
                    <h3>{currentLang === 'bg' ? 'Стенограма и OCR' : 'Transcript & OCR'}</h3>
                    <span class="section-subhint">{currentLang === 'bg' ? 'Синхронизиран текст от видеото' : 'Synced text from video'}</span>
                  </div>
                  <button class="btn btn-outline btn-sm" onclick={() => (showOcr = !showOcr)}>
                    {showOcr ? 'Скрий OCR' : 'Покажи OCR'}
                  </button>
                </div>

                {#if showOcr && cur.ocr && cur.ocr.length > 0}
                  <div class="ocr-box" style="margin-bottom: 12px; padding: 8px; border-radius: 6px; background: rgba(255, 255, 255, 0.03); border: 1px solid #2b3040;">
                    <div style="font-size: 11px; font-weight: 600; color: #94a3b8; margin-bottom: 4px;">🖼️ OCR Текстове от екрана:</div>
                    {#each cur.ocr as o}
                      <span class="tag" style="margin: 2px;" onclick={() => seek(Number(o.t.split(':')[0]) * 60 + Number(o.t.split(':')[1]))} role="button" tabindex="0">
                        [{o.t}] {o.text}
                      </span>
                    {/each}
                  </div>
                {/if}

                <div class="transcript-flow">
                  {#each segments as sg, idx}
                    <span
                      class="flow-segment"
                      class:segment-active={Math.abs(playhead - sg.sec) <= 3}
                      onclick={() => seek(sg.sec)}
                      role="button"
                      tabindex="0"
                      title={`[${sg.t}] Кликнете за превъртане`}
                    >
                      <span class="sg-time">{sg.t}</span>
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
                    <h3>{currentLang === 'bg' ? 'Твърдения и преценка' : 'Claims & Assessment'}</h3>
                    <span class="section-subhint">{currentLang === 'bg' ? 'Проверка в български и международни източници' : 'Fact-check and verification'}</span>
                  </div>
                </div>

                {#each claims as c, i}
                  <div class="claim-block claim-verdict-{vk(c.v)}" class:claim-pulse={activeClaim === i} onclick={() => (activeClaim = i)} role="button" tabindex="0">
                    <div class="claim-header">
                      <span class="claim-index-pill">Т{i + 1}</span>
                      <select
                        class="claim-type-select"
                        bind:value={c.type}
                        onclick={(e) => e.stopPropagation()}
                        title="Промени типа на твърдението"
                      >
                        {#each taxonomy.claimTypes as ct}
                          <option value={ct}>{ct}</option>
                        {/each}
                        <option value="__manage__">+ Управление...</option>
                      </select>
                      <span class="claim-source-tag speech">🎙️ Реч</span>
                      <span class="verdict-tag {vk(c.v)}">{verdictBg[c.v]}</span>
                      <button class="btn btn-shortcut btn-sm" onclick={(e) => { e.stopPropagation(); seek(Number(c.at.split('s')[0])); }}>📍 Към цитата</button>
                      <button class="btn btn-text btn-sm" onclick={(e) => { e.stopPropagation(); seek(Number(c.at.split('s')[0])); }}>⏱ {c.at}</button>
                      {#if c.evidence.length}
                        <button class="btn btn-shortcut btn-sm" onclick={(e) => { e.stopPropagation(); step = 2; }}>🎯 Към доказателства ({c.evidence.length})</button>
                      {/if}
                    </div>

                    {#if editingClaimIdx === i}
                      <div class="claim-edit-panel" style="margin: 8px 0;" onclick={(e) => e.stopPropagation()}>
                        <textarea bind:value={editingClaimText} style="width: 100%; min-height: 70px; padding: 6px;"></textarea>
                        <div style="display: flex; gap: 6px; margin-top: 4px;">
                          <button class="primary btn-sm" onclick={() => saveEditClaim(i)}>💾 Запази и провери</button>
                          <button class="ghost btn-sm" onclick={() => (editingClaimIdx = null)}>Отказ</button>
                        </div>
                      </div>
                    {:else}
                      <div class="claim-text">{c.text}</div>
                    {/if}

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
                          </div>
                        {/each}
                      </div>
                      <div class="reasoning-summary-footer summary-{vk(c.v)}">
                        <span class="summary-badge">📌 Обобщение:</span>
                        <span class="summary-text">{c.summary}</span>
                      </div>
                    </div>

                    <div class="claim-actions-footer" style="display: flex; gap: 8px; align-items: center; margin-top: 8px; flex-wrap: wrap;" onclick={(e) => e.stopPropagation()}>
                      <button class="btn-feedback" onclick={() => startEditClaim(i, c.text)}>✏️ Редактирай твърдението</button>
                      <span class="feedback-hint">Оценка на резултата:</span>
                      <button class="btn-feedback correct" class:active={currentDetail?.claims[i] && feedbackMap[currentDetail.claims[i].claim.id] === 'correct'} onclick={() => handleFeedback(i, 'correct')}>👍 Вярна преценка</button>
                      <button class="btn-feedback wrong" class:active={currentDetail?.claims[i] && feedbackMap[currentDetail.claims[i].claim.id] === 'wrong_verdict'} onclick={() => handleFeedback(i, 'wrong_verdict')}>👎 Грешна преценка</button>
                      <button class="btn-feedback" class:active={currentDetail?.claims[i] && feedbackMap[currentDetail.claims[i].claim.id] === 'unrelated_evidence'} onclick={() => handleFeedback(i, 'unrelated_evidence')}>⚠️ Несвързано</button>
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
        </aside>
      </div>
    {/if}

    {#if view === 'sources'}
      <main class="lib">
        <h2>{currentLang === 'bg' ? 'Каталог на надеждни източници' : 'Source Library'}</h2>
        <div class="grid g2" style="margin-top: 10px;">
          {#each sourceCatalog as s}
            <div class="card" style="display: flex; align-items: center; gap: 8px;">
              <b>{s.name}</b>
              <span class="tag">{s.type}</span>
              <span class="grow"></span>
              <input type="checkbox" bind:checked={s.on} />
            </div>
          {/each}
        </div>
      </main>
    {/if}
  </div>
</div>

<WbTaxonomyModal bind:open={taxModalOpen} bind:taxonomy />

{#if pasteModalOpen}
  <div class="tax-backdrop" onclick={() => (pasteModalOpen = false)} role="presentation">
    <div class="tax-dialog" style="max-width: 520px;" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <div class="tax-header">
        <div>
          <h2 style="margin: 0; font-size: 16px;">Внеси TikTok URL адреси</h2>
          <span class="hint">Поставете връзки от TikTok за анализ</span>
        </div>
        <button class="ghost" onclick={() => (pasteModalOpen = false)}>✕</button>
      </div>
      <div class="tax-body" style="padding: 14px;">
        <textarea bind:value={pasteUrls} placeholder="https://www.tiktok.com/@user/video/..." style="width: 100%; min-height: 120px;"></textarea>
      </div>
      <div class="tax-footer" style="padding: 10px 14px; display: flex; justify-content: flex-end; gap: 8px;">
        <button class="ghost" onclick={() => (pasteModalOpen = false)}>Отказ</button>
        <button class="primary" onclick={submitPaste}>Добави</button>
      </div>
    </div>
  </div>
{/if}

{#if selectedDossierPerson}
  <PersonDossierModal
    person={selectedDossierPerson}
    {currentLang}
    onClose={() => (selectedDossierPerson = null)}
  />
{/if}

<style>
  .wb-screen { display: flex; flex-direction: column; height: calc(100vh - 90px); overflow: hidden; font: 13px system-ui, sans-serif; background: #12141a; color: #dde1ea; }
  .wb-subbar { display: flex; gap: 8px; align-items: center; padding: 7px 14px; background: #1a1d26; border-bottom: 1px solid #2b3040; flex-shrink: 0; flex-wrap: wrap; }
  .wb-body { flex: 1; overflow: hidden; display: flex; flex-direction: column; min-height: 0; }
  .case { display: grid; height: 100%; min-height: 0; }
  .grow { flex: 1; }
  button { background: #262b3a; color: inherit; border: 1px solid #353b50; padding: 5px 10px; border-radius: 6px; cursor: pointer; }
  button.on { background: #3b4fa0; }
  button.primary { background: #3b6fe0; border-color: #3b6fe0; }
  button.ghost { background: transparent; }
  .btn-sm { padding: 2px 7px; font-size: 11px; }
  input, select, textarea { background: #0e1016; color: inherit; border: 1px solid #353b50; border-radius: 6px; padding: 5px; }
  textarea { width: 100%; min-height: 110px; box-sizing: border-box; }
  .pill { background: #2b3a66; padding: 3px 8px; border-radius: 12px; }
  .lib { padding: 14px; overflow-y: auto; }
  .tabs, .filters { display: flex; gap: 8px; align-items: center; margin-bottom: 10px; flex-wrap: wrap; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 8px; border-bottom: 1px solid #2b3040; }
  tbody tr { cursor: pointer; }
  tbody tr:hover { background: #1c2030; }
  .tag { background: #262b3a; border-radius: 4px; padding: 1px 6px; margin-right: 4px; font-size: 11px; }
  .claim-type-select { background: #262b3a; color: #dde1ea; border: 1px solid #353b50; border-radius: 4px; padding: 2px 6px; font-size: 11px; cursor: pointer; }
  .chip { padding: 2px 8px; border-radius: 10px; }
  .ok { background: #1f5a3a; } .bad { background: #6a2530; } .warn { background: #6a5520; }
  .hint { color: #8b93a8; font-size: 12px; }
  .stepper { position: relative; border-right: 1px solid #2b3040; display: flex; flex-direction: column; gap: 6px; padding: 12px; background: #161922; overflow-y: auto; box-sizing: border-box; }
  .stepper-resizer { position: absolute; top: 0; right: -4px; bottom: 0; width: 8px; cursor: col-resize; z-index: 20; transition: background 0.15s ease; }
  .stepper-resizer:hover, .stepper-resizer.active { background: #3b6fe0; }
  .dock { position: sticky; top: 0; background: #0e1016; border: 1px solid #2b3040; border-radius: 10px; padding: 8px; margin-bottom: 8px; }
  .dockvid { height: 210px; position: relative; margin-bottom: 6px; overflow: hidden; background: #000; border-radius: 6px; }
  .docked-video-elem { width: 100%; height: 100%; object-fit: contain; }
  .dock-placeholder { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 24px; color: #555; position: relative; }
  .dockts { position: absolute; bottom: 6px; right: 8px; font-size: 11px; color: #999; }
  .marks { position: relative; margin-bottom: 6px; }
  .marks input { width: 100%; }
  .marks .mk { position: absolute; top: -4px; width: 6px; height: 14px; border-radius: 2px; cursor: pointer; }
  .crumb { margin-bottom: 6px; }
  .step { text-align: left; padding: 8px 10px; }
  .chk { padding: 4px 6px; border-radius: 4px; font-size: 12px; color: #8b93a8; }
  .chk.done { color: #4ade80; }
  .hist { font-size: 11px; color: #8b93a8; display: flex; flex-direction: column; gap: 3px; max-height: 100px; overflow-y: auto; margin-bottom: 6px; }
  .canvas { padding: 14px; overflow-y: auto; }
  .canvas .title { display: flex; align-items: center; gap: 10px; font-size: 16px; font-weight: bold; margin-bottom: 12px; }
  .menu { position: relative; }
  .dropdown { position: absolute; top: 100%; right: 0; background: #1c2030; border: 1px solid #353b50; border-radius: 6px; padding: 6px; display: flex; flex-direction: column; gap: 4px; z-index: 50; min-width: 180px; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .section-card { background: #161922; border: 1px solid #2b3040; border-radius: 10px; padding: 12px; }
  .section-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
  .section-subhint { font-size: 11px; color: #8b93a8; }
  .transcript-flow { max-height: calc(100vh - 270px); overflow-y: auto; line-height: 1.6; }
  .flow-segment { display: inline; cursor: pointer; border-radius: 3px; padding: 1px 2px; transition: background 0.15s; }
  .flow-segment:hover { background: rgba(59, 130, 246, 0.15); }
  .flow-segment.segment-active { background: rgba(59, 130, 246, 0.35); font-weight: 600; }
  .sg-time { font-size: 10px; color: #64748b; font-family: monospace; margin-right: 2px; }
  .claim-block { background: #12141a; border: 1px solid #2b3040; border-radius: 8px; padding: 10px; margin-bottom: 10px; transition: border-color 0.15s; }
  .claim-block:hover, .claim-block.claim-pulse { border-color: #3b6fe0; }
  .claim-header { display: flex; align-items: center; gap: 6px; margin-bottom: 8px; flex-wrap: wrap; }
  .claim-index-pill { background: #2b3a66; padding: 2px 6px; border-radius: 4px; font-weight: bold; font-size: 11px; }
  .claim-source-tag { font-size: 11px; padding: 2px 6px; border-radius: 4px; background: #262b3a; }
  .verdict-tag { font-size: 11px; font-weight: bold; padding: 2px 6px; border-radius: 4px; text-transform: uppercase; }
  .verdict-tag.supported { background: #1f5a3a; color: #4ade80; }
  .verdict-tag.contradicted { background: #6a2530; color: #f87171; }
  .verdict-tag.unverifiable { background: #6a5520; color: #fbbf24; }
  .claim-text { font-size: 14px; font-weight: 600; margin-bottom: 8px; line-height: 1.4; }
  .claim-reasoning { background: #161922; border-radius: 6px; padding: 8px; margin-bottom: 8px; font-size: 12px; }
  .reasoning-head { display: flex; justify-content: space-between; margin-bottom: 6px; font-weight: 600; color: #94a3b8; }
  .reasoning-stmt-card { background: #0e1016; border-radius: 4px; padding: 6px; margin-bottom: 4px; border-left: 3px solid #3b6fe0; }
  .reasoning-stmt-card.stmt-supported { border-left-color: #4ade80; }
  .reasoning-stmt-card.stmt-contradicted { border-left-color: #f87171; }
  .reasoning-stmt-card.stmt-unverifiable { border-left-color: #fbbf24; }
  .stmt-top-row { display: flex; gap: 6px; align-items: center; margin-bottom: 3px; font-size: 11px; }
  .stmt-status-pill { font-weight: 600; }
  .stmt-detail { color: #cbd5e1; }
  .stmt-ref-tag { background: #1e293b; color: #38bdf8; padding: 1px 4px; border-radius: 3px; font-size: 10px; font-family: monospace; }
  .reasoning-summary-footer { margin-top: 6px; padding-top: 4px; border-top: 1px solid #2b3040; font-style: italic; color: #94a3b8; }
  .btn-feedback { background: #262b3a; border: 1px solid #353b50; padding: 3px 8px; border-radius: 4px; font-size: 11px; cursor: pointer; }
  .btn-feedback:hover, .btn-feedback.active { background: #3b4fa0; border-color: #3b6fe0; }
  .btn-feedback.correct.active { background: #1f5a3a; border-color: #4ade80; }
  .btn-feedback.wrong.active { background: #6a2530; border-color: #f87171; }
  .feedback-hint { font-size: 11px; color: #8b93a8; }
  .rail { width: 290px; border-left: 1px solid #2b3040; padding: 12px; background: #161922; overflow-y: auto; }
  .rail h4 { margin: 0 0 8px 0; font-size: 13px; }
  .st { padding: 6px 8px; border-radius: 6px; margin-bottom: 8px; font-size: 12px; }
  .spread { margin-bottom: 12px; font-size: 12px; }
  .bar { display: flex; align-items: center; gap: 6px; margin: 3px 0; font-size: 11px; }
  .bar i { height: 6px; background: #3b6fe0; border-radius: 3px; display: inline-block; }
  .sug { background: #1c2030; border-radius: 6px; padding: 8px; margin-bottom: 6px; font-size: 12px; }
  .row { display: flex; gap: 6px; align-items: center; margin-top: 4px; }
  .grid { display: grid; gap: 10px; }
  .g2 { grid-template-columns: 1fr 1fr; }
  .card { background: #161922; border: 1px solid #2b3040; border-radius: 6px; padding: 10px; }
  .panel { background: #161922; border: 1px solid #2b3040; border-radius: 8px; padding: 14px; }
  .tax-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 100; }
  .tax-dialog { background: #161922; border: 1px solid #353b50; border-radius: 10px; width: 90%; max-width: 600px; max-height: 85vh; overflow-y: auto; }
  .tax-header { display: flex; justify-content: space-between; align-items: center; padding: 14px; border-bottom: 1px solid #2b3040; }
</style>
