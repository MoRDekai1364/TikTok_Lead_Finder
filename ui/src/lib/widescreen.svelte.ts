import { fetchDisplayHardwareInfo, type NativeDisplayHardwareInfo } from './api';

export type DisplayPresetId = 'auto' | '13' | '14' | '15' | '16' | '17' | '18' | 'desktop' | 'off';
export type ResolvedDisplayProfile = '13' | '14' | '15' | '16' | '17' | '18' | 'desktop' | 'standard';

export interface DisplayPresetMeta {
  id: DisplayPresetId;
  shortLabelBg: string;
  shortLabelEn: string;
  titleBg: string;
  titleEn: string;
  descBg: string;
  descEn: string;
  diagonalBadge: string;
  featured?: boolean;
}

export const DISPLAY_PRESETS: DisplayPresetMeta[] = [
  {
    id: 'auto',
    shortLabelBg: 'Авто',
    shortLabelEn: 'Auto',
    titleBg: 'Автоматично разпознаване',
    titleEn: 'Automatic Detection',
    descBg: 'Определя профила по физическа резолюция, пропорция (16:10 / 16:9) и мащабиране.',
    descEn: 'Selects preset from physical resolution, aspect ratio (16:10 / 16:9), and DPI scale.',
    diagonalBadge: 'AUTO',
  },
  {
    id: '13',
    shortLabelBg: '13"',
    shortLabelEn: '13"',
    titleBg: '13" Компактен дисплей',
    titleEn: '13" Ultrabook Display',
    descBg: 'Компактна типография, единична дясна колона и прибрана горна лента за 13.3"–13.6" екрани.',
    descEn: 'Compact typography, single-column docked drawer, and slim header for 13.3"–13.6" screens.',
    diagonalBadge: '13"',
  },
  {
    id: '14',
    shortLabelBg: '14"',
    shortLabelEn: '14"',
    titleBg: '14" Лаптоп (16:10 / 16:9)',
    titleEn: '14" Pro Laptop (16:10 / 16:9)',
    descBg: 'Балансирано странично скачване (58% панел) и двуколонни доказателства за 14"–14.5" екрани.',
    descEn: 'Balanced 58% docked workspace and 2-column evidence cards for 14"–14.5" screens.',
    diagonalBadge: '14"',
  },
  {
    id: '15',
    shortLabelBg: '15"',
    shortLabelEn: '15"',
    titleBg: '15" Стандартен лаптоп (15.6")',
    titleEn: '15" Standard Laptop (15.6")',
    descBg: 'Спестява вертикална височина при 16:9 екрани и активира двупанелен преглед.',
    descEn: 'Saves vertical height on 16:9 panels and enables dual-pane video + claims view.',
    diagonalBadge: '15"',
  },
  {
    id: '16',
    shortLabelBg: '16"',
    shortLabelEn: '16"',
    titleBg: '16" Работен лаптоп (16:10)',
    titleEn: '16" Workstation Laptop (16:10)',
    descBg: 'Използва допълнителната 16:10 височина за 3-зонов работен екран и 3-колонни списъци.',
    descEn: 'Leverages 16:10 vertical space for a 3-zone workspace and 3-column entity grids.',
    diagonalBadge: '16"',
  },
  {
    id: '17',
    shortLabelBg: '17"',
    shortLabelEn: '17"',
    titleBg: '17" Широк лаптоп (17.3")',
    titleEn: '17" Large Laptop (17.3")',
    descBg: 'Разширена медийна колона, 66% аналитичен панел и 4-колонни екрани с обекти.',
    descEn: 'Expanded media rail, 66% analytical workspace, and 4-column subject grids.',
    diagonalBadge: '17"',
  },
  {
    id: '18',
    shortLabelBg: '18" ★',
    shortLabelEn: '18" ★',
    titleBg: '18" Професионален дисплей (16:10)',
    titleEn: '18" Flagship Display (16:10)',
    descBg: 'Специална калибрация за 18" 16:10 (QHD+/UHD+ при 100%–150% DPI): пълна 3-колонна среда, двуколонни оценки по твърдения и двустраничен четец на справки.',
    descEn: 'Precision-calibrated for 18" 16:10 (QHD+/UHD+ across 100%–150% DPI): full 3-pane command desk, 2-column statement cards, and folio report reader.',
    diagonalBadge: '18" ★',
    featured: true,
  },
  {
    id: 'desktop',
    shortLabelBg: '24"+',
    shortLabelEn: '24"+',
    titleBg: '24"+ Настолен / Ултраширок монитор',
    titleEn: '24"+ Desktop / Ultrawide Monitor',
    descBg: 'Максимална многоколонна плътност за външни QHD, 4K и 21:9 монитори.',
    descEn: 'Maximum multi-column density for external QHD, 4K, and 21:9 monitors.',
    diagonalBadge: '24"+',
  },
  {
    id: 'off',
    shortLabelBg: 'Единичен',
    shortLabelEn: 'Single',
    titleBg: 'Класически единичен изглед',
    titleEn: 'Classic Single-Column View',
    descBg: 'Изключва автоматичното разделяне на екрана; детайлите се отварят само при избор.',
    descEn: 'Disables automatic split-docking; detail drawer opens on demand.',
    diagonalBadge: '1×',
  },
];

const VALID_PRESETS: DisplayPresetId[] = ['auto', '13', '14', '15', '16', '17', '18', 'desktop', 'off'];

function loadSavedPreset(): DisplayPresetId {
  if (typeof localStorage === 'undefined') return 'auto';
  const saved = localStorage.getItem('leadfinder_display_preset') as DisplayPresetId | null;
  if (saved && VALID_PRESETS.includes(saved)) {
    return saved;
  }
  const legacy = localStorage.getItem('leadfinder_widescreen_mode');
  if (legacy === 'off') return 'off';
  if (legacy === 'on') return '18';
  return 'auto';
}

interface ProfileGeometry {
  fontSizePx: number;
  dockedDrawerWidth: string;
  drawerLeftCol: string;
  videoStageHeight: string;
  inboxFeedWidth: string;
  peopleCardMin: string;
  dossierMaxW: string;
  dossierMaxH: string;
  dossierTocW: string;
  topbarPadY: string;
  subbarPadY: string;
  evidenceCardMin: string;
  statementCardMin: string;
  rowHeight: number;
}

class WidescreenStore {
  viewportWidth = $state(typeof window !== 'undefined' ? window.innerWidth : 1536);
  viewportHeight = $state(typeof window !== 'undefined' ? window.innerHeight : 960);
  screenWidth = $state(typeof window !== 'undefined' ? window.screen.width : 1920);
  screenHeight = $state(typeof window !== 'undefined' ? window.screen.height : 1200);
  devicePixelRatio = $state(typeof window !== 'undefined' ? (window.devicePixelRatio || 1) : 1);
  nativeHardware = $state<NativeDisplayHardwareInfo | null>(null);
  preset = $state<DisplayPresetId>(loadSavedPreset());

  manualOverride = $derived<'auto' | 'on' | 'off'>(
    this.preset === 'auto' ? 'auto' : this.preset === 'off' ? 'off' : 'on'
  );

  physicalWidth = $derived.by(() => {
    if (this.nativeHardware && this.nativeHardware.physical_width > 0) {
      return this.nativeHardware.physical_width;
    }
    return Math.round(this.screenWidth * this.devicePixelRatio);
  });

  physicalHeight = $derived.by(() => {
    if (this.nativeHardware && this.nativeHardware.physical_height > 0) {
      return this.nativeHardware.physical_height;
    }
    return Math.round(this.screenHeight * this.devicePixelRatio);
  });

  effectiveScale = $derived.by(() => {
    if (this.nativeHardware && this.nativeHardware.scale_factor > 0) {
      return this.nativeHardware.scale_factor;
    }
    return this.devicePixelRatio || 1;
  });

  aspectRatio = $derived.by(() => {
    if (this.nativeHardware && this.nativeHardware.aspect_ratio > 0) {
      return this.nativeHardware.aspect_ratio;
    }
    return this.screenHeight > 0 ? this.screenWidth / this.screenHeight : 1.6;
  });

  is16by10 = $derived(this.aspectRatio >= 1.56 && this.aspectRatio <= 1.64);

  aspectLabel = $derived.by(() => {
    const a = this.aspectRatio;
    if (a >= 2.25) return '32:9';
    if (a >= 2.05) return '21:9';
    if (a >= 1.72 && a <= 1.82) return '16:9';
    if (a >= 1.56 && a <= 1.64) return '16:10';
    if (a >= 1.46 && a < 1.56) return '3:2';
    return `${a.toFixed(2)}:1`;
  });

  resolutionSummary = $derived.by(() => {
    const scalePct = Math.round(this.effectiveScale * 100);
    return `${this.physicalWidth}×${this.physicalHeight} (${this.aspectLabel} • ${scalePct}%)`;
  });

  hardwareDetectedProfile = $derived.by((): ResolvedDisplayProfile => {
    const physW = this.physicalWidth;
    const physH = this.physicalHeight;
    const scale = this.effectiveScale;
    const logW = this.nativeHardware ? this.nativeHardware.logical_width : this.screenWidth;
    const aspect = this.aspectRatio;
    const monName = (this.nativeHardware?.monitor_name || '').toLowerCase();

    if (/180|18\.0|ne180|b180|n180|blade 18|strix.*18|scar.*18|m18|helios 18|titan 18/.test(monName)) {
      return '18';
    }
    if (/173|17\.3|17\.0|b173|nv173|n173/.test(monName)) {
      return '17';
    }
    if (/160|16\.0|ne160|b160/.test(monName)) {
      return '16';
    }
    if (/156|15\.6|b156|nv156|n156/.test(monName)) {
      return '15';
    }
    if (/140|14\.0|14\.5|b140|nv140/.test(monName)) {
      return '14';
    }
    if (/133|13\.3|13\.6|b133/.test(monName)) {
      return '13';
    }

    if (
      aspect >= 2.05 ||
      (physW >= 3440 && aspect >= 1.72 && scale <= 1.25) ||
      (physW >= 2560 && aspect >= 1.74 && scale <= 1.05 && logW >= 2400) ||
      logW >= 2250
    ) {
      return 'desktop';
    }

    const is16x10 = aspect >= 1.56 && aspect <= 1.64;

    if (is16x10) {
      if (
        (physW >= 2560 && physH >= 1600 && scale <= 1.55 && logW >= 1680) ||
        (physW >= 3840 && physH >= 2400 && logW >= 1700) ||
        (physW === 1920 && physH === 1200 && scale <= 1.05)
      ) {
        return '18';
      }
      if (logW >= 1520 && logW < 1680) {
        return '16';
      }
      if (logW >= 1380 && logW < 1520) {
        return '14';
      }
      return '13';
    }

    if (logW >= 1840) {
      return '17';
    }
    if (logW >= 1660) {
      return '17';
    }
    if (logW >= 1480) {
      return '15';
    }
    if (logW >= 1365) {
      return '14';
    }
    return '13';
  });

  resolvedProfile = $derived.by((): ResolvedDisplayProfile => {
    if (this.preset === 'off') return 'standard';
    if (this.preset === 'auto') return this.hardwareDetectedProfile;
    return this.preset;
  });

  detectedMonitorWidescreen = $derived.by(() => {
    return this.aspectRatio >= 1.55 && (this.physicalWidth >= 1920 || this.screenWidth >= 1400);
  });

  isEighteenInch = $derived(this.resolvedProfile === '18');

  isActive = $derived.by(() => {
    const p = this.resolvedProfile;
    const vw = this.viewportWidth;
    if (p === 'standard') return false;
    if (p === '13') return vw >= 1140;
    if (p === '14') return vw >= 1180;
    if (p === '15') return vw >= 1220;
    if (p === '16') return vw >= 1240;
    if (p === '17') return vw >= 1260;
    if (p === '18') return vw >= 1200;
    return vw >= 1360;
  });

  isDualPane = $derived.by(() => {
    if (!this.isActive) return false;
    const p = this.resolvedProfile;
    const vw = this.viewportWidth;
    if (p === '13') return false;
    if (p === '14') return vw >= 1480;
    if (p === '15') return vw >= 1380;
    if (p === '16') return vw >= 1340;
    if (p === '17') return vw >= 1340;
    if (p === '18') return vw >= 1220;
    if (p === 'desktop') return vw >= 1380;
    return false;
  });

  isStatementGrid = $derived.by(() => {
    if (!this.isActive) return false;
    const p = this.resolvedProfile;
    const vw = this.viewportWidth;
    if (p === '18') return vw >= 1420;
    if (p === 'desktop') return vw >= 1750;
    if (p === '17') return vw >= 1820;
    return false;
  });

  isEvidenceGrid = $derived.by(() => {
    if (!this.isActive) return false;
    const p = this.resolvedProfile;
    const vw = this.viewportWidth;
    if (p === '13' || p === 'standard') return false;
    if (p === '14') return vw >= 1320;
    return vw >= 1240;
  });

  isUltraWide = $derived.by(() => {
    if (this.resolvedProfile === 'standard') return false;
    return this.resolvedProfile === 'desktop' && this.viewportWidth >= 2100;
  });

  geometry = $derived.by((): ProfileGeometry => {
    const p = this.resolvedProfile;
    const scale = this.effectiveScale;

    switch (p) {
      case '13':
        return {
          fontSizePx: 13.5,
          dockedDrawerWidth: 'clamp(500px, 55vw, 760px)',
          drawerLeftCol: '100%',
          videoStageHeight: 'clamp(230px, 33vh, 295px)',
          inboxFeedWidth: '300px',
          peopleCardMin: '275px',
          dossierMaxW: '96vw',
          dossierMaxH: '93vh',
          dossierTocW: '205px',
          topbarPadY: '0.32rem',
          subbarPadY: '0.38rem',
          evidenceCardMin: '260px',
          statementCardMin: '260px',
          rowHeight: 82,
        };
      case '14':
        return {
          fontSizePx: 14,
          dockedDrawerWidth: 'clamp(560px, 58vw, 940px)',
          drawerLeftCol: 'clamp(300px, 36%, 340px)',
          videoStageHeight: 'clamp(260px, 36vh, 335px)',
          inboxFeedWidth: '320px',
          peopleCardMin: '290px',
          dossierMaxW: '95vw',
          dossierMaxH: '93vh',
          dossierTocW: '220px',
          topbarPadY: '0.36rem',
          subbarPadY: '0.42rem',
          evidenceCardMin: '245px',
          statementCardMin: '260px',
          rowHeight: 84,
        };
      case '15':
        return {
          fontSizePx: 14.25,
          dockedDrawerWidth: 'clamp(660px, 62vw, 1200px)',
          drawerLeftCol: 'clamp(310px, 35.5%, 365px)',
          videoStageHeight: 'clamp(270px, 37vh, 345px)',
          inboxFeedWidth: '335px',
          peopleCardMin: '305px',
          dossierMaxW: '95vw',
          dossierMaxH: '93vh',
          dossierTocW: '230px',
          topbarPadY: '0.34rem',
          subbarPadY: '0.4rem',
          evidenceCardMin: '250px',
          statementCardMin: '265px',
          rowHeight: 84,
        };
      case '16':
        return {
          fontSizePx: 14.5,
          dockedDrawerWidth: 'clamp(740px, 64vw, 1420px)',
          drawerLeftCol: 'clamp(330px, 35.5%, 395px)',
          videoStageHeight: 'clamp(315px, 41vh, 415px)',
          inboxFeedWidth: '350px',
          peopleCardMin: '310px',
          dossierMaxW: 'min(95.5vw, 1760px)',
          dossierMaxH: '93.5vh',
          dossierTocW: '242px',
          topbarPadY: '0.4rem',
          subbarPadY: '0.45rem',
          evidenceCardMin: '260px',
          statementCardMin: '275px',
          rowHeight: 86,
        };
      case '17':
        return {
          fontSizePx: 14.75,
          dockedDrawerWidth: 'clamp(820px, 66vw, 1640px)',
          drawerLeftCol: 'clamp(350px, 35.5%, 425px)',
          videoStageHeight: 'clamp(335px, 42vh, 440px)',
          inboxFeedWidth: '365px',
          peopleCardMin: '320px',
          dossierMaxW: 'min(96vw, 1920px)',
          dossierMaxH: '93.5vh',
          dossierTocW: '252px',
          topbarPadY: '0.42rem',
          subbarPadY: '0.48rem',
          evidenceCardMin: '270px',
          statementCardMin: '285px',
          rowHeight: 88,
        };
      case '18': {
        const eighteenFont = scale >= 1.45 ? 14.15 : scale >= 1.2 ? 14.65 : 15.1;
        return {
          fontSizePx: eighteenFont,
          dockedDrawerWidth: 'clamp(860px, 69.5vw, 2040px)',
          drawerLeftCol: 'clamp(345px, 34.5%, 460px)',
          videoStageHeight: 'clamp(355px, 44vh, 500px)',
          inboxFeedWidth: 'clamp(350px, 28vw, 435px)',
          peopleCardMin: '296px',
          dossierMaxW: 'min(96.5vw, 2240px)',
          dossierMaxH: '94.5vh',
          dossierTocW: '268px',
          topbarPadY: '0.38rem',
          subbarPadY: '0.42rem',
          evidenceCardMin: '245px',
          statementCardMin: '242px',
          rowHeight: 84,
        };
      }
      case 'desktop':
        return {
          fontSizePx: 15.25,
          dockedDrawerWidth: 'clamp(1080px, 71vw, 2800px)',
          drawerLeftCol: 'clamp(420px, 35.5%, 520px)',
          videoStageHeight: 'clamp(390px, 45vh, 540px)',
          inboxFeedWidth: '400px',
          peopleCardMin: '335px',
          dossierMaxW: 'min(95vw, 2400px)',
          dossierMaxH: '94vh',
          dossierTocW: '280px',
          topbarPadY: '0.48rem',
          subbarPadY: '0.52rem',
          evidenceCardMin: '290px',
          statementCardMin: '310px',
          rowHeight: 90,
        };
      default:
        return {
          fontSizePx: 15,
          dockedDrawerWidth: '640px',
          drawerLeftCol: '100%',
          videoStageHeight: '400px',
          inboxFeedWidth: '340px',
          peopleCardMin: '350px',
          dossierMaxW: '1260px',
          dossierMaxH: '92vh',
          dossierTocW: '240px',
          topbarPadY: '0.5rem',
          subbarPadY: '0.6rem',
          evidenceCardMin: '280px',
          statementCardMin: '300px',
          rowHeight: 92,
        };
    }
  });

  inboxRowHeight = $derived(this.geometry.rowHeight);

  activeBadgeLabel(lang: 'bg' | 'en'): string {
    const prof = this.resolvedProfile;
    if (this.preset === 'auto') {
      if (prof === 'desktop') return lang === 'bg' ? 'Авто • 24"+' : 'Auto • 24"+';
      if (prof === 'standard') return lang === 'bg' ? 'Авто' : 'Auto';
      return lang === 'bg' ? `Авто • ${prof}"${prof === '18' ? ' ★' : ''}` : `Auto • ${prof}"${prof === '18' ? ' ★' : ''}`;
    }
    const found = DISPLAY_PRESETS.find((x) => x.id === this.preset);
    if (!found) return '18" ★';
    return lang === 'bg' ? found.shortLabelBg : found.shortLabelEn;
  }

  syncDomAttributes() {
    if (typeof document === 'undefined') return;
    const root = document.documentElement;
    const g = this.geometry;
    root.setAttribute('data-widescreen', this.isActive ? 'true' : 'false');
    root.setAttribute('data-ultrawide', this.isUltraWide ? 'true' : 'false');
    root.setAttribute('data-display-profile', this.resolvedProfile);
    root.setAttribute('data-display-preset', this.preset);
    root.setAttribute('data-dual-pane', this.isDualPane ? 'true' : 'false');
    root.setAttribute('data-eighteen-inch', this.isEighteenInch ? 'true' : 'false');
    root.style.setProperty('--ui-font-size', `${g.fontSizePx}px`);
    root.style.setProperty('--drawer-docked-width', g.dockedDrawerWidth);
    root.style.setProperty('--drawer-left-col', g.drawerLeftCol);
    root.style.setProperty('--video-stage-height', g.videoStageHeight);
    root.style.setProperty('--inbox-feed-width', g.inboxFeedWidth);
    root.style.setProperty('--people-card-min', g.peopleCardMin);
    root.style.setProperty('--dossier-max-w', g.dossierMaxW);
    root.style.setProperty('--dossier-max-h', g.dossierMaxH);
    root.style.setProperty('--dossier-toc-w', g.dossierTocW);
    root.style.setProperty('--topbar-pad-y', g.topbarPadY);
    root.style.setProperty('--subbar-pad-y', g.subbarPadY);
    root.style.setProperty('--evidence-card-min', g.evidenceCardMin);
    root.style.setProperty('--statement-card-min', g.statementCardMin);
  }

  updateMetrics() {
    if (typeof window === 'undefined') return;
    this.viewportWidth = window.innerWidth;
    this.viewportHeight = window.innerHeight;
    this.screenWidth = window.screen.width;
    this.screenHeight = window.screen.height;
    this.devicePixelRatio = window.devicePixelRatio || 1;
    this.syncDomAttributes();
  }

  async initHardwareDetection() {
    this.updateMetrics();
    const hw = await fetchDisplayHardwareInfo();
    if (hw) {
      this.nativeHardware = hw;
      this.syncDomAttributes();
    }
  }

  setPreset(next: DisplayPresetId) {
    this.preset = next;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('leadfinder_display_preset', next);
      localStorage.setItem(
        'leadfinder_widescreen_mode',
        next === 'auto' ? 'auto' : next === 'off' ? 'off' : 'on'
      );
    }
    this.syncDomAttributes();
  }

  cycleMode() {
    const order: DisplayPresetId[] = ['auto', '13', '14', '15', '16', '17', '18', 'desktop', 'off'];
    const idx = order.indexOf(this.preset);
    const next = order[(idx + 1) % order.length];
    this.setPreset(next);
  }
}

export const widescreen = new WidescreenStore();
