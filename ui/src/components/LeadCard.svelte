<script lang="ts">
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { LeadCard } from '../lib/types';
  import { formatVerdictDisplay } from '../lib/types';

  let {
    card,
    isSelected,
    isChecked,
    currentLang,
    onSelect,
    onToggleCheck,
    onKeep,
    onDiscard,
    onResearchMore,
    isCompact = false,
  }: {
    card: LeadCard;
    isSelected: boolean;
    isChecked: boolean;
    currentLang: Language;
    isCompact?: boolean;
    onSelect: (id: string) => void;
    onToggleCheck: (id: string) => void;
    onKeep: (id: string) => void;
    onDiscard: (id: string) => void;
    onResearchMore: (id: string) => void;
  } = $props();

  const t = $derived(translations[currentLang]);

  function formatDuration(sec: number | null): string {
    if (!sec) return '00:00';
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }

  function formatViews(v: number): string {
    if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(1)}M`;
    if (v >= 1_000) return `${(v / 1_000).toFixed(1)}K`;
    return v.toString();
  }

  function verdictClass(v: string | null): string {
    switch (v?.toUpperCase()) {
      case 'CONTRADICTED':
        return 'chip-contradicted';
      case 'SUPPORTED':
        return 'chip-supported';
      case 'UNVERIFIABLE':
        return 'chip-unverifiable';
      case 'NOT_CHECKABLE':
        return 'chip-not-checkable';
      case 'ALREADY_CHECKED':
        return 'chip-already-checked';
      case 'PROCESSING':
        return 'chip-processing';
      default:
        return 'chip-default';
    }
  }
</script>

<div
  class="lead-card"
  class:selected={isSelected}
  class:checked={isChecked}
  class:compact={isCompact}
  onclick={() => onSelect(card.id)}
  role="option"
  tabindex="0"
  aria-selected={isSelected}
  onkeydown={(e) => {
    if (e.key === ' ' || e.key === 'Enter') {
      onSelect(card.id);
    }
  }}
>
  <div class="select-col" onclick={(e) => e.stopPropagation()} role="none">
    <input
      type="checkbox"
      checked={isChecked}
      onchange={() => onToggleCheck(card.id)}
      aria-label="Избор за групово действие"
    />
  </div>

  <div class="thumb-col">
    {#if card.thumbnail_url}
      <img src={card.thumbnail_url} alt="" class="thumb-img" />
    {:else}
      <div class="thumb-placeholder" aria-hidden="true">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="5 3 19 12 5 21 5 3"></polygon>
        </svg>
      </div>
    {/if}
    <span class="duration-badge">{formatDuration(card.duration_s)}</span>
  </div>

  <div class="info-col">
    <div class="card-header">
      <div class="creator-meta">
        <span class="creator-handle">@{card.creator_handle || 'tiktok_user'}</span>
        <span class="views-count">• {formatViews(card.views)} {currentLang === 'bg' ? 'гледания' : 'views'}</span>
      </div>

      <div class="badges-row">
        {#if card.lane === 'pasted' || (card.reason_flagged && card.reason_flagged.includes('paste'))}
          <span class="badge badge-pasted" title="Ръчно добавено видео">📌 Pasted</span>
        {/if}

        {#if card.transcript_source === 'found'}
          <span class="badge badge-found" title={t.transcriptBadgeFound}>found</span>
        {:else if card.transcript_source === 'generated'}
          <span class="badge badge-gen" title={t.transcriptBadgeGen}>ASR</span>
        {/if}

        {#if card.cluster_size > 1}
          <span class="badge badge-cluster" title={`${card.cluster_size} видеа в нарратива`}>
            🔗 {card.cluster_size}
          </span>
        {/if}

        {#if card.top_claim_verdict === 'ALREADY_CHECKED'}
          <span class="badge badge-archive" title={t.alreadyCheckedBadge}>FactCheck</span>
        {/if}

        {#if ['deepfake', 'confirmed_deepfake', 'ai_generated_video', 'ai_voice_clone', 'robotic_tts'].includes((card.aegis_verdict || '').toLowerCase()) || ((card.aegis_audio_score ?? 0) >= 0.60 || (card.aegis_video_score ?? 0) >= 0.60)}
          <span class="badge badge-deepfake" title={card.aegis_target_person ? `Манипулиран запис: ${card.aegis_target_person}` : 'Засечен манипулиран запис'}>
            🛡️ {card.aegis_target_person ? card.aegis_target_person : 'Манипулация'}
            {#if (card.aegis_audio_score ?? 0) >= (card.aegis_video_score ?? 0)}
              {Math.round((card.aegis_audio_score ?? 0) * 100)}%
            {:else}
              {Math.round((card.aegis_video_score ?? 0) * 100)}%
            {/if}
          </span>
        {/if}

        {#if card.research_running}
          <span class="badge badge-researching" title={t.researchRunningBadge}>⚡</span>
        {/if}

        <span class="verdict-chip {verdictClass(card.top_claim_verdict)}">
          {formatVerdictDisplay(card.top_claim_verdict, card.top_claim_label, currentLang)}
        </span>
      </div>
    </div>

    <div class="top-claim-text" title={card.top_claim_text || ''}>
      {card.top_claim_text || (currentLang === 'bg' ? 'Няма намерени проверими фактически твърдения.' : 'No factual claims found.')}
    </div>

    <div class="card-footer">
      {#if card.progress_pct !== null && card.progress_pct !== undefined && card.progress_pct < 100}
        <div class="card-progress-track active" title={`${card.stage_desc || 'В обработка'} - ${card.progress_pct ?? 50}%`}>
          <div class="card-progress-fill animated-progress" style="width: {Math.max(12, card.progress_pct ?? 50)}%"></div>
          <span class="card-progress-label">
            ⚙️ {card.stage_desc || (currentLang === 'bg' ? 'В обработка...' : 'Processing...')} ({card.progress_pct ?? 50}%)
          </span>
        </div>
      {:else}
        <div class="meters-row">
          <div class="meter-item" title={`${t.meterTranscript}: ${Math.round((card.transcript_quality || 0) * 100)}%`}>
            <span class="meter-label">Q</span>
            <div class="meter-bar">
              <div class="meter-fill" style="width: {Math.round((card.transcript_quality || 0) * 100)}%"></div>
            </div>
          </div>

          <div class="meter-item" title={`${t.meterCheckworthiness}: ${Math.round((card.max_checkworthiness || 0) * 100)}%`}>
            <span class="meter-label">C</span>
            <div class="meter-bar">
              <div class="meter-fill fill-check" style="width: {Math.round((card.max_checkworthiness || 0) * 100)}%"></div>
            </div>
          </div>

          <div class="meter-item" title={`${t.meterEvidence}: ${Math.round((card.max_evidence_strength || 0) * 100)}%`}>
            <span class="meter-label">E</span>
            <div class="meter-bar">
              <div class="meter-fill fill-evidence" style="width: {Math.round((card.max_evidence_strength || 0) * 100)}%"></div>
            </div>
          </div>
        </div>

        <div class="reason-flagged" title={card.reason_flagged}>
          {card.reason_flagged}
        </div>
      {/if}

      <div class="hover-actions" onclick={(e) => e.stopPropagation()} role="none">
        <button class="action-btn btn-keep" title={t.keepAction} onclick={() => onKeep(card.id)} aria-label="Запази">
          ✓
        </button>
        <button
          class="action-btn btn-discard"
          title={currentLang === 'bg' ? 'Премахни / Отхвърли видеото (✕)' : 'Remove / Discard video (✕)'}
          onclick={(e) => {
            e.stopPropagation();
            onDiscard(card.id);
          }}
          aria-label="Премахни"
        >
          ✕
        </button>
        <button
          class="action-btn btn-research"
          class:is-researching={card.research_running}
          disabled={card.research_running}
          title={card.research_running ? (currentLang === 'bg' ? 'В момента тече проучване...' : 'Research running...') : t.researchAction}
          onclick={(e) => {
            e.stopPropagation();
            onResearchMore(card.id);
          }}
          aria-label="Проучи"
        >
          {#if card.research_running}
            <span class="card-spin">⚙</span>
          {:else}
            R
          {/if}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .lead-card {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    padding: 0.45rem 0.7rem;
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    height: 100%;
    box-sizing: border-box;
    overflow: hidden;
    margin: 0;
    cursor: pointer;
    transition: all 0.12s ease-in-out;
    position: relative;
    user-select: none;
  }

  .lead-card:hover {
    border-color: var(--primary-accent);
    background: var(--card-hover-bg);
  }

  .lead-card.selected {
    border-color: var(--primary-accent);
    box-shadow: 0 0 0 2px var(--primary-accent-alpha);
    background: var(--card-selected-bg);
  }

  .lead-card.compact {
    padding: 0.35rem 0.5rem;
    gap: 0.5rem;
  }

  .lead-card.compact .thumb-col {
    width: 48px;
    height: 48px;
    min-width: 48px;
  }

  .lead-card.compact .creator-handle {
    max-width: 120px;
  }

  .lead-card.compact .reason-flagged {
    display: none;
  }

  .lead-card.compact .badge {
    padding: 1px 3px;
    font-size: 0.62rem;
  }

  .select-col {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .thumb-col {
    position: relative;
    width: 60px;
    height: 60px;
    min-width: 60px;
    flex-shrink: 0;
    border-radius: 6px;
    overflow: hidden;
    background: #000;
  }

  .thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .thumb-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #666;
  }

  .duration-badge {
    position: absolute;
    bottom: 3px;
    right: 3px;
    background: rgba(0, 0, 0, 0.75);
    color: #fff;
    font-size: 0.68rem;
    font-weight: 700;
    padding: 1px 3px;
    border-radius: 3px;
  }

  .info-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    overflow: hidden;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    min-width: 0;
  }

  .creator-meta {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.78rem;
    font-weight: 600;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .creator-handle {
    color: var(--primary-accent);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .views-count {
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .badges-row {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-shrink: 0;
  }

  .badge {
    font-size: 0.68rem;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .badge-found {
    background: rgba(16, 185, 129, 0.15);
    color: var(--verdict-supported);
  }

  .badge-gen {
    background: rgba(59, 130, 246, 0.15);
    color: #3b82f6;
  }

  .badge-cluster {
    background: var(--pill-bg);
    color: var(--text-body);
    border: 1px solid var(--border-color);
  }

  .badge-archive {
    background: rgba(168, 85, 247, 0.15);
    color: #a855f7;
  }

  .verdict-chip {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 999px;
    white-space: nowrap;
  }

  .chip-contradicted {
    background: rgba(225, 29, 72, 0.15);
    color: var(--verdict-contradicted);
  }

  .chip-supported {
    background: rgba(16, 185, 129, 0.15);
    color: var(--verdict-supported);
  }

  .chip-unverifiable {
    background: rgba(217, 119, 6, 0.15);
    color: var(--verdict-unverifiable);
  }

  .chip-not-checkable {
    background: rgba(100, 116, 139, 0.15);
    color: #64748b;
  }

  .chip-already-checked {
    background: rgba(168, 85, 247, 0.15);
    color: #a855f7;
  }

  .chip-processing {
    background: rgba(59, 130, 246, 0.18);
    color: #3b82f6;
  }

  .chip-default {
    background: var(--pill-bg);
    color: var(--text-muted);
  }

  .top-claim-text {
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text-heading);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
    font-size: 0.75rem;
    min-width: 0;
    height: 24px;
  }

  .meters-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-shrink: 0;
  }

  .meter-item {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .meter-label {
    font-size: 0.65rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .meter-bar {
    width: 28px;
    height: 5px;
    background: var(--border-color);
    border-radius: 999px;
    overflow: hidden;
  }

  .meter-fill {
    height: 100%;
    background: #3b82f6;
  }

  .fill-check {
    background: #f59e0b;
  }

  .fill-evidence {
    background: #10b981;
  }

  .reason-flagged {
    flex: 1;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hover-actions {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-shrink: 0;
    opacity: 0.4;
    transition: opacity 0.15s ease;
  }

  .lead-card:hover .hover-actions {
    opacity: 1;
  }

  .action-btn {
    border: none;
    border-radius: 4px;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.75rem;
    font-weight: 700;
    cursor: pointer;
    transition: transform 0.1s ease;
  }

  .action-btn:hover {
    transform: scale(1.1);
  }

  .btn-keep {
    background: rgba(16, 185, 129, 0.2);
    color: var(--verdict-supported);
  }

  .btn-discard {
    background: rgba(225, 29, 72, 0.2);
    color: var(--verdict-contradicted);
  }

  .btn-research {
    background: rgba(59, 130, 246, 0.2);
    color: #3b82f6;
  }

  .action-btn.btn-research.is-researching {
    background: rgba(59, 130, 246, 0.4);
    color: #93c5fd;
    cursor: wait;
  }

  .badge-researching {
    background: rgba(59, 130, 246, 0.25);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.5);
    animation: badgePulse 1.5s infinite;
  }

  .card-spin {
    display: inline-block;
    animation: cardSpin 1.2s linear infinite;
  }

  @keyframes cardSpin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @keyframes badgePulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.6; transform: scale(0.95); }
  }

  .badge-pasted {
    background: rgba(99, 102, 241, 0.2);
    color: var(--primary-accent);
    border: 1px solid rgba(99, 102, 241, 0.4);
  }

  .badge-deepfake {
    background: rgba(239, 68, 68, 0.2);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.5);
    font-weight: 800;
  }

  .card-progress-track {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 16px;
    background: rgba(0, 0, 0, 0.08);
    border-radius: 999px;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .card-progress-track.active {
    height: 16px;
    border: 1px solid rgba(99, 102, 241, 0.3);
    background: rgba(99, 102, 241, 0.08);
  }

  .card-progress-fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    background: linear-gradient(90deg, #6366f1, #06b6d4);
    transition: width 0.3s ease;
  }

  .animated-progress {
    background-image: linear-gradient(
      45deg,
      rgba(255, 255, 255, 0.25) 25%,
      transparent 25%,
      transparent 50%,
      rgba(255, 255, 255, 0.25) 50%,
      rgba(255, 255, 255, 0.25) 75%,
      transparent 75%,
      transparent
    );
    background-size: 1rem 1rem;
    animation: progressStripe 1.2s linear infinite;
  }

  @keyframes progressStripe {
    0% {
      background-position: 1rem 0;
    }
    100% {
      background-position: 0 0;
    }
  }

  .card-progress-label {
    position: relative;
    font-size: 0.68rem;
    font-weight: 800;
    color: var(--text-heading);
    z-index: 1;
    line-height: 1;
    max-width: 100%;
    padding: 0 0.4rem;
    box-sizing: border-box;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-shadow: 0 0 3px rgba(0, 0, 0, 0.3);
  }
</style>
