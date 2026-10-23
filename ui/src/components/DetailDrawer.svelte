<script lang="ts">
  import {
    aiCheckTranscript,
    deleteVideo,
    fetchEvidencePreview,
    fetchVideoDetail,
    fetchVideosByCreator,
    getCoreUrl,
    logDecision,
    researchPerson,
    retranscribeVideo,
    setCreatorWatchlist,
    updateTranscript,
  } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { ClaimWithEvidence, LeadCard, Person, VideoDetail } from '../lib/types';

  let {
    videoId,
    currentLang,
    onClose,
    onSelectVideo,
    onDiscard,
    onDeleteVideo,
  }: {
    videoId: string;
    currentLang: Language;
    onClose: () => void;
    onSelectVideo?: (id: string) => void;
    onDiscard?: (id: string) => void;
    onDeleteVideo?: (id: string) => void;
  } = $props();

  const t = $derived(translations[currentLang]);

  let detail = $state<VideoDetail | null>(null);
  let isLoading = $state(true);
  let isEditingTranscript = $state(false);
  let editedText = $state('');
  let isSavingTranscript = $state(false);
  let isAiChecking = $state(false);
  let isRetranscribing = $state(false);
  let copyToast = $state<string | null>(null);
  let videoElement = $state<HTMLVideoElement | null>(null);
  let currentTimeMs = $state(0);
  let metadataOpen = $state(false);
  let activeClaimId = $state<number | null>(null);
  let highlightedSegmentMs = $state<number | null>(null);
  let creatorVideos = $state<LeadCard[]>([]);
  let isLoadingCreatorVideos = $state(false);
  let isTogglingWatchlist = $state(false);
  let previewModalData = $state<{ url: string; title: string; text: string; domain: string } | null>(null);
  let isLoadingPreview = $state(false);
  let isResearchingPersonId = $state<number | null>(null);
  let expandedEvidenceId = $state<number | null>(null);

  const transcriptDisplaySegments = $derived.by(() => {
    if (!detail?.transcript) return [];
    if (detail.transcript.segments && detail.transcript.segments.length > 0) {
      return detail.transcript.segments;
    }
    const full = detail.transcript.edited_text || detail.transcript.text || '';
    if (!full.trim()) return [];
    const sentences = full.split(/(?<=[.!?])\s+/).filter(Boolean);
    const estDuration = (detail.video.duration_s || 60) * 1000;
    const step = Math.max(1000, Math.round(estDuration / Math.max(1, sentences.length)));
    return sentences.map((st, i) => ({
      start_ms: i * step,
      end_ms: (i + 1) * step,
      text: st,
    }));
  });

  let activeLoadId = '';

  $effect(() => {
    const id = videoId;
    if (id) {
      activeLoadId = id;
      loadDetail(id);
    }
  });

  async function loadDetail(id: string) {
    isLoading = true;
    isEditingTranscript = false;
    currentTimeMs = 0;
    creatorVideos = [];
    try {
      const res = await fetchVideoDetail(id);
      if (activeLoadId === id) {
        detail = res;
        if (res?.transcript) {
          editedText = res.transcript.edited_text || res.transcript.text;
        } else {
          editedText = '';
        }
        if (res?.creator?.id) {
          loadCreatorVideos(res.creator.id);
        } else if (res?.video.creator_handle) {
          loadCreatorVideos(res.video.creator_handle);
        }
      }
    } catch {
      if (activeLoadId === id) {
        detail = null;
      }
    } finally {
      if (activeLoadId === id) {
        isLoading = false;
      }
    }
  }

  async function loadCreatorVideos(creatorIdOrHandle: string) {
    isLoadingCreatorVideos = true;
    try {
      const list = await fetchVideosByCreator(creatorIdOrHandle, 8);
      creatorVideos = list.filter((v) => v.id !== videoId);
    } catch {
      creatorVideos = [];
    } finally {
      isLoadingCreatorVideos = false;
    }
  }

  function selectOtherVideo(id: string) {
    if (onSelectVideo) {
      onSelectVideo(id);
    } else {
      activeLoadId = id;
      loadDetail(id);
    }
  }

  function handleTimeUpdate() {
    if (videoElement) {
      currentTimeMs = Math.round(videoElement.currentTime * 1000);
    }
  }

  function seekToMs(ms: number) {
    if (videoElement) {
      videoElement.currentTime = ms / 1000;
      videoElement.play();
    }
  }

  function jumpToClaim(claimId: number) {
    activeClaimId = claimId;
    const el = document.getElementById(`claim-${claimId}`);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
    setTimeout(() => {
      if (activeClaimId === claimId) {
        activeClaimId = null;
      }
    }, 2500);
  }

  function jumpToSegment(startMs?: number | null) {
    if (startMs === undefined || startMs === null) return;
    seekToMs(startMs);
    highlightedSegmentMs = startMs;
    const el = document.getElementById(`seg-${startMs}`);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
    setTimeout(() => {
      if (highlightedSegmentMs === startMs) {
        highlightedSegmentMs = null;
      }
    }, 2500);
  }

  function getSegmentClaimInfo(seg: { start_ms: number; end_ms: number; text?: string }) {
    if (!detail?.claims || detail.claims.length === 0) return null;
    for (let i = 0; i < detail.claims.length; i++) {
      const c = detail.claims[i].claim;
      const s = c.start_ms ?? 0;
      const e = c.end_ms ?? 0;
      const timeMatch = (s > 0 || e > 0) && (
        (seg.start_ms >= s && seg.start_ms < e) ||
        (seg.end_ms > s && seg.end_ms <= e) ||
        (s >= seg.start_ms && e <= seg.end_ms)
      );
      const segText = (seg.text || '').trim().toLowerCase();
      const claimText = (c.text_bg || '').trim().toLowerCase();
      const textMatch = segText.length > 15 && (claimText.includes(segText) || segText.includes(claimText.slice(0, 30)));
      if (timeMatch || textMatch) {
        return {
          claimId: c.id,
          index: i + 1,
          verdict: (c.verdict || '').toLowerCase(),
          text: c.text_bg,
        };
      }
    }
    return null;
  }

  async function handleLoadEvidencePreview(url: string) {
    if (!url) return;
    isLoadingPreview = true;
    try {
      const res = await fetchEvidencePreview(url);
      previewModalData = res;
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при зареждане на източника' : 'Failed to load source';
      setTimeout(() => (copyToast = null), 3000);
    } finally {
      isLoadingPreview = false;
    }
  }

  async function handleResearchPerson(person: Person) {
    isResearchingPersonId = person.id;
    try {
      const updated = await researchPerson(person.id, person.canonical_name);
      if (detail?.people) {
        detail.people = detail.people.map(p => p.id === person.id ? { ...p, ...updated } : p);
      }
      copyToast = currentLang === 'bg' ? `✓ Проучен: ${person.canonical_name}` : `✓ Researched: ${person.canonical_name}`;
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при проучване на лицето' : 'Research error';
    } finally {
      isResearchingPersonId = null;
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  async function handleRemoveThisVideo() {
    if (!detail) return;
    const vid = detail.video.id;
    onClose();
    if (onDiscard) {
      onDiscard(vid);
    } else if (onDeleteVideo) {
      onDeleteVideo(vid);
    } else {
      await deleteVideo(vid);
    }
  }

  async function handleSaveTranscript() {
    if (!detail || isSavingTranscript) return;
    isSavingTranscript = true;
    try {
      detail = await updateTranscript(detail.video.id, editedText, true);
      isEditingTranscript = false;
      copyToast = currentLang === 'bg' ? '✓ Стенограмата е запазена!' : '✓ Transcript saved!';
    } finally {
      isSavingTranscript = false;
      setTimeout(() => (copyToast = null), 2500);
    }
  }

  async function handleCopyTranscript() {
    if (!detail?.transcript) return;
    const txt = detail.transcript.edited_text || detail.transcript.text;
    await navigator.clipboard.writeText(txt);
    copyToast = currentLang === 'bg' ? '✓ Копирано в клипборда!' : '✓ Copied to clipboard!';
    setTimeout(() => (copyToast = null), 2500);
  }

  async function handleWarmStartRedact() {
    if (!detail || isRetranscribing) return;
    isRetranscribing = true;
    copyToast = currentLang === 'bg' ? '⚡ Преработка с речника и AI...' : '⚡ Running warm-start redaction...';
    try {
      const res = await retranscribeVideo(detail.video.id, true, true);
      if (res?.corrected_text && detail.transcript) {
        detail.transcript.edited_text = res.corrected_text;
        editedText = res.corrected_text;
      }
      copyToast = currentLang === 'bg' ? '✓ Преработено с речника и AI!' : '✓ Redacted with lexicon & AI!';
      await loadDetail(detail.video.id);
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при преработка' : 'Redaction error';
    } finally {
      isRetranscribing = false;
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  async function handleFullRetranscribe() {
    if (!detail || isRetranscribing) return;
    isRetranscribing = true;
    copyToast = currentLang === 'bg' ? '🔄 Изпратено за Whisper...' : '🔄 Whisper queued...';
    try {
      await retranscribeVideo(detail.video.id, false, true);
      copyToast = currentLang === 'bg' ? '✓ Whisper задачата е стартирана!' : '✓ Whisper task queued!';
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при Whisper задача' : 'Whisper queue error';
    } finally {
      isRetranscribing = false;
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  async function handleAiCheckAndCopy() {
    if (!detail?.transcript || isAiChecking) return;
    isAiChecking = true;
    copyToast = currentLang === 'bg' ? 'AI проверка на текста...' : 'Running AI spellcheck...';
    try {
      const res = await aiCheckTranscript(detail.video.id);
      if (res && res.corrected_text) {
        detail.transcript.edited_text = res.corrected_text;
        editedText = res.corrected_text;
        await navigator.clipboard.writeText(res.corrected_text);
        copyToast = currentLang === 'bg' ? '✓ Коригирано с AI и копирано!' : '✓ AI corrected & copied!';
      }
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при AI проверка' : 'AI check error';
    } finally {
      isAiChecking = false;
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  async function handleTriggerClaimVerify(claimText: string) {
    if (!detail) return;
    copyToast = currentLang === 'bg' ? '🔍 Проверка в мрежата...' : '🔍 Searching online...';
    try {
      await logDecision(detail.video.id, 'research_more', claimText);
      copyToast = currentLang === 'bg' ? '✓ Изпратено за онлайн проверка!' : '✓ Enqueued for web research!';
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при изпращане' : 'Verification error';
    } finally {
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  async function handleCopyCitation(cwith: ClaimWithEvidence) {
    if (!detail) return;
    const author = detail.creator?.handle ? `@${detail.creator.handle}` : (detail.video.creator_handle ? `@${detail.video.creator_handle}` : 'неизвестен');
    const startS = Math.round((cwith.claim.start_ms || 0) / 1000);
    const endS = Math.round((cwith.claim.end_ms || 0) / 1000);
    const citation = `"${cwith.claim.text_bg}" (Видео ID: ${detail.video.id}, Автор: ${author}, Времеви отрязък: ${startS}s-${endS}s, Присъда: ${cwith.claim.label || cwith.claim.verdict || 'Неверифицирано'})`;
    await navigator.clipboard.writeText(citation);
    copyToast = currentLang === 'bg' ? '✓ Цитатът е копиран!' : '✓ Citation copied!';
    setTimeout(() => (copyToast = null), 2500);
  }

  async function handleToggleWatchlist() {
    if (!detail?.creator || isTogglingWatchlist) return;
    isTogglingWatchlist = true;
    const nextState = !detail.creator.watchlist;
    try {
      await setCreatorWatchlist(detail.creator.handle, nextState);
      detail.creator.watchlist = nextState;
      copyToast = nextState
        ? (currentLang === 'bg' ? '✓ Добавен в наблюдение' : '✓ Added to watchlist')
        : (currentLang === 'bg' ? '✓ Премахнат от наблюдение' : '✓ Removed from watchlist');
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при обновяване' : 'Watchlist update error';
    } finally {
      isTogglingWatchlist = false;
      setTimeout(() => (copyToast = null), 2500);
    }
  }

  function computePersonCredibility(person: Person) {
    const pAny = person as any;
    const dbContradicted = pAny.contradicted_count ?? 0;
    const dbSupported = pAny.supported_count ?? 0;
    const dbTotal = pAny.total_claims ?? 0;

    const nameLower = person.canonical_name.toLowerCase();
    const related = (detail?.claims || []).filter((c) => {
      const sp = (c.claim.speaker || '').toLowerCase();
      const attr = (c.claim.attribution || []).some((a) => (a.actor || '').toLowerCase().includes(nameLower));
      return sp.includes(nameLower) || attr;
    });

    const hasContradicted = related.some((c) => (c.claim.verdict || '').toUpperCase() === 'CONTRADICTED') || dbContradicted > 0;
    const hasSupported = related.some((c) => (c.claim.verdict || '').toUpperCase() === 'SUPPORTED') || dbSupported > 0;

    let score = 65;
    let level = 'neutral';
    let label = 'Споменато лице';
    let levelLabel = 'Неутрален';
    let badgeClass = 'cred-neutral';

    if (hasContradicted) {
      score = Math.max(15, 35 - dbContradicted * 5);
      level = 'high-risk';
      label = '⚠️ Повишен риск от дезинформация';
      levelLabel = 'Нисък рейтинг на достоверност';
      badgeClass = 'cred-low';
    } else if (hasSupported) {
      score = Math.min(95, 80 + dbSupported * 3);
      level = 'supported';
      label = '✅ Висока достоверност';
      levelLabel = 'Потвърдени факти';
      badgeClass = 'cred-high';
    } else if (related.length > 0 || dbTotal > 0) {
      score = 50;
      level = 'unverifiable';
      label = 'ℹ️ Изисква проверка';
      levelLabel = 'Непроверени твърдения';
      badgeClass = 'cred-moderate';
    }

    return {
      score,
      level,
      label,
      levelLabel,
      badgeClass,
      stats: {
        total: related.length + dbTotal,
        contradicted: (related.filter(c => (c.claim.verdict || '').toUpperCase() === 'CONTRADICTED').length) + dbContradicted,
        supported: (related.filter(c => (c.claim.verdict || '').toUpperCase() === 'SUPPORTED').length) + dbSupported,
      }
    };
  }

  function computeAuthorRisk(creator: any) {
    const flagged = creator?.flagged_count ?? 0;
    const hasContradicted = detail?.claims?.some((c) => (c.claim.verdict || '').toUpperCase() === 'CONTRADICTED');
    if (flagged >= 3 || hasContradicted) {
      return { level: 'high', label: '🔴 Висок риск', badgeClass: 'risk-high' };
    }
    if (flagged > 0) {
      return { level: 'moderate', label: '🟡 Умерен риск', badgeClass: 'risk-moderate' };
    }
    return { level: 'low', label: '🟢 Нисък риск', badgeClass: 'risk-low' };
  }

  function stanceClass(stance: string): string {
    switch (stance.toLowerCase()) {
      case 'contradicts':
        return 'stance-contradicts';
      case 'supports':
        return 'stance-supports';
      default:
        return 'stance-neutral';
    }
  }

  function stanceLabel(stance: string): string {
    switch (stance.toLowerCase()) {
      case 'contradicts':
        return t.stanceContradicts;
      case 'supports':
        return t.stanceSupports;
      default:
        return t.stanceNeutral;
    }
  }

  const videoSrc = $derived(
    detail?.video.id ? `${getCoreUrl()}/api/media/${detail.video.id}/video.mp4` : ''
  );
</script>

<aside class="detail-drawer" aria-label={t.drawerTitle}>
  <div class="drawer-header">
    <div class="drawer-title-group">
      <h2 class="drawer-title">{t.drawerTitle}</h2>
      {#if detail}
        <span class="video-ref-id">ID: {detail.video.id}</span>
      {/if}
    </div>
    <div class="drawer-header-actions">
      <button
        class="btn btn-danger btn-sm drawer-remove-btn"
        onclick={handleRemoveThisVideo}
        title={currentLang === 'bg' ? 'Премахни видеото от списъка (✕)' : 'Remove video (✕)'}
      >
        ✕ {currentLang === 'bg' ? 'Премахни видеото' : 'Remove video'}
      </button>
      <button class="close-btn" onclick={onClose} title={t.drawerClose} aria-label={t.drawerClose}>
        ✕
      </button>
    </div>
  </div>

  {#if isLoading}
    <div class="loading-state">
      <div class="spinner" aria-hidden="true"></div>
      <span>{currentLang === 'bg' ? 'Зареждане на детайлите...' : 'Loading video details...'}</span>
    </div>
  {:else if detail}
    <div class="drawer-body">
      <div class="media-section">
        <video
          bind:this={videoElement}
          src={videoSrc}
          controls
          ontimeupdate={handleTimeUpdate}
          class="video-player"
        >
          <track kind="captions" />
        </video>
      </div>

      <div class="section-card transcript-section">
        <div class="section-header">
          <div>
            <h3>{t.transcriptHeading}</h3>
            <span class="section-subhint">
              {currentLang === 'bg'
                ? 'Цветовете отразяват присъдата: Червено = Опровергано, Зелено = Потвърдено, Жълто = Непроверимо'
                : 'Colors indicate verdict: Red = Contradicted, Green = Supported, Yellow = Unverifiable'}
            </span>
          </div>
          <div class="transcript-btn-group">
            {#if copyToast}
              <span class="copy-toast">{copyToast}</span>
            {/if}
            <button
              class="btn btn-secondary btn-sm"
              onclick={handleCopyTranscript}
              title={currentLang === 'bg' ? 'Копирай текста в клипборда' : 'Copy transcript to clipboard'}
            >
              📋 {currentLang === 'bg' ? 'Копирай' : 'Copy'}
            </button>
            <button
              class="btn btn-secondary btn-sm"
              onclick={handleWarmStartRedact}
              disabled={isRetranscribing}
              title={currentLang === 'bg' ? 'Преработка с българския речник и AI без изтриване' : 'Fast lexicon & AI warm-start redaction'}
            >
              {isRetranscribing ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Бърза преработка' : 'Warm-start')}
            </button>
            <button
              class="btn btn-secondary btn-sm"
              onclick={handleFullRetranscribe}
              disabled={isRetranscribing}
              title={currentLang === 'bg' ? 'Пълно повторно разпознаване с Whisper' : 'Full Whisper re-transcription'}
            >
              🔄 Whisper
            </button>
            <button
              class="btn btn-secondary btn-sm"
              onclick={handleAiCheckAndCopy}
              disabled={isAiChecking}
              title={currentLang === 'bg' ? 'AI проверка на думите и копиране' : 'AI spellcheck & copy'}
            >
              {isAiChecking ? '...' : '🤖 AI'}
            </button>
            <a
              href={`https://www.google.com/search?udm=50&q=${encodeURIComponent('what would the fixed text look like?\n' + (editedText || detail.transcript?.text || ''))}`}
              target="_blank"
              rel="noopener noreferrer"
              class="btn btn-secondary btn-sm google-sge-btn"
              title={currentLang === 'bg' ? 'Отвори външен AI Overview инструмент в Google за поправка на стенограмата' : 'Open external Google AI Overview tool for transcription fix'}
            >
              🔎 Google AI Overview ↗
            </a>
            {#if !isEditingTranscript}
              <button class="btn btn-secondary btn-sm" onclick={() => (isEditingTranscript = true)}>
                ✏️ {t.transcriptEditBtn}
              </button>
            {:else}
              <button class="btn btn-primary btn-sm" onclick={handleSaveTranscript} disabled={isSavingTranscript}>
                {isSavingTranscript ? '...' : t.transcriptSaveBtn}
              </button>
            {/if}
          </div>
        </div>

        {#if isEditingTranscript}
          <textarea class="transcript-editor" bind:value={editedText} rows="6"></textarea>
        {:else if detail.transcript}
          <div class="transcript-segments">
            {#each transcriptDisplaySegments as seg}
              {@const cinfo = getSegmentClaimInfo(seg)}
              <span
                id={`seg-${seg.start_ms}`}
                class="transcript-segment"
                class:active={currentTimeMs >= seg.start_ms && currentTimeMs <= seg.end_ms}
                class:highlighted-pulse={highlightedSegmentMs === seg.start_ms}
                class:claim-mark-contradicted={cinfo?.verdict === 'contradicted'}
                class:claim-mark-supported={cinfo?.verdict === 'supported'}
                class:claim-mark-unverifiable={cinfo?.verdict === 'unverifiable' || cinfo?.verdict === 'already_checked'}
                onclick={() => {
                  seekToMs(seg.start_ms);
                  if (cinfo) {
                    jumpToClaim(cinfo.claimId);
                  }
                }}
                role="button"
                tabindex="0"
                onkeydown={(e) => {
                  if (e.key === 'Enter') {
                    seekToMs(seg.start_ms);
                    if (cinfo) jumpToClaim(cinfo.claimId);
                  }
                }}
                title={cinfo ? `Твърдение #${cinfo.index}: ${cinfo.verdict}` : `Сегмент: ${Math.round(seg.start_ms / 1000)}s`}
              >
                {#if cinfo}
                  <span
                    class="claim-badge-inline {cinfo.verdict}"
                    onclick={(e) => {
                      e.stopPropagation();
                      jumpToClaim(cinfo.claimId);
                    }}
                    role="button"
                    tabindex="0"
                    onkeydown={(e) => e.key === 'Enter' && jumpToClaim(cinfo.claimId)}
                    title="Отиди на твърдение #{cinfo.index}"
                  >
                    [Т{cinfo.index}]
                  </span>
                {/if}
                {seg.text}{' '}
              </span>
            {/each}
          </div>
        {:else}
          <p class="empty-hint">{currentLang === 'bg' ? 'Няма налична стенограма.' : 'No transcript available.'}</p>
        {/if}
      </div>

      <div class="section-card claims-section">
        <div class="section-header">
          <div>
            <h3>{t.claimsHeading}</h3>
            <span class="section-subhint">
              {currentLang === 'bg'
                ? 'Директни връзки за фактчекинг в български и международни източници'
                : 'Direct fact-checking search tools and external evidence verification'}
            </span>
          </div>
        </div>

        {#if detail.claims.length === 0}
          <p class="empty-hint">{t.noClaimsFound}</p>
        {:else}
          {#each detail.claims as cwith, cIdx}
            {@const claim = cwith.claim}
            <div
              id={`claim-${claim.id}`}
              class="claim-block"
              class:claim-pulse={activeClaimId === claim.id}
            >
              <div class="claim-header">
                <span class="claim-index-pill">Т{cIdx + 1}</span>
                <span class="claim-type-tag">{claim.claim_type}</span>
                <span class="verdict-tag {claim.verdict?.toLowerCase()}">
                  {claim.label || claim.verdict}
                </span>

                <button
                  class="btn btn-shortcut btn-sm"
                  onclick={() => jumpToSegment(claim.start_ms)}
                  title="Отиди на точния цитат в стенограмата"
                >
                  📍 {currentLang === 'bg' ? 'Към цитата в текста' : 'Jump to transcript'}
                </button>

                <button
                  class="btn btn-text btn-sm"
                  onclick={() => seekToMs(claim.start_ms || 0)}
                  title="Пусни видеото от този момент"
                >
                  ⏱ {Math.round((claim.start_ms || 0) / 1000)}s - {Math.round((claim.end_ms || 0) / 1000)}s
                </button>

                <button
                  class="btn btn-text btn-sm"
                  onclick={() => handleCopyCitation(cwith)}
                  title="Копирай цитат с метаданни за доклад"
                >
                  📋 {currentLang === 'bg' ? 'Цитат' : 'Cite'}
                </button>
              </div>

              <div class="claim-text">{claim.text_bg}</div>

              {#if claim.attribution && claim.attribution.length > 0}
                <div class="attribution-box">
                  {#each claim.attribution as attr}
                    <div class="attr-row">
                      <span class="attr-label">{t.assertedPart}</span>
                      <span class="attr-val">{attr.asserted_part || attr.actor}</span>
                    </div>
                    <div class="attr-row">
                      <span class="attr-label">{t.attributedPart}</span>
                      <span class="attr-val">{attr.attributed_part || `${attr.act} ${attr.about}`}</span>
                    </div>
                  {/each}
                </div>
              {/if}

              {#if claim.reasoning_bg}
                <div class="claim-reasoning">
                  <strong>{currentLang === 'bg' ? 'Мотиви и оценка:' : 'Reasoning:'}</strong> {claim.reasoning_bg}
                </div>
              {/if}

              <div class="factcheck-action-bar">
                <span class="factcheck-bar-title">🔍 {currentLang === 'bg' ? 'Проверка на фактите:' : 'Fact-check this claim:'}</span>
                <div class="factcheck-links-grid">
                  <a
                    href={`https://www.google.com/search?q=${encodeURIComponent('site:factcheck.bg OR site:bnr.bg ' + claim.text_bg)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="factcheck-link-btn factcheck-bg"
                    title="Търсене в архивите на Factcheck.bg и БНР Проверено"
                  >
                    🇧🇬 Factcheck.bg / БНР ↗
                  </a>
                  <a
                    href={`https://toolbox.google.com/factcheck/explorer/search/list:recent?query=${encodeURIComponent(claim.text_bg)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="factcheck-link-btn factcheck-google"
                    title="Търсене в Google Fact Check Explorer"
                  >
                    🔎 Google Fact Check Tools ↗
                  </a>
                  <a
                    href={`https://www.google.com/search?q=${encodeURIComponent('site:bta.bg ' + claim.text_bg)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="factcheck-link-btn factcheck-bta"
                    title="Търсене в архива на Българска телеграфна агенция (БТА)"
                  >
                    📰 БТА Архив ↗
                  </a>
                  <a
                    href={`https://www.google.com/search?q=${encodeURIComponent(claim.text_bg)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="factcheck-link-btn factcheck-web"
                    title="Общо търсене на твърдението в Google"
                  >
                    🌐 Google Търсене ↗
                  </a>
                  <button
                    class="btn btn-secondary btn-sm factcheck-verify-btn"
                    onclick={() => handleTriggerClaimVerify(claim.text_bg)}
                    title="Стартирай автоматично проучване и извличане на доказателства с AI"
                  >
                    ⚡ {currentLang === 'bg' ? 'Провери онлайн (AI & Web)' : 'Online Verify'}
                  </button>
                </div>
              </div>

              <div class="evidence-list">
                <h4>{t.evidenceHeading}</h4>
                {#if cwith.evidence.length === 0}
                  <div class="evidence-empty-box">
                    <p class="empty-hint">{t.noEvidenceFound}</p>
                    <button
                      class="btn btn-outline btn-sm"
                      onclick={() => handleTriggerClaimVerify(claim.text_bg)}
                    >
                      ⚡ {currentLang === 'bg' ? 'Търси доказателства в мрежата сега' : 'Search web evidence now'}
                    </button>
                  </div>
                {:else}
                  {#each cwith.evidence as ev}
                    <div class="evidence-item" class:expanded={expandedEvidenceId === ev.id}>
                      <div class="evidence-meta">
                        <span class="stance-badge {stanceClass(ev.stance || '')}">{stanceLabel(ev.stance || '')}</span>
                        <span class="evidence-source">{ev.source_name || ev.source_kind}</span>
                        {#if ev.published_at}
                          <span class="evidence-date">• {ev.published_at.substring(0, 10)}</span>
                        {/if}
                        {#if ev.source_url || (ev as any).url}
                          <button
                            class="btn btn-outline btn-xs preview-act-btn"
                            onclick={() => handleLoadEvidencePreview(ev.source_url || (ev as any).url)}
                            title="Зареди и виж скрапнатия пълен текст на статията"
                          >
                            📖 {currentLang === 'bg' ? 'Зареди статия / преглед' : 'Load article / preview'}
                          </button>
                        {/if}
                      </div>
                      <div class="evidence-snippet">
                        {ev.passage || (ev as any).snippet}
                      </div>
                      <div class="evidence-actions-row">
                        {#if ev.source_url || (ev as any).url}
                          <a
                            href={ev.source_url || (ev as any).url}
                            target="_blank"
                            rel="noopener noreferrer"
                            class="evidence-link"
                          >
                            {(ev as any).title || ev.source_url || (ev as any).url} ↗
                          </a>
                        {/if}
                        <button
                          class="btn btn-text btn-xs toggle-expand-btn"
                          onclick={() => (expandedEvidenceId = expandedEvidenceId === ev.id ? null : ev.id)}
                        >
                          {expandedEvidenceId === ev.id
                            ? (currentLang === 'bg' ? 'Свий цитата ▲' : 'Collapse ▲')
                            : (currentLang === 'bg' ? 'Пълен цитат ▼' : 'Full passage ▼')}
                        </button>
                      </div>
                    </div>
                  {/each}
                {/if}
              </div>
            </div>
          {/each}
        {/if}
      </div>

      {#if detail.people.length > 0}
        <div class="section-card people-section">
          <div class="section-header">
            <div>
              <h3>{t.peopleHeading}</h3>
              <span class="section-subhint">
                {currentLang === 'bg'
                  ? 'Автоматично профилиране, оценка на достоверност и проверка на изказванията'
                  : 'Automated figure dossier, credibility scale & statement verification'}
              </span>
            </div>
          </div>

          <div class="people-grid">
            {#each detail.people as person}
              {@const cred = computePersonCredibility(person)}
              <div class="person-card">
                <div class="person-card-header">
                  <div class="person-name-group">
                    <span class="person-name">{person.canonical_name}</span>
                    {#if person.role || (person as any).role_hint}
                      <span class="person-role">{person.role || (person as any).role_hint}</span>
                    {/if}
                  </div>
                  <span class="person-cred-badge {cred.badgeClass}">{cred.label}</span>
                </div>

                {#if person.summary}
                  <div class="person-bio-box">
                    <p class="person-bio-text">{person.summary}</p>
                  </div>
                {/if}

                <div class="credibility-meter">
                  <div class="meter-bar-track">
                    <div
                      class="meter-bar-fill {cred.badgeClass}"
                      style={`width: ${cred.score}%`}
                    ></div>
                  </div>
                  <div class="meter-labels">
                    <span>{currentLang === 'bg' ? 'Оценка на достоверност' : 'Credibility score'}</span>
                    <strong>{cred.score}% ({cred.levelLabel})</strong>
                  </div>
                  {#if cred.stats && cred.stats.total > 0}
                    <div class="person-stats-mini">
                      <span>{currentLang === 'bg' ? 'Свързани твърдения:' : 'Claims:'} <strong>{cred.stats.total}</strong></span>
                      {#if cred.stats.contradicted > 0}
                        <span class="stat-warn">• {currentLang === 'bg' ? 'Опровергани:' : 'False:'} <strong>{cred.stats.contradicted}</strong></span>
                      {/if}
                      {#if cred.stats.supported > 0}
                        <span class="stat-ok">• {currentLang === 'bg' ? 'Потвърдени:' : 'Supported:'} <strong>{cred.stats.supported}</strong></span>
                      {/if}
                    </div>
                  {/if}
                </div>

                <div class="person-actions-row">
                  <button
                    class="btn btn-primary btn-sm person-verify-btn"
                    onclick={() => handleResearchPerson(person)}
                    disabled={isResearchingPersonId === person.id}
                    title="Стартирай автоматично проучване, скрапване и профилиране на лицето с AI"
                  >
                    {isResearchingPersonId === person.id ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Проучи в мрежата с AI' : 'AI Web Research')}
                  </button>
                  <a
                    href={`https://www.google.com/search?q=${encodeURIComponent('site:factcheck.bg OR site:bnr.bg "' + person.canonical_name + '"')}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="person-link-btn"
                    title="Проверка на изказвания на това лице във Factcheck.bg / БНР"
                  >
                    🇧🇬 Factcheck / БНР ↗
                  </a>
                  <a
                    href={`https://bg.wikipedia.org/w/index.php?search=${encodeURIComponent(person.canonical_name)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="person-link-btn"
                    title="Търсене в Уикипедия на български"
                  >
                    🌐 Уикипедия ↗
                  </a>
                  <a
                    href={`https://news.google.com/search?q=${encodeURIComponent(person.canonical_name)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="person-link-btn"
                    title="Новини за личността в Google Новини"
                  >
                    📰 Новини ↗
                  </a>
                  {#if person.wikidata_id}
                    <a
                      href={`https://www.wikidata.org/wiki/${person.wikidata_id}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      class="person-link-btn wikidata-link"
                      title="Wikidata ID"
                    >
                      Wikidata ({person.wikidata_id}) ↗
                    </a>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      {#if detail.creator || detail.video.creator_handle}
        {@const creator = detail.creator || { handle: detail.video.creator_handle, display_name: detail.video.creator_display_name, flagged_count: 0, watchlist: false }}
        {@const authorRisk = computeAuthorRisk(creator)}
        <div class="section-card creator-section">
          <div class="section-header">
            <div>
              <h3>{t.creatorHeading}</h3>
              <span class="section-subhint">
                {currentLang === 'bg'
                  ? 'Профил на автора, рисков профил и предишни засечени видеа'
                  : 'Author profile, risk tier and previous high-risk flagged videos'}
              </span>
            </div>
            <span class="author-risk-badge {authorRisk.badgeClass}">{authorRisk.label}</span>
          </div>

          <div class="creator-box">
            <div class="creator-main-row">
              <div>
                <a
                  href={`https://www.tiktok.com/@${creator.handle}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  class="creator-name-link"
                  title="Отвори профила в TikTok"
                >
                  @{creator.handle} ↗
                </a>
                {#if creator.display_name}
                  <span class="creator-sub">({creator.display_name})</span>
                {/if}
              </div>

              <button
                class="btn btn-sm"
                class:btn-secondary={creator.watchlist}
                class:btn-outline={!creator.watchlist}
                onclick={handleToggleWatchlist}
                disabled={isTogglingWatchlist}
              >
                {creator.watchlist
                  ? (currentLang === 'bg' ? '👁 В наблюдение' : '👁 Watchlisted')
                  : (currentLang === 'bg' ? '+ Добави в наблюдение' : '+ Add to watchlist')}
              </button>
              <a
                href={`https://www.google.com/search?q=${encodeURIComponent('site:tiktok.com/@' + creator.handle + ' OR "' + (creator.display_name || creator.handle) + '"')}`}
                target="_blank"
                rel="noopener noreferrer"
                class="btn btn-outline btn-sm"
                title="Търсене за този автор в Google"
              >
                🔍 Търси в Google
              </a>
            </div>

            <div class="creator-stat-row">
              <span>{t.creatorFlaggedCount} <strong>{creator.flagged_count ?? 0}</strong></span>
              {#if creator.follower_count}
                <span>• Последователи: <strong>{(creator.follower_count).toLocaleString()}</strong></span>
              {/if}
              <span>• Регион: <strong>{creator.region || 'BG'}</strong></span>
            </div>
          </div>

          <div class="author-other-videos">
            <div class="author-other-header">
              <h4>🎬 {currentLang === 'bg' ? 'Други видеа от този автор в базата:' : 'Other videos by this author:'}</h4>
              {#if isLoadingCreatorVideos}
                <span class="loading-mini-spin">...</span>
              {/if}
            </div>

            {#if creatorVideos.length > 0}
              <div class="other-videos-grid">
                {#each creatorVideos as ov}
                  <div
                    class="other-video-card"
                    onclick={() => selectOtherVideo(ov.id)}
                    role="button"
                    tabindex="0"
                    onkeydown={(e) => e.key === 'Enter' && selectOtherVideo(ov.id)}
                  >
                    <div class="other-vid-top">
                      <span class="other-vid-id">ID: {ov.id}</span>
                      <span class="verdict-tag {ov.top_claim_verdict?.toLowerCase()}">
                        {ov.top_claim_label || ov.top_claim_verdict || 'НОВО'}
                      </span>
                    </div>
                    <div class="other-vid-claim">
                      {ov.top_claim_text || 'Няма извлечено заглавие'}
                    </div>
                    <div class="other-vid-stats">
                      <span>👁 {(ov.views || 0).toLocaleString()}</span>
                      {#if ov.duration_s}
                        <span>⏱ {Math.round(ov.duration_s)}s</span>
                      {/if}
                      <span>Ранг: {Math.round((ov.lead_score || 0) * 100)}%</span>
                      <span class="inspect-btn">Инспектирай ↗</span>
                    </div>
                  </div>
                {/each}
              </div>
            {:else if !isLoadingCreatorVideos}
              <div class="creator-empty-box">
                <p class="empty-hint">
                  {currentLang === 'bg'
                    ? 'Няма други записани видеа от този автор в локалната база.'
                    : 'No other videos by this creator in local database.'}
                </p>
                <div class="creator-empty-actions">
                  <a
                    href={`https://www.tiktok.com/@${creator.handle}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="btn btn-outline btn-sm"
                  >
                    🎬 Отвори видеата на @{creator.handle} в TikTok ↗
                  </a>
                </div>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <div class="section-card metadata-section">
        <button class="collapsible-header" onclick={() => (metadataOpen = !metadataOpen)}>
          <span>{t.metadataHeading}</span>
          <span>{metadataOpen ? '▲' : '▼'}</span>
        </button>
        {#if metadataOpen}
          <pre class="metadata-dump">{JSON.stringify(detail.video, null, 2)}</pre>
        {/if}
      </div>
    </div>
  {/if}

  {#if previewModalData}
    <div class="modal-backdrop" onclick={() => (previewModalData = null)} role="presentation">
      <div class="preview-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
        <div class="preview-modal-header">
          <div>
            <span class="preview-domain-pill">{previewModalData.domain || 'Уеб източник'}</span>
            <h3 class="preview-modal-title">{previewModalData.title || previewModalData.url}</h3>
          </div>
          <button class="close-btn" onclick={() => (previewModalData = null)}>✕</button>
        </div>
        <div class="preview-modal-body">
          {#if previewModalData.text}
            {#each previewModalData.text.split('\n\n') as para}
              {#if para.trim()}
                <p class="preview-paragraph">{para.trim()}</p>
              {/if}
            {/each}
          {:else}
            <p class="empty-hint">Няма извлечен текст от този източник.</p>
          {/if}
        </div>
        <div class="preview-modal-footer">
          <a
            href={previewModalData.url}
            target="_blank"
            rel="noopener noreferrer"
            class="btn btn-primary btn-sm"
          >
            Отвори оригиналната страница ↗
          </a>
          <button class="btn btn-secondary btn-sm" onclick={() => (previewModalData = null)}>
            Затвори
          </button>
        </div>
      </div>
    </div>
  {/if}
</aside>

<style>
  .detail-drawer {
    width: 620px;
    height: 100%;
    background: var(--surface-bg);
    border-left: 1px solid var(--border-color);
    display: flex;
    flex-direction: column;
    box-shadow: -4px 0 20px rgba(0, 0, 0, 0.12);
    position: relative;
    z-index: 50;
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1.15rem;
    border-bottom: 1px solid var(--border-color);
  }

  .drawer-title-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .drawer-title {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--text-heading);
  }

  .video-ref-id {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 1.1rem;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
  }

  .close-btn:hover {
    color: var(--text-heading);
    background: var(--hover-bg);
  }

  .drawer-body {
    flex: 1;
    overflow-y: auto;
    padding: 1rem 1.15rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 300px;
    gap: 0.75rem;
    color: var(--text-muted);
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 3px solid var(--border-color);
    border-top-color: var(--primary-accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .media-section {
    width: 100%;
    background: #000;
    border-radius: 8px;
    overflow: hidden;
  }

  .video-player {
    width: 100%;
    max-height: 280px;
    display: block;
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
    border-radius: 6px;
    padding: 0.8rem;
    margin-bottom: 0.75rem;
    transition: box-shadow 0.2s ease, border-color 0.2s ease;
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
    font-size: 0.8rem;
    color: var(--text-body);
    margin-bottom: 0.65rem;
    line-height: 1.4;
    background: var(--card-bg);
    padding: 0.45rem 0.65rem;
    border-radius: 4px;
    border: 1px dashed var(--border-color);
  }

  .factcheck-action-bar {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.55rem 0.65rem;
    margin-bottom: 0.65rem;
  }

  .factcheck-bar-title {
    display: block;
    font-size: 0.74rem;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    margin-bottom: 0.4rem;
  }

  .factcheck-links-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .factcheck-link-btn {
    font-size: 0.74rem;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 4px;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--border-color);
    background: var(--surface-bg);
    color: var(--text-body);
    transition: all 0.12s ease;
  }

  .factcheck-link-btn:hover {
    background: var(--hover-bg);
    color: var(--text-heading);
    border-color: var(--primary-accent);
  }

  .factcheck-bg {
    color: #0284c7;
    border-color: rgba(2, 132, 199, 0.3);
  }

  .factcheck-google {
    color: #4f46e5;
    border-color: rgba(79, 70, 229, 0.3);
  }

  .factcheck-bta {
    color: #059669;
    border-color: rgba(5, 150, 105, 0.3);
  }

  .factcheck-verify-btn {
    font-size: 0.74rem;
    font-weight: 700;
  }

  .evidence-list h4 {
    margin: 0.5rem 0 0.35rem 0;
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .evidence-empty-box {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--card-bg);
    border: 1px dashed var(--border-color);
    padding: 0.5rem 0.65rem;
    border-radius: 4px;
    gap: 0.5rem;
  }

  .evidence-item {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.45rem 0.65rem;
    margin-bottom: 0.35rem;
    font-size: 0.78rem;
  }

  .evidence-meta {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin-bottom: 0.25rem;
  }

  .stance-badge {
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.68rem;
  }

  .stance-contradicts {
    background: rgba(225, 29, 72, 0.15);
    color: var(--verdict-contradicted);
  }

  .stance-supports {
    background: rgba(16, 185, 129, 0.15);
    color: var(--verdict-supported);
  }

  .stance-neutral {
    background: rgba(100, 116, 139, 0.15);
    color: #64748b;
  }

  .evidence-source {
    font-weight: 600;
    color: var(--text-body);
  }

  .evidence-date {
    color: var(--text-muted);
  }

  .evidence-snippet {
    color: var(--text-body);
    line-height: 1.35;
    margin-bottom: 0.25rem;
  }

  .evidence-link {
    color: var(--primary-accent);
    text-decoration: none;
    font-weight: 600;
  }

  .evidence-link:hover {
    text-decoration: underline;
  }

  .people-grid {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .person-card {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.65rem 0.8rem;
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .person-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .person-name-group {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
  }

  .person-name {
    font-weight: 700;
    color: var(--text-heading);
    font-size: 0.88rem;
  }

  .person-role {
    font-size: 0.74rem;
    color: var(--text-muted);
  }

  .person-cred-badge {
    font-size: 0.7rem;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 999px;
  }

  .cred-low {
    background: rgba(225, 29, 72, 0.15);
    color: var(--verdict-contradicted);
  }

  .cred-high {
    background: rgba(16, 185, 129, 0.15);
    color: var(--verdict-supported);
  }

  .cred-moderate {
    background: rgba(217, 119, 6, 0.15);
    color: var(--verdict-unverifiable);
  }

  .cred-neutral {
    background: rgba(100, 116, 139, 0.15);
    color: #64748b;
  }

  .credibility-meter {
    background: var(--card-bg);
    padding: 0.4rem 0.55rem;
    border-radius: 5px;
    border: 1px solid var(--border-color);
  }

  .meter-bar-track {
    width: 100%;
    height: 6px;
    background: var(--border-color);
    border-radius: 999px;
    overflow: hidden;
    margin-bottom: 0.25rem;
  }

  .meter-bar-fill {
    height: 100%;
    border-radius: 999px;
    transition: width 0.3s ease;
  }

  .meter-bar-fill.cred-low {
    background: var(--verdict-contradicted);
  }

  .meter-bar-fill.cred-high {
    background: var(--verdict-supported);
  }

  .meter-bar-fill.cred-moderate {
    background: var(--verdict-unverifiable);
  }

  .meter-bar-fill.cred-neutral {
    background: #64748b;
  }

  .meter-labels {
    display: flex;
    justify-content: space-between;
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .person-actions-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    align-items: center;
  }

  .person-link-btn {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 4px;
    text-decoration: none;
    border: 1px solid var(--border-color);
    background: var(--card-bg);
    color: var(--text-body);
    transition: all 0.12s ease;
  }

  .person-link-btn:hover {
    border-color: var(--primary-accent);
    color: var(--text-heading);
  }

  .wikidata-link {
    color: var(--primary-accent);
  }

  .person-verify-btn {
    font-size: 0.72rem;
    font-weight: 700;
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

  .author-other-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.45rem;
  }

  .author-other-header h4 {
    margin: 0;
    font-size: 0.82rem;
    font-weight: 700;
    color: var(--text-heading);
  }

  .other-videos-grid {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .other-video-card {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 5px;
    padding: 0.45rem 0.65rem;
    cursor: pointer;
    transition: background 0.12s ease, border-color 0.12s ease;
  }

  .other-video-card:hover {
    background: var(--hover-bg);
    border-color: var(--primary-accent);
  }

  .other-vid-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.25rem;
  }

  .other-vid-id {
    font-size: 0.7rem;
    color: var(--text-muted);
  }

  .other-vid-claim {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-heading);
    margin-bottom: 0.3rem;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    line-height: 1.35;
  }

  .other-vid-stats {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .inspect-btn {
    margin-left: auto;
    color: var(--primary-accent);
    font-weight: 700;
  }

  .collapsible-header {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: none;
    border: none;
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--text-heading);
    cursor: pointer;
    padding: 0;
  }

  .metadata-dump {
    margin-top: 0.5rem;
    padding: 0.5rem;
    background: var(--input-bg);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    font-size: 0.72rem;
    overflow-x: auto;
  }

  .empty-hint {
    color: var(--text-muted);
    font-size: 0.8rem;
    margin: 0;
  }

  .drawer-header-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .drawer-remove-btn {
    font-size: 0.76rem;
    padding: 0.25rem 0.6rem;
    background: rgba(225, 29, 72, 0.2);
    color: var(--verdict-contradicted);
    border: 1px solid rgba(225, 29, 72, 0.4);
    border-radius: 4px;
    cursor: pointer;
  }

  .drawer-remove-btn:hover {
    background: rgba(225, 29, 72, 0.35);
  }

  .google-sge-btn {
    text-decoration: none;
    color: #60a5fa !important;
    border-color: rgba(96, 165, 250, 0.4);
  }

  .google-sge-btn:hover {
    background: rgba(96, 165, 250, 0.15);
  }

  .preview-act-btn {
    font-size: 0.68rem;
    padding: 0.15rem 0.45rem;
    margin-left: auto;
  }

  .evidence-actions-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 0.4rem;
  }

  .toggle-expand-btn {
    font-size: 0.7rem;
    color: var(--primary-accent);
    cursor: pointer;
  }

  .person-bio-box {
    margin: 0.4rem 0;
    padding: 0.45rem 0.65rem;
    background: rgba(255, 255, 255, 0.03);
    border-left: 2px solid var(--primary-accent);
    border-radius: 0 4px 4px 0;
  }

  .person-bio-text {
    margin: 0;
    font-size: 0.78rem;
    line-height: 1.4;
    color: var(--text-body);
  }

  .person-stats-mini {
    display: flex;
    gap: 0.5rem;
    font-size: 0.7rem;
    color: var(--text-muted);
    margin-top: 0.25rem;
  }

  .stat-warn {
    color: var(--verdict-contradicted);
    font-weight: 600;
  }

  .stat-ok {
    color: var(--verdict-supported);
    font-weight: 600;
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

  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(4px);
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
  }

  .preview-modal {
    background: var(--surface-bg, #1a1a1a);
    border: 1px solid var(--border-color, #333);
    border-radius: 8px;
    width: 100%;
    max-width: 720px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.5);
  }

  .preview-modal-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--border-color, #333);
  }

  .preview-domain-pill {
    font-size: 0.7rem;
    background: rgba(59, 130, 246, 0.2);
    color: #60a5fa;
    padding: 0.15rem 0.45rem;
    border-radius: 4px;
    font-weight: 600;
  }

  .preview-modal-title {
    margin: 0.35rem 0 0 0;
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--text-heading);
    line-height: 1.35;
  }

  .preview-modal-body {
    padding: 1.25rem;
    overflow-y: auto;
    flex: 1;
    font-size: 0.88rem;
    line-height: 1.6;
    color: var(--text-body);
  }

  .preview-paragraph {
    margin: 0 0 0.85rem 0;
  }

  .preview-modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1.25rem;
    border-top: 1px solid var(--border-color, #333);
    background: rgba(0, 0, 0, 0.15);
  }
</style>