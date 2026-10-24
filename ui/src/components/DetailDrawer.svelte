<script lang="ts">
  import {
    aiCheckTranscript,
    deleteEvidence,
    deleteVideo,
    fetchEvidencePreview,
    fetchPeople,
    fetchVideoDetail,
    fetchVideosByCreator,
    getCoreUrl,
    googleAiFixTranscript,
    googleAudioTranscribe,
    logDecision,
    openExternalUrl,
    queryGoogleAi,
    reassignVideoPerson,
    removePersonFromVideo,
    researchPerson,
    retranscribeVideo,
    setCreatorWatchlist,
    updateTranscript,
  } from '../lib/api';
  import { isJobActive, startAiJob, finishAiJob, getActiveAiJobs } from '../lib/aiTasks.svelte';
  import type { GoogleAiQueryResult } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { ClaimWithEvidence, EntityType, LeadCard, Person, VideoDetail } from '../lib/types';
  import { formatVerdictDisplay } from '../lib/types';
  import PersonDossierModal from './PersonDossierModal.svelte';

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
  let isGoogleAiFixing = $state(false);
  let isGoogleAudioTranscribing = $state(false);
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
  let googleAiModalData = $state<GoogleAiQueryResult | null>(null);
  let isGoogleAiQuerying = $state(false);
  let isLoadingPreview = $state(false);
  let isResearchingPersonId = $state<number | null>(null);
  let expandedEvidenceId = $state<number | null>(null);
  let isPlaying = $state(false);
  let videoContainerRef = $state<HTMLElement | null>(null);
  let isVideoOutOfView = $state(false);
  let disambiguatingPerson = $state<Person | null>(null);
  let viewingPersonDossier = $state<Person | null>(null);
  let availablePeople = $state<Person[]>([]);
  let disambiguateNewName = $state('');
  let disambiguateRole = $state('');
  let disambiguateTags = $state('');
  let disambiguateTargetPersonId = $state<number | null>(null);
  let disambiguateEntityType = $state<EntityType>('person');

  function getEntityTypeInfo(type?: string) {
    switch (type) {
      case 'institution':
        return { label: currentLang === 'bg' ? 'Институция' : 'Institution', icon: '🏛️', cssClass: 'type-institution' };
      case 'channel':
        return { label: currentLang === 'bg' ? 'Канал / Медия' : 'Channel / Media', icon: '📺', cssClass: 'type-channel' };
      case 'country':
        return { label: currentLang === 'bg' ? 'Държава' : 'Country', icon: '🌍', cssClass: 'type-country' };
      case 'group':
        return { label: currentLang === 'bg' ? 'Група / Партия' : 'Group / Party', icon: '👥', cssClass: 'type-group' };
      case 'event':
        return { label: currentLang === 'bg' ? 'Събитие' : 'Event', icon: '📅', cssClass: 'type-event' };
      default:
        return { label: currentLang === 'bg' ? 'Личност' : 'Person', icon: '👤', cssClass: 'type-person' };
    }
  }

  const transcriptDisplaySegments = $derived.by(() => {
    if (!detail?.transcript) return [];
    if (detail.transcript.segments && detail.transcript.segments.length > 0) {
      return detail.transcript.segments;
    }
    const full = detail.transcript.edited_text || detail.transcript.text || '';
    if (!full.trim()) return [];
    const clean = full.replace(/\*\*\d{2}:\d{2}\*\*\n*/g, '').trim();
    const sentences = clean.split(/(?<=[.!?])\s+/).filter(Boolean);
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

  async function handleDeleteEvidence(claimId: number, evidenceId: number) {
    try {
      await deleteEvidence(evidenceId);
      if (detail?.claims) {
        const c = detail.claims.find(x => x.claim.id === claimId);
        if (c) {
          c.evidence = c.evidence.filter(e => e.id !== evidenceId);
        }
      }
      copyToast = currentLang === 'bg' ? '✓ Източникът беше премахнат' : '✓ Source removed';
      setTimeout(() => (copyToast = null), 2500);
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при премахване на източника' : 'Failed to remove source';
      setTimeout(() => (copyToast = null), 2500);
    }
  }

  function isJunkSource(ev: any): boolean {
    const u = (ev.source_url || ev.url || '').toLowerCase();
    if (!u) return false;
    return u.includes('familylink') || u.includes('families.google') || u.includes('bing.com') ||
           u.includes('pornhub') || u.includes('xxx') || u.includes('casino') || u.includes('betano') ||
           u.includes('alo.bg') || u.includes('kompanionki') || u.includes('otvet.mail.ru') ||
           u.includes('badoo.com') || u.includes('playhop.com') || u.includes('bolshoyvopros');
  }

  function getMatchingKeywords(claimText: string, passageText: string): string[] {
    const stopWords = new Set([
      'това', 'след', 'като', 'беше', 'който', 'която', 'което', 'които', 'през', 'защото',
      'всичко', 'няма', 'има', 'само', 'беше', 'може', 'биха', 'този', 'тази', 'тези',
      'един', 'една', 'едно', 'едни', 'дали', 'заради', 'върху', 'срещу', 'пред', 'към',
      'във', 'със', 'при', 'него', 'нея', 'тях', 'вече', 'още', 'много', 'малко', 'къде',
      'какво', 'кога', 'защо', 'така', 'дали', 'нали', 'беше', 'бяха', 'също', 'ще', 'да'
    ]);
    const words = (claimText || '').toLowerCase().match(/[\p{L}\p{N}]{4,}/gu) || [];
    const pLower = (passageText || '').toLowerCase();
    const matched = new Set<string>();
    for (const w of words) {
      if (!stopWords.has(w) && pLower.includes(w)) {
        matched.add(w);
      }
    }
    return Array.from(matched).slice(0, 5);
  }

  function sourceCategoryLabel(kind: string): { label: string; icon: string; cls: string } {
    switch ((kind || '').toLowerCase()) {
      case 'primary':
        return { label: currentLang === 'bg' ? 'Официална институция' : 'Primary / Gov', icon: '🏛️', cls: 'cat-primary' };
      case 'factcheck':
        return { label: currentLang === 'bg' ? 'Фактчек проверка' : 'Fact Check', icon: '🛡️', cls: 'cat-factcheck' };
      case 'news':
        return { label: currentLang === 'bg' ? 'Новинарска медия' : 'News Media', icon: '📰', cls: 'cat-news' };
      default:
        return { label: currentLang === 'bg' ? 'Външен уеб източник' : 'External Web', icon: '🌐', cls: 'cat-other' };
    }
  }

  async function handleResearchPerson(person: Person) {
    if (isJobActive('person-research', person.id)) return;
    startAiJob('person-research', person.id, person.canonical_name);
    copyToast = currentLang === 'bg' ? `⚡ Стартирано OSINT за ${person.canonical_name}...` : `⚡ Researching ${person.canonical_name}...`;
    try {
      const updated = await researchPerson(person.id, person.canonical_name);
      if (detail?.people) {
        detail.people = detail.people.map(p => p.id === person.id ? { ...p, ...updated } : p);
      }
      finishAiJob('person-research', person.id, currentLang === 'bg' ? `✓ Досието на ${person.canonical_name} е готово!` : `✓ Researched: ${person.canonical_name}!`, true);
      copyToast = currentLang === 'bg' ? `✓ Проучен: ${person.canonical_name}` : `✓ Researched: ${person.canonical_name}`;
    } catch {
      finishAiJob('person-research', person.id, currentLang === 'bg' ? 'Грешка при проучване на лицето' : 'Research error', false);
      copyToast = currentLang === 'bg' ? 'Грешка при проучване на лицето' : 'Research error';
    } finally {
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
    if (!detail?.video?.id || isJobActive('whisper', detail.video.id)) return;
    const vid = detail.video.id;
    startAiJob('whisper', vid, `Whisper: ${vid}`);
    copyToast = currentLang === 'bg' ? '🔄 Изпратено за Whisper...' : '🔄 Whisper queued...';
    try {
      await retranscribeVideo(vid, false, true);
      finishAiJob('whisper', vid, currentLang === 'bg' ? `✓ Whisper завърши за ${vid}` : `✓ Whisper completed for ${vid}`, true);
      copyToast = currentLang === 'bg' ? '✓ Whisper задачата е стартирана!' : '✓ Whisper task queued!';
    } catch {
      finishAiJob('whisper', vid, currentLang === 'bg' ? 'Грешка при Whisper задача' : 'Whisper queue error', false);
      copyToast = currentLang === 'bg' ? 'Грешка при Whisper задача' : 'Whisper queue error';
    } finally {
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
        copyToast = currentLang === 'bg' ? '✓ Коригирано с AI!' : '✓ AI corrected!';
      }
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при AI проверка' : 'AI check error';
    } finally {
      isAiChecking = false;
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  async function handleGoogleAiFix() {
    if (!detail?.video?.id || isJobActive('google-fix', detail.video.id)) return;
    const vid = detail.video.id;
    startAiJob('google-fix', vid, `AI Поправка: ${vid}`);
    copyToast = currentLang === 'bg' ? '✨ AI поправка на готовия транскрипт...' : '✨ Refining transcript with AI...';
    try {
      const res = await googleAiFixTranscript(vid);
      if (res && res.corrected_text) {
        if (detail && detail.video.id === vid && detail.transcript) {
          detail.transcript.edited_text = res.corrected_text;
          editedText = res.corrected_text;
        }
        finishAiJob('google-fix', vid, currentLang === 'bg' ? `✓ AI поправката за ${vid} е готова!` : `✓ AI fix ready for ${vid}!`, true);
        copyToast = currentLang === 'bg' ? '✓ Поправено с AI!' : '✓ Corrected with AI!';
      }
    } catch {
      finishAiJob('google-fix', vid, currentLang === 'bg' ? 'Грешка при AI поправка' : 'AI fix error', false);
      copyToast = currentLang === 'bg' ? 'Грешка при AI поправка' : 'AI fix error';
    } finally {
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  async function handleGoogleAudioTranscribe() {
    if (!detail?.video?.id || isJobActive('google-audio', detail.video.id)) return;
    const vid = detail.video.id;
    startAiJob('google-audio', vid, `Google AI: ${vid}`);
    copyToast = currentLang === 'bg' ? '🎙️ Google AI аудио транскрибиране...' : '🎙️ Google AI transcribing audio...';
    try {
      const res = await googleAudioTranscribe(vid);
      if (res && res.corrected_text) {
        if (detail && detail.video.id === vid) {
          if (!detail.transcript) {
            detail.transcript = {
              video_id: vid,
              source: 'google_audio',
              engine: 'google_web_speech',
              model: null,
              language: 'bg',
              quality: null,
              text: res.corrected_text,
              segments: [],
              edited_text: res.corrected_text,
            };
          } else {
            detail.transcript.edited_text = res.corrected_text;
          }
          editedText = res.corrected_text;
          await loadDetail(vid);
        }
        finishAiJob('google-audio', vid, currentLang === 'bg' ? `✓ Google AI аудио за ${vid} е готово!` : `✓ Google AI audio ready for ${vid}!`, true);
        copyToast = currentLang === 'bg' ? '✓ Google AI транскрипцията е готова!' : '✓ Google AI transcription ready!';
      }
    } catch {
      finishAiJob('google-audio', vid, currentLang === 'bg' ? 'Грешка при Google AI транскрибиране' : 'Google AI transcription error', false);
      copyToast = currentLang === 'bg' ? 'Грешка при Google AI транскрибиране' : 'Google AI transcription error';
    } finally {
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  function togglePlayPause() {
    if (!videoElement) return;
    if (videoElement.paused) {
      videoElement.play();
      isPlaying = true;
    } else {
      videoElement.pause();
      isPlaying = false;
    }
  }

  function seekRelative(seconds: number) {
    if (!videoElement) return;
    videoElement.currentTime = Math.max(0, videoElement.currentTime + seconds);
  }

  function handleDrawerScroll(e: Event) {
    const target = e.target as HTMLElement;
    if (!target) return;
    if (videoContainerRef) {
      const rect = videoContainerRef.getBoundingClientRect();
      const parentRect = target.getBoundingClientRect();
      isVideoOutOfView = rect.bottom < parentRect.top + 80;
    } else {
      isVideoOutOfView = target.scrollTop > 100;
    }
  }

  function formatDuration(sec: number | null): string {
    if (!sec) return '00:00';
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }

  function formatViews(v: number | null): string {
    if (!v) return '0';
    if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(1)}M`;
    if (v >= 1_000) return `${(v / 1_000).toFixed(1)}K`;
    return v.toString();
  }

  function formatTime(ms: number): string {
    const totalSec = Math.floor(ms / 1000);
    const m = Math.floor(totalSec / 60);
    const s = Math.floor(totalSec % 60);
    return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }

  function handleSegmentEdit(seg: any, newTxt: string) {
    seg.text = newTxt;
    editedText = transcriptDisplaySegments.map((s) => s.text).join(' ');
  }

  async function toggleTranscriptEditing() {
    if (isEditingTranscript) {
      await handleSaveTranscript();
    } else {
      if (!editedText) {
        editedText = detail?.transcript?.edited_text || detail?.transcript?.text || '';
      }
      isEditingTranscript = true;
    }
  }

  async function handleOpenDisambiguate(person: Person) {
    disambiguatingPerson = person;
    disambiguateTargetPersonId = person.id;
    disambiguateNewName = person.canonical_name;
    disambiguateRole = person.role || '';
    disambiguateTags = (person.tags || []).join(', ');
    disambiguateEntityType = (person.entity_type as EntityType) || 'person';
    try {
      availablePeople = await fetchPeople();
    } catch {}
  }

  async function handleConfirmDisambiguation() {
    if (!detail || !disambiguatingPerson) return;
    try {
      if (disambiguateTargetPersonId && disambiguateTargetPersonId !== disambiguatingPerson.id) {
        await reassignVideoPerson(detail.video.id, disambiguatingPerson.id, {
          new_person_id: disambiguateTargetPersonId,
        });
      } else if (
        (disambiguateNewName.trim() && disambiguateNewName.trim() !== disambiguatingPerson.canonical_name) ||
        disambiguateEntityType !== (disambiguatingPerson.entity_type || 'person') ||
        disambiguateRole.trim() !== (disambiguatingPerson.role || '')
      ) {
        const tagsArr = disambiguateTags.split(',').map((t) => t.trim().replace(/^\[|\]$/g, '')).filter(Boolean);
        await reassignVideoPerson(detail.video.id, disambiguatingPerson.id, {
          canonical_name: disambiguateNewName.trim() || disambiguatingPerson.canonical_name,
          role: disambiguateRole.trim() || undefined,
          tags: tagsArr,
          entity_type: disambiguateEntityType,
        });
      }
      const fresh = await fetchVideoDetail(detail.video.id);
      detail.people = fresh.people;
      disambiguatingPerson = null;
      copyToast = currentLang === 'bg' ? '✓ Лицето е коригирано!' : '✓ Person updated!';
      setTimeout(() => (copyToast = null), 2500);
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при пренасочване' : 'Reassignment error';
      setTimeout(() => (copyToast = null), 2500);
    }
  }

  async function handleRemovePersonFromVideo(person: Person) {
    if (!detail) return;
    if (!confirm(currentLang === 'bg' ? `Премахване на ${person.canonical_name} от това видео?` : `Remove ${person.canonical_name}?`)) return;
    try {
      await removePersonFromVideo(detail.video.id, person.id);
      detail.people = detail.people.filter((p) => p.id !== person.id);
    } catch {}
  }

  async function handleOpenGoogleAiResearch(query: string, mode: 'research' | 'refine' | 'factcheck' = 'research') {
    if (!query.trim() || isGoogleAiQuerying) return;
    isGoogleAiQuerying = true;
    copyToast = currentLang === 'bg' ? '🌐 Google AI проучване...' : '🌐 Running Google AI research...';
    try {
      const res = await queryGoogleAi(query, mode);
      googleAiModalData = res;
    } catch {
      copyToast = currentLang === 'bg' ? 'Грешка при Google AI проучване' : 'Google AI research error';
    } finally {
      isGoogleAiQuerying = false;
      setTimeout(() => (copyToast = null), 3000);
    }
  }

  function handleApplyGoogleAiRefinedText(text: string) {
    if (!detail?.transcript) return;
    detail.transcript.edited_text = text;
    editedText = text;
    googleAiModalData = null;
    copyToast = currentLang === 'bg' ? '✓ Текстът е приложен!' : '✓ Text applied!';
    setTimeout(() => (copyToast = null), 2500);
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
      {#if getActiveAiJobs().length > 0}
        <span class="active-jobs-badge" title={getActiveAiJobs().map(j => j.label).join('\n')}>
          ⚡ {getActiveAiJobs().length} {currentLang === 'bg' ? 'активни задачи' : 'active jobs'}
        </span>
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
    <div class="drawer-body" onscroll={handleDrawerScroll}>
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
              onclick={handleFullRetranscribe}
              disabled={isJobActive('whisper', detail.video.id)}
              title="Повторно гласово разпознаване от нулата с Whisper"
            >
              {isJobActive('whisper', detail.video.id) ? '...' : '🔄 Whisper'}
            </button>
            <button
              class="btn btn-primary btn-sm btn-ai-fix"
              onclick={handleGoogleAiFix}
              disabled={isJobActive('google-fix', detail.video.id)}
              title="Поправя фонетични грешки и имена в готовия транскрипт (без презаписване на клипборда)"
            >
              {isJobActive('google-fix', detail.video.id) ? '...' : '✨ AI Поправка'}
            </button>
            <button
              class="btn btn-secondary btn-sm btn-google-audio"
              onclick={handleGoogleAudioTranscribe}
              disabled={isJobActive('google-audio', detail.video.id)}
              title="Директно транскрибиране на гласа от видеото с Google AI Speech"
            >
              {isJobActive('google-audio', detail.video.id) ? '...' : '🎙️ Google AI'}
            </button>
            <button
              class="btn btn-secondary btn-sm"
              onclick={toggleTranscriptEditing}
              title="Ръчна редакция на текста на място с прослушване"
            >
              {isEditingTranscript ? (currentLang === 'bg' ? '✓ Запази' : '✓ Save') : '✏️ ' + (currentLang === 'bg' ? 'Редактирай' : 'Edit')}
            </button>
            <button
              class="btn btn-secondary btn-sm"
              onclick={handleCopyTranscript}
              title="Копирай транскрипта в клипборда"
            >
              📋 {currentLang === 'bg' ? 'Копирай' : 'Copy'}
            </button>
          </div>
        </div>

        <div class="media-section feed-mode" bind:this={videoContainerRef}>
          <div class="feed-top-meta">
            <div class="feed-creator-badge">
              <span class="creator-name">@{detail.video.creator_handle || 'tiktok_creator'}</span>
              {#if detail.video.views}
                <span class="creator-views">• {formatViews(detail.video.views)} гледания</span>
              {/if}
            </div>
            {#if detail.video.duration_s}
              <span class="feed-duration-pill">{formatDuration(detail.video.duration_s)}</span>
            {/if}
          </div>
          <div class="feed-video-container">
            <video
              bind:this={videoElement}
              src={videoSrc}
              poster={detail.video.thumbnail_url || undefined}
              controls
              playsinline
              ontimeupdate={handleTimeUpdate}
              onplay={() => (isPlaying = true)}
              onpause={() => (isPlaying = false)}
              class="feed-vertical-player"
            >
              <track kind="captions" />
            </video>
          </div>
        </div>

        {#if detail.transcript}
          <div class="transcript-segments" class:editing-active={isEditingTranscript}>
            {#if isEditingTranscript}
              <div class="editing-banner">
                💡 {currentLang === 'bg'
                  ? 'Кликнете върху сегмент за прослушване, редактирайте текста на място в реално време.'
                  : 'Click any segment to hear audio, edit the text in real time.'}
              </div>
            {/if}
            {#each transcriptDisplaySegments as seg}
              {@const cinfo = getSegmentClaimInfo(seg)}
              {#if isEditingTranscript}
                <span
                  class="transcript-segment-editable"
                  contenteditable="true"
                  onclick={() => seekToMs(seg.start_ms)}
                  onkeydown={(e) => e.key === 'Enter' && seekToMs(seg.start_ms)}
                  oninput={(e) => handleSegmentEdit(seg, (e.target as HTMLElement).innerText)}
                  role="textbox"
                  tabindex="0"
                  title="Кликни за прослушване, пиши за редакция"
                >{seg.text} </span>
              {:else}
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
              {/if}
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
                  {formatVerdictDisplay(claim.verdict, claim.label, currentLang)}
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
                    onclick={(e) => {
                      e.preventDefault();
                      openExternalUrl(`https://www.google.com/search?q=${encodeURIComponent('site:factcheck.bg OR site:bnr.bg ' + claim.text_bg)}`);
                    }}
                  >
                    🇧🇬 Factcheck.bg / БНР ↗
                  </a>
                  <a
                    href={`https://toolbox.google.com/factcheck/explorer/search/list:recent?query=${encodeURIComponent(claim.text_bg)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="factcheck-link-btn factcheck-google"
                    title="Търсене в Google Fact Check Explorer"
                    onclick={(e) => {
                      e.preventDefault();
                      openExternalUrl(`https://toolbox.google.com/factcheck/explorer/search/list:recent?query=${encodeURIComponent(claim.text_bg)}`);
                    }}
                  >
                    🔎 Google Fact Check Tools ↗
                  </a>
                  <a
                    href={`https://www.google.com/search?q=${encodeURIComponent('site:bta.bg ' + claim.text_bg)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="factcheck-link-btn factcheck-bta"
                    title="Търсене в архива на Българска телеграфна агенция (БТА)"
                    onclick={(e) => {
                      e.preventDefault();
                      openExternalUrl(`https://www.google.com/search?q=${encodeURIComponent('site:bta.bg ' + claim.text_bg)}`);
                    }}
                  >
                    📰 БТА Архив ↗
                  </a>
                  <a
                    href={`https://www.google.com/search?q=${encodeURIComponent(claim.text_bg)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="factcheck-link-btn factcheck-web"
                    title="Общо търсене на твърдението в Google"
                    onclick={(e) => {
                      e.preventDefault();
                      openExternalUrl(`https://www.google.com/search?q=${encodeURIComponent(claim.text_bg)}`);
                    }}
                  >
                    🌐 Google Търсене ↗
                  </a>
                  <button
                    class="btn btn-primary btn-sm factcheck-verify-btn"
                    onclick={() => handleTriggerClaimVerify(claim.text_bg)}
                    title="Провери твърдението в българските медии и Factcheck.bg и извлечи цитати"
                  >
                    ⚡ {currentLang === 'bg' ? 'Провери твърдението' : 'Verify Claim'}
                  </button>
                </div>
              </div>

              <div class="evidence-list">
                <h4>{t.evidenceHeading}</h4>
                {#if cwith.evidence.filter(ev => !isJunkSource(ev)).length === 0}
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
                  {#each cwith.evidence.filter(ev => !isJunkSource(ev)) as ev}
                    <div class="evidence-item" class:expanded={expandedEvidenceId === ev.id}>
                      <div class="evidence-connection-bar">
                        <span class="source-category-pill {sourceCategoryLabel(ev.source_kind).cls}">
                          {sourceCategoryLabel(ev.source_kind).icon} {sourceCategoryLabel(ev.source_kind).label}
                        </span>
                        <span class="stance-badge {stanceClass(ev.stance || '')}">{stanceLabel(ev.stance || '')}</span>
                        <span class="connection-strength-pill">
                          🎯 {ev.stance === 'supports' ? (currentLang === 'bg' ? 'Потвърждаващ контекст' : 'Supporting Evidence') : ev.stance === 'contradicts' ? (currentLang === 'bg' ? 'Опровергаващ факт' : 'Contradicting Evidence') : (currentLang === 'bg' ? 'Тематичен контекст' : 'Related Context')}
                        </span>
                      </div>
                      <div class="evidence-meta">
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
                      {#if getMatchingKeywords(claim.text_bg, ev.passage || (ev as any).snippet || '').length > 0}
                        <div class="evidence-match-keywords">
                          <span class="keywords-label">🔑 {currentLang === 'bg' ? 'Съвпадащи факти / термини:' : 'Matched terms:'}</span>
                          {#each getMatchingKeywords(claim.text_bg, ev.passage || (ev as any).snippet || '') as kw}
                            <span class="keyword-pill">{kw}</span>
                          {/each}
                        </div>
                      {/if}
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
                            onclick={(e) => {
                              const u = ev.source_url || (ev as any).url;
                              if (u) {
                                e.preventDefault();
                                openExternalUrl(u);
                              }
                            }}
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
                        <button
                          class="btn btn-outline btn-xs delete-evidence-btn"
                          onclick={() => handleDeleteEvidence(claim.id, ev.id)}
                          title={currentLang === 'bg' ? 'Премахни този източник от твърдението' : 'Remove this source from claim'}
                        >
                          🗑️ {currentLang === 'bg' ? 'Премахни' : 'Remove'}
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
                    <div class="person-title-row">
                      <span class="person-name">{person.canonical_name}</span>
                      <span class="person-entity-badge {getEntityTypeInfo(person.entity_type).cssClass}">
                        {getEntityTypeInfo(person.entity_type).icon} {getEntityTypeInfo(person.entity_type).label}
                      </span>
                    </div>
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
                    class="btn btn-secondary btn-sm"
                    onclick={() => (viewingPersonDossier = person)}
                    title="Виж пълното 2000-думи разузнавателно OSINT досие"
                  >
                    📖 {currentLang === 'bg' ? 'Пълно досие' : 'Dossier'}
                  </button>
                  <button
                    class="btn btn-primary btn-sm person-verify-btn"
                    onclick={() => handleResearchPerson(person)}
                    disabled={isJobActive('person-research', person.id)}
                    title="Стартирай пълно 9-степенно OSINT разузнаване на лицето с AI"
                  >
                    {isJobActive('person-research', person.id) ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Проучи лицето' : 'OSINT Research')}
                  </button>
                  <button
                    class="btn btn-secondary btn-sm"
                    onclick={() => handleOpenDisambiguate(person)}
                    title="Смени или коригирай това лице (напр. политик вместо футболист)"
                  >
                    ✏️ {currentLang === 'bg' ? 'Смени / Коригирай' : 'Disambiguate'}
                  </button>
                  <button
                    class="btn btn-danger btn-sm"
                    onclick={() => handleRemovePersonFromVideo(person)}
                    title="Премахни от това видео"
                  >
                    ✕
                  </button>
                  <a
                    href={`https://www.google.com/search?q=${encodeURIComponent('site:factcheck.bg OR site:bnr.bg "' + person.canonical_name + '"')}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="person-link-btn"
                    title="Проверка на изказвания на това лице във Factcheck.bg / БНР"
                    onclick={(e) => {
                      e.preventDefault();
                      openExternalUrl(`https://www.google.com/search?q=${encodeURIComponent('site:factcheck.bg OR site:bnr.bg "' + person.canonical_name + '"')}`);
                    }}
                  >
                    🇧🇬 Factcheck / БНР ↗
                  </a>
                  <a
                    href={`https://bg.wikipedia.org/w/index.php?search=${encodeURIComponent(person.canonical_name)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="person-link-btn"
                    title="Търсене в Уикипедия на български"
                    onclick={(e) => {
                      e.preventDefault();
                      openExternalUrl(`https://bg.wikipedia.org/w/index.php?search=${encodeURIComponent(person.canonical_name)}`);
                    }}
                  >
                    🌐 Уикипедия ↗
                  </a>
                  <a
                    href={`https://news.google.com/search?q=${encodeURIComponent(person.canonical_name)}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="person-link-btn"
                    title="Новини за личността в Google Новини"
                    onclick={(e) => {
                      e.preventDefault();
                      openExternalUrl(`https://news.google.com/search?q=${encodeURIComponent(person.canonical_name)}`);
                    }}
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
                      onclick={(e) => {
                        e.preventDefault();
                        openExternalUrl(`https://www.wikidata.org/wiki/${person.wikidata_id}`);
                      }}
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
                  onclick={(e) => {
                    e.preventDefault();
                    openExternalUrl(`https://www.tiktok.com/@${creator.handle}`);
                  }}
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
                onclick={(e) => {
                  e.preventDefault();
                  openExternalUrl(`https://www.google.com/search?q=${encodeURIComponent('site:tiktok.com/@' + creator.handle + ' OR "' + (creator.display_name || creator.handle) + '"')}`);
                }}
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
                        {formatVerdictDisplay(ov.top_claim_verdict, ov.top_claim_label, currentLang)}
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
                    onclick={(e) => {
                      e.preventDefault();
                      openExternalUrl(`https://www.tiktok.com/@${creator.handle}`);
                    }}
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
            onclick={(e) => {
              if (previewModalData?.url) {
                e.preventDefault();
                openExternalUrl(previewModalData.url);
              }
            }}
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

  {#if googleAiModalData}
    <div class="modal-backdrop" onclick={() => (googleAiModalData = null)} role="presentation">
      <div class="preview-modal google-ai-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
        <div class="preview-modal-header">
          <div>
            <span class="preview-domain-pill">{googleAiModalData.source === 'google_ai_mode' ? 'Google AI Mode (udm=50)' : 'AI Синтез & Проучване'}</span>
            <h3 class="preview-modal-title">{googleAiModalData.mode === 'refine' ? 'Прецизиран текст' : googleAiModalData.query}</h3>
          </div>
          <button class="close-btn" onclick={() => (googleAiModalData = null)}>✕</button>
        </div>
        <div class="preview-modal-body">
          {#if googleAiModalData.text}
            {#each googleAiModalData.text.split('\n\n') as para}
              {#if para.trim()}
                <p class="preview-paragraph">{para.trim()}</p>
              {/if}
            {/each}
          {:else}
            <p class="empty-hint">Няма извлечен отговор.</p>
          {/if}

          {#if googleAiModalData.citations && googleAiModalData.citations.length > 0}
            <div class="google-ai-citations-box">
              <h4 class="citations-title">Намерени първични източници и цитати ({googleAiModalData.citations.length})</h4>
              <ul class="citations-list">
                {#each googleAiModalData.citations as c}
                  <li class="citation-entry">
                    <a
                      href={c.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      class="citation-link"
                      onclick={(e) => {
                        if (c.url) {
                          e.preventDefault();
                          openExternalUrl(c.url);
                        }
                      }}
                    >
                      ↗ {c.title || c.url}
                    </a>
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
        </div>
        <div class="preview-modal-footer">
          {#if googleAiModalData.mode === 'refine'}
            <button
              class="btn btn-primary btn-sm"
              onclick={() => handleApplyGoogleAiRefinedText(googleAiModalData?.text || '')}
            >
              💾 Приложи в стенограмата
            </button>
          {/if}
          <button
            class="btn btn-secondary btn-sm"
            onclick={() => {
              if (googleAiModalData?.text) {
                navigator.clipboard.writeText(googleAiModalData.text);
                copyToast = '✓ Копирано в клипборда!';
                setTimeout(() => (copyToast = null), 2500);
              }
            }}
          >
            📋 Копирай
          </button>
          <button class="btn btn-secondary btn-sm" onclick={() => (googleAiModalData = null)}>
            Затвори
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if isVideoOutOfView && detail}
    <div class="floating-audio-widget">
      <button class="float-btn float-play" onclick={togglePlayPause} title={isPlaying ? 'Пауза' : 'Възпроизвеждане'}>
        {isPlaying ? '⏸' : '▶'}
      </button>
      <div class="float-controls-group">
        <button class="float-btn float-seek" onclick={() => seekRelative(-5)} title="Назад 5s">
          ⏪
        </button>
        <button class="float-btn float-seek" onclick={() => seekRelative(5)} title="Напред 5s">
          ⏩
        </button>
        <span class="float-time-text">{formatTime(currentTimeMs)}</span>
      </div>
    </div>
  {/if}

  {#if disambiguatingPerson}
    <div class="modal-backdrop" onclick={() => (disambiguatingPerson = null)} role="presentation">
      <div class="modal-dialog" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
        <div class="modal-head">
          <h3>✏️ {currentLang === 'bg' ? 'Смяна на лице:' : 'Disambiguate:'} {disambiguatingPerson.canonical_name}</h3>
          <button class="close-btn" onclick={() => (disambiguatingPerson = null)}>✕</button>
        </div>
        <div class="modal-body">
          <p class="modal-hint">
            {currentLang === 'bg'
              ? 'Ако автоматичното разпознаване е асоциирало видеото с грешно лице (напр. футболист вместо политик), изберете правилното лице от базата или въведете ново.'
              : 'Reassign this detection to another person in the database or specify a new profile.'}
          </p>
          <label>
            <span>{currentLang === 'bg' ? 'Избери съществуваща личност от базата:' : 'Select existing person:'}</span>
            <select bind:value={disambiguateTargetPersonId}>
              <option value={null}>-- {currentLang === 'bg' ? 'Или въведи ново лице отдолу' : 'Or enter new details below'} --</option>
              {#each availablePeople as p}
                <option value={p.id}>{p.canonical_name} ({p.role || p.category || 'Личност'})</option>
              {/each}
            </select>
          </label>
          <hr class="modal-divider" />
          <label>
            <span>{currentLang === 'bg' ? 'Тип субект:' : 'Entity Type:'}</span>
            <select bind:value={disambiguateEntityType}>
              <option value="person">👤 {currentLang === 'bg' ? 'Личност (човек)' : 'Person (individual)'}</option>
              <option value="institution">🏛️ {currentLang === 'bg' ? 'Институция (орган, комисия, съд)' : 'Institution'}</option>
              <option value="channel">📺 {currentLang === 'bg' ? 'Канал / Медия (предаване, YouTube, медия)' : 'Channel / Media'}</option>
              <option value="country">🌍 {currentLang === 'bg' ? 'Държава (геополитически субект)' : 'Country'}</option>
              <option value="group">👥 {currentLang === 'bg' ? 'Група / Партия (партия, коалиция, движение)' : 'Group / Party'}</option>
              <option value="event">📅 {currentLang === 'bg' ? 'Събитие (избори, инцидент, протест)' : 'Event'}</option>
            </select>
          </label>
          <label>
            <span>{currentLang === 'bg' ? 'Или въведи ново правилно име:' : 'Or enter new name:'}</span>
            <input type="text" bind:value={disambiguateNewName} placeholder="напр. Костадин Костадинов (политик)" />
          </label>
          <label>
            <span>{currentLang === 'bg' ? 'Длъжност / Категория:' : 'Role / Category:'}</span>
            <input type="text" bind:value={disambiguateRole} placeholder="Политик, лидер на партия" />
          </label>
          <label>
            <span>{currentLang === 'bg' ? 'Тагове:' : 'Tags:'}</span>
            <input type="text" bind:value={disambiguateTags} placeholder="[политик], [партия X], [депутат]" />
          </label>
        </div>
        <div class="modal-foot">
          <button class="btn btn-secondary btn-sm" onclick={() => (disambiguatingPerson = null)}>
            {currentLang === 'bg' ? 'Отказ' : 'Cancel'}
          </button>
          <button class="btn btn-primary btn-sm" onclick={handleConfirmDisambiguation}>
            {currentLang === 'bg' ? '✓ Потвърди корекцията' : '✓ Confirm'}
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if viewingPersonDossier}
    <PersonDossierModal
      person={viewingPersonDossier}
      {currentLang}
      onClose={() => (viewingPersonDossier = null)}
      onResearchTrigger={async (p) => {
        await handleResearchPerson(p);
        const updated = detail?.people?.find((item) => item.id === p.id);
        if (updated) viewingPersonDossier = updated;
      }}
    />
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

  .active-jobs-badge {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--primary-accent);
    background: var(--primary-accent-alpha);
    border: 1px solid var(--primary-accent);
    padding: 2px 7px;
    border-radius: 999px;
    animation: activePulse 2s infinite ease-in-out;
  }

  @keyframes activePulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.65; }
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
    height: 420px;
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

  .floating-audio-widget {
    position: absolute;
    bottom: 1.5rem;
    right: 1.5rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: var(--surface-bg);
    border: 1px solid var(--primary-accent);
    padding: 0.35rem 0.65rem;
    border-radius: 999px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
    z-index: 60;
  }

  .float-btn {
    border: none;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: transform 0.1s ease;
  }

  .float-btn:hover {
    transform: scale(1.08);
  }

  .float-play {
    width: 36px;
    height: 36px;
    background: var(--primary-accent);
    color: #fff;
    font-size: 1rem;
  }

  .float-seek {
    width: 28px;
    height: 28px;
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    color: var(--text-heading);
    font-size: 0.75rem;
  }

  .float-controls-group {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .float-time-text {
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--text-heading);
    margin-left: 0.25rem;
  }

  .modal-hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-muted);
    line-height: 1.4;
  }

  .modal-divider {
    border: none;
    border-top: 1px solid var(--border-color);
    margin: 0.4rem 0;
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

  .evidence-connection-bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.35rem;
    margin-bottom: 0.35rem;
    padding-bottom: 0.3rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .source-category-pill {
    font-size: 0.68rem;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 4px;
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
  }

  .cat-primary {
    background: rgba(59, 130, 246, 0.15);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.3);
  }

  .cat-factcheck {
    background: rgba(168, 85, 247, 0.15);
    color: #c084fc;
    border: 1px solid rgba(168, 85, 247, 0.3);
  }

  .cat-news {
    background: rgba(245, 158, 11, 0.15);
    color: #fbbf24;
    border: 1px solid rgba(245, 158, 11, 0.3);
  }

  .cat-other {
    background: rgba(148, 163, 184, 0.12);
    color: #94a3b8;
    border: 1px solid rgba(148, 163, 184, 0.25);
  }

  .connection-strength-pill {
    font-size: 0.68rem;
    font-weight: 600;
    color: var(--text-muted);
    background: rgba(255, 255, 255, 0.04);
    padding: 1px 6px;
    border-radius: 4px;
    margin-left: auto;
  }

  .evidence-match-keywords {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin: 0.25rem 0 0.4rem 0;
    padding: 0.25rem 0.4rem;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 4px;
  }

  .keywords-label {
    font-size: 0.68rem;
    color: var(--text-muted);
    font-weight: 600;
  }

  .keyword-pill {
    font-size: 0.65rem;
    font-weight: 600;
    background: rgba(99, 102, 241, 0.18);
    color: #a5b4fc;
    padding: 1px 5px;
    border-radius: 3px;
  }

  .delete-evidence-btn {
    font-size: 0.68rem;
    padding: 0.15rem 0.45rem;
    color: var(--text-muted);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .delete-evidence-btn:hover {
    color: #ef4444;
    border-color: rgba(239, 68, 68, 0.4);
    background: rgba(239, 68, 68, 0.1);
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
    flex-direction: column;
    gap: 0.2rem;
  }

  .person-title-row {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .person-name {
    font-weight: 700;
    color: var(--text-heading);
    font-size: 0.88rem;
  }

  .person-entity-badge {
    font-size: 0.65rem;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 4px;
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
  }

  .person-entity-badge.type-person {
    background: rgba(37, 99, 235, 0.12);
    color: var(--primary-accent);
    border: 1px solid rgba(37, 99, 235, 0.3);
  }

  .person-entity-badge.type-institution {
    background: rgba(147, 51, 234, 0.12);
    color: #9333ea;
    border: 1px solid rgba(147, 51, 234, 0.3);
  }

  .person-entity-badge.type-channel {
    background: rgba(234, 88, 12, 0.12);
    color: #ea580c;
    border: 1px solid rgba(234, 88, 12, 0.3);
  }

  .person-entity-badge.type-country {
    background: rgba(16, 185, 129, 0.12);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.3);
  }

  .person-entity-badge.type-group {
    background: rgba(217, 119, 6, 0.12);
    color: #d97706;
    border: 1px solid rgba(217, 119, 6, 0.3);
  }

  .person-entity-badge.type-event {
    background: rgba(236, 72, 153, 0.12);
    color: #ec4899;
    border: 1px solid rgba(236, 72, 153, 0.3);
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

  .google-ai-citations-box {
    margin-top: 1.25rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border-color, #333);
  }

  .citations-title {
    margin: 0 0 0.5rem 0;
    font-size: 0.82rem;
    font-weight: 700;
    color: var(--text-heading);
  }

  .citations-list {
    margin: 0;
    padding-left: 1.2rem;
    font-size: 0.8rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .citation-link {
    color: var(--primary-accent, #3b82f6);
    text-decoration: none;
    word-break: break-all;
  }

  .citation-link:hover {
    text-decoration: underline;
  }
</style>