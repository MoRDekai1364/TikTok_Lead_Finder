export type Lane = 'leads' | 'already_checked' | 'low_quality' | 'failed' | 'screened_out' | 'pasted';

export type Screen = 'inbox' | 'creators' | 'people' | 'seeds' | 'calibration' | 'models' | 'settings' | 'export' | 'help';

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

export interface CreatorWebSource {
  title: string;
  url: string;
  snippet: string;
  domain: string;
}

export interface CreatorLocalStats {
  total_videos: number;
  flagged_videos: number;
  contradicted_claims: number;
  supported_claims: number;
}

export interface CreatorRiskAssessment {
  risk_level: string;
  risk_score: number;
  confidence: number;
  confidence_level: string;
  confidence_reason: string;
  rationale: string;
  web_sources: CreatorWebSource[];
  local_stats: CreatorLocalStats;
  last_calculated_at: string;
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
  risk_assessment_json?: string | null;
  risk_assessment?: CreatorRiskAssessment | null;
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
    power_source?: string;
  };
  chosen: {
    power_source?: string;
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

export interface CustomScanConfig {
  target_mode: 'watchlist' | 'creators' | 'keywords' | 'hashtags' | 'all' | 'no_keywords';
  targets?: string[];
  allow_no_keywords?: boolean;
  max_per_target?: number;
  min_views?: number | null;
  min_likes?: number | null;
  max_age_days?: number | null;
  auto_process?: boolean;
  max_targets?: number | null;
}

export interface VideoRefSummary {
  id: string;
  url: string;
  creator_handle?: string | null;
  creator_display_name?: string | null;
  caption?: string | null;
  views?: number | null;
  likes?: number | null;
  posted_at?: string | null;
  duration_s?: number | null;
}

export interface CustomScanResult {
  targets_scanned: number;
  videos_found: number;
  videos_ingested: number;
  videos_skipped_existing: number;
  videos_skipped_filter: number;
  ingested_videos: VideoRefSummary[];
  errors: string[];
}

export interface GeminiKeyInfo {
  index: number;
  masked: string;
  is_active: boolean;
}

export interface GeminiStatus {
  configured: boolean;
  masked_key?: string | null;
  cloud_enabled: boolean;
  model: string;
  total_keys?: number;
  active_key_index?: number;
  keys?: GeminiKeyInfo[];
}

export interface PowerInfo {
  power_source: 'ac' | 'battery';
  ac_online: boolean;
  battery_percent: number | null;
  has_battery: boolean;
  battery_life_time_seconds?: number | null;
}

export interface AsrCandidateBenchmark {
  model: string;
  elapsed_ms: number;
  rtf: number;
  wer: number;
  raw_capacity_per_day: number;
}

export interface LlmCandidateBenchmark {
  model: string;
  param_billions?: number;
  backend: string;
  tokens_per_second: number;
  mean_call_seconds: number;
  json_validity_rate?: number;
}

export interface BenchmarkProgress {
  is_running: boolean;
  power_source: 'ac' | 'battery';
  current_stage: string;
  stage_index: number;
  total_stages: number;
  progress_pct: number;
  status_text: string;
  status_text_en: string;
  current_model: string | null;
  asr_results: AsrCandidateBenchmark[];
  llm_results: LlmCandidateBenchmark[];
  final_capacity: number | null;
  error: string | null;
  started_at: string | null;
  completed_at: string | null;
}

export interface BenchmarkStatusResponse {
  is_running: boolean;
  progress: BenchmarkProgress;
  power: PowerInfo;
  ac_profile: HardwareProfile | null;
  battery_profile: HardwareProfile | null;
  active_profile: HardwareProfile | null;
}

export interface CapacityTierInfo {
  tierName: string;
  badgeClass: 'ultra' | 'high' | 'recommended' | 'moderate' | 'basic';
  icon: string;
  percentOfTarget: number;
  explanation: string;
  equivHoursPerDay: number;
  equivChannels: number;
  secondsPerVideo: number;
}

export function getCapacityTierInfo(cap: number, lang: 'bg' | 'en' = 'bg'): CapacityTierInfo {
  const roundCap = Math.max(0, Math.round(cap || 0));
  const percentOfTarget = Math.round((roundCap / 300) * 100);
  const equivHoursPerDay = Math.max(1, Math.round((roundCap * 45) / 3600));
  const equivChannels = Math.max(1, Math.round(roundCap / 15));
  const secondsPerVideo = roundCap > 0 ? Math.max(1, Math.round(43200 / roundCap)) : 0;

  if (roundCap >= 1200) {
    return {
      tierName: lang === 'bg' ? 'Експертна AI работна станция (Ultra Tier)' : 'Pro AI Workstation (Ultra Tier)',
      badgeClass: 'ultra',
      icon: '🏆',
      percentOfTarget,
      explanation: lang === 'bg'
        ? 'Изключителна производителност! Системата ви е на ниво професионална AI работна станция. Може да следи десетки TikTok канали едновременно без никакво забавяне.'
        : 'Exceptional throughput! Pro workstation grade. Effortlessly monitors dozens of TikTok channels in real-time.',
      equivHoursPerDay,
      equivChannels,
      secondsPerVideo,
    };
  }

  if (roundCap >= 600) {
    return {
      tierName: lang === 'bg' ? 'Висока производителност (High Performance)' : 'High Performance Desktop',
      badgeClass: 'high',
      icon: '⚡',
      percentOfTarget,
      explanation: lang === 'bg'
        ? `Много висока скорост! Надвишава препоръчителния минимум (300 видеа/ден) с ${percentOfTarget - 100}%. Спокойно сканира и проверява целия дневен TikTok нюзфийд.`
        : `Very fast processing! Exceeds target baseline (300/day) by ${percentOfTarget - 100}%. Effortlessly screens the entire daily feed.`,
      equivHoursPerDay,
      equivChannels,
      secondsPerVideo,
    };
  }

  if (roundCap >= 300) {
    return {
      tierName: lang === 'bg' ? 'Препоръчително ниво (Recommended Tier)' : 'Recommended Baseline Tier',
      badgeClass: 'recommended',
      icon: '🚀',
      percentOfTarget,
      explanation: lang === 'bg'
        ? 'Отлична скорост. Напълно покрива препоръчителния таргет на LeadFinder за постоянен автоматичен анализ на съдържание.'
        : 'Optimal baseline speed. Fully satisfies the target quota for continuous automated screening.',
      equivHoursPerDay,
      equivChannels,
      secondsPerVideo,
    };
  }

  if (roundCap >= 150) {
    return {
      tierName: lang === 'bg' ? 'Стандартно / Мобилно ниво (Standard Tier)' : 'Standard / Mobile Tier',
      badgeClass: 'moderate',
      icon: '⚖️',
      percentOfTarget,
      explanation: lang === 'bg'
        ? 'Добра скорост за лична употреба и по-малки списъци. При по-големи опашки се препоръчва работа на заден план.'
        : 'Good for personal use and smaller watchlists. Background processing is recommended for larger batches.',
      equivHoursPerDay,
      equivChannels,
      secondsPerVideo,
    };
  }

  return {
    tierName: lang === 'bg' ? 'Енергоспестяващ / Базов режим (Eco Tier)' : 'Power-Saver / Basic Tier',
    badgeClass: 'basic',
    icon: '🔋',
    percentOfTarget,
    explanation: lang === 'bg'
      ? 'Икономичен режим за пестене на енергия. За максимална скорост включете зарядното или ползвайте безплатен Google AI ключ.'
      : 'Power-saver mode. Connect to AC power or enable free Google AI cloud assist for maximum speed.',
    equivHoursPerDay,
    equivChannels,
    secondsPerVideo,
  };
}

export interface ModelRoleConfig {
  role: string;
  provider: string;
  model_name: string;
  custom_url?: string | null;
  temperature?: number | null;
}

export interface ConsensusSettings {
  enabled: boolean;
  threshold: number;
  secondary_provider: string;
  secondary_model: string;
  secondary_custom_url?: string | null;
}

export interface ModelHubRolesConfig {
  roles: Record<string, ModelRoleConfig>;
  consensus: ConsensusSettings;
}

export interface InstalledModelInfo {
  id: string;
  name: string;
  kind: string;
  filename: string;
  file_path: string;
  size_bytes: number;
  size_display: string;
  modified_at: string;
  recommended_roles: string[];
}

export interface CuratedModelCatalogItem {
  id: string;
  name: string;
  author: string;
  kind: string;
  download_url: string;
  filename: string;
  size_bytes: number;
  size_display: string;
  vram_estimate: string;
  description_bg: string;
  description_en: string;
  recommended_for: string[];
  is_installed: boolean;
}

export interface ModelDownloadProgress {
  active: boolean;
  model_id: string;
  filename: string;
  bytes_downloaded: number;
  total_bytes: number;
  progress_pct: number;
  speed_bps: number;
  status: string;
  error_message?: string | null;
}

export interface ExternalAiServerModel {
  id: string;
  name: string;
  parameter_size?: string | null;
  quantization?: string | null;
  size_bytes?: number | null;
}

export interface ExternalAiServerStatus {
  server_type: string;
  endpoint_url: string;
  is_online: boolean;
  latency_ms?: number | null;
  models: ExternalAiServerModel[];
}

export interface ModelTestRequest {
  provider: string;
  model: string;
  custom_url?: string | null;
  prompt: string;
  system_prompt?: string | null;
}

export interface ModelTestResponse {
  success: boolean;
  elapsed_ms: number;
  tokens_in: number;
  tokens_out: number;
  tokens_per_second: number;
  raw_text: string;
  parsed_json?: any;
  error?: string | null;
}
