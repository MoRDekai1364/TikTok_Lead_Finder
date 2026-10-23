<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchVideoDetail, getCoreUrl, updateTranscript } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { VideoDetail } from '../lib/types';

  let {
    videoId,
    currentLang,
    onClose,
  }: {
    videoId: string;
    currentLang: Language;
    onClose: () => void;
  } = $props();

  const t = $derived(translations[currentLang]);

  let detail = $state<VideoDetail | null>(null);
  let isLoading = $state(true);
  let isEditingTranscript = $state(false);
  let editedText = $state('');
  let isSavingTranscript = $state(false);
  let videoElement = $state<HTMLVideoElement | null>(null);
  let currentTimeMs = $state(0);
  let metadataOpen = $state(false);

  onMount(async () => {
    isLoading = true;
    try {
      detail = await fetchVideoDetail(videoId);
      if (detail?.transcript) {
        editedText = detail.transcript.edited_text || detail.transcript.text;
      }
    } finally {
      isLoading = false;
    }
  });

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

  async function handleSaveTranscript() {
    if (!detail || isSavingTranscript) return;
    isSavingTranscript = true;
    try {
      detail = await updateTranscript(detail.video.id, editedText);
      isEditingTranscript = false;
    } finally {
      isSavingTranscript = false;
    }
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
    <button class="close-btn" onclick={onClose} title={t.drawerClose} aria-label={t.drawerClose}>
      ✕
    </button>
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
          <h3>{t.transcriptHeading}</h3>
          {#if !isEditingTranscript}
            <button class="btn btn-secondary btn-sm" onclick={() => (isEditingTranscript = true)}>
              {t.transcriptEditBtn}
            </button>
          {:else}
            <button class="btn btn-primary btn-sm" onclick={handleSaveTranscript} disabled={isSavingTranscript}>
              {isSavingTranscript ? '...' : t.transcriptSaveBtn}
            </button>
          {/if}
        </div>

        {#if isEditingTranscript}
          <textarea class="transcript-editor" bind:value={editedText} rows="6"></textarea>
        {:else if detail.transcript}
          <div class="transcript-segments">
            {#each detail.transcript.segments as seg}
              <span
                class="transcript-segment"
                class:active={currentTimeMs >= seg.start_ms && currentTimeMs <= seg.end_ms}
                onclick={() => seekToMs(seg.start_ms)}
                role="button"
                tabindex="0"
                onkeydown={(e) => e.key === 'Enter' && seekToMs(seg.start_ms)}
              >
                {seg.text}{' '}
              </span>
            {/each}
          </div>
        {:else}
          <p class="empty-hint">{currentLang === 'bg' ? 'Няма налична стенограма.' : 'No transcript available.'}</p>
        {/if}
      </div>

      <div class="section-card claims-section">
        <h3>{t.claimsHeading}</h3>
        {#if detail.claims.length === 0}
          <p class="empty-hint">{t.noClaimsFound}</p>
        {:else}
          {#each detail.claims as cwith}
            <div class="claim-block">
              <div class="claim-header">
                <span class="claim-type-tag">{cwith.claim.claim_type}</span>
                <span class="verdict-tag {cwith.claim.verdict?.toLowerCase()}">
                  {cwith.claim.label || cwith.claim.verdict}
                </span>
                <button class="btn btn-text btn-sm" onclick={() => seekToMs(cwith.claim.start_ms)}>
                  ⏱ {Math.round(cwith.claim.start_ms / 1000)}s - {Math.round(cwith.claim.end_ms / 1000)}s
                </button>
              </div>

              <div class="claim-text">{cwith.claim.text_bg}</div>

              {#if cwith.claim.attribution.length > 0}
                <div class="attribution-box">
                  {#each cwith.claim.attribution as attr}
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

              {#if cwith.claim.reasoning_bg}
                <div class="claim-reasoning">
                  <strong>{currentLang === 'bg' ? 'Мотиви:' : 'Reasoning:'}</strong> {cwith.claim.reasoning_bg}
                </div>
              {/if}

              <div class="evidence-list">
                <h4>{t.evidenceHeading}</h4>
                {#if cwith.evidence.length === 0}
                  <p class="empty-hint">{t.noEvidenceFound}</p>
                {:else}
                  {#each cwith.evidence as ev}
                    <div class="evidence-item">
                      <div class="evidence-meta">
                        <span class="stance-badge {stanceClass(ev.stance)}">{stanceLabel(ev.stance)}</span>
                        <span class="evidence-source">{ev.source_name}</span>
                        {#if ev.published_at}
                          <span class="evidence-date">• {ev.published_at.substring(0, 10)}</span>
                        {/if}
                      </div>
                      <div class="evidence-snippet">{ev.snippet}</div>
                      {#if ev.url}
                        <a href={ev.url} target="_blank" rel="noopener noreferrer" class="evidence-link">
                          {ev.title || ev.url} ↗
                        </a>
                      {/if}
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
          <h3>{t.peopleHeading}</h3>
          <div class="people-grid">
            {#each detail.people as person}
              <div class="person-card">
                <span class="person-name">{person.canonical_name}</span>
                {#if person.role_hint}
                  <span class="person-role">{person.role_hint}</span>
                {/if}
                {#if person.wikidata_id}
                  <a
                    href="https://www.wikidata.org/wiki/{person.wikidata_id}"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="wikidata-badge"
                  >
                    Wikidata ({person.wikidata_id}) ↗
                  </a>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/if}

      {#if detail.creator}
        <div class="section-card creator-section">
          <h3>{t.creatorHeading}</h3>
          <div class="creator-box">
            <div>
              <span class="creator-name">@{detail.creator.handle}</span>
              {#if detail.creator.display_name}
                <span class="creator-sub">({detail.creator.display_name})</span>
              {/if}
            </div>
            <div class="creator-stat-row">
              <span>{t.creatorFlaggedCount} <strong>{detail.creator.flagged_count}</strong></span>
              <span>• Watchlist: <strong>{detail.creator.watchlist ? 'Да' : 'Не'}</strong></span>
            </div>
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
</aside>

<style>
  .detail-drawer {
    width: 540px;
    height: 100%;
    background: var(--surface-bg);
    border-left: 1px solid var(--border-color);
    display: flex;
    flex-direction: column;
    box-shadow: -4px 0 16px rgba(0, 0, 0, 0.08);
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
    margin: 0 0 0.65rem 0;
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-heading);
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5rem;
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
    line-height: 1.5;
  }

  .transcript-segment {
    cursor: pointer;
    padding: 1px 2px;
    border-radius: 3px;
    transition: background 0.12s ease;
  }

  .transcript-segment:hover {
    background: var(--hover-bg);
  }

  .transcript-segment.active {
    background: var(--primary-accent-alpha);
    color: var(--primary-accent);
    font-weight: 600;
  }

  .claim-block {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.75rem;
    margin-bottom: 0.65rem;
  }

  .claim-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.45rem;
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

  .verdict-tag.unverifiable {
    background: rgba(217, 119, 6, 0.15);
    color: var(--verdict-unverifiable);
  }

  .claim-text {
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text-heading);
    margin-bottom: 0.5rem;
  }

  .attribution-box {
    background: var(--card-bg);
    border-left: 3px solid var(--primary-accent);
    padding: 0.45rem 0.65rem;
    border-radius: 0 4px 4px 0;
    margin-bottom: 0.5rem;
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
    margin-bottom: 0.5rem;
  }

  .evidence-list h4 {
    margin: 0.5rem 0 0.35rem 0;
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
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
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 0.45rem;
  }

  .person-card {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.45rem 0.65rem;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.8rem;
  }

  .person-name {
    font-weight: 700;
    color: var(--text-heading);
  }

  .person-role {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .wikidata-badge {
    font-size: 0.68rem;
    color: var(--primary-accent);
    text-decoration: none;
  }

  .creator-box {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.85rem;
  }

  .creator-name {
    font-weight: 700;
    color: var(--primary-accent);
  }

  .creator-sub {
    color: var(--text-muted);
    font-size: 0.78rem;
  }

  .creator-stat-row {
    font-size: 0.78rem;
    color: var(--text-muted);
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
</style>
