export type Lane = 'leads' | 'already_checked' | 'low_quality' | 'failed' | 'screened_out' | 'pasted';

export type Screen = 'inbox' | 'creators' | 'people' | 'seeds' | 'calibration' | 'settings' | 'export' | 'help';

export interface LeadCard {
  id: string;
  creator_handle: string | null;
  creator_display_name: string | null;
  creator_follower_count: number | null;
  views: number;
  duration_s: number | null;
  thumbnail_url: string | null;
  top_claim_text: string | null;
  top_claim_verdict: string | null;
  top_claim_label: string | null;
  lead_score: number;
  lane: string;
  transcript_quality: number | null;
  transcript_source: string | null;
  max_checkworthiness: number | null;
  max_evidence_strength: number | null;
  cluster_size: number;
  cluster_total_views: number;
  research_running: boolean;
  reason_flagged: string;
  progress_pct?: number | null;
  etr_seconds?: number | null;
  stage_desc?: string | null;
}

export interface AttributionItem {
  actor: string;
  act: string;
  target: string | null;
  about: string;
  asserted_part: string | null;
  attributed_part: string | null;
}

export interface Claim {
  id: number;
  video_id: string;
  text_bg: string;
  claim_type: string;
  speaker: string | null;
  attribution: AttributionItem[];
  start_ms: number;
  end_ms: number;
  checkworthiness: number;
  verdict: string | null;
  label: string | null;
  reasoning_bg: string | null;
  evidence_strength: number | null;
  verified_at: string | null;
  cluster_id: number | null;
}

export interface Evidence {
  id: number;
  claim_id: number;
  source_kind: string;
  source_name: string;
  url: string;
  title: string | null;
  published_at: string | null;
  snippet: string;
  stance: string;
  similarity: number | null;
}

export interface DetailedPersonProfile {
  report_markdown?: string;
  identity?: string;
  contact_locations?: string;
  network_associates?: string;
  education_career?: string;
  financial_assets?: string;
  legal_regulatory?: string;
  digital_media_presence?: string;
  affiliations_beliefs?: string;
  psychographic_behavioral?: string;
}

export type EntityType = 'person' | 'institution' | 'channel' | 'country' | 'group' | 'event';

export interface Person {
  id: number;
  canonical_name: string;
  variants?: string[];
  role?: string | null;
  role_hint?: string | null;
  public_figure?: boolean;
  is_public_figure?: boolean;
  wikidata_id?: string | null;
  summary?: string | null;
  researched_at?: string | null;
  mentions?: number | null;
  found_in?: string | null;
  tags?: string[];
  rating?: number;
  category?: string | null;
  notes?: string | null;
  detailed_profile_json?: string | null;
  entity_type?: EntityType | string;
}

export interface Creator {
  id: string;
  handle: string;
  display_name: string | null;
  region: string | null;
  follower_count: number | null;
  first_seen_at: string;
  watchlist: boolean;
  flagged_count: number;
  expansion_depth: number;
}

export interface TranscriptSegment {
  start_ms: number;
  end_ms: number;
  text: string;
  speaker: string | null;
}

export interface Transcript {
  video_id: string;
  source: string;
  engine: string | null;
  model: string | null;
  language: string | null;
  quality: number | null;
  text: string;
  segments: TranscriptSegment[];
  edited_text: string | null;
}

export interface VideoDetail {
  video: {
    id: string;
    url: string;
    creator_handle: string | null;
    creator_display_name: string | null;
    creator_follower_count: number | null;
    caption: string | null;
    hashtags: string[];
    posted_at: string | null;
    views: number | null;
    likes: number | null;
    shares: number | null;
    comments: number | null;
    duration_s: number | null;
    phash: string | null;
    state: string;
    state_reason: string | null;
    media_path: string | null;
    lead_score: number | null;
    lane: string | null;
  };
  creator: Creator | null;
  transcript: Transcript | null;
  claims: { claim: Claim; evidence: Evidence[] }[];
  people: Person[];
  decisions: { id: number; action: string; decided_at: string; note: string | null }[];
}

export interface Seed {
  id: number;
  kind: string;
  value: string;
  active: boolean;
  priority: number;
  last_ran_at: string | null;
}

export interface HardwareProfile {
  id: number;
  hardware: {
    cpu: { brand: string; physical_cores: number; logical_cores: number };
    ram_mb: number;
    gpus: { vendor: string; name: string; vram_mb: number }[];
    compute_backends: string[];
  };
  chosen: {
    effective: {
      asr_engine: string;
      asr_model: string;
      llm_provider: string;
      llm_model: string;
      ocr_engine: string;
      concurrency: { extract: number; claims: number; verify: number };
    };
    overrides?: Record<string, string>;
  };
  capacity_per_day: number;
  measured_at: string;
}

export interface QueueCounts {
  queued?: number;
  running?: number;
  succeeded?: number;
  failed?: number;
  deferred?: number;
  lane_counts?: Record<string, number>;
}

export interface ScaleStatus {
  aimd: {
    stages: Record<string, { current_concurrency: number; active_tasks: number; last_latency_ms: number }>;
    backpressure_active: boolean;
    constrained_stage: string | null;
  };
  rss_bytes: number;
  disk_free_bytes: number;
  media_bytes: number;
  open_breakers: number;
}

export function formatVerdictDisplay(verdict: string | null | undefined, label: string | null | undefined, lang: string = 'bg'): string {
  const normLabel = (label || '').trim().toLowerCase();
  const normVerdict = (verdict || '').trim().toUpperCase();

  if (lang === 'bg') {
    switch (normLabel) {
      case 'false_claim':
        return 'Невярно твърдение';
      case 'misleading':
        return 'Подвеждащо';
      case 'missing_context':
        return 'Липсва контекст';
      case 'satire_or_opinion':
        return 'Мнение / Сатира';
      case 'supported':
      case 'verified':
        return 'Потвърдено';
      case 'unverifiable':
      case 'unknown':
        return 'Непотвърдено';
    }

    switch (normVerdict) {
      case 'CONTRADICTED':
        return 'Опровергано';
      case 'SUPPORTED':
        return 'Потвърдено';
      case 'UNVERIFIABLE':
        return 'Непотвърдено';
      case 'NOT_CHECKABLE':
        return 'Непроверимо';
      case 'ALREADY_CHECKED':
        return 'Проверено в архив';
      case 'PROCESSING':
        return 'В обработка...';
      default:
        return normLabel || normVerdict || 'НОВО';
    }
  } else {
    switch (normLabel) {
      case 'false_claim':
        return 'False claim';
      case 'misleading':
        return 'Misleading';
      case 'missing_context':
        return 'Missing context';
      case 'satire_or_opinion':
        return 'Opinion';
      case 'supported':
      case 'verified':
        return 'Supported';
      case 'unverifiable':
      case 'unknown':
        return 'Unverifiable';
    }

    switch (normVerdict) {
      case 'CONTRADICTED':
        return 'Contradicted';
      case 'SUPPORTED':
        return 'Supported';
      case 'UNVERIFIABLE':
        return 'Unverifiable';
      case 'NOT_CHECKABLE':
        return 'Not checkable';
      case 'ALREADY_CHECKED':
        return 'Already checked';
      case 'PROCESSING':
        return 'Processing...';
      default:
        return normLabel || normVerdict || 'NEW';
    }
  }
}
