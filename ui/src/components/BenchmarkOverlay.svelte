<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchCalibrationStatus, triggerBenchmark } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import type { BenchmarkProgress, PowerInfo } from '../lib/types';

  let {
    isOpen,
    currentLang,
    powerInfo,
    progress,
    onClose,
    onBenchmarkStarted,
  }: {
    isOpen: boolean;
    currentLang: Language;
    powerInfo: PowerInfo | null;
    progress: BenchmarkProgress | null;
    onClose: () => void;
    onBenchmarkStarted?: () => void;
  } = $props();

  let isStarting = $state(false);
  let errorMessage = $state<string | null>(null);

  const isBattery = $derived(
    progress?.power_source === 'battery' || powerInfo?.power_source === 'battery'
  );

  const stages = [
    { id: 'hardware_probe', labelBg: 'Хардуер', labelEn: 'Hardware', num: 1 },
    { id: 'whisper_bench', labelBg: 'Whisper ASR', labelEn: 'Whisper ASR', num: 2 },
    { id: 'llm_bench', labelBg: 'LLM Анализ', labelEn: 'LLM Engine', num: 3 },
    { id: 'capacity_calc', labelBg: 'Капацитет', labelEn: 'Capacity', num: 4 },
  ];

  function getStageActive(stageIndex: number): boolean {
    const cur = progress?.stage_index ?? 0;
    return cur === stageIndex;
  }

  function getStageDone(stageIndex: number): boolean {
    const cur = progress?.stage_index ?? 0;
    if (progress && !progress.is_running && progress.completed_at) return true;
    return cur > stageIndex;
  }

  async function handleStartRun(targetPower?: 'ac' | 'battery') {
    isStarting = true;
    errorMessage = null;
    try {
      const pSource = targetPower || (isBattery ? 'battery' : 'ac');
      await triggerBenchmark({ power_source: pSource, force_rerun: true });
      if (onBenchmarkStarted) {
        onBenchmarkStarted();
      }
    } catch (e: any) {
      errorMessage = e?.message || (currentLang === 'bg' ? 'Грешка при стартиране на бенчмарк' : 'Failed to start benchmark');
    } finally {
      isStarting = false;
    }
  }
</script>

{#if isOpen}
  <div
    class="overlay-backdrop"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
  >
    <div class="overlay-dialog" role="dialog" aria-modal="true">
      <header class="overlay-header">
        <div class="header-left">
          <div class="header-icon-box" class:battery-icon={isBattery}>
            {isBattery ? '🔋' : '⚡'}
          </div>
          <div>
            <div class="header-title-row">
              <h3 class="overlay-title">
                {currentLang === 'bg'
                  ? 'Хардуерно калибриране и бенчмарк'
                  : 'Hardware Calibration & Benchmark'}
              </h3>
              <span class="power-badge" class:is-battery={isBattery}>
                {#if isBattery}
                  🔋 {currentLang === 'bg' ? 'Режим на батерия' : 'Battery Mode'}
                  {#if powerInfo?.battery_percent !== null && powerInfo?.battery_percent !== undefined}
                    ({powerInfo.battery_percent}%)
                  {/if}
                {:else}
                  ⚡ {currentLang === 'bg' ? 'Мрежово захранване (AC)' : 'AC Plugged In'}
                {/if}
              </span>
            </div>
            <p class="overlay-sub">
              {currentLang === 'bg'
                ? 'Системата поддържа два профила: максимална скорост на ток и пестене на енергия на батерия.'
                : 'Dual profiles supported: high-throughput on AC power & battery-efficient mode on battery.'}
            </p>
          </div>
        </div>
        <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>
      </header>

      <div class="stages-stepper">
        {#each stages as st}
          <div
            class="step-pill"
            class:active={getStageActive(st.num)}
            class:done={getStageDone(st.num)}
          >
            <span class="step-num">
              {#if getStageDone(st.num)}✓{:else}{st.num}{/if}
            </span>
            <span class="step-text">
              {currentLang === 'bg' ? st.labelBg : st.labelEn}
            </span>
          </div>
          {#if st.num < 4}
            <div class="step-line" class:done={getStageDone(st.num)}></div>
          {/if}
        {/each}
      </div>

      <div class="overlay-body">
        {#if errorMessage}
          <div class="error-banner">
            ⚠️ {errorMessage}
          </div>
        {/if}

        {#if progress?.is_running}
          <div class="running-card">
            <div class="progress-bar-container">
              <div class="progress-track">
                <div
                  class="progress-fill"
                  style="width: {Math.max(5, Math.min(100, progress.progress_pct))}%"
                ></div>
              </div>
              <div class="progress-meta">
                <span class="progress-text">
                  {currentLang === 'bg' ? progress.status_text : (progress.status_text_en || progress.status_text)}
                </span>
                <span class="progress-pct">{progress.progress_pct}%</span>
              </div>
            </div>

            {#if progress.current_model}
              <div class="current-model-pill">
                <span class="pulse-indicator"></span>
                <span class="cm-label">{currentLang === 'bg' ? 'Текущ тест:' : 'Testing:'}</span>
                <span class="cm-value mono">{progress.current_model}</span>
              </div>
            {/if}

            <div class="choice-alert">
              <div class="ca-text">
                <strong class="ca-title">
                  {currentLang === 'bg' ? 'Можете да продължите работа:' : 'You can continue working:'}
                </strong>
                <p class="ca-desc">
                  {currentLang === 'bg'
                    ? 'Бенчмаркът изисква 30-60 секунди. Можете да изчакате тук или да продължите към приложението - тестът ще завърши на заден план.'
                    : 'The benchmark takes 30-60 seconds. You may wait here or dismiss this overlay to use the app in background.'}
                </p>
              </div>
              <button class="btn btn-primary btn-bg-action" onclick={onClose}>
                🚀 {currentLang === 'bg' ? 'Продължи на заден план (Към приложението)' : 'Continue in Background'}
              </button>
            </div>
          </div>
        {:else if progress && !progress.is_running && progress.completed_at}
          <div class="completed-card">
            <div class="success-banner">
              <span class="success-icon">✓</span>
              <div>
                <h4 class="success-title">
                  {currentLang === 'bg' ? 'Калибрирането завърши успешно!' : 'Calibration Complete!'}
                </h4>
                <p class="success-desc">
                  {currentLang === 'bg'
                    ? `Измерен дневен капацитет за профил "${isBattery ? 'Батерия' : 'AC Захранване'}":`
                    : `Measured daily capacity for "${isBattery ? 'Battery' : 'AC Power'}" profile:`}
                  <strong class="capacity-stat">
                    {Math.round(progress.final_capacity || 0).toLocaleString()} {currentLang === 'bg' ? 'видеа / 24ч' : 'videos / 24h'}
                  </strong>
                </p>
              </div>
            </div>

            <div class="power-switch-notice">
              <span class="ps-icon">💡</span>
              <span class="ps-text">
                {currentLang === 'bg'
                  ? 'При включване или изключване на захранващия кабел, LeadFinder автоматично превключва към съответния профил.'
                  : 'When plugging or unplugging the power charger, LeadFinder automatically activates the matching profile.'}
              </span>
            </div>
          </div>
        {:else}
          <div class="idle-card">
            <p class="idle-desc">
              {currentLang === 'bg'
                ? 'Няма активен бенчмарк за текущия източник на захранване. Стартирайте калибрирането, за да измерите реалната скорост на вашия процесор и видеокарта.'
                : 'No benchmark has been recorded for the current power mode. Run calibration to measure your exact CPU and GPU throughput.'}
            </p>
            <div class="idle-actions">
              <button
                class="btn btn-primary"
                onclick={() => handleStartRun(isBattery ? 'battery' : 'ac')}
                disabled={isStarting}
              >
                {isStarting ? '⏳ ...' : (isBattery ? '🔋 Стартирай бенчмарк за Батерия' : '⚡ Стартирай бенчмарк за AC Захранване')}
              </button>
            </div>
          </div>
        {/if}

        {#if (progress?.asr_results && progress.asr_results.length > 0) || (progress?.llm_results && progress.llm_results.length > 0)}
          <div class="results-container">
            <h4 class="results-heading">
              {currentLang === 'bg' ? 'Резултати от тестовете на кандидатите' : 'Candidate Benchmark Results'}
            </h4>

            {#if progress?.asr_results && progress.asr_results.length > 0}
              <div class="table-card">
                <div class="table-title">🎙️ Whisper ASR Модели</div>
                <table class="bench-table">
                  <thead>
                    <tr>
                      <th>{currentLang === 'bg' ? 'Модел' : 'Model'}</th>
                      <th>{currentLang === 'bg' ? 'RTF Скорост' : 'RTF Speed'}</th>
                      <th>WER %</th>
                      <th>{currentLang === 'bg' ? 'Време' : 'Time'}</th>
                      <th>{currentLang === 'bg' ? 'Капацитет' : 'Capacity'}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each progress.asr_results as r}
                      <tr>
                        <td class="mono font-bold">{r.model}</td>
                        <td>
                          <span class="pill-rtf" class:good={r.rtf < 0.15} class:med={r.rtf >= 0.15 && r.rtf < 0.35} class:slow={r.rtf >= 0.35}>
                            {r.rtf.toFixed(3)}x
                          </span>
                        </td>
                        <td>{(r.wer * 100).toFixed(1)}%</td>
                        <td>{r.elapsed_ms} ms</td>
                        <td class="text-green font-bold">
                          {Math.round(r.raw_capacity_per_day).toLocaleString()} / ден
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}

            {#if progress?.llm_results && progress.llm_results.length > 0}
              <div class="table-card mt-3">
                <div class="table-title">🧠 LLM Извличане на факти и твърдения</div>
                <table class="bench-table">
                  <thead>
                    <tr>
                      <th>{currentLang === 'bg' ? 'Модел' : 'Model'}</th>
                      <th>{currentLang === 'bg' ? 'Бекенд' : 'Backend'}</th>
                      <th>{currentLang === 'bg' ? 'Скорост (ток/сек)' : 'Tokens/sec'}</th>
                      <th>{currentLang === 'bg' ? 'Средно време' : 'Mean latency'}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each progress.llm_results as l}
                      <tr>
                        <td class="mono font-bold">{l.model}</td>
                        <td><span class="backend-pill">{l.backend}</span></td>
                        <td class="font-bold text-cyan">{l.tokens_per_second.toFixed(1)} tok/s</td>
                        <td>{l.mean_call_seconds.toFixed(2)} s</td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <footer class="overlay-footer">
        <div class="footer-left">
          {#if !progress?.is_running}
            <button
              class="btn btn-secondary"
              onclick={() => handleStartRun(isBattery ? 'battery' : 'ac')}
              disabled={isStarting}
            >
              🔄 {currentLang === 'bg' ? 'Повтори бенчмарка' : 'Re-run Benchmark'}
            </button>
          {/if}
        </div>
        <div class="footer-right">
          {#if progress?.is_running}
            <button class="btn btn-secondary" onclick={onClose}>
              {currentLang === 'bg' ? 'Продължи на заден план' : 'Run in background'}
            </button>
          {:else}
            <button class="btn btn-primary" onclick={onClose}>
              ✓ {currentLang === 'bg' ? 'Готово / Към приложението' : 'Done / Open App'}
            </button>
          {/if}
        </div>
      </footer>
    </div>
  </div>
{/if}

<style>
  .overlay-backdrop {
    position: fixed;
    inset: 0;
    z-index: 10000;
    background: rgba(0, 0, 0, 0.75);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
    box-sizing: border-box;
  }

  .overlay-dialog {
    width: 100%;
    max-width: 820px;
    max-height: 92vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-surface, #181a20);
    border: 1px solid var(--border-color, #2b303c);
    border-radius: 14px;
    box-shadow: 0 25px 60px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }

  .overlay-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 1.25rem 1.5rem;
    border-bottom: 1px solid var(--border-color, #2b303c);
    background: rgba(255, 255, 255, 0.02);
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .header-icon-box {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    background: rgba(245, 158, 11, 0.15);
    border: 1px solid rgba(245, 158, 11, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
  }

  .header-icon-box.battery-icon {
    background: rgba(16, 185, 129, 0.15);
    border-color: rgba(16, 185, 129, 0.35);
  }

  .header-title-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .overlay-title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--text-heading, #f3f4f6);
  }

  .power-badge {
    display: inline-flex;
    align-items: center;
    font-size: 0.75rem;
    font-weight: 600;
    padding: 0.25rem 0.6rem;
    border-radius: 999px;
    background: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
    border: 1px solid rgba(245, 158, 11, 0.3);
  }

  .power-badge.is-battery {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
    border-color: rgba(16, 185, 129, 0.3);
  }

  .overlay-sub {
    margin: 0.25rem 0 0 0;
    font-size: 0.85rem;
    color: var(--text-muted, #9ca3af);
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-muted, #9ca3af);
    font-size: 1.25rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    border-radius: 6px;
  }

  .close-btn:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.08);
  }

  .stages-stepper {
    display: flex;
    align-items: center;
    padding: 0.85rem 1.5rem;
    background: rgba(0, 0, 0, 0.18);
    border-bottom: 1px solid var(--border-color, #2b303c);
  }

  .step-pill {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.7rem;
    border-radius: 999px;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-muted, #9ca3af);
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .step-pill.active {
    color: #60a5fa;
    border-color: #3b82f6;
    background: rgba(59, 130, 246, 0.15);
  }

  .step-pill.done {
    color: #34d399;
    border-color: #10b981;
    background: rgba(16, 185, 129, 0.12);
  }

  .step-num {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.72rem;
    background: rgba(255, 255, 255, 0.1);
  }

  .step-line {
    flex: 1;
    height: 2px;
    background: rgba(255, 255, 255, 0.08);
    margin: 0 0.5rem;
  }

  .step-line.done {
    background: #10b981;
  }

  .overlay-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .error-banner {
    padding: 0.85rem 1rem;
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.35);
    border-radius: 8px;
    color: #fca5a5;
    font-size: 0.9rem;
  }

  .running-card {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color, #2b303c);
    border-radius: 12px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .progress-track {
    width: 100%;
    height: 10px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 999px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #3b82f6, #10b981);
    transition: width 0.3s ease;
  }

  .progress-meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 0.5rem;
    font-size: 0.88rem;
  }

  .progress-text {
    color: var(--text-heading, #f3f4f6);
    font-weight: 500;
  }

  .progress-pct {
    font-weight: 700;
    color: #60a5fa;
  }

  .current-model-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    background: rgba(59, 130, 246, 0.12);
    border: 1px solid rgba(59, 130, 246, 0.25);
    padding: 0.35rem 0.85rem;
    border-radius: 8px;
    font-size: 0.82rem;
    width: fit-content;
  }

  .pulse-indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #60a5fa;
    box-shadow: 0 0 8px #60a5fa;
    animation: pulse 1.5s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.4; transform: scale(0.8); }
  }

  .cm-label {
    color: var(--text-muted, #9ca3af);
  }

  .cm-value {
    color: #93c5fd;
    font-weight: 600;
  }

  .choice-alert {
    background: rgba(99, 102, 241, 0.08);
    border: 1px solid rgba(99, 102, 241, 0.25);
    border-radius: 10px;
    padding: 1rem 1.25rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1.5rem;
  }

  .ca-title {
    display: block;
    color: #e0e7ff;
    font-size: 0.92rem;
    margin-bottom: 0.25rem;
  }

  .ca-desc {
    margin: 0;
    font-size: 0.82rem;
    color: #a5b4fc;
    line-height: 1.4;
  }

  .btn-bg-action {
    white-space: nowrap;
    font-weight: 600;
    padding: 0.6rem 1.1rem;
  }

  .completed-card {
    background: rgba(16, 185, 129, 0.05);
    border: 1px solid rgba(16, 185, 129, 0.25);
    border-radius: 12px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .success-banner {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .success-icon {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: #10b981;
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.25rem;
    font-weight: 700;
  }

  .success-title {
    margin: 0 0 0.25rem 0;
    color: #34d399;
    font-size: 1.1rem;
  }

  .success-desc {
    margin: 0;
    font-size: 0.88rem;
    color: var(--text-body, #d1d5db);
  }

  .capacity-stat {
    color: #6ee7b7;
    font-size: 1.05rem;
    margin-left: 0.35rem;
  }

  .power-switch-notice {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 0.85rem;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 8px;
    font-size: 0.8rem;
    color: var(--text-muted, #9ca3af);
  }

  .idle-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px dashed var(--border-color, #2b303c);
    border-radius: 12px;
    padding: 2rem;
    text-align: center;
  }

  .idle-desc {
    color: var(--text-muted, #9ca3af);
    font-size: 0.95rem;
    max-width: 500px;
    margin: 0 auto 1.5rem auto;
    line-height: 1.5;
  }

  .results-container {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .results-heading {
    margin: 0;
    font-size: 0.95rem;
    color: var(--text-heading, #f3f4f6);
    font-weight: 600;
  }

  .table-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color, #2b303c);
    border-radius: 10px;
    overflow: hidden;
  }

  .table-title {
    padding: 0.6rem 1rem;
    background: rgba(255, 255, 255, 0.04);
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-muted, #9ca3af);
    border-bottom: 1px solid var(--border-color, #2b303c);
  }

  .bench-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.82rem;
  }

  .bench-table th, .bench-table td {
    padding: 0.55rem 1rem;
    text-align: left;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .bench-table th {
    color: var(--text-muted, #9ca3af);
    font-weight: 600;
    font-size: 0.75rem;
    text-transform: uppercase;
  }

  .pill-rtf {
    display: inline-block;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-weight: 600;
    font-family: monospace;
    font-size: 0.78rem;
  }

  .pill-rtf.good {
    background: rgba(16, 185, 129, 0.15);
    color: #34d399;
  }

  .pill-rtf.med {
    background: rgba(245, 158, 11, 0.15);
    color: #fbbf24;
  }

  .pill-rtf.slow {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
  }

  .backend-pill {
    padding: 0.15rem 0.45rem;
    border-radius: 4px;
    background: rgba(99, 102, 241, 0.15);
    color: #a5b4fc;
    font-size: 0.75rem;
  }

  .text-green { color: #34d399; }
  .text-cyan { color: #38bdf8; }
  .font-bold { font-weight: 600; }
  .mono { font-family: monospace; }
  .mt-3 { margin-top: 0.75rem; }

  .overlay-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.5rem;
    border-top: 1px solid var(--border-color, #2b303c);
    background: rgba(255, 255, 255, 0.02);
  }

  .btn {
    padding: 0.5rem 1rem;
    border-radius: 8px;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: background 0.15s, opacity 0.15s;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #3b82f6;
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #2563eb;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-body, #e5e7eb);
    border: 1px solid var(--border-color, #374151);
  }

  .btn-secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.14);
  }
</style>
