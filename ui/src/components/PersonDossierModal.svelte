<script lang="ts">
  import type { Language } from '../lib/i18n';
  import type { Person, DetailedPersonProfile, EntityIntelEvent, SanctionsMatch, EntityPivotRef, AegisTemporalAudit } from '../lib/types';
  import { researchPerson, fetchPersonIntelStream, refreshPersonIntelStream, fetchPersonSanctions, fetchPersonWayback, refreshPersonWayback } from '../lib/api';

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
    let md = `# OSINT РАЗУЗНАВАТЕЛНО ДОСИЕ: ${person.canonical_name}\n\n`;
    if (person.role) md += `**Длъжност:** ${person.role}\n`;
    if (person.category) md += `**Категория:** ${person.category}\n`;
    if (person.rating) md += `**Рейтинг на интерес:** ${person.rating}/100\n`;
    if (person.tags && person.tags.length > 0) md += `**Тагове:** ${person.tags.map((t) => `[${t}]`).join(' ')}\n`;
    if (person.summary) md += `\n## Обобщение на личността\n${person.summary}\n`;
    if (parsedData?.profile) {
      const prof = parsedData.profile;
      if (prof.identity) md += `\n## 1. Идентичност и лични данни\n${prof.identity}\n`;
      if (prof.contact_locations) md += `\n## 2. Локации и комуникационни канали\n${prof.contact_locations}\n`;
      if (prof.network_associates) md += `\n## 3. Мрежа от контакти и съдружници\n${prof.network_associates}\n`;
      if (prof.education_career) md += `\n## 4. Образование и кариерно развитие\n${prof.education_career}\n`;
      if (prof.financial_assets) md += `\n## 5. Финансов профил и корпоративни интереси\n${prof.financial_assets}\n`;
      if (prof.legal_regulatory) md += `\n## 6. Правен и регулаторен статус\n${prof.legal_regulatory}\n`;
      if (prof.digital_media_presence) md += `\n## 7. Медийно и дигитално присъствие\n${prof.digital_media_presence}\n`;
      if (prof.affiliations_beliefs) md += `\n## 8. Политически и обществени принадлежности\n${prof.affiliations_beliefs}\n`;
      if (prof.psychographic_behavioral) md += `\n## 9. Психографски и поведенчески профил\n${prof.psychographic_behavioral}\n`;
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
    return low.includes('deepfake') || low.includes('aegis') || low.includes('клониран') || low.includes('face-swap');
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
        blocks.push({ type: 'h3', text: line.slice(4).trim() });
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
        blocks.push({ type: 'quote', text: line.slice(2).trim() });
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

  const chapters = $derived.by(() => {
    if (!parsedBlocks) return [];
    return parsedBlocks
      .filter((b) => (b.type === 'h1' || b.type === 'h2') && b.id)
      .map((b) => ({
        id: b.id!,
        text: (b as any).text,
        isMain: b.type === 'h1',
      }));
  });

  function escapeHtml(str: string): string {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  function formatInline(str: string): string {
    const escaped = escapeHtml(str);
    let formatted = escaped.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
    if (searchFilter.trim().length >= 2) {
      const filterEsc = escapeHtml(searchFilter.trim()).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const re = new RegExp(`(${filterEsc})`, 'gi');
      formatted = formatted.replace(re, '<mark class="dossier-search-hit">$1</mark>');
    }
    return formatted;
  }

  function printDossier() {
    window.print();
  }

  function downloadFile() {
    if (!person || !fullMarkdown) return;
    const safeName = person.canonical_name.replace(/[^a-zA-Z0-9\u0400-\u04FF_-]/g, '_');
    const filename = `${person.id}_${safeName}_OSINT_dossier.md`;
    const blob = new Blob([fullMarkdown], { type: 'text/markdown;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    showToast(currentLang === 'bg' ? '✓ Докладът е изтеглен като .md файл!' : '✓ Dossier file downloaded!');
  }

  async function copyContent() {
    if (!fullMarkdown) return;
    try {
      await navigator.clipboard.writeText(fullMarkdown);
      showToast(currentLang === 'bg' ? '✓ Пълният доклад е копиран!' : '✓ Full report copied!');
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при копиране' : 'Copy error');
    }
  }

  async function triggerResearch() {
    if (!person || isResearching) return;
    isResearching = true;
    showToast(currentLang === 'bg' ? '⚡ Извършва се дълбоко OSINT разузнаване (5k-10k думи)...' : '⚡ Running full deep 10k-word OSINT investigation...');
    try {
      if (onResearchTrigger) {
        await onResearchTrigger(person);
      }
      const updated = await researchPerson(person.id, person.canonical_name);
      Object.assign(person, updated);
      if (updated.detailed_profile_json) {
        try {
          const parsed = JSON.parse(updated.detailed_profile_json);
          if (parsed.temporal_audit) temporalAudit = parsed.temporal_audit;
        } catch {}
      }
      activeTab = 'report';
      showToast(currentLang === 'bg' ? '✓ Досието е успешно генерирано!' : '✓ Full dossier successfully generated!');
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при генериране на доклада' : 'Dossier generation error');
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
            <h3>📖 {currentLang === 'bg' ? 'OSINT Разузнавателно Досие:' : 'OSINT Dossier:'} {person.canonical_name}</h3>
            <span class="person-entity-badge {getEntityTypeInfo(person.entity_type).cssClass}">
              {getEntityTypeInfo(person.entity_type).icon} {getEntityTypeInfo(person.entity_type).label}
            </span>
            {#if wordCount > 0}
              <span class="dossier-word-badge">
                📄 {wordCount.toLocaleString()} {currentLang === 'bg' ? 'думи' : 'words'}
              </span>
              <span class="dossier-time-badge">
                ⏱️ ~{readingTimeMinutes} {currentLang === 'bg' ? 'мин четене' : 'min read'}
              </span>
            {/if}
          </div>
          <span class="dossier-sub">{person.role || person.category || 'Публична личност'}</span>
        </div>

        <div class="modal-actions-col">
          {#if toastMsg}
            <span class="dossier-toast">{toastMsg}</span>
          {/if}

          <button
            class="btn btn-secondary btn-sm"
            onclick={downloadFile}
            title={currentLang === 'bg' ? 'Изтегли пълния файл с доклада (.md)' : 'Download full markdown dossier file'}
          >
            💾 {currentLang === 'bg' ? 'Изтегли .md' : 'Download .md'}
          </button>

          <button
            class="btn btn-secondary btn-sm"
            onclick={copyContent}
            title={currentLang === 'bg' ? 'Копирай целия доклад в клипборда' : 'Copy dossier markdown'}
          >
            📋 {currentLang === 'bg' ? 'Копирай' : 'Copy'}
          </button>

          <button
            class="btn btn-secondary btn-sm"
            onclick={printDossier}
            title={currentLang === 'bg' ? 'Принтирай или запази като PDF' : 'Print or save as PDF'}
          >
            🖨️ {currentLang === 'bg' ? 'Печат' : 'Print'}
          </button>

          <button
            class="btn btn-primary btn-sm"
            onclick={triggerResearch}
            disabled={isResearching}
            title={currentLang === 'bg' ? 'Стартирай пълно дълбоко OSINT разузнаване с AI (5000-10000+ думи)' : 'Run 10,000-word deep OSINT research'}
          >
            {isResearching ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Генерирай 10k OSINT досие' : 'Re-run 10k OSINT')}
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
          📄 {currentLang === 'bg' ? 'Пълен разузнавателен доклад (5k - 10k думи)' : 'Exhaustive Dossier Report (5k - 10k words)'}
        </button>
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'wayback_archive'}
          onclick={() => (activeTab = 'wayback_archive')}
        >
          🏛️ {currentLang === 'bg' ? 'Wayback & Дигитална Памет' : 'Wayback & Archives'}
          {#if (temporalAudit?.historical_snapshots?.length || 0) + (temporalAudit?.internet_archive_docs?.length || 0) > 0}
            <span class="tab-badge">{(temporalAudit?.historical_snapshots?.length || 0) + (temporalAudit?.internet_archive_docs?.length || 0)}</span>
          {/if}
        </button>
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'intel_stream'}
          onclick={() => (activeTab = 'intel_stream')}
        >
          ⚡ {currentLang === 'bg' ? 'Динамичен новинарски поток' : 'Live Intel Stream'}
          {#if intelEvents.length > 0}
            <span class="tab-badge">{intelEvents.length}</span>
          {/if}
        </button>
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'sanctions'}
          onclick={() => (activeTab = 'sanctions')}
        >
          🛡️ {currentLang === 'bg' ? 'Санкции & PEP радар' : 'Sanctions & PEP Radar'}
          {#if displaySanctions.length > 0}
            <span class="tab-badge tab-badge-alert">{displaySanctions.length}</span>
          {/if}
        </button>
        <button
          class="dossier-tab-btn"
          class:active={activeTab === 'categories'}
          onclick={() => (activeTab = 'categories')}
        >
          📊 {currentLang === 'bg' ? '9-категорийна OSINT матрица' : '9-Point OSINT Matrix'}
        </button>
      </div>

      <div class="modal-body dossier-body">
        {#if activeTab === 'report'}
          {#if wordCount > 0}
            <div class="report-meta-box">
              <span class="meta-item">📁 <strong>{currentLang === 'bg' ? 'Локален файл:' : 'Local file:'}</strong> <code>data/osint/{person.id}_{person.canonical_name.replace(/[^a-zA-Z0-9\u0400-\u04FF_-]/g, '_')}_dossier.md</code></span>
              <span class="meta-item">🔍 <strong>{currentLang === 'bg' ? 'Обхват:' : 'Scope:'}</strong> {currentLang === 'bg' ? 'Многоизмерен анализ, биографични факти, фирми, съдебни дела, AEGIS deepfake мониторинг и мрежа' : 'Multi-dimensional background, corporate holdings, court records, AEGIS deepfakes & network'}</span>
            </div>

            {#if parsedData?.delta_summary}
              <div class="dossier-delta-banner">
                <span class="delta-badge-icon">🔄</span>
                <div class="delta-body">
                  <strong>{currentLang === 'bg' ? 'Динамичен делта мониторинг (Какво се промени от предходния скан):' : 'Living Dossier Delta:'}</strong>
                  <span>{parsedData.delta_summary}</span>
                </div>
              </div>
            {/if}

            {#if displaySanctions.length > 0}
              <div class="dossier-sanctions-alert">
                <span class="sanctions-badge-icon">⚠️</span>
                <div class="sanctions-body">
                  <strong>{currentLang === 'bg' ? 'ВНИМАНИЕ: Открити международни санкционни съответствия / PEP статус' : 'WARNING: International Sanctions / PEP Matches Detected'}</strong>
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
                <span class="pivots-title">🏢 {currentLang === 'bg' ? 'Картографирани бизнес пивоти:' : 'Discovered Corporate Pivots:'}</span>
                <div class="pivots-row">
                  {#each parsedData.entity_pivots as pv}
                    <span class="pivot-badge">
                      <strong>{pv.entity_name}</strong>
                      <small>({pv.entity_kind})</small>
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
                  placeholder={currentLang === 'bg' ? 'Търси в 10k-досието (фирма, име, дело, дата, deepfake)...' : 'Search in dossier (company, name, case, date, deepfake)...'}
                />
                {#if searchFilter}
                  <button class="clear-search-btn" onclick={() => (searchFilter = '')}>✕</button>
                {/if}
              </div>
            </div>

            {#if chapters.length > 0}
              <div class="chapters-quick-nav">
                <span class="chapters-nav-title">📑 {currentLang === 'bg' ? 'Глави на досието:' : 'Chapters:'}</span>
                <div class="chapters-pill-row">
                  {#each chapters as ch}
                    <a href={`#${ch.id}`} class="chapter-nav-pill" class:pill-main={ch.isMain}>
                      {ch.text.replace(/^#+\s*/, '').slice(0, 34)}
                    </a>
                  {/each}
                </div>
              </div>
            {/if}

            {#if hasAegisDeepfakeMention}
              <div class="dossier-aegis-banner">
                <span class="aegis-badge-icon">🛡️</span>
                <div>
                  <strong>{currentLang === 'bg' ? 'AEGIS Форензичен мониторинг:' : 'AEGIS Forensic Monitoring:'}</strong>
                  <span>{currentLang === 'bg' ? 'В досието са включени данни за цифрова синтетика, клонирани гласове или deepfake атаки срещу субекта.' : 'Dossier includes synthetic media records, cloned voices or deepfake impersonations targeting this figure.'}</span>
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
                  <h3 class="prose-h3">{@html formatInline(block.text)}</h3>
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
          {:else}
            <div class="empty-dossier">
              <p>{currentLang === 'bg' ? 'Все още няма генериран пълен разузнавателен доклад за това лице.' : 'No complete OSINT report generated yet for this figure.'}</p>
              <button
                class="btn btn-primary btn-sm"
                onclick={triggerResearch}
                disabled={isResearching}
              >
                {isResearching ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Генерирай пълно 10k OSINT досие сега' : 'Generate Full 10k OSINT Dossier Now')}
              </button>
            </div>
          {/if}
        {:else if activeTab === 'wayback_archive'}
          <div class="wayback-archive-container">
            <div class="wayback-header">
              <div class="wayback-info">
                <h4>🏛️ {currentLang === 'bg' ? 'AEGIS Времеви Архив & Wayback Машина' : 'AEGIS Temporal Archive & Wayback Machine'}</h4>
                <p>{currentLang === 'bg' ? 'Исторически крос-референс чрез Wayback Machine (archive.org), дигитални документи, ревизионна история на Уикипедия и детекция на заличаване на дигиталната памет.' : 'Historical cross-referencing via Wayback Machine (archive.org), Internet Archive documents, Wikipedia edit revisions, and scrubbed provenance tracking.'}</p>
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
                  <strong>{currentLang === 'bg' ? 'Оценка на риска от заличаване (Scrubbing / Retraction):' : 'Scrubbing & Alteration Risk Verdict:'}</strong>
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
                <h4>⚡ {currentLang === 'bg' ? 'Постоянен динамичен новинарски поток' : 'Live Intelligence Feed'}</h4>
                <p>{currentLang === 'bg' ? 'Автоматизирано извличане от Google News RSS, БТА, БНР и независими медийни агенции в реално време.' : 'Real-time multi-source feed from Google News RSS, BTA, BNR & national agencies.'}</p>
              </div>
              <button
                class="btn btn-primary btn-sm"
                onclick={triggerRefreshIntel}
                disabled={isRefreshingIntel}
              >
                {isRefreshingIntel ? '⏳ ' + (currentLang === 'bg' ? 'Обновяване...' : 'Refreshing...') : '🔄 ' + (currentLang === 'bg' ? 'Обнови новинарски поток' : 'Refresh Intel Stream')}
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
                    <p class="event-snippet">{ev.snippet}</p>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="empty-dossier">
                <p>{currentLang === 'bg' ? 'Все още няма записани новинарски събития за това лице. Натиснете "Обнови новинарски поток", за да извлечете актуални публикации.' : 'No live intel events recorded yet. Click "Refresh Intel Stream" to query live news feeds.'}</p>
                <button
                  class="btn btn-primary btn-sm"
                  onclick={triggerRefreshIntel}
                  disabled={isRefreshingIntel}
                >
                  {isRefreshingIntel ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Сканирай живия новинарски поток' : 'Scan Live News Feeds')}
                </button>
              </div>
            {/if}
          </div>
        {:else if activeTab === 'sanctions'}
          <div class="sanctions-radar-container">
            <div class="sanctions-radar-header">
              <div>
                <h4>🛡️ {currentLang === 'bg' ? 'Глобален санкционен & PEP радар' : 'Global Sanctions & PEP Radar'}</h4>
                <p>{currentLang === 'bg' ? 'Кръстосана проверка срещу OpenSanctions, ООН, US OFAC SDN (Закон Магнитски), ЕС, Великобритания (OFSI), Швейцария (SECO), НАЗК (Украйна) и Интерпол.' : 'Live cross-reference against OpenSanctions, UN, US OFAC SDN (Magnitsky), EU Consolidated, UK OFSI, Swiss SECO, Ukraine NAZK & Interpol.'}</p>
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
                      <p class="sanctions-summary">{sm.summary}</p>
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
                <h5>{currentLang === 'bg' ? 'Няма открити санкции или активни мерки' : 'No Sanctions or Enforcement Measures Found'}</h5>
                <p>{currentLang === 'bg' ? 'Субектът не фигурира в активните санкционни бази на САЩ (OFAC), ЕС, Обединеното кралство, Швейцария или Интерпол.' : 'Subject does not appear on active US OFAC, EU, UK OFSI, Swiss SECO or Interpol red notice lists.'}</p>
              </div>
            {/if}
          </div>
        {:else}
          {#if person.summary}
            <div class="summary-box">
              <h4>{currentLang === 'bg' ? 'Резюме на лицето' : 'Executive Summary'}</h4>
              <p>{person.summary}</p>
            </div>
          {/if}

          {#if parsedData?.profile}
            {@const prof = parsedData.profile}
            <div class="osint-categories-grid">
              {#if prof.identity}
                <div class="osint-card">
                  <h5>1. {currentLang === 'bg' ? 'Идентичност и лични данни' : 'Identity & Core Data'}</h5>
                  <p>{prof.identity}</p>
                </div>
              {/if}

              {#if prof.contact_locations}
                <div class="osint-card">
                  <h5>2. {currentLang === 'bg' ? 'Локации и дигитални адреси' : 'Locational History & Channels'}</h5>
                  <p>{prof.contact_locations}</p>
                </div>
              {/if}

              {#if prof.network_associates}
                <div class="osint-card">
                  <h5>3. {currentLang === 'bg' ? 'Мрежа от контакти и съдружници' : 'Network & Associates'}</h5>
                  <p>{prof.network_associates}</p>
                </div>
              {/if}

              {#if prof.education_career}
                <div class="osint-card">
                  <h5>4. {currentLang === 'bg' ? 'Образование и кариерно развитие' : 'Education & Career Timeline'}</h5>
                  <p>{prof.education_career}</p>
                </div>
              {/if}

              {#if prof.financial_assets}
                <div class="osint-card">
                  <h5>5. {currentLang === 'bg' ? 'Финансов профил и корпоративни интереси' : 'Financial & Wealth Profile'}</h5>
                  <p>{prof.financial_assets}</p>
                </div>
              {/if}

              {#if prof.legal_regulatory}
                <div class="osint-card">
                  <h5>6. {currentLang === 'bg' ? 'Правен и регулаторен статус (PEP)' : 'Legal, Criminal & PEP Status'}</h5>
                  <p>{prof.legal_regulatory}</p>
                </div>
              {/if}

              {#if prof.digital_media_presence}
                <div class="osint-card">
                  <h5>7. {currentLang === 'bg' ? 'Медийно присъствие и отзвук' : 'Digital & Media Footprint'}</h5>
                  <p>{prof.digital_media_presence}</p>
                </div>
              {/if}

              {#if prof.affiliations_beliefs}
                <div class="osint-card">
                  <h5>8. {currentLang === 'bg' ? 'Политически принадлежности и каузи' : 'Affiliations & Beliefs'}</h5>
                  <p>{prof.affiliations_beliefs}</p>
                </div>
              {/if}

              {#if prof.psychographic_behavioral}
                <div class="osint-card">
                  <h5>9. {currentLang === 'bg' ? 'Комуникационен стил и наративи' : 'Behavioral & Narrative Traits'}</h5>
                  <p>{prof.psychographic_behavioral}</p>
                </div>
              {/if}
            </div>
          {:else}
            <div class="empty-dossier">
              <p>{currentLang === 'bg' ? 'Няма въведени 9-категорийни данни.' : 'No 9-point matrix data yet.'}</p>
              <button
                class="btn btn-primary btn-sm"
                onclick={triggerResearch}
                disabled={isResearching}
              >
                {isResearching ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Стартирай разузнаване' : 'Start Research')}
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
    width: 92%;
    max-width: 960px;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5);
    overflow: hidden;
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
