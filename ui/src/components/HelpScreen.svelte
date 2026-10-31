<script lang="ts">
  import type { Language } from '../lib/i18n';
  import BookReaderToc from './BookReaderToc.svelte';
  import type { TocItem } from './BookReaderToc.svelte';

  let { currentLang }: { currentLang: Language } = $props();

  let scrollContainer = $state<HTMLDivElement | null>(null);

  const helpTocItems = $derived.by((): TocItem[] => {
    const bg = currentLang === 'bg';
    return [
      { id: 'help-sec-signs', label: bg ? '1. Означения и индикатори' : '1. Badges & Indicators', level: 1 },
      { id: 'help-sec-actions', label: bg ? '2. Клавишни комбинации' : '2. Keyboard Shortcuts', level: 1 },
      { id: 'help-sec-lanes', label: bg ? '3. Категории списъци' : '3. Queue Categories', level: 1 },
      { id: 'help-sec-verdicts', label: bg ? '4. Оценки на твърденията' : '4. Claim Assessments', level: 1 },
      { id: 'help-sec-subjects', label: bg ? '5. Типове обекти и справки' : '5. Subject Types & Reports', level: 1 },
      { id: 'help-sec-dictionary', label: bg ? '6. Потребителски речник' : '6. Custom Dictionary', level: 1 },
    ];
  });
</script>

<div class="help-reader-layout">
  <BookReaderToc
    variant="sidebar"
    items={helpTocItems}
    scrollContainer={scrollContainer}
    title={currentLang === 'bg' ? 'Съдържание' : 'Contents'}
  />

  <div class="help-scroll-wrapper" bind:this={scrollContainer}>
    <div class="help-container">
      <header class="help-header">
        <div class="help-title-group">
          <h1>{currentLang === 'bg' ? 'Помощен център и ръководство' : 'Help Center & Documentation'}</h1>
          <p class="help-subtitle">
            {currentLang === 'bg'
              ? 'Пълно ръководство за означенията, клавишните комбинации, оценките на твърденията и аналитичните справки.'
              : 'Complete reference for badges, keyboard shortcuts, queue categories, assessments, and subject reports.'}
          </p>
        </div>
      </header>

      <div class="help-content">
        <section id="help-sec-signs" class="help-card">
          <h2>1. {currentLang === 'bg' ? 'Означения и кратки индикатори' : 'Single-Letter Badges & Indicators'}</h2>

          <div class="definition-grid">
            <div class="definition-item">
              <div class="badge-display meter-q">Q</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Качество на стенограмата (Quality)' : 'Transcript Quality'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Показва яснотата на разпознатия говор (0–100%). При налични оригинални субтитри достига до 100%, а при гласово разпознаване отразява чистотата на звука.'
                    : 'Measures transcript and audio clarity (0–100%). Videos with original captions reach ~100%.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display meter-c">C</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Значимост за проверка (Check-worthiness)' : 'Check-worthiness'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Оценява дали във видеото се изричат конкретни проверими факти (числа, дати, закони, решения на институции, събития) вместо лично мнение.'
                    : 'Scores whether statements contain concrete verifiable facts rather than personal opinions.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display meter-e">E</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Сила на доказателствата (Evidence)' : 'Evidence Strength'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Измерва степента на съответствие и авторитетността на намерените външни източници — официални регистри, БТА, БНР и проверки на факти.'
                    : 'Measures the authority and relevance of matched external primary records and news archives.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-topbar">Q: 14</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Чакащи задачи в опашката' : 'Queued Tasks'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Броят задачи, които изчакват обработка (изтегляне, стенограма, разпознаване на текст от екрана и проверка на твърдения).'
                    : 'Number of pending jobs waiting in the local processing queue.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-topbar">R: 2</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Задачи в изпълнение' : 'Running Tasks'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Броят задачи, които се изпълняват паралелно в момента. Всички активни проучвания се виждат и в прозореца „Известия“.'
                    : 'Number of worker tasks actively running right now. Also visible in the Notifications center.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-btn">R</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Допълнително проучване' : 'Research More'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Стартира разширено търсене по външни източници, публични регистри и история на автора.'
                    : 'Triggers deeper retrieval across external archives, public registers, and creator history.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-btn">U</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Отмяна на последното действие' : 'Undo Last Action'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Връща последно запазеното или отхвърленото видео обратно в текущия списък.'
                    : 'Immediately reverses your last keep or discard decision.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-special">ASR</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Автоматично разпознаване на реч' : 'Automatic Speech Recognition'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Показва, че стенограмата е извлечена автоматично от аудиозаписа чрез Whisper или Google Speech.'
                    : 'Indicates the transcript was generated directly from video audio.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-special">{currentLang === 'bg' ? 'Субтитри' : 'Captions'}</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Оригинални субтитри' : 'Original Captions'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Видеото разполага с вградени субтитри в TikTok, които са извлечени директно.'
                    : 'Original captions were retrieved directly from TikTok metadata.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-special">{currentLang === 'bg' ? '📌 Добавено' : '📌 Pasted'}</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Ръчно добавен адрес' : 'Manually Added Link'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Адресът е поставен ръчно от потребителя и се обработва с най-висок приоритет.'
                    : 'Added manually by the user and processed with highest priority.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-special">{currentLang === 'bg' ? 'Проверено' : 'Checked'}</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Налична публикация за проверка' : 'Existing Fact-Check Match'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Открито е съвпадение с вече публикувана проверка във Factcheck.bg, БТА, БНР или международни редакции.'
                    : 'Matched an existing published fact-check article from verified editorial sources.'}
                </p>
              </div>
            </div>

            <div class="definition-item">
              <div class="badge-display badge-special">⏳</div>
              <div class="definition-body">
                <strong>{currentLang === 'bg' ? 'Проучване в ход' : 'Ongoing Research'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Индикатор в горната лента и в „Известия“, показващ кои проучвания на обекти, автори или видеа се изпълняват в момента.'
                    : 'Top-bar and Notification Center indicator showing live research tasks in progress.'}
                </p>
              </div>
            </div>
          </div>
        </section>

        <section id="help-sec-actions" class="help-card">
          <h2>2. {currentLang === 'bg' ? 'Клавишни комбинации и бърза навигация' : 'Keyboard Shortcuts & Navigation'}</h2>

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
                <td>{currentLang === 'bg' ? 'Преминава към следващата карта в списъка.' : 'Moves selection down to the next card.'}</td>
              </tr>
              <tr>
                <td><kbd>K</kbd></td>
                <td>{currentLang === 'bg' ? 'Предишно видео' : 'Previous Video'}</td>
                <td>{currentLang === 'bg' ? 'Връща се към предходната карта в списъка.' : 'Moves selection up to the previous card.'}</td>
              </tr>
              <tr>
                <td><kbd>Space</kbd></td>
                <td>{currentLang === 'bg' ? 'Детайлен панел' : 'Toggle Detail Panel'}</td>
                <td>{currentLang === 'bg' ? 'Отваря или затваря панела за преглед на съдържанието и доказателствата.' : 'Opens or closes the video detail panel and player.'}</td>
              </tr>
              <tr>
                <td><kbd>Enter</kbd></td>
                <td>{currentLang === 'bg' ? 'Запази' : 'Keep Video'}</td>
                <td>{currentLang === 'bg' ? 'Отбелязва видеото като важно и го запазва за последваща работа или експорт.' : 'Marks the video as important and saves it for export.'}</td>
              </tr>
              <tr>
                <td><kbd>Backspace</kbd></td>
                <td>{currentLang === 'bg' ? 'Отхвърли' : 'Discard Video'}</td>
                <td>{currentLang === 'bg' ? 'Премества видеото извън активния списък.' : 'Removes the video from the current active queue.'}</td>
              </tr>
              <tr>
                <td><kbd>R</kbd></td>
                <td>{currentLang === 'bg' ? 'Проучи' : 'Research More'}</td>
                <td>{currentLang === 'bg' ? 'Стартира допълнително търсене на документи и проверка на автора.' : 'Starts additional source retrieval and creator verification.'}</td>
              </tr>
              <tr>
                <td><kbd>U</kbd></td>
                <td>{currentLang === 'bg' ? 'Отмени' : 'Undo'}</td>
                <td>{currentLang === 'bg' ? 'Връща последно премахнатото или запазеното видео.' : 'Restores the last kept or discarded video.'}</td>
              </tr>
              <tr>
                <td><kbd>/</kbd></td>
                <td>{currentLang === 'bg' ? 'Търсене' : 'Search'}</td>
                <td>{currentLang === 'bg' ? 'Поставя курсора в полето за търсене.' : 'Moves focus to the search input.'}</td>
              </tr>
              <tr>
                <td><kbd>Esc</kbd></td>
                <td>{currentLang === 'bg' ? 'Затвори' : 'Close'}</td>
                <td>{currentLang === 'bg' ? 'Затваря отворения прозорец или детайлния панел.' : 'Closes active modal or detail panel.'}</td>
              </tr>
            </tbody>
          </table>
        </section>

        <section id="help-sec-lanes" class="help-card">
          <h2>3. {currentLang === 'bg' ? 'Категории списъци във входящата кутия' : 'Inbox Queue Categories'}</h2>

          <div class="lanes-list">
            <div class="lane-info-card">
              <h3>📌 {currentLang === 'bg' ? 'Ръчно добавени' : 'Manually Added'}</h3>
              <p>
                {currentLang === 'bg'
                  ? 'Видеа, чиито адреси са поставени ръчно в горното поле. Обработват се незабавно с най-висок приоритет.'
                  : 'Videos pasted manually by the user, processed immediately with top priority.'}
              </p>
            </div>

            <div class="lane-info-card">
              <h3>⭐ {currentLang === 'bg' ? 'Важни видеа' : 'Priority Videos'}</h3>
              <p>
                {currentLang === 'bg'
                  ? 'Видеа с конкретни проверими твърдения, подредени по обществена значимост, разпространение и сила на доказателствата.'
                  : 'Videos with verifiable factual claims sorted by public relevance and reach.'}
              </p>
            </div>

            <div class="lane-info-card">
              <h3>🔍 {currentLang === 'bg' ? 'Вече проверени' : 'Already Checked'}</h3>
              <p>
                {currentLang === 'bg'
                  ? 'Видеа, за чиито твърдения вече има публикувана журналистическа проверка с пряка връзка към статията.'
                  : 'Videos matching already published fact-checks in verified archives.'}
              </p>
            </div>

            <div class="lane-info-card">
              <h3>📉 {currentLang === 'bg' ? 'Ниско качество' : 'Low Quality'}</h3>
              <p>
                {currentLang === 'bg'
                  ? 'Клипове с неясен звук, само музика или брой гледания под зададения праг.'
                  : 'Clips with poor audio clarity or minimal reach.'}
              </p>
            </div>

            <div class="lane-info-card">
              <h3>🚫 {currentLang === 'bg' ? 'Без фактически твърдения' : 'Screened Out'}</h3>
              <p>
                {currentLang === 'bg'
                  ? 'Видеа без конкретни проверими факти (развлекателни клипове, шеги или общи коментари).'
                  : 'Videos without concrete verifiable factual assertions.'}
              </p>
            </div>

            <div class="lane-info-card">
              <h3>⚠️ {currentLang === 'bg' ? 'Неуспешно изтегляне' : 'Failed Download'}</h3>
              <p>
                {currentLang === 'bg'
                  ? 'Видеа, които са изтрити, ограничени от автора или недостъпни поради мрежова грешка.'
                  : 'Videos unavailable due to deletion, privacy settings, or network failure.'}
              </p>
            </div>
          </div>
        </section>

        <section id="help-sec-verdicts" class="help-card">
          <h2>4. {currentLang === 'bg' ? 'Оценки на твърденията' : 'Claim Assessments'}</h2>

          <div class="verdict-guide">
            <div class="verdict-row contradicted">
              <span class="verdict-tag">ОПРОВЕРГАНО</span>
              <div class="verdict-desc">
                <strong>{currentLang === 'bg' ? 'Опровергано от външни документи' : 'Contradicted'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Твърдението противоречи пряко на официални регистри, нормативни актове, стенограми или независими журналистически проверки с цитиран външен адрес.'
                    : 'Directly contradicted by official records, legislation, or independent fact-checks with cited external URLs.'}
                </p>
              </div>
            </div>

            <div class="verdict-row supported">
              <span class="verdict-tag">ПОТВЪРДЕНО</span>
              <div class="verdict-desc">
                <strong>{currentLang === 'bg' ? 'Потвърдено от първични източници' : 'Supported'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Твърдението съответства на данните в официални институционални източници (НСИ, БНБ, Търговски регистър, съдебни актове) или потвърдени агенционни новини.'
                    : 'Corroborated by official institutional records or verified news dispatches.'}
                </p>
              </div>
            </div>

            <div class="verdict-row unverifiable">
              <span class="verdict-tag">НЕПРОВЕРИМО</span>
              <div class="verdict-desc">
                <strong>{currentLang === 'bg' ? 'Недостатъчно публични данни' : 'Unverifiable'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Изказването съдържа конкретно твърдение, но в откритите източници липсват преки първични документи за неговото потвърждаване или опровергаване.'
                    : 'Formulated as a factual claim, but external primary records are insufficient to confirm or refute it.'}
                </p>
              </div>
            </div>

            <div class="verdict-row not-checkable">
              <span class="verdict-tag">МНЕНИЕ</span>
              <div class="verdict-desc">
                <strong>{currentLang === 'bg' ? 'Неподлежащо на фактическа проверка' : 'Not Checkable'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Лична оценка, коментар, прогноза или реторичен въпрос без проверими фактически величини.'
                    : 'Personal opinion, commentary, or future prediction.'}
                </p>
              </div>
            </div>

            <div class="verdict-row already-checked">
              <span class="verdict-tag">ВЕЧЕ ПРОВЕРЕНО</span>
              <div class="verdict-desc">
                <strong>{currentLang === 'bg' ? 'Налична архивна проверка' : 'Already Checked'}</strong>
                <p>
                  {currentLang === 'bg'
                    ? 'Твърдението вече е разгледано в публикувана статия за проверка на факти с директна препратка.'
                    : 'Already covered in a published fact-check article with direct citation.'}
                </p>
              </div>
            </div>
          </div>
        </section>

        <section id="help-sec-subjects" class="help-card">
          <h2>5. {currentLang === 'bg' ? 'Типове обекти и аналитични справки' : 'Subject Types & Analytical Reports'}</h2>
          <p class="intro-p">
            {currentLang === 'bg'
              ? 'Разделът „Лица и обекти“ поддържа 8 типа субекти: Личност (👤), Институция (🏛️), Канал / Медия (📺), Държава (🌍), Група / Партия (👥), Събитие (📅), Инфраструктура (🛣️ — магистрали, мостове, язовири, жп линии, централи) и Сграда / Обект (🏢 — болници, училища, паметници на културата, комплекси).'
              : 'The People & Subjects workspace supports 8 entity types: Person, Institution, Channel / Media, Country, Group / Party, Event, Infrastructure, and Building / Facility.'}
          </p>
          <p class="intro-p">
            {currentLang === 'bg'
              ? 'Всяка аналитична справка включва странично „Съдържание“ за бързо преминаване към отделна глава (напр. Глава 2 за Търговския регистър, АОП и имуществени декларации или Глава 5 за съдебни дела, КПКОНПИ, ДНСК и санкции), както и автоматично цветно открояване: датите са отбелязани в зелено, ЕИК/дела/решения в синьо, парични суми и дялове в кехлибарено, а дружества и контролни органи в индиго.'
              : 'Every analytical report includes a Book-Reader Table of Contents for instant chapter navigation and automatic highlighting: dates in green, registry/case IDs in blue, monetary amounts in amber, and companies/oversight bodies in indigo.'}
          </p>
        </section>

        <section id="help-sec-dictionary" class="help-card">
          <h2>6. {currentLang === 'bg' ? 'Потребителски речник и езикови поправки' : 'Custom Dictionary & ASR Repairs'}</h2>

          <p class="intro-p">
            {currentLang === 'bg'
              ? 'Можете да добавяте собствени поправки за често сгрешени думи при гласово разпознаване, разговорни изрази и защитени съкращения във файла config/lexicon.bg.toml.'
              : 'Configure custom Whisper corrections, dialect mappings, and protected acronyms in config/lexicon.bg.toml.'}
          </p>

          <div class="param-box">
            <h3>1. {currentLang === 'bg' ? 'Автоматични поправки при разпознаване на говор' : 'ASR Word Repairs'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Двойки със сгрешена и правилна форма, които се заменят автоматично:'
                : 'Drop-in replacements for recurring speech recognition errors:'}
            </p>
            <pre class="code-block"><code>[[asr_repairs]]
wrong = "февро"
right = "в евро"

[[asr_repairs]]
wrong = "дересе"
right = "ДДС"</code></pre>
          </div>

          <div class="param-box">
            <h3>2. {currentLang === 'bg' ? 'Разговорни и диалектни думи' : 'Dialect & Colloquial Terms'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Съответствие между разговорна дума и книжовно значение:'
                : 'Mapping informal terms to standard Bulgarian:'}
            </p>
            <pre class="code-block"><code>[[dialect_glossary]]
term = "демек"
standard = "тоест"

[[dialect_glossary]]
term = "скивай"
standard = "виж"</code></pre>
          </div>

          <div class="param-box">
            <h3>3. {currentLang === 'bg' ? 'Защитени термини и съкращения' : 'Protected Acronyms & Terms'}</h3>
            <p>
              {currentLang === 'bg'
                ? 'Имена на институции и термини, които не се променят при автоматична обработка:'
                : 'Acronyms and terms preserved verbatim:'}
            </p>
            <pre class="code-block"><code>domain_vocabulary = [
  "еврозоната", "инфлация", "ДДС", "НСИ", "БНБ", "НОИ", "КЕВР", "КПКОНПИ", "ДНСК", "АПИ"
]</code></pre>
          </div>
        </section>
      </div>
    </div>
  </div>
</div>

<style>
  .help-reader-layout {
    flex: 1 1 0;
    min-height: 0;
    width: 100%;
    height: 100%;
    display: flex;
    overflow: hidden;
    background: var(--bg-canvas);
  }

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

  .help-container {
    max-width: 1080px;
    width: 100%;
    margin: 0 auto;
    padding: 1.5rem 1.25rem 4rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    box-sizing: border-box;
  }

  .help-content {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .help-header {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1.2rem 1.5rem;
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

  .help-card {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1.4rem 1.5rem;
  }

  .help-card h2 {
    margin: 0 0 1.15rem 0;
    font-size: 1.12rem;
    color: var(--text-heading);
  }

  .definition-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(420px, 1fr));
    gap: 0.9rem;
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
    min-width: 38px;
    height: 36px;
    padding: 0 6px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 800;
    font-size: 0.88rem;
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
    min-width: 52px;
  }

  .badge-btn {
    background: rgba(99, 102, 241, 0.15);
    color: var(--primary-accent);
    border: 1px solid rgba(99, 102, 241, 0.3);
  }

  .badge-special {
    font-size: 0.72rem;
    min-width: 68px;
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
    font-size: 0.74rem;
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
    margin: 0 0 0.85rem 0;
    font-size: 0.85rem;
    color: var(--text-body);
    line-height: 1.5;
  }

  .param-box {
    margin-bottom: 1rem;
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
