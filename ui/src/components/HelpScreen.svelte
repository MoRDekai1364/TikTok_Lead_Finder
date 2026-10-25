<script lang="ts">
  import type { Language } from '../lib/i18n';

  let { currentLang }: { currentLang: Language } = $props();

  let activeSection = $state<'signs' | 'actions' | 'lanes' | 'verdicts' | 'dictionary'>('signs');
  let scrollContainer = $state<HTMLDivElement | null>(null);

  function setSection(section: 'signs' | 'actions' | 'lanes' | 'verdicts' | 'dictionary') {
    activeSection = section;
    if (scrollContainer) {
      scrollContainer.scrollTo({ top: 0, behavior: 'instant' });
    }
  }
</script>

<div class="help-scroll-wrapper" bind:this={scrollContainer}>
  <div class="help-container">
  <header class="help-header">
    <div class="help-title-group">
      <h1>{currentLang === 'bg' ? 'Помощен център & Ръководство' : 'Help Center & Documentation'}</h1>
      <p class="help-subtitle">
        {currentLang === 'bg'
          ? 'Пълно ръководство за всички съкращения, еднобуквени означения, клавишни комбинации и функции в TikTok Lead Finder.'
          : 'Complete reference for single-letter badges, keyboard shortcuts, lanes, verdicts, and dictionary parameters.'}
      </p>
    </div>

    <nav class="help-nav" aria-label="Секции за помощ">
      <button
        class="help-nav-btn"
        class:active={activeSection === 'signs'}
        onclick={() => setSection('signs')}
      >
        🔤 {currentLang === 'bg' ? 'Означения (Q, C, E, R)' : 'Signings (Q, C, E, R)'}
      </button>
      <button
        class="help-nav-btn"
        class:active={activeSection === 'actions'}
        onclick={() => setSection('actions')}
      >
        ⌨️ {currentLang === 'bg' ? 'Действия & Клавиши' : 'Actions & Keys'}
      </button>
      <button
        class="help-nav-btn"
        class:active={activeSection === 'lanes'}
        onclick={() => setSection('lanes')}
      >
        📂 {currentLang === 'bg' ? 'Секции (Lanes)' : 'Lanes & Queues'}
      </button>
      <button
        class="help-nav-btn"
        class:active={activeSection === 'verdicts'}
        onclick={() => setSection('verdicts')}
      >
        ⚖️ {currentLang === 'bg' ? 'Присъди' : 'Verdicts'}
      </button>
      <button
        class="help-nav-btn"
        class:active={activeSection === 'dictionary'}
        onclick={() => setSection('dictionary')}
      >
        📖 {currentLang === 'bg' ? 'Речници & Жаргон' : 'Dictionary & Slang'}
      </button>
    </nav>
  </header>

  <div class="help-content">
    {#if activeSection === 'signs'}
      <section class="help-card">
        <h2>{currentLang === 'bg' ? 'Еднобуквени означения и мини-индикатори' : 'Single-Letter Signings & Meter Badges'}</h2>
        
        <div class="definition-grid">
          <div class="definition-item">
            <div class="badge-display meter-q">Q</div>
            <div class="definition-body">
              <strong>Quality (Качество на стенограмата)</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Показва качеството и акустичната увереност на разпознатия говор (0–100%). Ако видеото има оригинални субтитри от автора, Q е близко до 100%. При Whisper ASR отразява яснотата на речта и отсъствието на фонов шум.'
                  : 'Measures transcript and audio acoustic clarity (0–100%). Videos with original captions reach ~100%. For Whisper ASR, it reflects speech clarity versus background music/noise.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display meter-c">C</div>
            <div class="definition-body">
              <strong>Check-worthiness (Значимост за проверка)</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Оценява дали във видеото се изричат конкретни фактически твърдения (числа, имена на институции, закони, събития), или е само лично мнение. Колкото по-високо е C, толкова по-голяма е журналистическата стойност за проверка.'
                  : 'Scores whether claims are verifiable factual statements (numbers, dates, institutions, events) rather than subjective opinions or satire. High C means high fact-check priority.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display meter-e">E</div>
            <div class="definition-body">
              <strong>Evidence Strength (Сила на доказателствата)</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Измерва колко категорични и надеждни са намерените доказателства от официални източници, агенции (БТА, БНР) и факт-чек архиви, съпоставени с твърдението.'
                  : 'Measures the similarity, strength, and authority of matched evidence passages retrieved from audited archives and official Bulgarian records.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-topbar">Q: 14</div>
            <div class="definition-body">
              <strong>Queued (В опашка - в горната лента)</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Броят задачи, които изчакват ред в работната опашка (сваляне, транскрибиране, OCR, извличане на твърдения).'
                  : 'Number of pending jobs currently queued in the local database awaiting execution.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-topbar">R: 2</div>
            <div class="definition-body">
              <strong>Running (В изпълнение - в горната лента)</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Броят задачи, които в момента активно се изпълняват паралелно на вашия компютър.'
                  : 'Number of worker tasks actively executing right now on your machine.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-btn">R</div>
            <div class="definition-body">
              <strong>Research More (Проучи задълбочено)</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Бутон и бърз клавиш за стартиране на разширено търсене: извлича допълнителни факти, проверява публични личности в Уикиданни и анализира историята на автора.'
                  : 'Button and keyboard shortcut to trigger deep retrieval across broader search domains, Wikidata public-figure resolution, and creator history.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-btn">U</div>
            <div class="definition-body">
              <strong>Undo (Отмени последното действие)</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Клавиш U веднага връща последно запазеното или отхвърленото видео обратно във входящия списък.'
                  : 'Keyboard shortcut U immediately reverses your last keep or discard decision, restoring the card.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-special">ASR</div>
            <div class="definition-body">
              <strong>Whisper Automatic Speech Recognition</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Показва, че текстът е разпознат автоматично от гласа на говорещия с модела Whisper.'
                  : 'Indicates the transcript was generated directly from video audio via Whisper speech recognition.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-special">found</div>
            <div class="definition-body">
              <strong>Оригинални субтитри</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Видеото съдържа авторски или вградени български субтитри от TikTok, които са извлечени директно.'
                  : 'Original author captions were retrieved directly from TikTok metadata without needing speech recognition.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-special">📌 Pasted</div>
            <div class="definition-body">
              <strong>Ръчно добавено видео</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Линкът е въведен ръчно от вас. Тези видеа са с най-висок приоритет и се класират най-отгоре.'
                  : 'Pasted manually by the user. These videos receive highest priority and are never suppressed by daily caps.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-special">FactCheck</div>
            <div class="definition-body">
              <strong>Архивен факт-чек</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Открито е съвпадение с вече съществуваща публикация от проверен факт-чекър (Factcheck.bg, БТА, AFP).'
                  : 'Matched an existing published fact-check article from verified sources.'}
              </p>
            </div>
          </div>

          <div class="definition-item">
            <div class="badge-display badge-special">⚡</div>
            <div class="definition-body">
              <strong>Активно проучване</strong>
              <p>
                {currentLang === 'bg'
                  ? 'В момента тече фонова проверка или задълбочено проучване за това видео.'
                  : 'Background deep research is actively in flight for this video.'}
              </p>
            </div>
          </div>
        </div>
      </section>

    {:else if activeSection === 'actions'}
      <section class="help-card">
        <h2>{currentLang === 'bg' ? 'Клавишни комбинации и бързи действия' : 'Keyboard Shortcuts & Fast Triage'}</h2>
        
        <table class="help-table">
          <thead>
            <tr>
              <th>{currentLang === 'bg' ? 'Клавиш' : 'Key'}</th>
              <th>{currentLang === 'bg' ? 'Действие' : 'Action'}</th>
              <th>{currentLang === 'bg' ? 'Описание' : 'Description'}</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td><kbd>J</kbd></td>
              <td>{currentLang === 'bg' ? 'Следващо видео' : 'Next Video'}</td>
              <td>{currentLang === 'bg' ? 'Придвижва се към следващата карта в списъка.' : 'Moves selection down to next card.'}</td>
            </tr>
            <tr>
              <td><kbd>K</kbd></td>
              <td>{currentLang === 'bg' ? 'Предишно видео' : 'Previous Video'}</td>
              <td>{currentLang === 'bg' ? 'Придвижва се към предишната карта в списъка.' : 'Moves selection up to previous card.'}</td>
            </tr>
            <tr>
              <td><kbd>Space</kbd></td>
              <td>{currentLang === 'bg' ? 'Преглед / Плейър' : 'Toggle Drawer'}</td>
              <td>{currentLang === 'bg' ? 'Отваря или затваря детайлния страничен панел с видеоплейъра.' : 'Opens or closes the video detail drawer and synchronized player.'}</td>
            </tr>
            <tr>
              <td><kbd>Enter</kbd></td>
              <td>{currentLang === 'bg' ? 'Запази (Keep)' : 'Keep Lead'}</td>
              <td>{currentLang === 'bg' ? 'Маркира видеото като стойностен лийд и го запазва за експорт.' : 'Marks video as a valuable kept lead for investigation and export.'}</td>
            </tr>
            <tr>
              <td><kbd>Backspace</kbd></td>
              <td>{currentLang === 'bg' ? 'Отхвърли (Discard)' : 'Discard Video'}</td>
              <td>{currentLang === 'bg' ? 'Отстранява видеото от текущия списък в архив.' : 'Discards the video from current queue to archive.'}</td>
            </tr>
            <tr>
              <td><kbd>R</kbd></td>
              <td>{currentLang === 'bg' ? 'Проучи (Research)' : 'Research More'}</td>
              <td>{currentLang === 'bg' ? 'Стартира разширено търсене на източници и проверка на автора.' : 'Triggers deep retrieval across external archives and creator history.'}</td>
            </tr>
            <tr>
              <td><kbd>U</kbd></td>
              <td>{currentLang === 'bg' ? 'Отмени (Undo)' : 'Undo Decision'}</td>
              <td>{currentLang === 'bg' ? 'Връща обратно последно премахнатото видео.' : 'Restores the last kept or discarded video back into the active list.'}</td>
            </tr>
            <tr>
              <td><kbd>/</kbd></td>
              <td>{currentLang === 'bg' ? 'Търсене' : 'Focus Search'}</td>
              <td>{currentLang === 'bg' ? 'Фокусира текстовото поле за бързо търсене.' : 'Jumps focus immediately to the search input.'}</td>
            </tr>
            <tr>
              <td><kbd>Esc</kbd></td>
              <td>{currentLang === 'bg' ? 'Затвори' : 'Close'}</td>
              <td>{currentLang === 'bg' ? 'Затваря детайлния панел или отменя търсенето.' : 'Closes active detail drawer or clears search focus.'}</td>
            </tr>
          </tbody>
        </table>
      </section>

    {:else if activeSection === 'lanes'}
      <section class="help-card">
        <h2>{currentLang === 'bg' ? 'Работни потоци (Lanes) и организация' : 'Lanes & Queue Sections'}</h2>
        
        <div class="lanes-list">
          <div class="lane-info-card">
            <h3>📌 {currentLang === 'bg' ? 'Ръчно добавени (Pasted)' : 'Pasted Links'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Всички видеа, чиито връзки сте поставили ръчно в горното поле. Те се обработват веднага с абсолютен приоритет и не се ограничават от дневния лимит.'
                : 'All videos pasted manually by the user. They bypass background queues and receive immediate processing.'}
            </p>
          </div>

          <div class="lane-info-card">
            <h3>⭐ {currentLang === 'bg' ? 'Водещи новини (Leads)' : 'Leads'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Видеа с проверими твърдения, сортирани по значимост, брой гледания и важност на темата. Това е основният входящ списък за проверка.'
                : 'Highest ranking candidates filtered for factual claims and virality. Main triage feed.'}
            </p>
          </div>

          <div class="lane-info-card">
            <h3>🔍 {currentLang === 'bg' ? 'Вече проверени (Already Checked)' : 'Already Checked'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Видеа, чиито твърдения напълно съвпадат с вече публикувани журналистически проверки на факти. Позволява бързо виждане на готовото опровержение.'
                : 'Videos containing claims that already have published fact-checks in audited archives.'}
            </p>
          </div>

          <div class="lane-info-card">
            <h3>📉 {currentLang === 'bg' ? 'Нисък интерес / качество (Low Quality)' : 'Low Quality / Noise'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Клипове с много ниско качество на звука (неясна реч, само музика) или с малко гледания под определения праг.'
                : 'Videos with poor transcript clarity or views below the threshold.'}
            </p>
          </div>

          <div class="lane-info-card">
            <h3>🚫 {currentLang === 'bg' ? 'Отхвърлени (Screened Out)' : 'Screened Out'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Видеа, в които липсват проверими фактически твърдения (напр. само танц, скеч, въпроси или лично мнение).'
                : 'Videos screened out early due to lack of verifiable factual assertions.'}
            </p>
          </div>

          <div class="lane-info-card">
            <h3>⚠️ {currentLang === 'bg' ? 'Неуспешни (Failed)' : 'Failed'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Видеа, които не са могли да бъдат изтеглени (изтрити от автора, заключени като лични или блокирани от мрежови грешки).'
                : 'Videos that failed acquisition due to being deleted, private, or experiencing network errors.'}
            </p>
          </div>
        </div>
      </section>

    {:else if activeSection === 'verdicts'}
      <section class="help-card">
        <h2>{currentLang === 'bg' ? 'Присъди и оценки на твърденията' : 'Fact-Check Verdicts'}</h2>
        
        <div class="verdict-guide">
          <div class="verdict-row contradicted">
            <span class="verdict-tag">CONTRADICTED</span>
            <div class="verdict-desc">
              <strong>{currentLang === 'bg' ? 'Опровергано / Невярно' : 'Contradicted'}</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Фактическото твърдение е в пряко противоречие с откритите официални данни, закони или проверка от поне два независими авторитетни източника.'
                  : 'The statement directly contradicts official records or verified reporting from independent news outlets.'}
              </p>
            </div>
          </div>

          <div class="verdict-row supported">
            <span class="verdict-tag">SUPPORTED</span>
            <div class="verdict-desc">
              <strong>{currentLang === 'bg' ? 'Потвърдено' : 'Supported'}</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Твърдението се подкрепя от първични източници (правителствени регистри, НСИ, БНБ) или потвърдени съобщения.'
                  : 'The assertion is corroborated by audited primary records or verified source publications.'}
              </p>
            </div>
          </div>

          <div class="verdict-row unverifiable">
            <span class="verdict-tag">UNVERIFIABLE</span>
            <div class="verdict-desc">
              <strong>{currentLang === 'bg' ? 'Непроверимо' : 'Unverifiable'}</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Твърдението е формулирано като факт, но в публичните източници липсва достатъчно надеждна информация за потвърждение или опровержение.'
                  : 'Formulated as a fact, but insufficient credible public evidence exists to prove or disprove it.'}
              </p>
            </div>
          </div>

          <div class="verdict-row not-checkable">
            <span class="verdict-tag">NOT_CHECKABLE</span>
            <div class="verdict-desc">
              <strong>{currentLang === 'bg' ? 'Неподлежащо на проверка' : 'Not Checkable'}</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Изказването представлява субективно мнение, метафора, бъдеща прогноза или емоционална оценка, която не може да бъде измерена обективно.'
                  : 'Subjective opinion, future speculation, artistic metaphor, or emotional statement.'}
              </p>
            </div>
          </div>

          <div class="verdict-row already-checked">
            <span class="verdict-tag">ALREADY_CHECKED</span>
            <div class="verdict-desc">
              <strong>{currentLang === 'bg' ? 'Вече проверено в архив' : 'Already Checked'}</strong>
              <p>
                {currentLang === 'bg'
                  ? 'Открит е готов факт-чек от сертифицирана журналистическа медия с пряка препратка към статията.'
                  : 'Exact match found in fact-checking database with direct link to published debunk.'}
              </p>
            </div>
          </div>
        </div>
      </section>

    {:else if activeSection === 'dictionary'}
      <section class="help-card">
        <h2>{currentLang === 'bg' ? 'Параметри за потребителски речници, жаргон и диалекти' : 'Dictionary & Dialect Parameters'}</h2>
        
        <p class="intro-p">
          {currentLang === 'bg'
            ? 'Можете да обогатите системата със собствени разговорни думи, диалекти, сленг и чести грешки на Whisper. Те се добавят в конфигурацията `config/lexicon.bg.toml` или директно в текстовите списъци в `models/lexicon/`.'
            : 'You can customize vocabulary, slang, dialect terms, and automatic Whisper corrections. Configuration is in `config/lexicon.bg.toml` and `models/lexicon/`.'}
        </p>

        <div class="param-box">
          <h3>1. {currentLang === 'bg' ? 'Чести грешки на Whisper (ASR Repairs)' : 'Whisper ASR Confusion Pairs'}</h3>
          <p>
            {currentLang === 'bg'
              ? 'Когато Whisper системно чува една дума грешно, посочете двойка `wrong` и `right`. Системата автоматично заменя сгрешената форма:'
              : 'Direct drop-in replacements for words Whisper mishears:'}
          </p>
          <pre class="code-block"><code>[[asr_repairs]]
wrong = "февро"
right = "в евро"

[[asr_repairs]]
wrong = "дересе"
right = "ДДС"

[[asr_repairs]]
wrong = "союза"
right = "съюза"</code></pre>
        </div>

        <div class="param-box">
          <h3>2. {currentLang === 'bg' ? 'Диалектни и разговорни думи (Dialect Glossary)' : 'Dialect & Slang Glossary'}</h3>
          <p>
            {currentLang === 'bg'
              ? 'За разговорни думи и диалекти се дефинира терминът (`term`) и стандартното книжовно значение (`standard`):'
              : 'Map informal or regional terms to standard Bulgarian vocabulary:'}
          </p>
          <pre class="code-block"><code>[[dialect_glossary]]
term = "маняк"
standard = "човек"

[[dialect_glossary]]
term = "демек"
standard = "тоест"

[[dialect_glossary]]
term = "скивай"
standard = "виж"

[[dialect_glossary]]
term = "чуек"
standard = "човек"</code></pre>
        </div>

        <div class="param-box">
          <h3>3. {currentLang === 'bg' ? 'Защитени думи и теми (Domain Vocabulary)' : 'Domain Vocabulary'}</h3>
          <p>
            {currentLang === 'bg'
              ? 'Списък от термини, имена и съкращения, които нормализаторът никога не трябва да променя или преправя:'
              : 'Words and acronyms that must always be recognized as valid:'}
          </p>
          <pre class="code-block"><code>domain_vocabulary = [
  "еврозоната", "инфлация", "ДДС", "НСИ", "БНБ", "НОИ", "КЕВР",
  "вайб", "криндж", "подкаст", "инфлуенсър", "бийф"
]</code></pre>
        </div>
      </section>
    {/if}
  </div>
</div>
</div>

<style>
  .help-scroll-wrapper {
    flex: 1 1 0;
    min-height: 0;
    max-height: 100%;
    width: 100%;
    height: 100%;
    overflow-y: auto !important;
    overflow-x: hidden;
    background: var(--bg-canvas);
    box-sizing: border-box;
    -webkit-overflow-scrolling: touch;
    overscroll-behavior-y: contain;
    outline: none;
    scrollbar-width: thin;
    scrollbar-color: var(--border-color) transparent;
  }

  .help-scroll-wrapper::-webkit-scrollbar {
    width: 8px;
  }

  .help-scroll-wrapper::-webkit-scrollbar-track {
    background: transparent;
  }

  .help-scroll-wrapper::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 4px;
  }

  .help-scroll-wrapper::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }

  .help-container {
    max-width: 1040px;
    width: 100%;
    margin: 0 auto;
    padding: 1.5rem 1rem 4rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    box-sizing: border-box;
    min-height: min-content;
  }

  .help-header {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1.25rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .help-title-group h1 {
    margin: 0 0 0.35rem 0;
    font-size: 1.35rem;
    font-weight: 700;
    color: var(--text-heading);
  }

  .help-subtitle {
    margin: 0;
    font-size: 0.88rem;
    color: var(--text-muted);
  }

  .help-nav {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    border-top: 1px solid var(--border-color);
    padding-top: 0.75rem;
  }

  .help-nav-btn {
    padding: 0.4rem 0.85rem;
    font-size: 0.82rem;
    font-weight: 600;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: var(--surface-bg);
    color: var(--text-body);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .help-nav-btn:hover {
    background: var(--hover-bg);
    border-color: var(--primary-accent);
  }

  .help-nav-btn.active {
    background: var(--primary-accent);
    color: #fff;
    border-color: var(--primary-accent);
  }

  .help-card {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1.5rem;
  }

  .help-card h2 {
    margin: 0 0 1.25rem 0;
    font-size: 1.15rem;
    color: var(--text-heading);
  }

  .definition-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(460px, 1fr));
    gap: 1rem;
  }

  .definition-item {
    display: flex;
    gap: 0.85rem;
    align-items: flex-start;
    padding: 0.85rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.02);
  }

  .badge-display {
    min-width: 36px;
    height: 36px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 800;
    font-size: 0.95rem;
    flex-shrink: 0;
  }

  .meter-q {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.3);
  }

  .meter-c {
    background: rgba(59, 130, 246, 0.15);
    color: #3b82f6;
    border: 1px solid rgba(59, 130, 246, 0.3);
  }

  .meter-e {
    background: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
    border: 1px solid rgba(245, 158, 11, 0.3);
  }

  .badge-topbar {
    background: var(--pill-bg);
    border: 1px solid var(--border-color);
    font-size: 0.75rem;
    min-width: 50px;
  }

  .badge-btn {
    background: rgba(99, 102, 241, 0.15);
    color: var(--primary-accent);
    border: 1px solid rgba(99, 102, 241, 0.3);
  }

  .badge-special {
    font-size: 0.72rem;
    min-width: 58px;
    background: rgba(0, 0, 0, 0.06);
    border: 1px solid var(--border-color);
  }

  .definition-body strong {
    display: block;
    font-size: 0.88rem;
    color: var(--text-heading);
    margin-bottom: 0.25rem;
  }

  .definition-body p {
    margin: 0;
    font-size: 0.82rem;
    color: var(--text-body);
    line-height: 1.45;
  }

  .help-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .help-table th,
  .help-table td {
    padding: 0.75rem 1rem;
    text-align: left;
    border-bottom: 1px solid var(--border-color);
  }

  .help-table th {
    background: rgba(0, 0, 0, 0.03);
    color: var(--text-heading);
    font-weight: 700;
  }

  kbd {
    display: inline-block;
    padding: 0.2rem 0.5rem;
    font-family: inherit;
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--text-heading);
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    box-shadow: 0 1px 1px rgba(0, 0, 0, 0.15);
  }

  .lanes-list {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .lane-info-card {
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.85rem 1rem;
    background: rgba(0, 0, 0, 0.015);
  }

  .lane-info-card h3 {
    margin: 0 0 0.35rem 0;
    font-size: 0.95rem;
    color: var(--text-heading);
  }

  .lane-info-card p {
    margin: 0;
    font-size: 0.82rem;
    color: var(--text-body);
    line-height: 1.45;
  }

  .verdict-guide {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .verdict-row {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
    padding: 0.85rem 1rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
  }

  .verdict-tag {
    font-size: 0.75rem;
    font-weight: 700;
    padding: 0.25rem 0.6rem;
    border-radius: 999px;
    white-space: nowrap;
  }

  .contradicted .verdict-tag {
    background: rgba(225, 29, 72, 0.15);
    color: var(--verdict-contradicted);
  }

  .supported .verdict-tag {
    background: rgba(16, 185, 129, 0.15);
    color: var(--verdict-supported);
  }

  .unverifiable .verdict-tag {
    background: rgba(217, 119, 6, 0.15);
    color: var(--verdict-unverifiable);
  }

  .not-checkable .verdict-tag {
    background: rgba(100, 116, 139, 0.15);
    color: #64748b;
  }

  .already-checked .verdict-tag {
    background: rgba(168, 85, 247, 0.15);
    color: #a855f7;
  }

  .verdict-desc strong {
    display: block;
    font-size: 0.88rem;
    color: var(--text-heading);
    margin-bottom: 0.2rem;
  }

  .verdict-desc p {
    margin: 0;
    font-size: 0.82rem;
    color: var(--text-body);
  }

  .intro-p {
    margin: 0 0 1rem 0;
    font-size: 0.85rem;
    color: var(--text-body);
    line-height: 1.45;
  }

  .param-box {
    margin-bottom: 1.25rem;
    padding: 0.85rem 1rem;
    border-radius: 6px;
    border: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.015);
  }

  .param-box h3 {
    margin: 0 0 0.35rem 0;
    font-size: 0.95rem;
    color: var(--text-heading);
  }

  .param-box p {
    margin: 0 0 0.5rem 0;
    font-size: 0.82rem;
    color: var(--text-body);
  }

  .code-block {
    background: var(--input-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.75rem 1rem;
    font-size: 0.8rem;
    overflow-x: auto;
    margin: 0;
    color: var(--text-heading);
  }
</style>