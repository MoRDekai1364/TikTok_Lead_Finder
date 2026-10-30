let videoId = $state<string | null>(null);
let creatorHandle = $state<string | null>(null);
let caption = $state<string | null>(null);
let thumbnailUrl = $state<string | null>(null);
let currentTime = $state(0);
let duration = $state(0);
let isPlaying = $state(false);
let volume = $state(1.0);
let isMuted = $state(false);
let activeElement: HTMLVideoElement | null = null;

export function getPlaybackState() {
  return {
    get videoId() {
      return videoId;
    },
    get creatorHandle() {
      return creatorHandle;
    },
    get caption() {
      return caption;
    },
    get thumbnailUrl() {
      return thumbnailUrl;
    },
    get currentTime() {
      return currentTime;
    },
    get duration() {
      return duration;
    },
    get isPlaying() {
      return isPlaying;
    },
    get volume() {
      return volume;
    },
    get isMuted() {
      return isMuted;
    },
    get hasActiveVideo() {
      return !!activeElement || !!videoId;
    },
  };
}

export function registerPlaybackElement(
  el: HTMLVideoElement,
  meta?: {
    videoId?: string;
    creatorHandle?: string;
    caption?: string;
    thumbnailUrl?: string;
  }
) {
  activeElement = el;
  if (meta?.videoId) videoId = meta.videoId;
  if (meta?.creatorHandle) creatorHandle = meta.creatorHandle;
  if (meta?.caption) caption = meta.caption;
  if (meta?.thumbnailUrl) thumbnailUrl = meta.thumbnailUrl;
  duration = el.duration && !isNaN(el.duration) ? el.duration : 0;
  currentTime = el.currentTime || 0;
  isPlaying = !el.paused;
  volume = el.volume;
  isMuted = el.muted;
}

export function unregisterPlaybackElement(el: HTMLVideoElement) {
  if (activeElement === el) {
    activeElement = null;
    isPlaying = false;
  }
}

export function updatePlaybackProgress(current: number, dur?: number) {
  currentTime = current;
  if (dur && !isNaN(dur) && dur > 0) {
    duration = dur;
  }
}

export function setPlaybackPlaying(playing: boolean) {
  isPlaying = playing;
}

export function togglePlayback() {
  if (!activeElement) return;
  if (activeElement.paused) {
    activeElement.play().catch(() => {});
    isPlaying = true;
  } else {
    activeElement.pause();
    isPlaying = false;
  }
}

export function seekPlayback(seconds: number) {
  if (!activeElement) return;
  const target = Math.max(0, Math.min(seconds, duration || seconds));
  activeElement.currentTime = target;
  currentTime = target;
}

export function seekPlaybackRelative(deltaSeconds: number) {
  if (!activeElement) return;
  seekPlayback(activeElement.currentTime + deltaSeconds);
}

export function setPlaybackVolume(vol: number) {
  if (!activeElement) return;
  const clamped = Math.max(0, Math.min(vol, 1));
  activeElement.volume = clamped;
  volume = clamped;
  if (clamped > 0 && isMuted) {
    activeElement.muted = false;
    isMuted = false;
  }
}

export function togglePlaybackMute() {
  if (!activeElement) return;
  activeElement.muted = !activeElement.muted;
  isMuted = activeElement.muted;
}
