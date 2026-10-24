<script lang="ts">
  import type { Language } from '../lib/i18n';
  import type { Person, DetailedPersonProfile } from '../lib/types';
  import { researchPerson } from '../lib/api';

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

  let activeTab = $state<'report' | 'categories'>('report');
  let isResearching = $state(false);
  let toastMsg = $state<string | null>(null);

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
      if (parsed.profile && typeof parsed.profile === 'object') {
        return {
          profile: parsed.profile as DetailedPersonProfile,
          report_markdown: (parsed.report_markdown as string) || '',
        };
      }
      return {
        profile: parsed as DetailedPersonProfile,
        report_markdown: (parsed.report_markdown as string) || '',
      };
    } catch {
      return null;
    }
  });

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

  type Block =
    | { type: 'h1'; text: string }
    | { type: 'h2'; text: string }
    | { type: 'h3'; text: string }
    | { type: 'p'; text: string }
    | { type: 'ul'; items: string[] }
    | { type: 'hr' }
    | { type: 'quote'; text: string };

  const parsedBlocks = $derived.by(() => {
    if (!fullMarkdown) return [];
    const lines = fullMarkdown.split('\n');
    const blocks: Block[] = [];
    let currentList: string[] = [];

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
        blocks.push({ type: 'h2', text: line.slice(3).trim() });
      } else if (line.startsWith('# ')) {
        flushList();
        blocks.push({ type: 'h1', text: line.slice(2).trim() });
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

  function escapeHtml(str: string): string {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  function formatInline(str: string): string {
    const escaped = escapeHtml(str);
    return escaped.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
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
    showToast(currentLang === 'bg' ? '⚡ Извършва се 2000-думи OSINT разузнаване...' : '⚡ Running full 2000-word OSINT investigation...');
    try {
      if (onResearchTrigger) {
        await onResearchTrigger(person);
      } else {
        const updated = await researchPerson(person.id, person.canonical_name);
        Object.assign(person, updated);
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
            class="btn btn-primary btn-sm"
            onclick={triggerResearch}
            disabled={isResearching}
            title={currentLang === 'bg' ? 'Стартирай пълно дълбоко OSINT разузнаване с AI (~2000 думи)' : 'Run 2000-word deep OSINT research'}
          >
            {isResearching ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Генерирай отново' : 'Re-run OSINT')}
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
          📄 {currentLang === 'bg' ? 'Пълен разузнавателен доклад (~2000 думи)' : 'Complete Investigative Report'}
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
              <span class="meta-item">🔍 <strong>{currentLang === 'bg' ? 'Обхват:' : 'Scope:'}</strong> {currentLang === 'bg' ? 'Многоизмерен анализ, биографични факти, фирми, съдебни дела и мрежа' : 'Multi-dimensional background, corporate holdings, court records & network'}</span>
            </div>

            <div class="dossier-report-prose">
              {#each parsedBlocks as block}
                {#if block.type === 'h1'}
                  <h1 class="prose-h1">{@html formatInline(block.text)}</h1>
                {:else if block.type === 'h2'}
                  <h2 class="prose-h2">{@html formatInline(block.text)}</h2>
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
                {isResearching ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Генерирай пълен 2000-думи доклад сега' : 'Generate Full 2000-Word Report Now')}
              </button>
            </div>
          {/if}
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
</style>
