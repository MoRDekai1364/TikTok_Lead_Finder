<script lang="ts">
  import { exitAndRunBackground, terminateApplication } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import type { QueueCounts } from '../lib/types';

  let {
    isOpen,
    currentLang,
    queueCounts,
    onClose,
  }: {
    isOpen: boolean;
    currentLang: Language;
    queueCounts: QueueCounts;
    onClose: () => void;
  } = $props();

  let selectedAction = $state<'background' | 'terminate'>('background');
  let neverAskAgain = $state(false);
  let isExecuting = $state(false);

  const pendingCount = $derived((queueCounts.queued || 0) + (queueCounts.running || 0));

  async function handleConfirm() {
    isExecuting = true;
    if (neverAskAgain) {
      localStorage.setItem('leadfinder_never_ask_exit', 'true');
      localStorage.setItem('leadfinder_exit_action', selectedAction);
    }
    if (selectedAction === 'background') {
      await exitAndRunBackground();
    } else {
      await terminateApplication();
    }
  }
</script>

{#if isOpen}
  <div
    class="exit-backdrop"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget && !isExecuting) onClose(); }}
    onkeydown={(e) => { if (e.key === 'Escape' && !isExecuting) onClose(); }}
  >
    <div class="exit-dialog" role="dialog" aria-modal="true">
      <header class="exit-header">
        <div class="header-icon">🚪</div>
        <div>
          <h3 class="exit-title">
            {currentLang === 'bg' ? 'Затваряне на TikTok Lead Finder' : 'Exit TikTok Lead Finder'}
          </h3>
          <p class="exit-sub">
            {currentLang === 'bg'
              ? 'Изберете как да продължи изпълнението на текущите задачи'
              : 'Choose how to handle active background tasks and processes'}
          </p>
        </div>
      </header>

      <div class="exit-body">
        {#if pendingCount > 0}
          <div class="pending-notice">
            <span class="notice-icon">⏳</span>
            <div class="notice-text">
              <strong>
                {currentLang === 'bg'
                  ? `В момента се обработват ${pendingCount} задачи`
                  : `Currently processing ${pendingCount} tasks`}
              </strong>
              <p>
                {currentLang === 'bg'
                  ? 'Има видеа в процес на извличане на аудио, транскрипция, проверка на твърдения или факт-чек проучвания.'
                  : 'Videos are being ingested, transcribed, or enriched with factual evidence.'}
              </p>
            </div>
          </div>
        {:else}
          <div class="pending-notice idle-notice">
            <span class="notice-icon">✨</span>
            <div class="notice-text">
              <strong>
                {currentLang === 'bg'
                  ? 'Опашката на задачите в момента е свободна'
                  : 'Job queue is currently idle'}
              </strong>
              <p>
                {currentLang === 'bg'
                  ? 'Няма чакащи видеа за обработка. Всички текущи данни са сигурно съхранени в базата.'
                  : 'No pending videos in queue. All data is securely persisted in local database.'}
              </p>
            </div>
          </div>
        {/if}

        <div class="choices-group">
          <label class="choice-card" class:selected={selectedAction === 'background'}>
            <input
              type="radio"
              name="exit_action"
              value="background"
              checked={selectedAction === 'background'}
              onchange={() => (selectedAction = 'background')}
              disabled={isExecuting}
            />
            <div class="choice-content">
              <div class="choice-head">
                <span class="choice-icon">⚙️</span>
                <span class="choice-title">
                  {currentLang === 'bg'
                    ? 'Продължи на заден план до приключване (Auto-terminate)'
                    : 'Run in background until tasks finish (Auto-terminate)'}
                </span>
              </div>
              <p class="choice-desc">
                {currentLang === 'bg'
                  ? 'Прозорецът се затваря, а системата довършва всички започнати видеа, ASR транскрипции и AI досиета. След като всичко е 100% завършено, приложението се изключва само.'
                  : 'The window closes while the core finishes pending videos, transcripts, and dossiers. Once all tasks finish, the application terminates automatically.'}
              </p>
            </div>
          </label>

          <label class="choice-card" class:selected={selectedAction === 'terminate'}>
            <input
              type="radio"
              name="exit_action"
              value="terminate"
              checked={selectedAction === 'terminate'}
              onchange={() => (selectedAction = 'terminate')}
              disabled={isExecuting}
            />
            <div class="choice-content">
              <div class="choice-head">
                <span class="choice-icon">🛑</span>
                <span class="choice-title">
                  {currentLang === 'bg'
                    ? 'Прекрати напълно сега (Пълен изход)'
                    : 'Terminate completely now (Full exit)'}
                </span>
              </div>
              <p class="choice-desc">
                {currentLang === 'bg'
                  ? 'Спира всички фонови услуги, модели и изчисления незабавно. Нито един процес няма да остане активен в паметта на компютъра.'
                  : 'Terminates all backend services, workers, and background processes immediately. Nothing will stay running.'}
              </p>
            </div>
          </label>
        </div>

        <label class="never-ask-row">
          <input
            type="checkbox"
            bind:checked={neverAskAgain}
            disabled={isExecuting}
          />
          <span>
            {currentLang === 'bg'
              ? 'Запомни избора ми и не показвай това съобщение повече'
              : 'Remember my choice and do not show this again'}
          </span>
        </label>
      </div>

      <footer class="exit-footer">
        <button
          class="btn btn-secondary"
          onclick={onClose}
          disabled={isExecuting}
        >
          {currentLang === 'bg' ? 'Отказ' : 'Cancel'}
        </button>

        <button
          class="btn"
          class:btn-primary={selectedAction === 'background'}
          class:btn-danger={selectedAction === 'terminate'}
          onclick={handleConfirm}
          disabled={isExecuting}
        >
          {#if isExecuting}
            <span class="spin">⏳</span> {currentLang === 'bg' ? 'Изпълнение...' : 'Executing...'}
          {:else if selectedAction === 'background'}
            ⚙️ {currentLang === 'bg' ? 'Минимизирай и довърши' : 'Run in background'}
          {:else}
            🛑 {currentLang === 'bg' ? 'Спри всичко и излез' : 'Terminate all & exit'}
          {/if}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .exit-backdrop {
    position: fixed;
    inset: 0;
    z-index: 1200;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: rgba(0, 0, 0, 0.76);
    backdrop-filter: blur(6px);
    animation: fadeIn 0.15s ease-out;
  }

  .exit-dialog {
    display: flex;
    flex-direction: column;
    width: 90%;
    max-width: 580px;
    background-color: var(--bg-surface, #1e2024);
    border: 1px solid var(--border-color, #333842);
    border-radius: 12px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.65);
    overflow: hidden;
    animation: scaleIn 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .exit-header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 18px 24px;
    background-color: var(--bg-surface-elevated, #25282f);
    border-bottom: 1px solid var(--border-color, #333842);
  }

  .header-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 42px;
    height: 42px;
    font-size: 22px;
    background: linear-gradient(135deg, #ef4444, #f97316);
    border-radius: 10px;
    box-shadow: 0 2px 8px rgba(239, 68, 68, 0.35);
  }

  .exit-title {
    margin: 0;
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--text-primary, #f3f4f6);
  }

  .exit-sub {
    margin: 2px 0 0 0;
    font-size: 0.8rem;
    color: var(--text-secondary, #9ca3af);
  }

  .exit-body {
    padding: 22px 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .pending-notice {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 14px;
    background-color: rgba(245, 158, 11, 0.12);
    border: 1px solid rgba(245, 158, 11, 0.3);
    border-radius: 8px;
  }

  .pending-notice.idle-notice {
    background-color: rgba(59, 130, 246, 0.1);
    border-color: rgba(59, 130, 246, 0.25);
  }

  .notice-icon {
    font-size: 1.3rem;
  }

  .notice-text {
    flex: 1;
    font-size: 0.82rem;
    line-height: 1.4;
  }

  .notice-text strong {
    color: var(--text-primary, #f3f4f6);
  }

  .notice-text p {
    margin: 3px 0 0 0;
    color: var(--text-secondary, #9ca3af);
  }

  .choices-group {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .choice-card {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px 16px;
    background-color: var(--bg-surface-elevated, #25282f);
    border: 1px solid var(--border-color, #333842);
    border-radius: 8px;
    cursor: pointer;
    transition: border-color 0.15s, background-color 0.15s;
  }

  .choice-card:hover {
    border-color: #3b82f6;
  }

  .choice-card.selected {
    border-color: #3b82f6;
    background-color: rgba(59, 130, 246, 0.08);
  }

  .choice-card input[type="radio"] {
    margin-top: 3px;
    accent-color: #3b82f6;
    cursor: pointer;
  }

  .choice-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .choice-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .choice-icon {
    font-size: 1.1rem;
  }

  .choice-title {
    font-size: 0.92rem;
    font-weight: 600;
    color: var(--text-primary, #f3f4f6);
  }

  .choice-desc {
    margin: 0;
    font-size: 0.78rem;
    line-height: 1.45;
    color: var(--text-secondary, #9ca3af);
  }

  .never-ask-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.8rem;
    color: var(--text-secondary, #9ca3af);
    cursor: pointer;
    user-select: none;
    padding-top: 4px;
  }

  .never-ask-row input[type="checkbox"] {
    accent-color: #3b82f6;
    cursor: pointer;
  }

  .exit-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    padding: 16px 24px;
    background-color: var(--bg-surface-elevated, #25282f);
    border-top: 1px solid var(--border-color, #333842);
  }

  .btn {
    padding: 8px 16px;
    font-size: 0.86rem;
    font-weight: 600;
    border-radius: 6px;
    cursor: pointer;
    border: none;
    transition: filter 0.15s, transform 0.1s;
  }

  .btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #3b82f6;
    color: #ffffff;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary, #f3f4f6);
    border: 1px solid var(--border-color, #4b5563);
  }

  .btn-danger {
    background: linear-gradient(135deg, #dc2626, #b91c1c);
    color: #ffffff;
  }

  .spin {
    display: inline-block;
    animation: rotate 1.5s linear infinite;
  }

  @keyframes rotate {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes scaleIn {
    from { transform: scale(0.96); opacity: 0; }
    to { transform: scale(1); opacity: 1; }
  }
</style>
