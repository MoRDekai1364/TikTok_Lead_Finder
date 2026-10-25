<script lang="ts">
  import { onMount } from 'svelte';
  import {
    initDesktopConnection,
    deleteVideo,
    exitAndRunBackground,
    fetchProfile,
    fetchQueueStatus,
    fetchScaleStatus,
    fetchVideos,
    listenToExitPrompt,
    logDecision,
    openExternalUrl,
    pasteLinks,
    pullMore,
    subscribeSse,
    terminateApplication,
  } from './lib/api';
  import type { Language } from './lib/i18n';
  import type { Lane, LeadCard, QueueCounts, ScaleStatus, Screen } from './lib/types';

  import CalibrationScreen from './components/CalibrationScreen.svelte';
  import CreatorsScreen from './components/CreatorsScreen.svelte';
  import PeopleScreen from './components/PeopleScreen.svelte';
  import DetailDrawer from './components/DetailDrawer.svelte';
  import ExitPromptModal from './components/ExitPromptModal.svelte';
  import ExportScreen from './components/ExportScreen.svelte';
  import HelpScreen from './components/HelpScreen.svelte';
  import GoogleAiWizardModal from './components/GoogleAiWizardModal.svelte';
  import ScanModal from './components/ScanModal.svelte';
  import SeedsScreen from './components/SeedsScreen.svelte';
  import SettingsScreen from './components/SettingsScreen.svelte';
  import TopBar from './components/TopBar.svelte';
  import VirtualInbox from './components/VirtualInbox.svelte';

  let activeScreen = $state<Screen>('inbox');
  let activeLane = $state<Lane>('leads');
  let currentLang = $state<Language>('bg');
  let theme = $state<'light' | 'dark'>('dark');

  let cards = $state<LeadCard[]>([]);
  let selectedId = $state<string | null>(null);
  let checkedIds = $state<Set<string>>(new Set());

  let drawerVideoId = $state<string | null>(null);
  let isVideoFeedOpen = $state<boolean>(
    typeof localStorage !== 'undefined' ? localStorage.getItem('leadfinder_video_feed') !== 'false' : true
  );

  const activeDrawerId = $derived(drawerVideoId);

  let queueCounts = $state<QueueCounts>({ queued: 0, running: 0, succeeded: 0, failed: 0 });
  let capacityPerDay = $state<number | null>(null);
  let scaleStatus = $state<ScaleStatus | null>(null);

  let undoStack = $state<{ id: string; prevLane: string }[]>([]);

  let inboxRef = $state<{ focusSearch: () => void } | null>(null);
  let isScanModalOpen = $state(false);
  let isGoogleAiWizardOpen = $state(false);
  let isExitPromptOpen = $state(false);

  onMount(() => {
    const savedTheme = localStorage.getItem('leadfinder_theme') as 'light' | 'dark' | null;
    if (savedTheme) theme = savedTheme;
    document.documentElement.setAttribute('data-theme', theme);

    const savedLang = localStorage.getItem('leadfinder_lang') as Language | null;
    if (savedLang) currentLang = savedLang;

    const savedFeed = localStorage.getItem('leadfinder_video_feed');
    if (savedFeed !== null) {
      isVideoFeedOpen = savedFeed !== 'false';
    }

    let unsub: () => void = () => {};
    let unlistenExit: () => void = () => {};
    let disposed = false;

    initDesktopConnection().then(() => {
      if (disposed) return;
      loadInitialData();
      unsub = subscribeSse((event) => {
        handleLiveEvent(event);
      });
    });

    listenToExitPrompt(() => {
      const neverAsk = localStorage.getItem('leadfinder_never_ask_exit') === 'true';
      const savedAction = localStorage.getItem('leadfinder_exit_action');
      if (neverAsk && savedAction === 'background') {
        void exitAndRunBackground();
      } else if (neverAsk && savedAction === 'terminate') {
        void terminateApplication();
      } else {
        isExitPromptOpen = true;
      }
    }).then((un) => {
      unlistenExit = un;
    });

    const handleDocumentClick = (e: MouseEvent) => {
      const a = (e.target as HTMLElement)?.closest('a');
      if (a && a.href && (a.target === '_blank' || a.href.startsWith('http'))) {
        e.preventDefault();
        openExternalUrl(a.href);
      }
    };
    document.addEventListener('click', handleDocumentClick);

    window.addEventListener('keydown', handleGlobalKeydown);

    return () => {
      disposed = true;
      unsub();
      unlistenExit();
      document.removeEventListener('click', handleDocumentClick);
      window.removeEventListener('keydown', handleGlobalKeydown);
    };
  });

  async function loadInitialData() {
    try {
      const [vids, qc, prof, sc] = await Promise.all([
        fetchVideos(activeLane),
        fetchQueueStatus(),
        fetchProfile(),
        fetchScaleStatus(),
      ]);

      cards = vids;
      queueCounts = qc;
      if (prof?.capacity_per_day) {
        capacityPerDay = Math.round(prof.capacity_per_day);
      }
      scaleStatus = sc;

      if (cards.length > 0 && !selectedId) {
        selectedId = cards[0].id;
      }
    } catch {}
  }

  function handleLiveEvent(_ev: unknown) {
    fetchVideos(activeLane).then((vids) => {
      cards = vids;
    });
    fetchQueueStatus().then((qc) => {
      queueCounts = qc;
    });
  }

  function toggleTheme() {
    theme = theme === 'dark' ? 'light' : 'dark';
    localStorage.setItem('leadfinder_theme', theme);
    document.documentElement.setAttribute('data-theme', theme);
  }

  function toggleLang() {
    currentLang = currentLang === 'bg' ? 'en' : 'bg';
    localStorage.setItem('leadfinder_lang', currentLang);
  }

  async function handlePasteUrls(text: string): Promise<any> {
    const res = await pasteLinks(text);
    if (!res || typeof res.ingested !== 'number') {
      throw new Error('Core unreachable');
    }
    if (res.ingested === 0) {
      throw new Error('No valid TikTok links found');
    }
    const vid = res.videos?.[0];
    if (vid) {
      activeLane = 'pasted';
      selectedId = vid.id;
      if (isVideoFeedOpen) {
        drawerVideoId = vid.id;
      }
    }
    void loadInitialData();
    return vid ?? null;
  }

  function handleJumpToVideo(id: string) {
    activeScreen = 'inbox';
    activeLane = 'pasted';
    selectedId = id;
    drawerVideoId = id;
  }

  async function handlePullMore(): Promise<{ promoted: number }> {
    const res = await pullMore(15);
    await loadInitialData();
    return { promoted: res.promoted ?? res.pulled_count ?? 0 };
  }

  async function handleKeep(id: string) {
    const idx = cards.findIndex((x) => x.id === id);
    if (idx !== -1) {
      const c = cards[idx];
      undoStack.push({ id, prevLane: c.lane });
      cards = cards.filter((x) => x.id !== id);
      advanceSelectionAfterRemoval(idx);
      await logDecision(id, 'keep');
    }
  }

  async function handleDiscard(id: string) {
    const idx = cards.findIndex((x) => x.id === id);
    if (idx !== -1) {
      const c = cards[idx];
      undoStack.push({ id, prevLane: c.lane });
      cards = cards.filter((x) => x.id !== id);
      advanceSelectionAfterRemoval(idx);
      await logDecision(id, 'discard');
    }
  }

  async function handleDeleteVideo(id: string) {
    const idx = cards.findIndex((x) => x.id === id);
    if (idx !== -1) {
      cards = cards.filter((x) => x.id !== id);
      advanceSelectionAfterRemoval(idx);
    }
    if (drawerVideoId === id) {
      drawerVideoId = null;
    }
    await deleteVideo(id);
  }

  async function handleResearchMore(id: string) {
    const c = cards.find((x) => x.id === id);
    if (c) {
      c.research_running = true;
      cards = [...cards];
      await logDecision(id, 'research_more');
    }
  }

  async function handleUndo() {
    const last = undoStack.pop();
    if (!last) return;
    await logDecision(last.id, 'undo');
    cards = await fetchVideos(activeLane);
    selectedId = last.id;
  }

  async function handleBatchKeep() {
    const toKeep = Array.from(checkedIds);
    checkedIds = new Set();
    for (const id of toKeep) {
      await logDecision(id, 'keep');
    }
    cards = await fetchVideos(activeLane);
  }

  async function handleBatchDiscard() {
    const toDiscard = Array.from(checkedIds);
    checkedIds = new Set();
    for (const id of toDiscard) {
      await logDecision(id, 'discard');
    }
    cards = await fetchVideos(activeLane);
  }

  function toggleVideoFeed() {
    isVideoFeedOpen = !isVideoFeedOpen;
    localStorage.setItem('leadfinder_video_feed', String(isVideoFeedOpen));
  }

  function handleCloseDrawer() {
    drawerVideoId = null;
  }

  function advanceSelectionAfterRemoval(prevIdx: number) {
    if (cards.length > 0) {
      const nextIdx = Math.max(0, Math.min(prevIdx, cards.length - 1));
      selectedId = cards[nextIdx].id;
      if (drawerVideoId) {
        drawerVideoId = cards[nextIdx].id;
      }
    } else {
      selectedId = null;
      drawerVideoId = null;
    }
  }

  function handleGlobalKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName?.toLowerCase();
    if (tag === 'input' || tag === 'textarea' || tag === 'select') {
      return;
    }

    if (e.key === 'j' || e.key === 'J' || e.key === 'ArrowDown') {
      e.preventDefault();
      moveSelection(1);
    } else if (e.key === 'k' || e.key === 'K' || e.key === 'ArrowUp') {
      e.preventDefault();
      moveSelection(-1);
    } else if (e.key === ' ') {
      e.preventDefault();
      toggleVideoFeed();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (selectedId) handleKeep(selectedId);
    } else if (e.key === 'Backspace' || e.key === 'Delete') {
      e.preventDefault();
      if (selectedId) handleDiscard(selectedId);
    } else if (e.key === 'r' || e.key === 'R') {
      e.preventDefault();
      if (selectedId) handleResearchMore(selectedId);
    } else if (e.key === 'u' || e.key === 'U') {
      e.preventDefault();
      handleUndo();
    } else if (e.key === '/') {
      e.preventDefault();
      if (inboxRef) inboxRef.focusSearch();
    } else if (e.key === 'Escape') {
      handleCloseDrawer();
    }
  }

  function moveSelection(direction: number) {
    if (cards.length === 0) return;
    const curIdx = cards.findIndex((c) => c.id === selectedId);
    let nextIdx = curIdx + direction;
    if (nextIdx < 0) nextIdx = 0;
    if (nextIdx >= cards.length) nextIdx = cards.length - 1;
    selectedId = cards[nextIdx].id;
    if (drawerVideoId) {
      drawerVideoId = cards[nextIdx].id;
    }
  }

  function handleSelectCard(id: string) {
    selectedId = id;
    if (drawerVideoId) {
      drawerVideoId = id;
    }
  }

  function handleToggleCheck(id: string) {
    if (checkedIds.has(id)) {
      checkedIds.delete(id);
    } else {
      checkedIds.add(id);
    }
    checkedIds = new Set(checkedIds);
  }

  async function handleChangeLane(lane: Lane) {
    activeLane = lane;
    checkedIds = new Set();
    cards = await fetchVideos(lane);
    selectedId = cards[0]?.id || null;
    if (drawerVideoId) {
      drawerVideoId = cards[0]?.id || null;
    }
  }
</script>

<div class="app-shell" data-theme={theme}>
  <TopBar
    {activeScreen}
    {currentLang}
    {theme}
    {queueCounts}
    {capacityPerDay}
    {scaleStatus}
    onNavigate={(s: Screen) => (activeScreen = s)}
    onPasteUrls={handlePasteUrls}
    onJumpToVideo={handleJumpToVideo}
    onOpenScanModal={() => (isScanModalOpen = true)}
    onOpenGoogleAiWizard={() => (isGoogleAiWizardOpen = true)}
    onRequestExit={() => (isExitPromptOpen = true)}
    onToggleTheme={toggleTheme}
    onToggleLang={toggleLang}
  />

  <main class="app-body">
    <div class="main-content-area">
      {#if activeScreen === 'inbox'}
        <VirtualInbox
          bind:this={inboxRef}
          {cards}
          {activeLane}
          {selectedId}
          {checkedIds}
          {currentLang}
          {isVideoFeedOpen}
          isDrawerOpen={Boolean(drawerVideoId)}
          onOpenDetail={(id) => (drawerVideoId = id)}
          onToggleVideoFeed={toggleVideoFeed}
          laneCounts={queueCounts.lane_counts}
          onSelectCard={handleSelectCard}
          onToggleCheck={handleToggleCheck}
          onChangeLane={handleChangeLane}
          onPullMore={handlePullMore}
          onKeep={handleKeep}
          onDiscard={handleDiscard}
          onResearchMore={handleResearchMore}
          onBatchKeep={handleBatchKeep}
          onBatchDiscard={handleBatchDiscard}
        />
      {:else if activeScreen === 'creators'}
        <CreatorsScreen {currentLang} />
      {:else if activeScreen === 'people'}
        <PeopleScreen {currentLang} />
      {:else if activeScreen === 'seeds'}
        <SeedsScreen {currentLang} />
      {:else if activeScreen === 'calibration'}
        <CalibrationScreen {currentLang} />
      {:else if activeScreen === 'settings'}
        <SettingsScreen {currentLang} onOpenGoogleAiWizard={() => (isGoogleAiWizardOpen = true)} />
      {:else if activeScreen === 'export'}
        <ExportScreen {currentLang} />
      {:else if activeScreen === 'help'}
        <HelpScreen {currentLang} />
      {/if}
    </div>

    {#if activeDrawerId}
      {#key activeDrawerId}
        <DetailDrawer
          videoId={activeDrawerId}
          {currentLang}
          onClose={handleCloseDrawer}
          onSelectVideo={(id) => {
            selectedId = id;
            drawerVideoId = id;
          }}
          onDiscard={handleDiscard}
          onDeleteVideo={handleDeleteVideo}
        />
      {/key}
    {/if}

    <ScanModal
      isOpen={isScanModalOpen}
      {currentLang}
      onClose={() => (isScanModalOpen = false)}
      onJumpToVideo={(id) => {
        activeScreen = 'inbox';
        activeLane = 'pasted';
        selectedId = id;
        if (isVideoFeedOpen) {
          drawerVideoId = id;
        }
        void loadInitialData();
      }}
      onScanComplete={() => {
        void loadInitialData();
      }}
    />

    <GoogleAiWizardModal
      isOpen={isGoogleAiWizardOpen}
      {currentLang}
      onClose={() => (isGoogleAiWizardOpen = false)}
    />

    <ExitPromptModal
      isOpen={isExitPromptOpen}
      {currentLang}
      {queueCounts}
      onClose={() => (isExitPromptOpen = false)}
    />
  </main>
</div>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    background-color: var(--bg-canvas);
    color: var(--text-body);
  }

  .app-body {
    flex: 1;
    min-height: 0;
    display: flex;
    position: relative;
    overflow: hidden;
  }

  .main-content-area {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
