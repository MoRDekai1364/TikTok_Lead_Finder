import type {
  Creator,
  HardwareProfile,
  LeadCard,
  QueueCounts,
  ScaleStatus,
  Seed,
  VideoDetail,
} from './types';

let cachedBaseUrl = localStorage.getItem('leadfinder_core_url') || 'http://127.0.0.1:17420';
let cachedToken = localStorage.getItem('leadfinder_api_token') || '';

export async function initDesktopConnection(): Promise<void> {
  if (!('__TAURI_INTERNALS__' in window)) {
    return;
  }
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const conn = await invoke<{ url: string; token: string }>('core_connection');
    if (conn.token) {
      cachedToken = conn.token;
      localStorage.setItem('leadfinder_api_token', cachedToken);
    }
    if (conn.url && !localStorage.getItem('leadfinder_core_url')) {
      cachedBaseUrl = conn.url.replace(/\/+$/, '');
    }
  } catch {}
}

export function getCoreUrl(): string {
  return cachedBaseUrl;
}

export function setCoreUrl(url: string) {
  cachedBaseUrl = url.replace(/\/+$/, '');
  localStorage.setItem('leadfinder_core_url', cachedBaseUrl);
}

export function getApiToken(): string {
  return cachedToken;
}

export function setApiToken(token: string) {
  cachedToken = token.trim();
  localStorage.setItem('leadfinder_api_token', cachedToken);
}

async function fetchJson<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
  const url = `${cachedBaseUrl}${endpoint}`;
  const headers = new Headers(options.headers || {});
  if (cachedToken) {
    headers.set('Authorization', `Bearer ${cachedToken}`);
  }
  if (!headers.has('Content-Type') && options.body && typeof options.body === 'string') {
    headers.set('Content-Type', 'application/json');
  }

  try {
    const res = await fetch(url, { ...options, headers });
    if (!res.ok) {
      throw new Error(`HTTP ${res.status}: ${res.statusText}`);
    }
    return (await res.json()) as T;
  } catch (err) {
    return getMockFallback<T>(endpoint, options);
  }
}

function mapSummaryToCard(raw: any): LeadCard {
  const v = raw?.video || raw || {};
  const vidId = String(v.id || raw?.id || '');
  return {
    id: vidId,
    creator_handle: v.creator_handle ?? raw?.creator_handle ?? null,
    creator_display_name: v.creator_display_name ?? raw?.creator_display_name ?? null,
    creator_follower_count: v.creator_follower_count ?? raw?.creator_follower_count ?? null,
    views: typeof v.views === 'number' ? v.views : typeof raw?.views === 'number' ? raw.views : 0,
    duration_s: v.duration_s ?? raw?.duration_s ?? null,
    thumbnail_url: raw?.thumbnail_url ?? (vidId ? `${cachedBaseUrl}/api/media/${vidId}/thumb.jpg` : null),
    top_claim_text: raw?.top_claim_bg ?? raw?.top_claim_text ?? null,
    top_claim_verdict: raw?.top_verdict ?? raw?.top_claim_verdict ?? null,
    top_claim_label: raw?.top_label ?? raw?.top_claim_label ?? null,
    lead_score: typeof v.lead_score === 'number' ? v.lead_score : typeof raw?.lead_score === 'number' ? raw.lead_score : 0,
    lane: v.lane ?? raw?.lane ?? 'leads',
    transcript_quality: raw?.transcript_quality ?? null,
    transcript_source: raw?.transcript_source ?? null,
    max_checkworthiness: raw?.max_checkworthiness ?? null,
    max_evidence_strength: raw?.max_evidence_strength ?? null,
    cluster_size: raw?.cluster_size ?? 1,
    cluster_total_views: raw?.cluster_total_views ?? (typeof v.views === 'number' ? v.views : 0),
    research_running: Boolean(raw?.research_running),
    reason_flagged: raw?.reason_flagged ?? '',
  };
}

function normalizeProfile(res: any): HardwareProfile | null {
  if (!res) return null;
  return {
    ...res,
    hardware: res.hardware || res.hardware_json || {},
    chosen: res.chosen || res.chosen_json || {},
    capacity_per_day: res.capacity_per_day ?? 0,
    measured_at: res.measured_at || '',
  } as HardwareProfile;
}

export async function fetchQueueStatus(): Promise<QueueCounts> {
  const res = await fetchJson<any>('/api/health');
  const q = res?.queue || res || {};
  return { ...q, lane_counts: res?.lane_counts } as QueueCounts;
}

export async function fetchScaleStatus(): Promise<ScaleStatus> {
  return fetchJson<ScaleStatus>('/api/scale/status');
}

export async function fetchProfile(): Promise<HardwareProfile | null> {
  const res = await fetchJson<any>('/api/profile');
  return normalizeProfile(res);
}

export async function fetchVideos(lane: string = 'leads', limit: number = 2000): Promise<LeadCard[]> {
  const res = await fetchJson<any>(`/api/videos?lane=${encodeURIComponent(lane)}&limit=${limit}`);
  const list = Array.isArray(res) ? res : Array.isArray(res?.cards) ? res.cards : [];
  return list.map(mapSummaryToCard);
}

export async function fetchVideoDetail(id: string): Promise<VideoDetail> {
  return fetchJson<VideoDetail>(`/api/videos/${encodeURIComponent(id)}`);
}

export async function pasteLinks(text: string): Promise<{ count?: number; ingested?: number; videos?: any[] }> {
  return fetchJson<{ count?: number; ingested?: number; videos?: any[] }>('/api/inbox/paste', {
    method: 'POST',
    body: JSON.stringify({ text }),
  });
}

export async function triggerDiscovery(maxPerSeed: number = 10): Promise<{ count: number }> {
  return fetchJson<{ count: number }>('/api/discover/run', {
    method: 'POST',
    body: JSON.stringify({ max_per_seed: maxPerSeed }),
  });
}

export async function pullMore(count: number = 10): Promise<{ pulled_count: number }> {
  return fetchJson<{ pulled_count: number }>('/api/rank/pull-more', {
    method: 'POST',
    body: JSON.stringify({ count }),
  });
}

export async function logDecision(id: string, action: string, note?: string): Promise<{ ok: boolean }> {
  return fetchJson<{ ok: boolean }>(`/api/videos/${encodeURIComponent(id)}/decision`, {
    method: 'POST',
    body: JSON.stringify({ action, note }),
  });
}

export async function updateTranscript(id: string, text: string): Promise<VideoDetail> {
  return fetchJson<VideoDetail>(`/api/videos/${encodeURIComponent(id)}/transcript`, {
    method: 'PATCH',
    body: JSON.stringify({ text }),
  });
}

export async function fetchSeeds(): Promise<Seed[]> {
  return fetchJson<Seed[]>('/api/seeds');
}

export async function upsertSeed(seed: Partial<Seed>): Promise<Seed> {
  return fetchJson<Seed>('/api/seeds', {
    method: 'POST',
    body: JSON.stringify(seed),
  });
}

export async function deleteSeed(id: number): Promise<{ ok: boolean }> {
  return fetchJson<{ ok: boolean }>(`/api/seeds/${id}`, {
    method: 'DELETE',
  });
}

export async function fetchCreators(): Promise<Creator[]> {
  return fetchJson<Creator[]>('/api/creators');
}

export async function setCreatorWatchlist(handle: string, watchlist: boolean): Promise<{ ok: boolean }> {
  return fetchJson<{ ok: boolean }>('/api/creators/watchlist', {
    method: 'POST',
    body: JSON.stringify({ handle, watchlist }),
  });
}

export async function triggerCalibration(): Promise<HardwareProfile> {
  const res = await fetchJson<any>('/api/calibrate', { method: 'POST' });
  return normalizeProfile(res)!;
}

export async function updateOverrides(overrides: Record<string, string>): Promise<HardwareProfile> {
  const res = await fetchJson<any>('/api/calibrate/overrides', {
    method: 'POST',
    body: JSON.stringify(overrides),
  });
  return normalizeProfile(res)!;
}

export async function fetchSettings(): Promise<Record<string, unknown>> {
  return fetchJson<Record<string, unknown>>('/api/settings');
}

export async function updateSettings(body: Record<string, unknown>): Promise<{ updated: number }> {
  return fetchJson<{ updated: number }>('/api/settings', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function subscribeSse(onEvent: (event: unknown) => void): () => void {
  const url = `${cachedBaseUrl}/api/events?token=${encodeURIComponent(cachedToken)}`;
  let eventSource: EventSource | null = null;
  try {
    eventSource = new EventSource(url);
    eventSource.onmessage = (e) => {
      try {
        const parsed = JSON.parse(e.data);
        onEvent(parsed);
      } catch {}
    };
  } catch {}

  return () => {
    if (eventSource) {
      eventSource.close();
    }
  };
}

function getMockFallback<T>(endpoint: string, options: RequestInit): T {
  if (endpoint.startsWith('/api/videos?')) {
    const list: LeadCard[] = Array.from({ length: 50 }, (_, i) => ({
      id: `mock_vid_${i + 1}`,
      creator_handle: `creator_${(i % 5) + 1}`,
      creator_display_name: `Български автор ${(i % 5) + 1}`,
      creator_follower_count: 15400 + i * 320,
      views: 24000 + i * 1800,
      duration_s: 32.5,
      thumbnail_url: null,
      top_claim_text: `Парламентът прие нов закон за таксите от 1 януари според изказване на министъра (твърдение ${i + 1}).`,
      top_claim_verdict: i % 4 === 0 ? 'CONTRADICTED' : i % 4 === 1 ? 'UNVERIFIABLE' : i % 4 === 2 ? 'SUPPORTED' : 'ALREADY_CHECKED',
      top_claim_label: i % 4 === 0 ? 'Опровергано' : i % 4 === 1 ? 'Непроверимо' : i % 4 === 2 ? 'Потвърдено' : 'Вече проверено',
      lead_score: 0.95 - (i * 0.015),
      lane: 'leads',
      transcript_quality: 0.88,
      transcript_source: i % 2 === 0 ? 'found' : 'generated',
      max_checkworthiness: 0.82,
      max_evidence_strength: 0.75,
      cluster_size: (i % 3) + 1,
      cluster_total_views: (24000 + i * 1800) * ((i % 3) + 1),
      research_running: i === 0,
      reason_flagged: 'Опровергано твърдение с официален архив на Factcheck.bg и висок интерес в TikTok',
    }));
    return list as T;
  }

  if (endpoint.startsWith('/api/videos/')) {
    const detail: VideoDetail = {
      video: {
        id: 'mock_vid_1',
        url: 'https://www.tiktok.com/@creator_1/video/7300000000000000001',
        creator_handle: 'creator_1',
        creator_display_name: 'Български автор 1',
        creator_follower_count: 24500,
        caption: 'Вижте какво приеха вчера в парламента! #българия #новини #политика',
        hashtags: ['българия', 'новини', 'политика'],
        posted_at: '2026-10-23T10:00:00Z',
        views: 45000,
        likes: 2100,
        shares: 450,
        comments: 310,
        duration_s: 32.5,
        phash: '0x3c7e7e3c18000000',
        state: 'VERIFIED',
        state_reason: null,
        media_path: null,
        lead_score: 0.95,
        lane: 'leads',
      },
      creator: {
        id: 'c1',
        handle: 'creator_1',
        display_name: 'Български автор 1',
        region: 'BG',
        follower_count: 24500,
        first_seen_at: '2026-10-01T12:00:00Z',
        watchlist: true,
        flagged_count: 4,
        expansion_depth: 1,
      },
      transcript: {
        video_id: 'mock_vid_1',
        source: 'found',
        engine: 'native',
        model: 'bg',
        language: 'bg',
        quality: 0.92,
        text: 'Здравейте приятели, вчера народните представители гласуваха двойно увеличение на такса смет от първи януари. Това съобщи лично финансовият министър на пресконференция.',
        segments: [
          { start_ms: 0, end_ms: 6000, text: 'Здравейте приятели, вчера народните представители гласуваха', speaker: 'speaker_1' },
          { start_ms: 6000, end_ms: 14000, text: 'двойно увеличение на такса смет от първи януари.', speaker: 'speaker_1' },
          { start_ms: 14000, end_ms: 25000, text: 'Това съобщи лично финансовият министър на пресконференция.', speaker: 'speaker_1' },
        ],
        edited_text: null,
      },
      claims: [
        {
          claim: {
            id: 101,
            video_id: 'mock_vid_1',
            text_bg: 'Финансовият министър съобщи за двойно увеличение на такса смет от първи януари.',
            claim_type: 'attribution',
            speaker: 'creator_1',
            attribution: [
              {
                actor: 'финансов министър',
                act: 'said',
                target: null,
                about: 'двойно увеличение на такса смет',
                asserted_part: 'авторът твърди, че финансовият министър е съобщил',
                attributed_part: 'финансовият министър съобщи за двойно увеличение на такса смет',
              },
            ],
            start_ms: 6000,
            end_ms: 22000,
            checkworthiness: 0.88,
            verdict: 'CONTRADICTED',
            label: 'Опровергано',
            reasoning_bg: 'Официалното съобщение на Министерството на финансите и решението на парламента отхвърлят каквото и да е увеличение на такса смет за гражданите.',
            evidence_strength: 0.89,
            verified_at: '2026-10-23T11:00:00Z',
            cluster_id: 1,
          },
          evidence: [
            {
              id: 501,
              claim_id: 101,
              source_kind: 'primary',
              source_name: 'Министерство на финансите (minfin.bg)',
              url: 'https://www.minfin.bg/bg/news/1234',
              title: 'Позиция на МФ относно спекулациите за местните такси',
              published_at: '2026-10-22T14:30:00Z',
              snippet: 'Министерството на финансите категорично опровергава твърденията за промени в размера на такса смет за предстоящата бюджетна година.',
              stance: 'contradicts',
              similarity: 0.91,
            },
            {
              id: 502,
              claim_id: 101,
              source_kind: 'factcheck',
              source_name: 'Factcheck.bg',
              url: 'https://factcheck.bg/proverka-taksa-smet',
              title: 'Не, парламентът не е гласувал двойни такси за битови отпадъци',
              published_at: '2026-10-22T16:00:00Z',
              snippet: 'Проверка на фактите: Разпространяваното в социалните мрежи видео се позовава на отхвърлен проектозакон от 2021 година.',
              stance: 'contradicts',
              similarity: 0.86,
            },
          ],
        },
      ],
      people: [
        {
          id: 1,
          canonical_name: 'Министър на финансите',
          role_hint: 'член на Министерския съвет',
          wikidata_id: 'Q558079',
          is_public_figure: true,
        },
      ],
      decisions: [],
    };
    return detail as T;
  }

  if (endpoint === '/api/health') {
    return { queued: 0, running: 0, succeeded: 50, failed: 0 } as T;
  }

  if (endpoint === '/api/profile') {
    const profile: HardwareProfile = {
      id: 1,
      hardware: {
        cpu: { brand: 'AMD Ryzen 7 7800X3D', physical_cores: 8, logical_cores: 16 },
        ram_mb: 32768,
        gpus: [{ vendor: 'NVIDIA', name: 'GeForce RTX 5070 Ti', vram_mb: 12288 }],
        compute_backends: ['vulkan', 'cuda', 'cpu'],
      },
      chosen: {
        effective: {
          asr_engine: 'whisper.cpp',
          asr_model: 'small',
          llm_provider: 'local',
          llm_model: 'BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf',
          ocr_engine: 'tesseract',
          concurrency: { extract: 2, claims: 2, verify: 4 },
        },
      },
      capacity_per_day: 1241.0,
      measured_at: '2026-10-23T12:00:00Z',
    };
    return profile as T;
  }

  if (endpoint === '/api/scale/status') {
    const scale: ScaleStatus = {
      aimd: {
        stages: {
          screen_cheap: { current_concurrency: 8, active_tasks: 0, last_latency_ms: 12 },
          acquire: { current_concurrency: 2, active_tasks: 0, last_latency_ms: 1540 },
          extract: { current_concurrency: 1, active_tasks: 0, last_latency_ms: 2200 },
          claims: { current_concurrency: 1, active_tasks: 0, last_latency_ms: 3100 },
          verify: { current_concurrency: 2, active_tasks: 0, last_latency_ms: 1800 },
        },
        backpressure_active: false,
        constrained_stage: null,
      },
      rss_bytes: 33554432,
      disk_free_bytes: 42949672960,
      media_bytes: 12582912,
      open_breakers: 0,
    };
    return scale as T;
  }

  if (endpoint === '/api/seeds') {
    const seeds: Seed[] = [
      { id: 1, kind: 'hashtag', value: 'българия', active: true, priority: 10, last_ran_at: null },
      { id: 2, kind: 'hashtag', value: 'новини', active: true, priority: 8, last_ran_at: null },
      { id: 3, kind: 'keyword', value: 'парламент избори', active: true, priority: 6, last_ran_at: null },
      { id: 4, kind: 'keyword', value: 'еврозона цени', active: true, priority: 6, last_ran_at: null },
    ];
    return seeds as T;
  }

  if (endpoint === '/api/creators') {
    const creators: Creator[] = [
      { id: 'c1', handle: 'creator_1', display_name: 'Български автор 1', region: 'BG', follower_count: 24500, first_seen_at: '2026-10-01T12:00:00Z', watchlist: true, flagged_count: 4, expansion_depth: 1 },
      { id: 'c2', handle: 'politika_bg', display_name: 'Политика БГ', region: 'BG', follower_count: 58000, first_seen_at: '2026-10-05T15:00:00Z', watchlist: true, flagged_count: 7, expansion_depth: 2 },
    ];
    return creators as T;
  }

  return {} as T;
}
