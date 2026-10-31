<script lang="ts">
  import type { Language } from '../lib/i18n';
  import type { Person, DetailedPersonProfile, EntityIntelEvent, SanctionsMatch, EntityPivotRef, AegisTemporalAudit } from '../lib/types';
  import { researchPerson, fetchPersonIntelStream, refreshPersonIntelStream, fetchPersonSanctions, fetchPersonWayback, refreshPersonWayback } from '../lib/api';
  import { startAiJob, finishAiJob } from '../lib/aiTasks.svelte';
  import { highlightInlineMarkdown, highlightPlainTextToHtml, escapeHtml } from '../lib/reportHighlighter';
  import BookReaderToc, { type TocItem } from './BookReaderToc.svelte';

  let {
    person,
    currentLang,
    onClose,
    onResearchTrigger,
  }: {
    person: Person | null;
    currentLang: Language;
    onClose: () => void;
    onResearchTrigger?: (p: Person) => Promise<void>;
  } = $props();

  let activeTab = $state<'report' | 'wayback_archive' | 'intel_stream' | 'sanctions' | 'categories'>('report');
  let isResearching = $state(false);
  let toastMsg = $state<string | null>(null);
  let searchFilter = $state('');
  let enableKeywordHighlights = $state(true);
  let reportScrollEl = $state<HTMLElement | null>(null);
  let intelEvents = $state<EntityIntelEvent[]>([]);
  let isRefreshingIntel = $state(false);
  let liveSanctions = $state<SanctionsMatch[]>([]);
  let sanctionsCheckedAt = $state<string | null>(null);
  let isCheckingSanctions = $state(false);
  let temporalAudit = $state<AegisTemporalAudit | null>(null);
  let isRefreshingWayback = $state(false);

  $effect(() => {
    if (person?.id) {
      loadIntelStream();
      loadSanctionsRadar();
      loadWaybackAudit();
    }
  });

  async function loadIntelStream() {
    if (!person) return;
    try {
      intelEvents = await fetchPersonIntelStream(person.id);
    } catch {
    }
  }

  async function triggerRefreshIntel() {
    if (!person || isRefreshingIntel) return;
    isRefreshingIntel = true;
    try {
      const res = await refreshPersonIntelStream(person.id);
      intelEvents = res.events;
      showToast(currentLang === 'bg' ? `✓ Открити ${res.newly_added} нови събития!` : `✓ Discovered ${res.newly_added} new events!`);
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при обновяване на потока' : 'Error refreshing stream');
    } finally {
      isRefreshingIntel = false;
    }
  }

  async function loadSanctionsRadar() {
    if (!person) return;
    isCheckingSanctions = true;
    try {
      const res = await fetchPersonSanctions(person.id);
      liveSanctions = res.matches;
      sanctionsCheckedAt = res.checked_at;
    } catch {
    } finally {
      isCheckingSanctions = false;
    }
  }

  async function loadWaybackAudit() {
    if (!person) return;
    if (parsedData?.temporal_audit) {
      temporalAudit = parsedData.temporal_audit;
      return;
    }
    try {
      const res = await fetchPersonWayback(person.id);
      if (res.ok && res.temporal_audit) {
        temporalAudit = res.temporal_audit;
      }
    } catch {
    }
  }

  async function triggerRefreshWayback() {
    if (!person || isRefreshingWayback) return;
    isRefreshingWayback = true;
    try {
      const res = await refreshPersonWayback(person.id);
      if (res.ok && res.temporal_audit) {
        temporalAudit = res.temporal_audit;
        showToast(currentLang === 'bg' ? '✓ Времевите архиви са обновени!' : '✓ Temporal archives refreshed!');
      }
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при обновяване на архивите' : 'Error refreshing archives');
    } finally {
      isRefreshingWayback = false;
    }
  }

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
      case 'infrastructure':
        return { label: currentLang === 'bg' ? 'Инфраструктура' : 'Infrastructure', icon: '🛤️', cssClass: 'type-infrastructure' };
      case 'building':
        return { label: currentLang === 'bg' ? 'Сграда / Обект' : 'Building', icon: '🏢', cssClass: 'type-building' };
      default:
        return { label: currentLang === 'bg' ? 'Личност' : 'Person', icon: '👤', cssClass: 'type-person' };
    }
  }

  function showToast(msg: string) {
    toastMsg = msg;
    setTimeout(() => {
      if (toastMsg === msg) toastMsg = null;
    }, 3000);
  }

  const parsedData = $derived.by(() => {
    if (!person || !person.detailed_profile_json) return null;
    try {
      const parsed = JSON.parse(person.detailed_profile_json);
      const prof = (parsed.profile && typeof parsed.profile === 'object') ? parsed.profile : parsed;
      return {
        profile: prof as DetailedPersonProfile,
        report_markdown: (parsed.report_markdown as string) || '',
        delta_summary: (parsed.delta_summary as string) || prof?.delta_summary || null,
        sanctions_matches: (parsed.sanctions_matches as SanctionsMatch[]) || prof?.sanctions_matches || [],
        entity_pivots: (parsed.entity_pivots as EntityPivotRef[]) || prof?.entity_pivots || [],
        temporal_audit: (parsed.temporal_audit as AegisTemporalAudit) || prof?.temporal_audit || null,
      };
    } catch {
      return null;
    }
  });

  const displaySanctions = $derived(liveSanctions.length > 0 ? liveSanctions : (parsedData?.sanctions_matches || []));

  const fullMarkdown = $derived.by(() => {
    if (parsedData?.report_markdown) {
      return parsedData.report_markdown;
    }
    if (!person) return '';
    let md = `# АНАЛИТИЧНА СПРАВКА: ${person.canonical_name}\n\n`;
    if (person.role) md += `**Длъжност / Статут:** ${person.role}\n`;
    if (person.category) md += `**Категория:** ${person.category}\n`;
    if (person.tags && person.tags.length > 0) md += `**Етикети:** ${person.tags.map((t) => `[${t}]`).join(' ')}\n`;
    if (person.summary) md += `\n## Обобщение\n${person.summary}\n`;
    if (parsedData?.profile) {
      const prof = parsedData.profile;
      if (prof.identity) md += `\n## 1. Основни данни и биография\n${prof.identity}\n`;
      if (prof.contact_locations) md += `\n## 2. Седалище и местоположения\n${prof.contact_locations}\n`;
      if (prof.network_associates) md += `\n## 3. Свързани лица и организации\n${prof.network_associates}\n`;
      if (prof.education_career) md += `\n## 4. Образование и длъжности\n${prof.education_career}\n`;
      if (prof.financial_assets) md += `\n## 5. Търговски регистър, дружества и активи\n${prof.financial_assets}\n`;
      if (prof.legal_regulatory) md += `\n## 6. Съдебни производства, КПКОНПИ и санкции\n${prof.legal_regulatory}\n`;
      if (prof.digital_media_presence) md += `\n## 7. Медийни публикации и архиви\n${prof.digital_media_presence}\n`;
      if (prof.affiliations_beliefs) md += `\n## 8. Членства и публични позиции\n${prof.affiliations_beliefs}\n`;
      if (prof.psychographic_behavioral) md += `\n## 9. Комуникационен профил и тези\n${prof.psychographic_behavioral}\n`;
    }
    return md;
  });

  const wordCount = $derived.by(() => {
    if (!fullMarkdown) return 0;
    return fullMarkdown.trim().split(/\s+/).filter(Boolean).length;
  });

  const readingTimeMinutes = $derived(Math.max(1, Math.ceil(wordCount / 240)));

  const hasAegisDeepfakeMention = $derived.by(() => {
    if (!fullMarkdown) return false;
    const low = fullMarkdown.toLowerCase();
    return low.includes('deepfake') || low.includes('клониран') || low.includes('face-swap');
  });

  type Block =
    | { type: 'h1'; text: string; id?: string }
    | { type: 'h2'; text: string; id?: string }
    | { type: 'h3'; text: string; id?: string }
    | { type: 'p'; text: string; id?: string }
    | { type: 'ul'; items: string[]; id?: string }
    | { type: 'hr'; id?: string }
    | { type: 'quote'; text: string; id?: string };

  const parsedBlocks = $derived.by(() => {
    if (!fullMarkdown) return [];
    const lines = fullMarkdown.split('\n');
    const blocks: Block[] = [];
    let currentList: string[] = [];
    let chapterIndex = 0;
    let subsecIndex = 0;

    function flushList() {
      if (currentList.length > 0) {
        blocks.push({ type: 'ul', items: [...currentList] });
        currentList = [];
      }
    }

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (!line) {
        flushList();
        continue;
      }
      if (line.startsWith('### ')) {
        flushList();
        subsecIndex += 1;
        blocks.push({ type: 'h3', text: line.slice(4).trim(), id: `subsec-${subsecIndex}` });
      } else if (line.startsWith('## ')) {
        flushList();
        chapterIndex += 1;
        blocks.push({ type: 'h2', text: line.slice(3).trim(), id: `chapter-${chapterIndex}` });
      } else if (line.startsWith('# ')) {
        flushList();
        chapterIndex += 1;
        blocks.push({ type: 'h1', text: line.slice(2).trim(), id: `chapter-${chapterIndex}` });
      } else if (line === '---' || line === '***') {
        flushList();
        blocks.push({ type: 'hr' });
      } else if (line.startsWith('> ')) {
        flushList();
        const qTxt = line.slice(2).trim();
        if (qTxt !== '[!NOTE]' && qTxt !== '[!IMPORTANT]' && qTxt !== '[!WARNING]') {
          blocks.push({ type: 'quote', text: qTxt });
        }
      } else if (line.startsWith('- ') || line.startsWith('* ') || line.startsWith('• ')) {
        currentList.push(line.replace(/^[-*•]\s+/, ''));
      } else {
        flushList();
        blocks.push({ type: 'p', text: line });
      }
    }
    flushList();
    return blocks;
  });

  const tocItems = $derived.by<TocItem[]>(() => {
    if (!parsedBlocks) return [];
    const list: TocItem[] = [];
    for (const b of parsedBlocks) {
      if ((b.type === 'h1' || b.type === 'h2' || b.type === 'h3') && b.id) {
        const cleanLabel = b.text.replace(/^#+\s*/, '').replace(/\*\*/g, '');
        if (/^(АНАЛИТИЧНА СПРАВКА|OSINT РАЗУЗНАВАТЕЛНО ДОСИЕ)/i.test(cleanLabel) && list.length === 0) {
          list.push({ id: b.id, label: currentLang === 'bg' ? 'Начало на справката' : 'Overview', level: 1 });
          continue;
        }
        const level: 1 | 2 | 3 = b.type === 'h1' ? 1 : b.type === 'h2' ? 2 : 2;
        list.push({ id: b.id, label: cleanLabel, level: b.type === 'h3' ? 2 : 1 });
      }
    }
    return list;
  });

  function formatInline(str: string): string {
    let formatted = highlightInlineMarkdown(str, enableKeywordHighlights);
    if (searchFilter.trim().length >= 2) {
      const filterEsc = escapeHtml(searchFilter.trim()).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const re = new RegExp(`(${filterEsc})`, 'gi');
      const parts = formatted.split(/(<[^>]+>)/g);
      for (let i = 0; i < parts.length; i += 2) {
        if (parts[i]) {
          parts[i] = parts[i].replace(re, '<mark class="dossier-search-hit">$1</mark>');
        }
      }
      formatted = parts.join('');
    }
    return formatted;
  }

  function printDossier() {
    window.print();
  }

  function downloadFile() {
    if (!person || !fullMarkdown) return;
    const safeName = person.canonical_name.replace(/[^a-zA-Z0-9\u0400-\u04FF_-]/g, '_');
    const filename = `${person.id}_${safeName}_spravka.md`;
    const blob = new Blob([fullMarkdown], { type: 'text/markdown;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    showToast(currentLang === 'bg' ? '✓ Справката е изтеглена (.md)' : '✓ Report downloaded');
  }

  async function copyContent() {
    if (!fullMarkdown) return;
    try {
      await navigator.clipboard.writeText(fullMarkdown);
      showToast(currentLang === 'bg' ? '✓ Справката е копирана' : '✓ Full report copied');
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при копиране' : 'Copy error');
    }
  }

  async function triggerResearch() {
    if (!person || isResearching) return;
    isResearching = true;
    const subjectId = person.id;
    const subjectName = person.canonical_name;
    startAiJob('person-research', subjectId, subjectName);
    showToast(currentLang === 'bg' ? '⏳ Изготвяне на аналитична справка...' : '⏳ Generating analytical report...');
    try {
      if (onResearchTrigger) {
        await onResearchTrigger(person);
      }
      const updated = await researchPerson(subjectId, subjectName);
      Object.assign(person, updated);
      if (updated.detailed_profile_json) {
        try {
          const parsed = JSON.parse(updated.detailed_profile_json);
          if (parsed.temporal_audit) temporalAudit = parsed.temporal_audit;
        } catch {}
      }
      activeTab = 'report';
      finishAiJob('person-research', subjectId, currentLang === 'bg' ? `✓ Справката за ${subjectName} е готова` : `✓ Report for ${subjectName} ready`, true);
      showToast(currentLang === 'bg' ? '✓ Справката е обновена' : '✓ Report updated');
    } catch {
      finishAiJob('person-research', subjectId, currentLang === 'bg' ? `Грешка при проучване на ${subjectName}` : `Research error for ${subjectName}`, false);
      showToast(currentLang === 'bg' ? 'Грешка при изготвяне на справката' : 'Report generation error');
    } finally {
      isResearching = false;
    }
  }
</script>

{#if person}
  <div class="modal-backdrop" onclick={onClose} role="presentation">
    <div
      class="modal-dialog dossier-modal"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      role="dialog"
      tabindex="-1"
    >
      <div class="modal-head">
        <div class="modal-title-col">
          <div class="title-row">
            <h3>📖 {currentLang === 'bg' ? 'Аналитична справка:' : 'Analytical Report:'} {person.canonical_name}</h3>
            <span class="person-entity-badge {getEntityTypeInfo(person.entity_type).cssClass}">
              {getEntityTypeInfo(person.entity_type).icon} {getEntityTypeInfo(person.entity_type).label}
            </span>
            {#if wordCount > 0}
              <span class="dossier-word-badge">
                📄 {wordCount.toLocaleString()} {currentLang === 'bg' ? 'думи' : 'words'}
              </span>
              <span class="dossier-time-badge">
                ⏱️ ~{readingTimeMinutes} {currentLang === 'bg' ? 'мин' : 'min'}
              </span>
            {/if}
          </div>
          <span class="dossier-sub">{person.role || person.category || getEntityTypeInfo(person.entity_type).label}</span>
        </div>

        <div class="modal-actions-col">
          {#if toastMsg}
            <span class="dossier-toast">{toastMsg}</span>
          {/if}

          <button
            class="btn btn-secondary btn-sm"
            onclick={downloadFile}
            title={currentLang === 'bg' ? 'Изтегли справката като .md файл' : 'Download report file (.md)'}
          >
            💾 {currentLang === 'bg' ? 'Изтегли .md' : 'Download .md'}
          </button>

          <button
            class="btn btn-secondary btn-sm"
            onclick={copyContent}
            title={currentLang === 'bg' ? 'Копирай цялата справка' : 'Copy full report'}
          >
            📋 {currentLang === 'bg' ? 'Копирай' : 'Copy'}
          </button>

          <button
            class="btn btn-secondary btn-sm"
            onclick={printDossier}
            title={currentLang === 'bg' ? 'Печат или запис като PDF' : 'Print or save as PDF'}
          >
            🖨️ {currentLang === 'bg' ? 'Печат' : 'Print'}
          </button>

          <button
            class="btn btn-primary btn-sm"
            onclick={triggerResearch}
            disabled={isResearching}
            title={currentLang === 'bg' ? 'Обнови справката по публични регистри, дела и архиви' : 'Refresh report from public registries and archives'}
          >
            {isResearching
              ? '⏳ ' + (currentLang === 'bg' ? 'Проучване...' : 'Running...')
              : '🔄 ' + (currentLang === 'bg' ? 'Обнови справката' : 'Refresh Report')}
          </button>

          <button class="close-btn" onclick={onClose}>✕</button>
        </div>
      </div>

      <div class="dossier-nav-tabs">
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'report'}
          onclick={() => (activeTab = 'report')}
        >
          📄 {currentLang === 'bg' ? 'Пълен доклад' : 'Full Report'}
        </button>
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'wayback_archive'}
          onclick={() => (activeTab = 'wayback_archive')}
        >
          🏛️ {currentLang === 'bg' ? 'Архивни версии' : 'Web Archives'}
          {#if (temporalAudit?.historical_snapshots?.length || 0) + (temporalAudit?.internet_archive_docs?.length || 0) > 0}
            <span class="tab-badge">{(temporalAudit?.historical_snapshots?.length || 0) + (temporalAudit?.internet_archive_docs?.length || 0)}</span>
          {/if}
        </button>
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'intel_stream'}
          onclick={() => (activeTab = 'intel_stream')}
        >
          📰 {currentLang === 'bg' ? 'Новинарски публикации' : 'News Feed'}
          {#if intelEvents.length > 0}
            <span class="tab-badge">{intelEvents.length}</span>
          {/if}
        </button>
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'sanctions'}
          onclick={() => (activeTab = 'sanctions')}
        >
          ⚖️ {currentLang === 'bg' ? 'Санкционни списъци' : 'Sanctions Lists'}
          {#if displaySanctions.length > 0}
            <span class="tab-badge tab-badge-alert">{displaySanctions.length}</span>
          {/if}
        </button>
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'categories'}
          onclick={() => (activeTab = 'categories')}
        >
          📊 {currentLang === 'bg' ? 'Обобщени данни по теми' : 'Topic Summary'}
        </button>
      </div>

      <div class="modal-body dossier-body" class:report-reader-mode={activeTab === 'report' && wordCount > 0}>
        {#if activeTab === 'report'}
          {#if wordCount > 0}
            <div class="dossier-book-layout">
              <BookReaderToc
                items={tocItems}
                scrollContainer={reportScrollEl}
                title={currentLang === 'bg' ? 'Съдържание' : 'Contents'}
                variant="sidebar"
              />

              <div class="dossier-book-page" bind:this={reportScrollEl}>
                <div class="report-meta-box">
                  <span class="meta-item">📁 <strong>{currentLang === 'bg' ? 'Файл:' : 'File:'}</strong> <code>data/osint/{person.id}_{person.canonical_name.replace(/[^a-zA-Z0-9\u0400-\u04FF_-]/g, '_')}_dossier.md</code></span>
                  <span class="meta-item">🔍 <strong>{currentLang === 'bg' ? 'Обхват:' : 'Scope:'}</strong> {currentLang === 'bg' ? 'Публични регистри, търговски участия, съдебни актове, обществени поръчки и архиви' : 'Public registries, corporate records, judicial filings, procurement & archives'}</span>
                </div>

                {#if parsedData?.delta_summary}
                  <div class="dossier-delta-banner">
                    <span class="delta-badge-icon">🔄</span>
                    <div class="delta-body">
                      <strong>{currentLang === 'bg' ? 'Промени спрямо предходната проверка:' : 'Changes Since Previous Check:'}</strong>
                      <span>{@html formatInline(parsedData.delta_summary)}</span>
                    </div>
                  </div>
                {/if}

                {#if displaySanctions.length > 0}
                  <div class="dossier-sanctions-alert">
                    <span class="sanctions-badge-icon">⚠️</span>
                    <div class="sanctions-body">
                      <strong>{currentLang === 'bg' ? 'Вписвания в международни санкционни или PEP списъци' : 'International Sanctions / PEP Records'}</strong>
                      <div class="sanctions-badges-row">
                        {#each displaySanctions as sm}
                          <span class="sanctions-pill">
                            <strong>[{sm.dataset}]</strong> {sm.caption} ({(sm.match_score * 100).toFixed(0)}%)
                          </span>
                        {/each}
                      </div>
                    </div>
                  </div>
                {/if}

                {#if parsedData?.entity_pivots && parsedData.entity_pivots.length > 0}
                  <div class="dossier-pivots-box">
                    <span class="pivots-title">🏢 {currentLang === 'bg' ? 'Свързани дружества и организации:' : 'Linked Entities & Companies:'}</span>
                    <div class="pivots-row">
                      {#each parsedData.entity_pivots as pv}
                        <span class="pivot-badge">
                          <strong>{pv.entity_name}</strong>
                          <small>({pv.entity_kind}{pv.registry_status ? ` • ${pv.registry_status}` : ''})</small>
                        </span>
                      {/each}
                    </div>
                  </div>
                {/if}

                <div class="dossier-toolbar">
                  <div class="dossier-search-bar">
                    <span class="search-icon">🔍</span>
                    <input
                      type="text"
                      bind:value={searchFilter}
                      placeholder={currentLang === 'bg' ? 'Търси в справката (дружество, ЕИК, дело, дата, институция)...' : 'Search in report (company, ID, case, date, institution)...'}
                    />
                    {#if searchFilter}
                      <button class="clear-search-btn" onclick={() => (searchFilter = '')}>✕</button>
                    {/if}
                  </div>

                  <div class="highlight-toolbar-group">
                    <button
                      type="button"
                      class="highlight-toggle-btn"
                      class:active={enableKeywordHighlights}
                      onclick={() => (enableKeywordHighlights = !enableKeywordHighlights)}
                    >
                      🎨 {currentLang === 'bg' ? 'Маркиране' : 'Highlights'}
                    </button>
                    {#if enableKeywordHighlights}
                      <div class="highlight-legend">
                        <span class="leg-chip leg-date">{currentLang === 'bg' ? 'Дати' : 'Dates'}</span>
                        <span class="leg-chip leg-id">{currentLang === 'bg' ? 'ЕИК / Дела' : 'IDs / Cases'}</span>
                        <span class="leg-chip leg-money">{currentLang === 'bg' ? 'Суми / %' : 'Amounts'}</span>
                        <span class="leg-chip leg-entity">{currentLang === 'bg' ? 'Дружества / Органи' : 'Entities'}</span>
                      </div>
                    {/if}
                  </div>
                </div>

                {#if hasAegisDeepfakeMention}
                  <div class="dossier-aegis-banner">
                    <span class="aegis-badge-icon">🎞️</span>
                    <div>
                      <strong>{currentLang === 'bg' ? 'Проверка на аудио-визуални записи:' : 'Media Authenticity Note:'}</strong>
                      <span>{currentLang === 'bg' ? 'Справката съдържа данни от проверка за синтетичен глас или видео манипулации.' : 'Report includes synthetic audio/video inspection findings.'}</span>
                    </div>
                  </div>
                {/if}

                <div class="dossier-report-prose">
                  {#each parsedBlocks as block}
                    {#if block.type === 'h1'}
                      <h1 class="prose-h1" id={block.id}>{@html formatInline(block.text)}</h1>
                    {:else if block.type === 'h2'}
                      <h2 class="prose-h2" id={block.id}>{@html formatInline(block.text)}</h2>
                    {:else if block.type === 'h3'}
                      <h3 class="prose-h3" id={block.id}>{@html formatInline(block.text)}</h3>
                    {:else if block.type === 'quote'}
                      <blockquote class="prose-quote">{@html formatInline(block.text)}</blockquote>
                    {:else if block.type === 'hr'}
                      <hr class="prose-hr" />
                    {:else if block.type === 'ul'}
                      <ul class="prose-ul">
                        {#each block.items as itm}
                          <li>{@html formatInline(itm)}</li>
                        {/each}
                      </ul>
                    {:else}
                      <p class="prose-p">{@html formatInline(block.text)}</p>
                    {/if}
                  {/each}
                </div>
              </div>
            </div>
          {:else}
            <div class="empty-dossier">
              <p>{currentLang === 'bg' ? 'Все още няма изготвена справка за този обект.' : 'No analytical report generated yet for this subject.'}</p>
              <button
                class="btn btn-primary btn-sm"
                onclick={triggerResearch}
                disabled={isResearching}
              >
                {isResearching ? '⏳ ...' : '🔄 ' + (currentLang === 'bg' ? 'Изготви справка' : 'Generate Report')}
              </button>
            </div>
          {/if}
        {:else if activeTab === 'wayback_archive'}
          <div class="wayback-archive-container">
            <div class="wayback-header">
              <div class="wayback-info">
                <h4>🏛️ {currentLang === 'bg' ? 'Архивни уеб версии и история на публикациите' : 'Web Archives & Revision History'}</h4>
                <p>{currentLang === 'bg' ? 'Проверка на запазени страници в Wayback Machine (archive.org), документи в Internet Archive и история на редакциите в Уикипедия.' : 'Saved snapshots from Wayback Machine (archive.org), Internet Archive documents, and Wikipedia edit revisions.'}</p>
                {#if temporalAudit}
                  <span class="wayback-stats-summary">
                    🕒 {currentLang === 'bg' ? 'Проверени архивни снимки:' : 'Snapshots checked:'} <strong>{temporalAudit.snapshots_checked}</strong>
                    {#if temporalAudit.oldest_capture}
                      | {currentLang === 'bg' ? 'Най-ранна снимка:' : 'Earliest capture:'} <strong>{temporalAudit.oldest_capture}</strong>
                    {/if}
                    {#if temporalAudit.newest_capture}
                      | {currentLang === 'bg' ? 'Най-нова снимка:' : 'Newest capture:'} <strong>{temporalAudit.newest_capture}</strong>
                    {/if}
                  </span>
                {/if}
              </div>
              <button
                class="btn btn-secondary btn-sm"
                onclick={triggerRefreshWayback}
                disabled={isRefreshingWayback}
              >
                {isRefreshingWayback ? '...' : '🔄 ' + (currentLang === 'bg' ? 'Обнови архивите' : 'Refresh Archives')}
              </button>
            </div>

            {#if temporalAudit}
              <div class="wayback-provenance-banner">
                <span class="provenance-icon">🛡️</span>
                <div class="provenance-body">
                  <strong>{currentLang === 'bg' ? 'Състояние на архивната следа:' : 'Archive Continuity:'}</strong>
                  <span class="risk-pill">{temporalAudit.scrubbed_or_altered_risk}</span>
                  {#if temporalAudit.provenance_notes && temporalAudit.provenance_notes.length > 0}
                    <ul class="provenance-notes-list">
                      {#each temporalAudit.provenance_notes as note}
                        <li>{note}</li>
                      {/each}
                    </ul>
                  {/if}
                </div>
              </div>

              {#if temporalAudit.historical_snapshots && temporalAudit.historical_snapshots.length > 0}
                <div class="wayback-section">
                  <h5>📸 {currentLang === 'bg' ? 'Исторически заснети копия в Wayback Machine:' : 'Wayback Machine Captured Snapshots:'}</h5>
                  <div class="wayback-grid">
                    {#each temporalAudit.historical_snapshots as snap}
                      <div class="wayback-card">
                        <div class="wayback-card-top">
                          <span class="wayback-service-badge">{snap.service}</span>
                          <span class="wayback-ts-badge">{snap.timestamp}</span>
                          <span class="wayback-status-badge">HTTP {snap.status}</span>
                        </div>
                        <div class="wayback-orig-url"><code>{snap.original_url}</code></div>
                        <a href={snap.url} target="_blank" rel="noopener noreferrer" class="wayback-link">
                          🔗 {currentLang === 'bg' ? 'Отвори архивната снимка в Wayback' : 'View Snapshot on Wayback Machine'} ↗
                        </a>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}

              {#if temporalAudit.internet_archive_docs && temporalAudit.internet_archive_docs.length > 0}
                <div class="wayback-section">
                  <h5>📚 {currentLang === 'bg' ? 'Архивирани книги, аудиозаписи и документи (Internet Archive):' : 'Archived Media & Books (Internet Archive):'}</h5>
                  <div class="archive-docs-list">
                    {#each temporalAudit.internet_archive_docs as doc}
                      <div class="archive-doc-card">
                        <div class="doc-header">
                          <span class="doc-mediatype-badge">{doc.mediatype || 'документ'}</span>
                          {#if doc.date}
                            <span class="doc-date-badge">{doc.date}</span>
                          {/if}
                        </div>
                        <h5 class="doc-title">{doc.title}</h5>
                        <a href={doc.detail_url} target="_blank" rel="noopener noreferrer" class="archive-doc-link">
                          📥 {currentLang === 'bg' ? 'Преглед в archive.org' : 'Inspect on archive.org'} ↗
                        </a>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}

              {#if temporalAudit.wiki_bio?.revisions && temporalAudit.wiki_bio.revisions.length > 0}
                <div class="wayback-section">
                  <h5>📝 {currentLang === 'bg' ? 'Хронология на редакциите в Уикипедия (Редакционни спорове и корекции):' : 'Wikipedia Revision Log (Edit Wars & Historical Modifiers):'}</h5>
                  <div class="wiki-revisions-list">
                    {#each temporalAudit.wiki_bio.revisions as rev}
                      <div class="wiki-revision-item">
                        <span class="rev-time">🕒 {rev.timestamp}</span>
                        <span class="rev-user">👤 <code>{rev.user}</code></span>
                        <span class="rev-comment">{rev.comment ? `„${rev.comment}“` : (currentLang === 'bg' ? '(без коментар)' : '(no comment)')}</span>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}
            {:else}
              <div class="empty-wayback-box">
                <span class="empty-icon">🏛️</span>
                <h5>{currentLang === 'bg' ? 'Няма заредени архивни данни' : 'No Temporal Archive Data Loaded'}</h5>
                <p>{currentLang === 'bg' ? 'Натиснете бутона по-горе за проверка на историческите снимки в Wayback Machine и Internet Archive.' : 'Click the button above to query Wayback Machine and Internet Archive records.'}</p>
              </div>
            {/if}
          </div>
        {:else if activeTab === 'intel_stream'}
          <div class="intel-stream-container">
            <div class="intel-stream-header">
              <div class="stream-info">
                <h4>📰 {currentLang === 'bg' ? 'Новинарски публикации и медиен мониторинг' : 'News Feed & Media Coverage'}</h4>
                <p>{currentLang === 'bg' ? 'Актуални публикации от Google News, БТА, БНР и национални медии.' : 'Recent coverage from Google News, BTA, BNR & national outlets.'}</p>
              </div>
              <button
                class="btn btn-primary btn-sm"
                onclick={triggerRefreshIntel}
                disabled={isRefreshingIntel}
              >
                {isRefreshingIntel ? '⏳ ' + (currentLang === 'bg' ? 'Обновяване...' : 'Refreshing...') : '🔄 ' + (currentLang === 'bg' ? 'Обнови новините' : 'Refresh News')}
              </button>
            </div>

            {#if intelEvents.length > 0}
              <div class="intel-events-list">
                {#each intelEvents as ev}
                  <div class="intel-event-card">
                    <div class="event-meta">
                      <span class="event-source">{ev.source_name}</span>
                      <span class="event-time">{new Date(ev.discovered_at).toLocaleString(currentLang === 'bg' ? 'bg-BG' : 'en-US')}</span>
                    </div>
                    <h5 class="event-title">
                      <a href={ev.url} target="_blank" rel="noopener noreferrer">
                        {ev.title} ↗
                      </a>
                    </h5>
                    <p class="event-snippet">{@html highlightPlainTextToHtml(ev.snippet, enableKeywordHighlights)}</p>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="empty-dossier">
                <p>{currentLang === 'bg' ? 'Все още няма заредени новинарски публикации за този обект.' : 'No news articles recorded yet.'}</p>
                <button
                  class="btn btn-primary btn-sm"
                  onclick={triggerRefreshIntel}
                  disabled={isRefreshingIntel}
                >
                  {isRefreshingIntel ? '...' : '🔄 ' + (currentLang === 'bg' ? 'Зареди новини' : 'Fetch News')}
                </button>
              </div>
            {/if}
          </div>
        {:else if activeTab === 'sanctions'}
          <div class="sanctions-radar-container">
            <div class="sanctions-radar-header">
              <div>
                <h4>⚖️ {currentLang === 'bg' ? 'Международни санкционни и PEP регистри' : 'International Sanctions & PEP Registries'}</h4>
                <p>{currentLang === 'bg' ? 'Проверка в OpenSanctions, ООН, US OFAC SDN (Магнитски), ЕС, Великобритания (OFSI), Швейцария (SECO), НАЗК и Интерпол.' : 'Cross-reference against OpenSanctions, UN, US OFAC SDN (Magnitsky), EU Consolidated, UK OFSI, Swiss SECO, Ukraine NAZK & Interpol.'}</p>
                {#if sanctionsCheckedAt}
                  <span class="sanctions-checked-time">🕒 {currentLang === 'bg' ? 'Последна проверка:' : 'Last checked:'} {new Date(sanctionsCheckedAt).toLocaleString(currentLang === 'bg' ? 'bg-BG' : 'en-US')}</span>
                {/if}
              </div>
              <button
                class="btn btn-secondary btn-sm"
                onclick={loadSanctionsRadar}
                disabled={isCheckingSanctions}
              >
                {isCheckingSanctions ? '...' : '🔄 ' + (currentLang === 'bg' ? 'Провери отново' : 'Re-verify Sanctions')}
              </button>
            </div>

            {#if displaySanctions.length > 0}
              <div class="sanctions-cards-list">
                {#each displaySanctions as sm}
                  <div class="sanctions-card">
                    <div class="sanctions-card-head">
                      <span class="sanctions-dataset-badge">{sm.dataset}</span>
                      <span class="sanctions-score-badge">
                        {currentLang === 'bg' ? 'Съвпадение:' : 'Match:'} {(sm.match_score * 100).toFixed(0)}%
                      </span>
                    </div>
                    <h5 class="sanctions-caption">{sm.caption}</h5>
                    {#if sm.program}
                      <div class="sanctions-row">
                        <strong>{currentLang === 'bg' ? 'Програма:' : 'Program:'}</strong> {sm.program}
                      </div>
                    {/if}
                    {#if sm.sanctor}
                      <div class="sanctions-row">
                        <strong>{currentLang === 'bg' ? 'Орган:' : 'Authority:'}</strong> {sm.sanctor}
                      </div>
                    {/if}
                    {#if sm.countries && sm.countries.length > 0}
                      <div class="sanctions-row">
                        <strong>{currentLang === 'bg' ? 'Юрисдикции:' : 'Jurisdictions:'}</strong> {sm.countries.join(', ')}
                      </div>
                    {/if}
                    {#if sm.summary}
                      <p class="sanctions-summary">{@html highlightPlainTextToHtml(sm.summary, enableKeywordHighlights)}</p>
                    {/if}
                    {#if sm.source_url}
                      <a href={sm.source_url} target="_blank" rel="noopener noreferrer" class="sanctions-link">
                        🔗 {currentLang === 'bg' ? 'Официален регистър / Източник' : 'Official Registry Record'} ↗
                      </a>
                    {/if}
                  </div>
                {/each}
              </div>
            {:else}
              <div class="clean-sanctions-box">
                <span class="clean-icon">✓</span>
                <h5>{currentLang === 'bg' ? 'Няма открити вписвания в санкционни списъци' : 'No Sanctions Records Found'}</h5>
                <p>{currentLang === 'bg' ? 'Не са установени активни санкционни мерки в проверените бази на ООН, САЩ (OFAC), ЕС, Великобритания, Швейцария и Интерпол.' : 'No active records on UN, US OFAC, EU, UK OFSI, Swiss SECO or Interpol lists.'}</p>
              </div>
            {/if}
          </div>
        {:else}
          {#if person.summary}
            <div class="summary-box">
              <h4>{currentLang === 'bg' ? 'Резюме' : 'Summary'}</h4>
              <p>{@html highlightPlainTextToHtml(person.summary, enableKeywordHighlights)}</p>
            </div>
          {/if}

          {#if parsedData?.profile}
            {@const prof = parsedData.profile}
            <div class="osint-categories-grid">
              {#if prof.identity}
                <div class="osint-card">
                  <h5>1. {currentLang === 'bg' ? 'Основни данни и биография' : 'Identity & Core Data'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.identity, enableKeywordHighlights)}</p>
                </div>
              {/if}

              {#if prof.contact_locations}
                <div class="osint-card">
                  <h5>2. {currentLang === 'bg' ? 'Седалище и местоположения' : 'Locations & Addresses'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.contact_locations, enableKeywordHighlights)}</p>
                </div>
              {/if}

              {#if prof.network_associates}
                <div class="osint-card">
                  <h5>3. {currentLang === 'bg' ? 'Свързани лица и организации' : 'Network & Associates'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.network_associates, enableKeywordHighlights)}</p>
                </div>
              {/if}

              {#if prof.education_career}
                <div class="osint-card">
                  <h5>4. {currentLang === 'bg' ? 'Образование и длъжности' : 'Education & Career'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.education_career, enableKeywordHighlights)}</p>
                </div>
              {/if}

              {#if prof.financial_assets}
                <div class="osint-card">
                  <h5>5. {currentLang === 'bg' ? 'Търговски регистър, дружества и активи' : 'Corporate & Financial Records'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.financial_assets, enableKeywordHighlights)}</p>
                </div>
              {/if}

              {#if prof.legal_regulatory}
                <div class="osint-card">
                  <h5>6. {currentLang === 'bg' ? 'Съдебни производства, КПКОНПИ и регулатори' : 'Judicial & Regulatory Records'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.legal_regulatory, enableKeywordHighlights)}</p>
                </div>
              {/if}

              {#if prof.digital_media_presence}
                <div class="osint-card">
                  <h5>7. {currentLang === 'bg' ? 'Медийно присъствие и публикации' : 'Media Coverage'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.digital_media_presence, enableKeywordHighlights)}</p>
                </div>
              {/if}

              {#if prof.affiliations_beliefs}
                <div class="osint-card">
                  <h5>8. {currentLang === 'bg' ? 'Членства и публични позиции' : 'Affiliations & Positions'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.affiliations_beliefs, enableKeywordHighlights)}</p>
                </div>
              {/if}

              {#if prof.psychographic_behavioral}
                <div class="osint-card">
                  <h5>9. {currentLang === 'bg' ? 'Комуникационен стил и теми' : 'Communication & Themes'}</h5>
                  <p>{@html highlightPlainTextToHtml(prof.psychographic_behavioral, enableKeywordHighlights)}</p>
                </div>
              {/if}
            </div>
          {:else}
            <div class="empty-dossier">
              <p>{currentLang === 'bg' ? 'Няма налични обобщени данни по теми.' : 'No structured topic summary yet.'}</p>
              <button
                class="btn btn-primary btn-sm"
                onclick={triggerResearch}
                disabled={isResearching}
              >
                {isResearching ? '⏳ ...' : '🔄 ' + (currentLang === 'bg' ? 'Изготви справка' : 'Generate Report')}
              </button>
            </div>
          {/if}
        {/if}
      </div>

      <div class="modal-foot">
        <button class="btn btn-secondary btn-sm" onclick={onClose}>
          {currentLang === 'bg' ? 'Затвори' : 'Close'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(3px);
  }

  .dossier-modal {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 10px;
    width: 95%;
    max-width: var(--dossier-max-w, 1260px);
    max-height: var(--dossier-max-h, 92vh);
    display: flex;
    flex-direction: column;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }

  :global([data-widescreen='true']) .dossier-modal {
    width: 96%;
    max-width: var(--dossier-max-w, min(95vw, 1780px));
    max-height: var(--dossier-max-h, 93.5vh);
  }

  :global([data-display-profile='18']) .dossier-modal {
    width: 96.5%;
    border-radius: 12px;
    box-shadow: 0 20px 52px rgba(0, 0, 0, 0.58);
  }

  :global([data-display-profile='18']) .dossier-book-layout {
    gap: 1.25rem;
  }

  :global([data-display-profile='18']) .dossier-book-page {
    padding: 0.25rem 0.65rem 0.75rem 0.25rem;
    line-height: 1.62;
  }

  :global([data-display-profile='13']) .modal-head,
  :global([data-display-profile='15']) .modal-head {
    padding: 0.65rem 0.95rem;
  }

  :global([data-display-profile='18']) .modal-head {
    padding: 0.75rem 1.25rem;
  }

  .dossier-body.report-reader-mode {
    overflow: hidden;
    padding: 0.9rem 1.1rem;
  }

  .dossier-book-layout {
    display: flex;
    gap: 1rem;
    flex: 1;
    min-height: 0;
    height: 100%;
  }

  .dossier-book-page {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    padding-right: 0.35rem;
  }

  .highlight-toolbar-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.45rem;
  }

  .highlight-toggle-btn {
    padding: 3px 9px;
    font-size: 0.74rem;
    font-weight: 600;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--card-bg);
    color: var(--text-muted);
    cursor: pointer;
  }

  .highlight-toggle-btn.active {
    border-color: rgba(16, 185, 129, 0.45);
    color: #10b981;
    background: rgba(16, 185, 129, 0.1);
  }

  .highlight-legend {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .leg-chip {
    font-size: 0.68rem;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 4px;
  }

  .leg-date {
    background: rgba(16, 185, 129, 0.16);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.32);
  }

  .leg-id {
    background: rgba(56, 189, 248, 0.15);
    color: #38bdf8;
    border: 1px solid rgba(56, 189, 248, 0.3);
  }

  .leg-money {
    background: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
    border: 1px solid rgba(245, 158, 11, 0.3);
  }

  .leg-entity {
    background: rgba(139, 92, 246, 0.15);
    color: #a78bfa;
    border: 1px solid rgba(139, 92, 246, 0.3);
  }

  .person-entity-badge.type-infrastructure {
    background: rgba(14, 165, 233, 0.14);
    color: #0ea5e9;
    border: 1px solid rgba(14, 165, 233, 0.32);
  }

  .person-entity-badge.type-building {
    background: rgba(20, 184, 166, 0.14);
    color: #14b8a6;
    border: 1px solid rgba(20, 184, 166, 0.32);
  }

  @media (max-width: 900px) {
    .dossier-book-layout {
      flex-direction: column;
    }
    .dossier-body.report-reader-mode {
      overflow-y: auto;
    }
  }

  .modal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.9rem 1.25rem;
    border-bottom: 1px solid var(--border-color);
    background: var(--card-bg);
    gap: 1rem;
    flex-wrap: wrap;
  }

  .modal-title-col {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    flex-wrap: wrap;
  }

  .modal-title-col h3 {
    margin: 0;
    font-size: 1.05rem;
    color: var(--text-heading);
    font-weight: 700;
  }

  .dossier-word-badge {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 0.15rem 0.5rem;
    border-radius: 9999px;
    background: rgba(37, 99, 235, 0.15);
    color: var(--primary-accent);
    border: 1px solid rgba(37, 99, 235, 0.3);
  }

  .dossier-time-badge {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 0.15rem 0.5rem;
    border-radius: 9999px;
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.3);
  }

  .dossier-toolbar {
    padding: 0.6rem 1.25rem 0.2rem;
  }

  .dossier-search-bar {
    display: flex;
    align-items: center;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.35rem 0.65rem;
    gap: 0.5rem;
  }

  .dossier-search-bar input {
    background: transparent;
    border: none;
    color: var(--text-heading);
    font-size: 0.82rem;
    width: 100%;
    outline: none;
  }

  .clear-search-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0 4px;
  }

  .chapters-quick-nav {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 1.25rem;
    background: rgba(0, 0, 0, 0.15);
    border-bottom: 1px solid var(--border-color);
    overflow-x: auto;
    white-space: nowrap;
  }

  .chapters-nav-title {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .chapters-pill-row {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .chapter-nav-pill {
    font-size: 0.7rem;
    padding: 2px 8px;
    border-radius: 4px;
    background: var(--card-bg);
    color: var(--text-body);
    border: 1px solid var(--border-color);
    text-decoration: none;
    transition: all 0.2s ease;
    flex-shrink: 0;
  }

  .chapter-nav-pill:hover {
    background: var(--primary-accent);
    color: #fff;
    border-color: var(--primary-accent);
  }

  .chapter-nav-pill.pill-main {
    font-weight: 700;
    border-color: rgba(99, 102, 241, 0.4);
    background: rgba(99, 102, 241, 0.1);
  }

  .dossier-aegis-banner {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    margin: 0.6rem 1.25rem 0;
    padding: 0.6rem 0.85rem;
    border-radius: 6px;
    background: rgba(239, 68, 68, 0.12);
    border: 1px solid rgba(239, 68, 68, 0.35);
    color: #fca5a5;
    font-size: 0.78rem;
  }

  .dossier-aegis-banner strong {
    color: #fee2e2;
  }

  .aegis-badge-icon {
    font-size: 1.2rem;
  }

  .dossier-search-hit {
    background: rgba(245, 158, 11, 0.4);
    color: #fff;
    border-radius: 2px;
    padding: 0 2px;
  }

  .person-entity-badge {
    font-size: 0.68rem;
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

  .dossier-sub {
    font-size: 0.8rem;
    color: var(--primary-accent);
  }

  .modal-actions-col {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .dossier-toast {
    font-size: 0.78rem;
    color: #16a34a;
    font-weight: 600;
  }

  .close-btn {
    background: transparent;
    border: none;
    font-size: 1.2rem;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    border-radius: 4px;
  }

  .close-btn:hover {
    color: var(--text-heading);
    background: var(--hover-bg);
  }

  .dossier-nav-tabs {
    display: flex;
    gap: 0.5rem;
    padding: 0.5rem 1.25rem;
    border-bottom: 1px solid var(--border-color);
    background: var(--bg-canvas);
  }

  .dossier-tab-btn {
    background: transparent;
    border: 1px solid transparent;
    padding: 0.4rem 0.85rem;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-muted);
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .dossier-tab-btn:hover {
    color: var(--text-heading);
    background: var(--hover-bg);
  }

  .dossier-tab-btn.active {
    color: var(--primary-accent);
    background: var(--card-bg);
    border-color: var(--border-color);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .dossier-body {
    padding: 1.25rem;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    flex: 1;
    min-height: 380px;
  }

  .report-meta-box {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.65rem 0.85rem;
    background: var(--bg-canvas);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  .report-meta-box code {
    font-size: 0.75rem;
    padding: 0.1rem 0.3rem;
    border-radius: 4px;
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    color: var(--text-body);
  }

  .dossier-report-prose {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1.5rem 1.75rem;
    color: var(--text-body);
    font-size: 0.92rem;
    line-height: 1.65;
  }

  .prose-h1 {
    font-size: 1.35rem;
    font-weight: 800;
    color: var(--text-heading);
    margin: 0.5rem 0 0.25rem;
    border-bottom: 2px solid var(--primary-accent);
    padding-bottom: 0.4rem;
  }

  .prose-h2 {
    font-size: 1.12rem;
    font-weight: 700;
    color: var(--text-heading);
    margin: 1.25rem 0 0.25rem;
    padding-bottom: 0.25rem;
    border-bottom: 1px solid var(--border-color);
  }

  .prose-h3 {
    font-size: 0.98rem;
    font-weight: 700;
    color: var(--primary-accent);
    margin: 0.75rem 0 0.15rem;
  }

  .prose-p {
    margin: 0;
  }

  .prose-quote {
    margin: 0.5rem 0;
    padding: 0.5rem 0.85rem;
    border-left: 3px solid var(--primary-accent);
    background: var(--bg-canvas);
    border-radius: 0 4px 4px 0;
    font-style: italic;
  }

  .prose-hr {
    border: none;
    border-top: 1px solid var(--border-color);
    margin: 1rem 0;
  }

  .prose-ul {
    margin: 0;
    padding-left: 1.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .prose-ul li {
    line-height: 1.5;
  }

  .summary-box {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
  }

  .summary-box h4 {
    margin: 0 0 0.4rem;
    font-size: 0.9rem;
    color: var(--text-heading);
  }

  .summary-box p {
    margin: 0;
    font-size: 0.85rem;
    line-height: 1.5;
    color: var(--text-body);
  }

  .osint-categories-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 0.85rem;
  }

  .osint-card {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.85rem;
  }

  .osint-card h5 {
    margin: 0 0 0.35rem;
    font-size: 0.82rem;
    font-weight: 700;
    color: var(--primary-accent);
  }

  .osint-card p {
    margin: 0;
    font-size: 0.82rem;
    color: var(--text-body);
    line-height: 1.45;
  }

  .empty-dossier {
    text-align: center;
    padding: 3rem 1.5rem;
    color: var(--text-muted);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.85rem;
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 0.75rem 1.25rem;
    border-top: 1px solid var(--border-color);
    background: var(--card-bg);
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.45rem 0.85rem;
    border-radius: 6px;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.15s ease;
  }

  .btn-sm {
    padding: 0.35rem 0.65rem;
    font-size: 0.78rem;
  }

  .btn-primary {
    background: var(--primary-accent);
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    opacity: 0.9;
  }

  .btn-secondary {
    background: var(--card-bg);
    border-color: var(--border-color);
    color: var(--text-body);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--hover-bg);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .dossier-delta-banner {
    display: flex;
    align-items: flex-start;
    gap: 0.85rem;
    padding: 0.85rem 1.15rem;
    margin-bottom: 1rem;
    background: rgba(13, 110, 253, 0.08);
    border: 1px solid rgba(13, 110, 253, 0.25);
    border-radius: 8px;
    font-size: 0.88rem;
  }

  .delta-badge-icon {
    font-size: 1.4rem;
  }

  .delta-body {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .dossier-sanctions-alert {
    display: flex;
    align-items: flex-start;
    gap: 0.85rem;
    padding: 0.85rem 1.15rem;
    margin-bottom: 1rem;
    background: rgba(220, 53, 69, 0.1);
    border: 1px solid rgba(220, 53, 69, 0.35);
    border-radius: 8px;
  }

  .sanctions-badge-icon {
    font-size: 1.4rem;
  }

  .sanctions-body {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .sanctions-badges-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .sanctions-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    background: rgba(220, 53, 69, 0.18);
    border: 1px solid rgba(220, 53, 69, 0.4);
    color: #ff6b6b;
    padding: 0.2rem 0.55rem;
    border-radius: 4px;
    font-size: 0.78rem;
  }

  .dossier-pivots-box {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
    padding: 0.65rem 1rem;
    margin-bottom: 1rem;
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    font-size: 0.82rem;
  }

  .pivots-title {
    font-weight: 700;
    color: var(--text-muted);
  }

  .pivots-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .pivot-badge {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--border-color);
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.78rem;
  }

  .tab-badge {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-muted);
    font-size: 0.72rem;
    padding: 0.1rem 0.4rem;
    border-radius: 10px;
    margin-left: 0.3rem;
  }

  .tab-badge-alert {
    background: #dc3545;
    color: #fff;
  }

  .intel-stream-container, .sanctions-radar-container {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .intel-stream-header, .sanctions-radar-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--border-color);
  }

  .stream-info h4, .sanctions-radar-header h4 {
    margin: 0 0 0.25rem 0;
    font-size: 1.1rem;
  }

  .stream-info p, .sanctions-radar-header p {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .sanctions-checked-time {
    display: inline-block;
    margin-top: 0.4rem;
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  .intel-events-list {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .intel-event-card {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.85rem 1.15rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .event-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.75rem;
  }

  .event-source {
    font-weight: 700;
    color: var(--primary-accent);
    background: rgba(13, 110, 253, 0.1);
    padding: 0.15rem 0.45rem;
    border-radius: 4px;
  }

  .event-time {
    color: var(--text-muted);
  }

  .event-title {
    margin: 0;
    font-size: 0.95rem;
  }

  .event-title a {
    color: var(--text-body);
    text-decoration: none;
    font-weight: 600;
  }

  .event-title a:hover {
    color: var(--primary-accent);
    text-decoration: underline;
  }

  .event-snippet {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-muted);
    line-height: 1.45;
  }

  .sanctions-cards-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 1rem;
  }

  .sanctions-card {
    background: var(--card-bg);
    border: 1px solid rgba(220, 53, 69, 0.4);
    border-radius: 8px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .sanctions-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .sanctions-dataset-badge {
    background: rgba(220, 53, 69, 0.15);
    color: #ff6b6b;
    border: 1px solid rgba(220, 53, 69, 0.3);
    font-size: 0.75rem;
    font-weight: 700;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
  }

  .sanctions-score-badge {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--text-muted);
  }

  .sanctions-caption {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 700;
  }

  .sanctions-row {
    font-size: 0.82rem;
  }

  .sanctions-summary {
    margin: 0.25rem 0 0 0;
    font-size: 0.82rem;
    color: var(--text-muted);
  }

  .sanctions-link {
    margin-top: 0.35rem;
    font-size: 0.8rem;
    color: var(--primary-accent);
    text-decoration: none;
  }

  .sanctions-link:hover {
    text-decoration: underline;
  }

  .clean-sanctions-box {
    text-align: center;
    padding: 3rem 1.5rem;
    background: rgba(40, 167, 69, 0.06);
    border: 1px solid rgba(40, 167, 69, 0.2);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
  }

  .clean-icon {
    font-size: 2.5rem;
    color: #28a745;
  }

  .clean-sanctions-box h5 {
    margin: 0;
    font-size: 1.15rem;
    color: #28a745;
  }

  .clean-sanctions-box p {
    margin: 0;
    font-size: 0.88rem;
    color: var(--text-muted);
    max-width: 500px;
  }

  .wayback-archive-container {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    padding-bottom: 2rem;
  }

  .wayback-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--border-color);
  }

  .wayback-info h4 {
    margin: 0 0 0.4rem 0;
    font-size: 1.25rem;
  }

  .wayback-info p {
    margin: 0 0 0.5rem 0;
    font-size: 0.88rem;
    color: var(--text-muted);
    line-height: 1.45;
  }

  .wayback-stats-summary {
    display: inline-block;
    font-size: 0.82rem;
    color: var(--primary-accent);
    background: rgba(13, 110, 253, 0.08);
    padding: 0.25rem 0.6rem;
    border-radius: 4px;
    border: 1px solid rgba(13, 110, 253, 0.2);
  }

  .wayback-provenance-banner {
    display: flex;
    gap: 1rem;
    background: rgba(255, 193, 7, 0.08);
    border: 1px solid rgba(255, 193, 7, 0.35);
    border-radius: 8px;
    padding: 1rem 1.25rem;
    align-items: flex-start;
  }

  .provenance-icon {
    font-size: 1.75rem;
  }

  .provenance-body {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    flex: 1;
  }

  .risk-pill {
    display: inline-block;
    align-self: flex-start;
    padding: 0.2rem 0.6rem;
    border-radius: 4px;
    font-size: 0.78rem;
    font-weight: 700;
    background: rgba(255, 193, 7, 0.25);
    color: #ffd166;
    border: 1px solid rgba(255, 193, 7, 0.4);
  }

  .provenance-notes-list {
    margin: 0.35rem 0 0 1.2rem;
    padding: 0;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .provenance-notes-list li {
    margin-bottom: 0.25rem;
  }

  .wayback-section {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .wayback-section h5 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .wayback-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 1rem;
  }

  .wayback-card {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .wayback-card-top {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .wayback-service-badge {
    background: rgba(13, 110, 253, 0.15);
    color: #4da3ff;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 700;
  }

  .wayback-ts-badge {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-family: monospace;
  }

  .wayback-status-badge {
    margin-left: auto;
    font-size: 0.75rem;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    background: rgba(40, 167, 69, 0.15);
    color: #28a745;
    font-weight: 600;
  }

  .wayback-orig-url {
    font-size: 0.82rem;
    word-break: break-all;
    color: var(--text-muted);
  }

  .wayback-link {
    font-size: 0.82rem;
    color: var(--primary-accent);
    text-decoration: none;
    font-weight: 600;
  }

  .wayback-link:hover {
    text-decoration: underline;
  }

  .archive-docs-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 1rem;
  }

  .archive-doc-card {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .doc-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .doc-mediatype-badge {
    font-size: 0.72rem;
    background: rgba(108, 117, 125, 0.2);
    padding: 0.15rem 0.45rem;
    border-radius: 4px;
    text-transform: uppercase;
    font-weight: 700;
  }

  .doc-date-badge {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .doc-title {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
    line-height: 1.35;
  }

  .archive-doc-link {
    margin-top: auto;
    font-size: 0.82rem;
    color: var(--primary-accent);
    text-decoration: none;
    font-weight: 600;
  }

  .archive-doc-link:hover {
    text-decoration: underline;
  }

  .wiki-revisions-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .wiki-revision-item {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.6rem 0.85rem;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.82rem;
  }

  .rev-time {
    color: var(--text-muted);
    white-space: nowrap;
    font-size: 0.78rem;
  }

  .rev-user {
    font-weight: 600;
    white-space: nowrap;
  }

  .rev-comment {
    color: var(--text-muted);
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty-wayback-box {
    text-align: center;
    padding: 3rem 1.5rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px dashed var(--border-color);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
  }

  .empty-wayback-box .empty-icon {
    font-size: 2.5rem;
    opacity: 0.6;
  }

  .empty-wayback-box h5 {
    margin: 0;
    font-size: 1.15rem;
  }

  .empty-wayback-box p {
    margin: 0;
    font-size: 0.88rem;
    color: var(--text-muted);
    max-width: 480px;
  }
</style>
