export interface ActiveAiJob {
  id: string;
  type:
    | 'google-audio'
    | 'google-fix'
    | 'whisper'
    | 'ai-check'
    | 'person-research'
    | 'auto-classify'
    | 'creator-risk'
    | 'video-research'
    | 'aegis-scan';
  targetId: string | number;
  label: string;
  stage?: string;
  startedAt: number;
}

let jobsState = $state<ActiveAiJob[]>([]);
let activeToasts = $state<Array<{ id: string; text: string; success: boolean }>>([]);

export function getActiveAiJobs(): ActiveAiJob[] {
  return jobsState;
}

export function getGlobalAiToasts() {
  return activeToasts;
}

export function isJobActive(type: string, targetId: string | number): boolean {
  const targetStr = String(targetId);
  return jobsState.some((j) => j.type === type && String(j.targetId) === targetStr);
}

export function startAiJob(
  type: ActiveAiJob['type'],
  targetId: string | number,
  label: string
): string {
  const id = `${type}:${targetId}`;
  if (!jobsState.some((j) => j.id === id)) {
    jobsState.push({
      id,
      type,
      targetId,
      label,
      startedAt: Date.now(),
    });
  }
  return id;
}

export function finishAiJob(
  type: ActiveAiJob['type'],
  targetId: string | number,
  toastMessage?: string,
  isSuccess: boolean = true
): void {
  const id = `${type}:${targetId}`;
  jobsState = jobsState.filter((j) => j.id !== id);
  if (toastMessage) {
    const toastId = `${Date.now()}-${Math.random()}`;
    activeToasts.push({ id: toastId, text: toastMessage, success: isSuccess });
    setTimeout(() => {
      activeToasts = activeToasts.filter((t) => t.id !== toastId);
    }, 4000);
  }
}
