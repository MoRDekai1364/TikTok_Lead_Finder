<script lang="ts">
  import { onMount } from 'svelte';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { Person, DetailedPersonProfile } from '../lib/types';
  import {
    fetchPeople,
    createPerson,
    updatePerson,
    deletePerson,
    researchPerson,
  } from '../lib/api';

  let { currentLang }: { currentLang: Language } = $props();
  const t = $derived(translations[currentLang]);

  let people = $state<Person[]>([]);
  let isLoading = $state(false);
  let searchQuery = $state('');
  let selectedCategory = $state('all');
  let selectedSort = $state('rating');
  let toastMsg = $state<string | null>(null);

  let researchingId = $state<number | null>(null);
  let editingPerson = $state<Person | null>(null);
  let viewingDossier = $state<Person | null>(null);
  let isCreatingNew = $state(false);

  let newName = $state('');
  let newRole = $state('');
  let newCategory = $state('Политик');
  let newRating = $state(70);
  let newTagsStr = $state('');
  let newNotes = $state('');

  const CATEGORIES = [
    { id: 'all', bg: 'Всички категории', en: 'All Categories' },
    { id: 'Политик', bg: 'Политици', en: 'Politicians' },
    { id: 'Медицинско лице', bg: 'Лекари и медици', en: 'Medics' },
    { id: 'Ключова фигура', bg: 'Ключови позиции', en: 'Key Positions' },
    { id: 'Магистрат', bg: 'Съдии и прокурори', en: 'Judiciary' },
    { id: 'Бизнесмен', bg: 'Бизнес и инвеститори', en: 'Business' },
    { id: 'Общественик', bg: 'Общественици и НПО', en: 'Public Figures' },
    { id: 'Медийна личност', bg: 'Журналисти и водещи', en: 'Media' },
  ];

  function showToast(msg: string) {
    toastMsg = msg;
    setTimeout(() => {
      if (toastMsg === msg) toastMsg = null;
    }, 3000);
  }

  async function loadData() {
    isLoading = true;
    try {
      people = await fetchPeople({
        query: searchQuery || undefined,
        category: selectedCategory !== 'all' ? selectedCategory : undefined,
        sort_by: selectedSort,
      });
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при зареждане на списъка' : 'Error loading people');
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadData();
  });

  async function handleSearch() {
    await loadData();
  }

  async function handleTriggerResearch(p: Person) {
    if (researchingId) return;
    researchingId = p.id;
    showToast(currentLang === 'bg' ? `⚡ Стартирано OSINT проучване за ${p.canonical_name}...` : `⚡ Researching ${p.canonical_name}...`);
    try {
      const updated = await researchPerson(p.id, p.canonical_name);
      people = people.map((item) => (item.id === p.id ? { ...item, ...updated } : item));
      showToast(currentLang === 'bg' ? `✓ Досието на ${p.canonical_name} е обновено!` : `✓ Dossier updated!`);
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при AI проучването' : 'Research error');
    } finally {
      researchingId = null;
    }
  }

  async function handleQuickAddTag(p: Person) {
    const input = prompt(currentLang === 'bg' ? 'Въведете нов таг (напр. [десен], [министър], [НАТО]):' : 'Enter tag:');
    if (!input || !input.trim()) return;
    const tag = input.trim();
    const currTags = p.tags || [];
    if (currTags.includes(tag)) return;
    const nextTags = [...currTags, tag];
    try {
      await updatePerson(p.id, { tags: nextTags });
      p.tags = nextTags;
      showToast(currentLang === 'bg' ? '✓ Тагът е добавен!' : '✓ Tag added!');
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при запис на тага' : 'Tag update error');
    }
  }

  async function handleRemoveTag(p: Person, tagToRemove: string) {
    const nextTags = (p.tags || []).filter((t) => t !== tagToRemove);
    try {
      await updatePerson(p.id, { tags: nextTags });
      p.tags = nextTags;
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка' : 'Error');
    }
  }

  async function handleRatingChange(p: Person, newR: number) {
    p.rating = newR;
    try {
      await updatePerson(p.id, { rating: newR });
    } catch {}
  }

  async function handleDeletePerson(p: Person) {
    if (!confirm(currentLang === 'bg' ? `Сигурни ли сте, че искате да премахнете ${p.canonical_name}?` : `Delete ${p.canonical_name}?`)) return;
    try {
      await deletePerson(p.id);
      people = people.filter((item) => item.id !== p.id);
      showToast(currentLang === 'bg' ? '✓ Премахнато' : '✓ Deleted');
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при премахване' : 'Delete error');
    }
  }

  async function handleSaveNewPerson() {
    if (!newName.trim()) return;
    const tagsArr = newTagsStr
      .split(',')
      .map((t) => t.trim().replace(/^\[|\]$/g, ''))
      .filter(Boolean);
    try {
      const res = await createPerson({
        canonical_name: newName.trim(),
        role: newRole.trim() || undefined,
        category: newCategory,
        rating: newRating,
        tags: tagsArr,
        notes: newNotes.trim() || undefined,
        public_figure: true,
      });
      if (res && res.id) {
        showToast(currentLang === 'bg' ? '✓ Лицето е добавено!' : '✓ Person created!');
        isCreatingNew = false;
        newName = '';
        newRole = '';
        newTagsStr = '';
        newNotes = '';
        await loadData();
      }
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при създаване' : 'Creation error');
    }
  }

  async function handleSaveEditedPerson() {
    if (!editingPerson || !editingPerson.canonical_name.trim()) return;
    try {
      await updatePerson(editingPerson.id, editingPerson);
      showToast(currentLang === 'bg' ? '✓ Промените са запазени!' : '✓ Saved!');
      editingPerson = null;
      await loadData();
    } catch {
      showToast(currentLang === 'bg' ? 'Грешка при запазване' : 'Save error');
    }
  }

  function parseProfile(p: Person): DetailedPersonProfile | null {
    if (!p.detailed_profile_json) return null;
    try {
      return JSON.parse(p.detailed_profile_json);
    } catch {
      return null;
    }
  }
</script>

<div class="people-screen">
  <header class="screen-header">
    <div class="header-titles">
      <h2>👥 {currentLang === 'bg' ? 'Регистър на публични личности и профилиране' : 'People Registry & OSINT Profiling'}</h2>
      <p class="header-desc">
        {currentLang === 'bg'
          ? 'Постоянен каталог на засечени политици, магистрати, лекари и общественици с рейтинг за приоритизиране, неограничени тагове и дълбоко разузнаване.'
          : 'Permanent directory of detected figures with prioritization rating, tags, and multi-dimensional OSINT dossiers.'}
      </p>
    </div>

    <div class="header-actions">
      {#if toastMsg}
        <span class="screen-toast">{toastMsg}</span>
      {/if}
      <button class="btn btn-primary btn-sm" onclick={() => (isCreatingNew = true)}>
        + {currentLang === 'bg' ? 'Добави личност' : 'Add Person'}
      </button>
    </div>
  </header>

  <div class="filter-bar">
    <div class="search-input-wrap">
      <input
        type="search"
        placeholder={currentLang === 'bg' ? 'Търсене по име, длъжност или таг...' : 'Search by name, role, tag...'}
        bind:value={searchQuery}
        onkeydown={(e) => e.key === 'Enter' && handleSearch()}
      />
      <button class="btn btn-secondary btn-sm" onclick={handleSearch}>
        🔍 {currentLang === 'bg' ? 'Търси' : 'Search'}
      </button>
    </div>

    <div class="select-group">
      <select bind:value={selectedCategory} onchange={loadData}>
        {#each CATEGORIES as cat}
          <option value={cat.id}>{currentLang === 'bg' ? cat.bg : cat.en}</option>
        {/each}
      </select>

      <select bind:value={selectedSort} onchange={loadData}>
        <option value="rating">{currentLang === 'bg' ? 'Сортирай по Рейтинг' : 'Sort by Rating'}</option>
        <option value="mentions">{currentLang === 'bg' ? 'Сортирай по Брой споменавания' : 'Sort by Mentions'}</option>
        <option value="researched">{currentLang === 'bg' ? 'Последно проучени' : 'Recently Researched'}</option>
        <option value="name">{currentLang === 'bg' ? 'Азбучен ред (Име)' : 'Alphabetical'}</option>
      </select>
    </div>
  </div>

  <main class="people-grid-container">
    {#if isLoading}
      <div class="loading-state">
        <div class="spinner"></div>
        <span>{currentLang === 'bg' ? 'Зареждане на списъка...' : 'Loading people...'}</span>
      </div>
    {:else if people.length === 0}
      <div class="empty-state">
        <p>{currentLang === 'bg' ? 'Няма намерени личности според избраните критерии.' : 'No people found.'}</p>
        <button class="btn btn-secondary btn-sm" onclick={() => (isCreatingNew = true)}>
          + {currentLang === 'bg' ? 'Създай първи запис' : 'Create first person'}
        </button>
      </div>
    {:else}
      <div class="people-grid">
        {#each people as person (person.id)}
          <div class="person-card">
            <div class="card-head">
              <div class="person-identity">
                <span class="person-avatar">{person.canonical_name.slice(0, 2).toUpperCase()}</span>
                <div>
                  <h3 class="person-name">{person.canonical_name}</h3>
                  <span class="person-category-badge">{person.category || 'Личност'}</span>
                </div>
              </div>

              <div class="rating-widget" title="Приоритетен рейтинг на лицето (0-100)">
                <span class="rating-label">Рейтинг:</span>
                <span class="rating-value">{person.rating ?? 50}</span>
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={person.rating ?? 50}
                  onchange={(e) => handleRatingChange(person, Number((e.target as HTMLInputElement).value))}
                  class="rating-slider"
                />
              </div>
            </div>

            {#if person.role}
              <div class="person-role">
                💼 {person.role}
              </div>
            {/if}

            <div class="tags-section">
              {#if person.tags && person.tags.length > 0}
                {#each person.tags as tag}
                  <span class="tag-pill">
                    #{tag}
                    <button class="tag-remove-btn" onclick={() => handleRemoveTag(person, tag)}>✕</button>
                  </span>
                {/each}
              {/if}
              <button class="tag-add-btn" onclick={() => handleQuickAddTag(person)}>
                + {currentLang === 'bg' ? 'таг' : 'tag'}
              </button>
            </div>

            {#if person.summary}
              <p class="person-summary">
                {person.summary}
              </p>
            {/if}

            <div class="claims-stat-bar">
              <span class="stat-item" title="Общо засечени видеа">
                🎬 {person.mentions ?? 0} {currentLang === 'bg' ? 'видеа' : 'videos'}
              </span>
              {#if person.researched_at}
                <span class="stat-item stat-researched" title={person.researched_at}>
                  ✓ {currentLang === 'bg' ? 'Проучен с OSINT' : 'Researched'}
                </span>
              {/if}
            </div>

            <div class="card-actions">
              <button
                class="btn btn-primary btn-sm"
                onclick={() => handleTriggerResearch(person)}
                disabled={researchingId === person.id}
                title="Стартирай автоматично 9-степенно OSINT проучване с AI"
              >
                {researchingId === person.id ? '...' : '⚡ ' + (currentLang === 'bg' ? 'Проучи лицето' : 'OSINT Research')}
              </button>

              <button
                class="btn btn-secondary btn-sm"
                onclick={() => (viewingDossier = person)}
                title="Отвори пълното досие и структуриран разузнавателен профил"
              >
                📖 {currentLang === 'bg' ? 'Досие' : 'Dossier'}
              </button>

              <button
                class="btn btn-secondary btn-sm"
                onclick={() => (editingPerson = { ...person })}
                title="Редактирай данни"
              >
                ✏️
              </button>

              <button
                class="btn btn-danger btn-sm"
                onclick={() => handleDeletePerson(person)}
                title="Премахни лицето"
              >
                ✕
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </main>
</div>

{#if isCreatingNew}
  <div class="modal-backdrop" onclick={() => (isCreatingNew = false)} role="presentation">
    <div class="modal-dialog" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <div class="modal-head">
        <h3>+ {currentLang === 'bg' ? 'Добавяне на нова личност' : 'Add Person'}</h3>
        <button class="close-btn" onclick={() => (isCreatingNew = false)}>✕</button>
      </div>

      <div class="modal-body">
        <label>
          <span>{currentLang === 'bg' ? 'Канонично име' : 'Canonical Name'} *</span>
          <input type="text" bind:value={newName} placeholder="напр. Бойко Борисов, д-р Иван Иванов" />
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Длъжност / Професия' : 'Role / Occupation'}</span>
          <input type="text" bind:value={newRole} placeholder="напр. Политик, лидер на партия, лекар-инфекционист" />
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Категория' : 'Category'}</span>
          <select bind:value={newCategory}>
            {#each CATEGORIES.filter((c) => c.id !== 'all') as c}
              <option value={c.id}>{currentLang === 'bg' ? c.bg : c.en}</option>
            {/each}
          </select>
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Рейтинг на важност (0 - 100)' : 'Priority Rating'}</span>
          <input type="range" min="0" max="100" bind:value={newRating} />
          <span class="range-hint">{newRating}</span>
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Тагове (разделени със запетая)' : 'Tags (comma separated)'}</span>
          <input type="text" bind:value={newTagsStr} placeholder="[десен], [парламент], [бивш министър]" />
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Бележки на анализатора' : 'Notes'}</span>
          <textarea bind:value={newNotes} rows="3"></textarea>
        </label>
      </div>

      <div class="modal-foot">
        <button class="btn btn-secondary btn-sm" onclick={() => (isCreatingNew = false)}>
          {currentLang === 'bg' ? 'Отказ' : 'Cancel'}
        </button>
        <button class="btn btn-primary btn-sm" onclick={handleSaveNewPerson} disabled={!newName.trim()}>
          {currentLang === 'bg' ? 'Създай' : 'Create'}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if editingPerson}
  <div class="modal-backdrop" onclick={() => (editingPerson = null)} role="presentation">
    <div class="modal-dialog" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <div class="modal-head">
        <h3>✏️ {currentLang === 'bg' ? 'Редактиране на' : 'Edit'} {editingPerson.canonical_name}</h3>
        <button class="close-btn" onclick={() => (editingPerson = null)}>✕</button>
      </div>

      <div class="modal-body">
        <label>
          <span>{currentLang === 'bg' ? 'Име' : 'Name'}</span>
          <input type="text" bind:value={editingPerson.canonical_name} />
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Длъжност' : 'Role'}</span>
          <input type="text" bind:value={editingPerson.role} />
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Категория' : 'Category'}</span>
          <select bind:value={editingPerson.category}>
            {#each CATEGORIES.filter((c) => c.id !== 'all') as c}
              <option value={c.id}>{currentLang === 'bg' ? c.bg : c.en}</option>
            {/each}
          </select>
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Рейтинг' : 'Rating'} ({editingPerson.rating ?? 50})</span>
          <input type="range" min="0" max="100" bind:value={editingPerson.rating} />
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Кратко резюме' : 'Summary'}</span>
          <textarea bind:value={editingPerson.summary} rows="3"></textarea>
        </label>

        <label>
          <span>{currentLang === 'bg' ? 'Бележки' : 'Notes'}</span>
          <textarea bind:value={editingPerson.notes} rows="2"></textarea>
        </label>
      </div>

      <div class="modal-foot">
        <button class="btn btn-secondary btn-sm" onclick={() => (editingPerson = null)}>
          {currentLang === 'bg' ? 'Отказ' : 'Cancel'}
        </button>
        <button class="btn btn-primary btn-sm" onclick={handleSaveEditedPerson}>
          {currentLang === 'bg' ? 'Запази промените' : 'Save Changes'}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if viewingDossier}
  {@const prof = parseProfile(viewingDossier)}
  <div class="modal-backdrop" onclick={() => (viewingDossier = null)} role="presentation">
    <div class="modal-dialog dossier-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <div class="modal-head">
        <div>
          <h3>📖 {currentLang === 'bg' ? 'OSINT Разузнавателно Досие:' : 'OSINT Dossier:'} {viewingDossier.canonical_name}</h3>
          <span class="dossier-sub">{viewingDossier.role || viewingDossier.category || 'Публична личност'}</span>
        </div>
        <button class="close-btn" onclick={() => (viewingDossier = null)}>✕</button>
      </div>

      <div class="modal-body dossier-body">
        {#if viewingDossier.summary}
          <div class="dossier-section summary-box">
            <h4>{currentLang === 'bg' ? 'Обобщение на личността' : 'Executive Summary'}</h4>
            <p>{viewingDossier.summary}</p>
          </div>
        {/if}

        {#if prof}
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
            <p>{currentLang === 'bg' ? 'Все още няма генерирано разширено 9-степенно досие за това лице.' : 'No detailed OSINT dossier generated yet.'}</p>
            <button
              class="btn btn-primary btn-sm"
              onclick={() => {
                const target = viewingDossier!;
                viewingDossier = null;
                handleTriggerResearch(target);
              }}
            >
              ⚡ {currentLang === 'bg' ? 'Генерирай 9-степенно OSINT проучване сега' : 'Generate Full OSINT Dossier Now'}
            </button>
          </div>
        {/if}
      </div>

      <div class="modal-foot">
        <button class="btn btn-secondary btn-sm" onclick={() => (viewingDossier = null)}>
          {currentLang === 'bg' ? 'Затвори' : 'Close'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .people-screen {
    flex: 1;
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--bg-canvas);
    color: var(--text-body);
  }

  .screen-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.5rem;
    background: var(--surface-bg);
    border-bottom: 1px solid var(--border-color);
  }

  .header-titles h2 {
    margin: 0;
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--text-heading);
  }

  .header-desc {
    margin: 0.2rem 0 0;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .screen-toast {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--primary-accent);
    background: var(--primary-accent-alpha);
    padding: 0.3rem 0.6rem;
    border-radius: 4px;
    border: 1px solid var(--primary-accent);
  }

  .filter-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.75rem 1.5rem;
    background: var(--surface-bg);
    border-bottom: 1px solid var(--border-color);
  }

  .search-input-wrap {
    display: flex;
    gap: 0.4rem;
    flex: 1;
    max-width: 480px;
  }

  .search-input-wrap input {
    flex: 1;
    padding: 0.4rem 0.75rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
    font-size: 0.85rem;
  }

  .select-group {
    display: flex;
    gap: 0.5rem;
  }

  .select-group select {
    padding: 0.4rem 0.75rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
    font-size: 0.82rem;
  }

  .people-grid-container {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem 1.5rem;
  }

  .people-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 1rem;
  }

  .person-card {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    transition: border-color 0.15s ease;
  }

  .person-card:hover {
    border-color: var(--primary-accent);
  }

  .card-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .person-identity {
    display: flex;
    align-items: center;
    gap: 0.65rem;
  }

  .person-avatar {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    background: var(--primary-accent);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 0.85rem;
    flex-shrink: 0;
  }

  .person-name {
    margin: 0;
    font-size: 0.98rem;
    font-weight: 700;
    color: var(--text-heading);
  }

  .person-category-badge {
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--text-muted);
    background: var(--pill-bg);
    border: 1px solid var(--border-color);
    padding: 1px 6px;
    border-radius: 4px;
  }

  .rating-widget {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
  }

  .rating-label {
    font-size: 0.68rem;
    color: var(--text-muted);
  }

  .rating-value {
    font-size: 0.82rem;
    font-weight: 700;
    color: var(--primary-accent);
  }

  .rating-slider {
    width: 80px;
    height: 4px;
    cursor: pointer;
  }

  .person-role {
    font-size: 0.82rem;
    color: var(--text-body);
  }

  .tags-section {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .tag-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--primary-accent);
    background: var(--primary-accent-alpha);
    border: 1px solid var(--primary-accent);
    padding: 1px 6px;
    border-radius: 4px;
  }

  .tag-remove-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 0.7rem;
    cursor: pointer;
    padding: 0;
  }

  .tag-remove-btn:hover {
    color: var(--verdict-contradicted);
  }

  .tag-add-btn {
    background: var(--pill-bg);
    border: 1px dashed var(--border-color);
    color: var(--text-muted);
    font-size: 0.7rem;
    padding: 1px 6px;
    border-radius: 4px;
    cursor: pointer;
  }

  .tag-add-btn:hover {
    color: var(--primary-accent);
    border-color: var(--primary-accent);
  }

  .person-summary {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-muted);
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .claims-stat-bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.74rem;
    color: var(--text-muted);
    border-top: 1px solid var(--border-color);
    padding-top: 0.4rem;
  }

  .stat-researched {
    color: var(--verdict-supported);
    font-weight: 600;
  }

  .card-actions {
    display: flex;
    gap: 0.4rem;
    margin-top: auto;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
  }

  .modal-dialog {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    width: 100%;
    max-width: 520px;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
  }

  .dossier-modal {
    max-width: 860px;
  }

  .modal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1.25rem;
    border-bottom: 1px solid var(--border-color);
  }

  .modal-head h3 {
    margin: 0;
    font-size: 1rem;
    color: var(--text-heading);
  }

  .dossier-sub {
    font-size: 0.78rem;
    color: var(--primary-accent);
  }

  .modal-body {
    padding: 1.25rem;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .modal-body label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.82rem;
    color: var(--text-heading);
    font-weight: 600;
  }

  .modal-body input,
  .modal-body select,
  .modal-body textarea {
    padding: 0.45rem 0.65rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-body);
    font-size: 0.85rem;
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 0.75rem 1.25rem;
    border-top: 1px solid var(--border-color);
  }

  .close-btn {
    background: transparent;
    border: none;
    font-size: 1.1rem;
    color: var(--text-muted);
    cursor: pointer;
  }

  .osint-categories-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 0.85rem;
  }

  .osint-card {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.75rem;
  }

  .osint-card h5 {
    margin: 0 0 0.35rem;
    font-size: 0.82rem;
    font-weight: 700;
    color: var(--primary-accent);
  }

  .osint-card p {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-body);
    line-height: 1.4;
  }

  .summary-box {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.85rem;
  }

  .summary-box h4 {
    margin: 0 0 0.4rem;
    font-size: 0.85rem;
    color: var(--text-heading);
  }

  .summary-box p {
    margin: 0;
    font-size: 0.85rem;
    line-height: 1.45;
  }

  .empty-dossier {
    text-align: center;
    padding: 2rem;
    color: var(--text-muted);
  }

  .loading-state,
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 300px;
    gap: 0.75rem;
    color: var(--text-muted);
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 3px solid var(--border-color);
    border-top-color: var(--primary-accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>