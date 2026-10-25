<script lang="ts">
  import { onMount } from 'svelte';
  import {
    cancelModelDownload,
    deleteModel,
    fetchInstalledModels,
    fetchModelCatalog,
    fetchModelDownloadStatus,
    fetchModelRoles,
    saveModelRoles,
    scanExternalAiServers,
    scanModelsFolder,
    startModelDownload,
    testModelPrompt,
  } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import type {
    CuratedModelCatalogItem,
    ExternalAiServerStatus,
    InstalledModelInfo,
    ModelDownloadProgress,
    ModelHubRolesConfig,
    ModelTestResponse,
  } from '../lib/types';

  let {
    currentLang,
    onOpenGoogleAiWizard,
  }: {
    currentLang: Language;
    onOpenGoogleAiWizard?: () => void;
  } = $props();

  let activeSubtab = $state<'roles' | 'catalog' | 'servers' | 'playground'>('roles');

  let installedModels = $state<InstalledModelInfo[]>([]);
  let modelsDir = $state<string>('');
  let catalog = $state<CuratedModelCatalogItem[]>([]);
  let rolesConfig = $state<ModelHubRolesConfig | null>(null);
  let externalServers = $state<ExternalAiServerStatus[]>([]);
  let downloadProgress = $state<ModelDownloadProgress | null>(null);

  let customModelUrl = $state('');
  let customModelFilename = $state('');
  let customServerUrl = $state('');

  let testProvider = $state('local');
  let testModelName = $state('');
  let testPrompt = $state('Извлечи ключовите проверими твърдения от текста: "Правителството обяви, че инфлацията е спаднала до 1.5% през септември."');
  let testSystemPrompt = $state('Върни отговора строго като валиден JSON обект с поле "claims", съдържащо списък от твърдения.');
  let testResult = $state<ModelTestResponse | null>(null);
  let isTesting = $state(false);

  let isSavingRoles = $state(false);
  let toastMsg = $state<string | null>(null);
  let toastError = $state(false);
  let isScanningServers = $state(false);
  let downloadTimer: ReturnType<typeof setInterval> | null = null;

  onMount(async () => {
    await reloadAll();
    startDownloadPolling();
    return () => {
      if (downloadTimer) clearInterval(downloadTimer);
    };
  });

  function startDownloadPolling() {
    if (downloadTimer) clearInterval(downloadTimer);
    downloadTimer = setInterval(async () => {
      try {
        const st = await fetchModelDownloadStatus();
        downloadProgress = st;
        if (st.status === 'completed' && !st.active) {
          await reloadInstalled();
          await reloadCatalog();
        }
      } catch {}
    }, 1500);
  }

  async function reloadAll() {
    await Promise.all([
      reloadInstalled(),
      reloadCatalog(),
      reloadRoles(),
      scanServers(),
    ]);
  }

  async function reloadInstalled() {
    try {
      const res = await fetchInstalledModels();
      installedModels = res.models || [];
      modelsDir = res.models_dir || '';
    } catch {}
  }

  async function reloadCatalog() {
    try {
      const res = await fetchModelCatalog();
      catalog = res.catalog || [];
    } catch {}
  }

  async function reloadRoles() {
    try {
      rolesConfig = await fetchModelRoles();
    } catch {}
  }

  async function scanServers() {
    isScanningServers = true;
    try {
      const res = await scanExternalAiServers(customServerUrl);
      externalServers = res.servers || [];
    } catch {
    } finally {
      isScanningServers = false;
    }
  }

  async function handleSaveRoles() {
    if (!rolesConfig) return;
    isSavingRoles = true;
    try {
      await saveModelRoles(rolesConfig);
      showToast(currentLang === 'bg' ? 'Ролите на моделите са запазени успешно!' : 'Model roles saved successfully!');
    } catch (e: any) {
      showToast(e?.message || 'Error saving roles', true);
    } finally {
      isSavingRoles = false;
    }
  }

  async function handleDownloadCatalogItem(item: CuratedModelCatalogItem) {
    try {
      await startModelDownload({
        model_id: item.id,
        url: item.download_url,
        filename: item.filename,
        kind: item.kind === 'whisper_bin' ? 'whisper' : 'llm',
      });
      showToast(currentLang === 'bg' ? `Свалянето на ${item.name} започна!` : `Downloading ${item.name}...`);
    } catch (e: any) {
      showToast(e?.message || 'Download error', true);
    }
  }

  async function handleCancelDownload() {
    try {
      await cancelModelDownload();
      showToast(currentLang === 'bg' ? 'Свалянето беше прекратено.' : 'Download cancelled.');
    } catch {}
  }

  async function handleCustomDownload() {
    if (!customModelUrl || !customModelFilename) {
      showToast(currentLang === 'bg' ? 'Моля, въведете URL и име на файла.' : 'Please provide URL and filename.', true);
      return;
    }
    const isWhisper = customModelFilename.endsWith('.bin');
    try {
      await startModelDownload({
        model_id: customModelFilename,
        url: customModelUrl,
        filename: customModelFilename,
        kind: isWhisper ? 'whisper' : 'llm',
      });
      showToast(currentLang === 'bg' ? `Свалянето на ${customModelFilename} започна!` : `Downloading ${customModelFilename}...`);
      customModelUrl = '';
      customModelFilename = '';
    } catch (e: any) {
      showToast(e?.message || 'Download error', true);
    }
  }

  async function handleDeleteModelFile(kind: string, filename: string) {
    const confirmMsg = currentLang === 'bg'
      ? `Сигурни ли сте, че искате да изтриете файла "${filename}"?`
      : `Are you sure you want to delete "${filename}"?`;
    if (!confirm(confirmMsg)) return;

    try {
      await deleteModel(kind, filename);
      showToast(currentLang === 'bg' ? `Файлът "${filename}" е изтрит.` : `Deleted "${filename}".`);
      await reloadInstalled();
      await reloadCatalog();
    } catch (e: any) {
      showToast(e?.message || 'Delete error', true);
    }
  }

  async function handleRescanFolder() {
    try {
      const res = await scanModelsFolder();
      installedModels = res.models || [];
      showToast(currentLang === 'bg' ? 'Папката беше пресканирана успешно!' : 'Folder rescanned successfully!');
    } catch (e: any) {
      showToast(e?.message || 'Scan error', true);
    }
  }

  async function handleRunTest() {
    if (!testPrompt) return;
    isTesting = true;
    testResult = null;
    try {
      const targetModel = testModelName || (installedModels[0]?.filename ?? 'BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf');
      testResult = await testModelPrompt({
        provider: testProvider,
        model: targetModel,
        custom_url: testProvider === 'ollama' || testProvider === 'lmstudio' || testProvider === 'custom' ? customServerUrl : undefined,
        prompt: testPrompt,
        system_prompt: testSystemPrompt || undefined,
      });
    } catch (e: any) {
      testResult = {
        success: false,
        elapsed_ms: 0,
        tokens_in: 0,
        tokens_out: 0,
        tokens_per_second: 0,
        raw_text: '',
        error: e?.message || 'Test failed',
      };
    } finally {
      isTesting = false;
    }
  }

  function showToast(msg: string, isErr = false) {
    toastMsg = msg;
    toastError = isErr;
    setTimeout(() => {
      toastMsg = null;
    }, 4000);
  }
</script>

<div class="models-screen">
  <header class="ms-header">
    <div class="ms-header-info">
      <div class="ms-title-row">
        <h2>🤖 {currentLang === 'bg' ? 'AI Модели и мулти-моделна оркестрация' : 'AI Models & Multi-Model Orchestra'}</h2>
        <span class="active-models-pill">
          {installedModels.length} {currentLang === 'bg' ? 'налични модела' : 'models installed'}
        </span>
      </div>
      <p class="ms-sub">
        {currentLang === 'bg'
          ? 'Разпределяйте специализирани AI модели за реч (ASR), екстракция на твърдения, проверка на факти и OSINT досиета. Инсталирайте нови модели с 1 клик или свържете Ollama и LM Studio.'
          : 'Assign dedicated AI models for Speech (ASR), Claim Extraction, Fact Verification, and OSINT Dossiers. 1-click downloads or external Ollama/LM Studio servers.'}
      </p>
    </div>

    <div class="ms-subtabs">
      <button class="subtab-btn" class:active={activeSubtab === 'roles'} onclick={() => activeSubtab = 'roles'}>
        🎭 {currentLang === 'bg' ? 'Роли в конвейера' : 'Pipeline Roles'}
      </button>
      <button class="subtab-btn" class:active={activeSubtab === 'catalog'} onclick={() => activeSubtab = 'catalog'}>
        📦 {currentLang === 'bg' ? 'Каталог и сваляне' : 'Catalog & Downloads'}
      </button>
      <button class="subtab-btn" class:active={activeSubtab === 'servers'} onclick={() => activeSubtab = 'servers'}>
        🔌 {currentLang === 'bg' ? 'Външни сървъри (Ollama)' : 'External Servers'}
      </button>
      <button class="subtab-btn" class:active={activeSubtab === 'playground'} onclick={() => activeSubtab = 'playground'}>
        🧪 {currentLang === 'bg' ? 'AI Тестово поле' : 'Model Playground'}
      </button>
    </div>
  </header>

  {#if toastMsg}
    <div class="toast-banner" class:is-err={toastError}>
      {toastError ? '⚠️' : '✓'} {toastMsg}
    </div>
  {/if}

  {#if downloadProgress && downloadProgress.active}
    <div class="download-banner">
      <div class="db-left">
        <span class="db-spinner">⏳</span>
        <div>
          <strong class="db-title">
            {currentLang === 'bg' ? 'Сваляне на модел:' : 'Downloading model:'} {downloadProgress.filename}
          </strong>
          <span class="db-meta">
            {(downloadProgress.bytes_downloaded / 1048576).toFixed(1)} MB / {(downloadProgress.total_bytes / 1048576).toFixed(1)} MB
            ({downloadProgress.progress_pct}%) • {(downloadProgress.speed_bps / 1048576).toFixed(1)} MB/s
          </span>
        </div>
      </div>
      <div class="db-right">
        <div class="db-progress-track">
          <div class="db-progress-fill" style="width: {downloadProgress.progress_pct}%"></div>
        </div>
        <button class="btn btn-secondary btn-sm" onclick={handleCancelDownload}>
          ✕ {currentLang === 'bg' ? 'Отказ' : 'Cancel'}
        </button>
      </div>
    </div>
  {/if}

  <main class="ms-body">
    {#if activeSubtab === 'roles'}
      <section class="roles-section">
        <div class="roles-grid">
          <div class="role-card">
            <div class="rc-header">
              <span class="rc-icon">🎙️</span>
              <div>
                <h4 class="rc-title">{currentLang === 'bg' ? 'Реч-към-Текст (Whisper ASR)' : 'Speech-to-Text (ASR)'}</h4>
                <span class="rc-desc">{currentLang === 'bg' ? 'Транскрибира говор от видеоклипове и аудио записи' : 'Transcribes spoken audio into timed segments'}</span>
              </div>
            </div>
            {#if rolesConfig?.roles?.asr}
              <div class="rc-controls">
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Провайдър:' : 'Provider:'}</span>
                  <select bind:value={rolesConfig.roles.asr.provider}>
                    <option value="whisper">Локален Whisper (offline)</option>
                    <option value="google">Google Web Speech (cloud assist)</option>
                  </select>
                </label>
                {#if rolesConfig.roles.asr.provider === 'whisper'}
                  <label class="control-row">
                    <span class="ctrl-label">{currentLang === 'bg' ? 'Модел:' : 'Model:'}</span>
                    <select bind:value={rolesConfig.roles.asr.model_name}>
                      <option value="large-v3-turbo">large-v3-turbo (Препоръчителен • SOTA BG)</option>
                      <option value="small">small (Балансиран за слаби процесори)</option>
                      <option value="base">base (Бърз • Режим на батерия)</option>
                      <option value="tiny">tiny (Минимален)</option>
                      {#each installedModels.filter(m => m.kind === 'whisper_bin') as wm}
                        <option value={wm.filename}>{wm.filename} ({wm.size_display})</option>
                      {/each}
                    </select>
                  </label>
                {/if}
              </div>
            {/if}
          </div>

          <div class="role-card">
            <div class="rc-header">
              <span class="rc-icon">⚡</span>
              <div>
                <h4 class="rc-title">{currentLang === 'bg' ? 'Извличане на факти и твърдения' : 'Claim Extractor'}</h4>
                <span class="rc-desc">{currentLang === 'bg' ? 'Изисква максимална скорост и спазване на JSON схеми' : 'High throughput extraction of checkworthy claims'}</span>
              </div>
            </div>
            {#if rolesConfig?.roles?.claim_extraction}
              <div class="rc-controls">
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Провайдър:' : 'Provider:'}</span>
                  <select bind:value={rolesConfig.roles.claim_extraction.provider}>
                    <option value="local">Локален GGUF (llama.cpp)</option>
                    <option value="gemini">Google Gemini (облачен)</option>
                    <option value="ollama">Ollama (локален сървър)</option>
                    <option value="lmstudio">LM Studio (локален сървър)</option>
                    <option value="heuristics">Офлайн евристичен анализатор</option>
                  </select>
                </label>
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Модел:' : 'Model:'}</span>
                  {#if rolesConfig.roles.claim_extraction.provider === 'local'}
                    <select bind:value={rolesConfig.roles.claim_extraction.model_name}>
                      {#each installedModels.filter(m => m.kind === 'llm_gguf') as lm}
                        <option value={lm.filename}>{lm.filename} ({lm.size_display})</option>
                      {/each}
                    </select>
                  {:else if rolesConfig.roles.claim_extraction.provider === 'gemini'}
                    <select bind:value={rolesConfig.roles.claim_extraction.model_name}>
                      <option value="gemini-2.5-flash">gemini-2.5-flash (Бърз, Препоръчителен)</option>
                      <option value="gemini-1.5-flash">gemini-1.5-flash</option>
                      <option value="gemini-1.5-pro">gemini-1.5-pro</option>
                    </select>
                  {:else}
                    <input type="text" bind:value={rolesConfig.roles.claim_extraction.model_name} placeholder="qwen2.5:3b, llama3.2..." />
                  {/if}
                </label>
              </div>
            {/if}
          </div>

          <div class="role-card">
            <div class="rc-header">
              <span class="rc-icon">🧠</span>
              <div>
                <h4 class="rc-title">{currentLang === 'bg' ? 'Проверка на факти и разсъждение' : 'Fact Verification & Reasoning'}</h4>
                <span class="rc-desc">{currentLang === 'bg' ? 'Сравнява доказателства от новини и издава финалния вердикт' : 'Deep logical reasoning against retrieved evidence'}</span>
              </div>
            </div>
            {#if rolesConfig?.roles?.fact_verification}
              <div class="rc-controls">
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Провайдър:' : 'Provider:'}</span>
                  <select bind:value={rolesConfig.roles.fact_verification.provider}>
                    <option value="local">Локален GGUF (llama.cpp)</option>
                    <option value="gemini">Google Gemini (облачен)</option>
                    <option value="ollama">Ollama (локален сървър)</option>
                    <option value="lmstudio">LM Studio (локален сървър)</option>
                  </select>
                </label>
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Модел:' : 'Model:'}</span>
                  {#if rolesConfig.roles.fact_verification.provider === 'local'}
                    <select bind:value={rolesConfig.roles.fact_verification.model_name}>
                      {#each installedModels.filter(m => m.kind === 'llm_gguf') as lm}
                        <option value={lm.filename}>{lm.filename} ({lm.size_display})</option>
                      {/each}
                    </select>
                  {:else if rolesConfig.roles.fact_verification.provider === 'gemini'}
                    <select bind:value={rolesConfig.roles.fact_verification.model_name}>
                      <option value="gemini-2.5-flash">gemini-2.5-flash</option>
                      <option value="gemini-1.5-pro">gemini-1.5-pro (Максимално разсъждение)</option>
                    </select>
                  {:else}
                    <input type="text" bind:value={rolesConfig.roles.fact_verification.model_name} placeholder="qwen2.5:7b, mistral..." />
                  {/if}
                </label>
              </div>
            {/if}
          </div>

          <div class="role-card">
            <div class="rc-header">
              <span class="rc-icon">🕵️</span>
              <div>
                <h4 class="rc-title">{currentLang === 'bg' ? 'OSINT Разследващи досиета' : 'OSINT Dossier Generator'}</h4>
                <span class="rc-desc">{currentLang === 'bg' ? 'Генерира пълни биографични профили и досиета от 2000 думи' : 'Synthesizes exhaustive OSINT intelligence dossiers'}</span>
              </div>
            </div>
            {#if rolesConfig?.roles?.osint_dossier}
              <div class="rc-controls">
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Провайдър:' : 'Provider:'}</span>
                  <select bind:value={rolesConfig.roles.osint_dossier.provider}>
                    <option value="gemini">Google Gemini (1M контекст, Препоръчителен)</option>
                    <option value="local">Локален GGUF</option>
                    <option value="ollama">Ollama</option>
                  </select>
                </label>
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Модел:' : 'Model:'}</span>
                  {#if rolesConfig.roles.osint_dossier.provider === 'gemini'}
                    <select bind:value={rolesConfig.roles.osint_dossier.model_name}>
                      <option value="gemini-2.5-flash">gemini-2.5-flash (Идеален за 2000-думи досие)</option>
                      <option value="gemini-1.5-pro">gemini-1.5-pro</option>
                    </select>
                  {:else}
                    <input type="text" bind:value={rolesConfig.roles.osint_dossier.model_name} placeholder="Модел с голям контекст..." />
                  {/if}
                </label>
              </div>
            {/if}
          </div>
        </div>

        {#if rolesConfig?.consensus}
          <div class="consensus-box">
            <div class="cb-header">
              <div class="cb-title-group">
                <span class="cb-icon">🤝</span>
                <div>
                  <h4 class="cb-title">{currentLang === 'bg' ? 'Мулти-моделен консенсус (Ensemble Consensus Mode)' : 'Multi-Model Consensus & Debate'}</h4>
                  <p class="cb-sub">
                    {currentLang === 'bg'
                      ? 'При съмнителни или гранични твърдения, системата активира втори независим AI модел за двойна проверка. При съгласие вердиктът получава 100% доверие, а при разминаване се отбелязват аргументите на двата модела.'
                      : 'Executes secondary verification. When models agree, confidence is maximized; when they split, both rationales are logged for human review.'}
                  </p>
                </div>
              </div>
              <label class="switch-label">
                <input type="checkbox" bind:checked={rolesConfig.consensus.enabled} />
                <span class="switch-slider"></span>
              </label>
            </div>

            {#if rolesConfig.consensus.enabled}
              <div class="cb-body">
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Вторичен модел за оценка:' : 'Secondary Arbiter Model:'}</span>
                  <select bind:value={rolesConfig.consensus.secondary_provider}>
                    <option value="gemini">Google Gemini (облачен)</option>
                    <option value="local">Локален GGUF</option>
                    <option value="ollama">Ollama</option>
                  </select>
                </label>
                <label class="control-row">
                  <span class="ctrl-label">{currentLang === 'bg' ? 'Име на модела:' : 'Model Name:'}</span>
                  {#if rolesConfig.consensus.secondary_provider === 'gemini'}
                    <select bind:value={rolesConfig.consensus.secondary_model}>
                      <option value="gemini-2.5-flash">gemini-2.5-flash</option>
                      <option value="gemini-1.5-pro">gemini-1.5-pro</option>
                    </select>
                  {:else if rolesConfig.consensus.secondary_provider === 'local'}
                    <select bind:value={rolesConfig.consensus.secondary_model}>
                      {#each installedModels.filter(m => m.kind === 'llm_gguf') as lm}
                        <option value={lm.filename}>{lm.filename}</option>
                      {/each}
                    </select>
                  {:else}
                    <input type="text" bind:value={rolesConfig.consensus.secondary_model} placeholder="llama3.1:latest..." />
                  {/if}
                </label>
              </div>
            {/if}
          </div>
        {/if}

        <div class="roles-actions">
          <button class="btn btn-primary" onclick={handleSaveRoles} disabled={isSavingRoles}>
            💾 {isSavingRoles ? '...' : (currentLang === 'bg' ? 'Запази разпределението на ролите' : 'Save Role Assignments')}
          </button>
        </div>
      </section>
    {:else if activeSubtab === 'catalog'}
      <section class="catalog-section">
        <div class="catalog-header-row">
          <div>
            <h3>{currentLang === 'bg' ? 'Каталог с препоръчителни модели (1-клик инсталация)' : 'Curated Model Catalog (1-Click Install)'}</h3>
            <p class="text-muted">{currentLang === 'bg' ? 'Всички модели са тествани за стабилност, съвместимост и висока точност на български.' : 'All models are benchmarked for Bulgarian language fidelity and speed.'}</p>
          </div>
          <button class="btn btn-secondary btn-sm" onclick={handleRescanFolder}>
            🔄 {currentLang === 'bg' ? 'Пресканирай папка models/' : 'Rescan Folder'}
          </button>
        </div>

        <div class="catalog-grid">
          {#each catalog as item}
            <div class="catalog-card" class:installed={item.is_installed}>
              <div class="cc-top">
                <div>
                  <h4 class="cc-name">{item.name}</h4>
                  <span class="cc-author">{item.author}</span>
                </div>
                <span class="cc-kind-badge" class:whisper={item.kind === 'whisper_bin'}>
                  {item.kind === 'whisper_bin' ? '🎙️ Whisper' : '🧠 LLM GGUF'}
                </span>
              </div>

              <p class="cc-desc">
                {currentLang === 'bg' ? item.description_bg : item.description_en}
              </p>

              <div class="cc-specs">
                <span class="spec-tag">💾 {item.size_display}</span>
                <span class="spec-tag">⚡ {item.vram_estimate}</span>
              </div>

              <div class="cc-footer">
                {#if item.is_installed}
                  <div class="installed-tag">✓ {currentLang === 'bg' ? 'Инсталиран' : 'Installed'}</div>
                  <button class="btn btn-danger btn-xs" onclick={() => handleDeleteModelFile(item.kind === 'whisper_bin' ? 'whisper' : 'llm', item.filename)}>
                    🗑️ {currentLang === 'bg' ? 'Изтрий' : 'Delete'}
                  </button>
                {:else}
                  <button
                    class="btn btn-primary btn-sm w-100"
                    onclick={() => handleDownloadCatalogItem(item)}
                    disabled={downloadProgress?.active}
                  >
                    ⬇️ {currentLang === 'bg' ? 'Инсталирай (1-клик)' : 'Install (1-Click)'}
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        </div>

        <div class="custom-import-card mt-4">
          <div class="cic-header">
            <h4>➕ {currentLang === 'bg' ? 'Добавяне на собствен модел (Custom Model Import)' : 'Add Custom Model'}</h4>
            <span class="text-muted">{currentLang === 'bg' ? 'Свалете директно от HuggingFace или добавете локален файл' : 'Download directly from HuggingFace or drop local files'}</span>
          </div>

          <div class="cic-form">
            <label class="control-row">
              <span class="ctrl-label">HuggingFace / Direct Download URL:</span>
              <input type="text" bind:value={customModelUrl} placeholder="https://huggingface.co/User/Repo/resolve/main/model.gguf" />
            </label>
            <label class="control-row">
              <span class="ctrl-label">{currentLang === 'bg' ? 'Име на файла в models/:' : 'Target Filename:'}</span>
              <input type="text" bind:value={customModelFilename} placeholder="my-custom-model.gguf" />
            </label>
            <div class="cic-actions">
              <button class="btn btn-primary btn-sm" onclick={handleCustomDownload} disabled={downloadProgress?.active}>
                ⬇️ {currentLang === 'bg' ? 'Свали модела в models/' : 'Download to models/'}
              </button>
            </div>
          </div>

          <div class="cic-hint">
            💡 {currentLang === 'bg'
              ? `Можете също така да поставите произволни .gguf или .bin файлове директно в директорията: ${modelsDir}`
              : `You can also drop .gguf or .bin files directly into: ${modelsDir}`}
          </div>
        </div>
      </section>
    {:else if activeSubtab === 'servers'}
      <section class="servers-section">
        <div class="servers-header">
          <div>
            <h3>{currentLang === 'bg' ? 'Външни локални AI Сървъри' : 'External Local AI Servers'}</h3>
            <p class="text-muted">{currentLang === 'bg' ? 'LeadFinder може автоматично да ползва вече работещи инстанции на Ollama, LM Studio или vLLM.' : 'Connect seamlessly to existing Ollama or LM Studio servers.'}</p>
          </div>
          <button class="btn btn-primary btn-sm" onclick={scanServers} disabled={isScanningServers}>
            🔄 {isScanningServers ? '...' : (currentLang === 'bg' ? 'Сканирай за сървъри' : 'Scan Servers')}
          </button>
        </div>

        <div class="servers-grid">
          {#each externalServers as srv}
            <div class="server-card" class:online={srv.is_online}>
              <div class="sc-header">
                <div>
                  <h4 class="sc-name">
                    {srv.server_type === 'ollama' ? '🦙 Ollama' : (srv.server_type === 'lmstudio' ? '🎨 LM Studio' : '🌐 Custom Server')}
                  </h4>
                  <span class="sc-url mono">{srv.endpoint_url}</span>
                </div>
                <span class="sc-status" class:online={srv.is_online}>
                  {srv.is_online ? `🟢 Онлайн (${srv.latency_ms}ms)` : '🔴 Офлайн'}
                </span>
              </div>

              {#if srv.is_online && srv.models.length > 0}
                <div class="sc-models-list">
                  <span class="sc-models-title">{currentLang === 'bg' ? 'Открити модели в сървъра:' : 'Available Models:'}</span>
                  <div class="sc-pills">
                    {#each srv.models as m}
                      <span class="model-badge">
                        {m.name}
                        {#if m.parameter_size}<small>({m.parameter_size})</small>{/if}
                      </span>
                    {/each}
                  </div>
                </div>
              {:else if srv.is_online}
                <p class="text-muted mt-2">{currentLang === 'bg' ? 'Сървърът е онлайн, но няма заредени модели.' : 'Server online, no loaded models.'}</p>
              {:else}
                <p class="text-muted mt-2">
                  {srv.server_type === 'ollama'
                    ? (currentLang === 'bg' ? 'Стартирайте "ollama serve" в терминала, за да го свържете автоматично.' : 'Run "ollama serve" to connect automatically.')
                    : (currentLang === 'bg' ? 'Стартирайте локалния сървър в LM Studio (Local Server -> Start).' : 'Start the Local Server tab in LM Studio.')}
                </p>
              {/if}
            </div>
          {/each}
        </div>

        <div class="custom-endpoint-box mt-4">
          <h4>{currentLang === 'bg' ? 'Персонализиран OpenAI-съвместим сървър (vLLM, LocalAI, vLLM)' : 'Custom OpenAI-Compatible Endpoint'}</h4>
          <div class="ceb-row">
            <input type="text" bind:value={customServerUrl} placeholder="http://localhost:8000/v1" />
            <button class="btn btn-secondary btn-sm" onclick={scanServers}>
              🔍 {currentLang === 'bg' ? 'Тествай връзката' : 'Probe URL'}
            </button>
          </div>
        </div>
      </section>
    {:else if activeSubtab === 'playground'}
      <section class="playground-section">
        <div class="pg-grid">
          <div class="pg-controls-card">
            <h3>🧪 {currentLang === 'bg' ? 'Тестване на AI Модел' : 'AI Model Playground'}</h3>

            <label class="control-row mt-2">
              <span class="ctrl-label">{currentLang === 'bg' ? 'Провайдър:' : 'Provider:'}</span>
              <select bind:value={testProvider}>
                <option value="local">Локален GGUF</option>
                <option value="gemini">Google Gemini</option>
                <option value="ollama">Ollama</option>
                <option value="lmstudio">LM Studio</option>
                <option value="heuristics">Heuristics</option>
              </select>
            </label>

            <label class="control-row">
              <span class="ctrl-label">{currentLang === 'bg' ? 'Модел:' : 'Model:'}</span>
              {#if testProvider === 'local'}
                <select bind:value={testModelName}>
                  {#each installedModels.filter(m => m.kind === 'llm_gguf') as lm}
                    <option value={lm.filename}>{lm.filename}</option>
                  {/each}
                </select>
              {:else if testProvider === 'gemini'}
                <select bind:value={testModelName}>
                  <option value="gemini-2.5-flash">gemini-2.5-flash</option>
                  <option value="gemini-1.5-pro">gemini-1.5-pro</option>
                </select>
              {:else}
                <input type="text" bind:value={testModelName} placeholder="qwen2.5:3b, llama3.1..." />
              {/if}
            </label>

            <label class="control-row">
              <span class="ctrl-label">System Prompt:</span>
              <textarea rows="2" bind:value={testSystemPrompt}></textarea>
            </label>

            <label class="control-row">
              <span class="ctrl-label">User Prompt:</span>
              <textarea rows="4" bind:value={testPrompt}></textarea>
            </label>

            <button class="btn btn-primary mt-2" onclick={handleRunTest} disabled={isTesting}>
              ▶️ {isTesting ? (currentLang === 'bg' ? 'Изпълнява се...' : 'Running...') : (currentLang === 'bg' ? 'Изпълни тест' : 'Run Test')}
            </button>
          </div>

          <div class="pg-results-card">
            <h3>📊 {currentLang === 'bg' ? 'Резултат от теста' : 'Test Output'}</h3>

            {#if testResult}
              <div class="test-metrics-bar">
                <span class="tm-pill">⏱️ {testResult.elapsed_ms} ms</span>
                <span class="tm-pill">⚡ {testResult.tokens_per_second.toFixed(1)} tok/s</span>
                <span class="tm-pill">📥 {testResult.tokens_in} in</span>
                <span class="tm-pill">📤 {testResult.tokens_out} out</span>
              </div>

              {#if testResult.error}
                <div class="test-error">
                  ❌ {testResult.error}
                </div>
              {:else}
                <div class="test-output-box">
                  <pre>{testResult.raw_text}</pre>
                </div>
              {/if}
            {:else}
              <p class="text-muted mt-4">{currentLang === 'bg' ? 'Натиснете "Изпълни тест", за да видите отговора и скоростта на избрания модел.' : 'Press "Run Test" to view output and generation speed.'}</p>
            {/if}
          </div>
        </div>
      </section>
    {/if}
  </main>
</div>

<style>
  .models-screen {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
    background: var(--bg-canvas);
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .ms-header {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 1rem;
  }

  .ms-title-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .ms-title-row h2 {
    margin: 0;
    font-size: 1.4rem;
    color: var(--text-heading);
  }

  .active-models-pill {
    padding: 0.25rem 0.75rem;
    border-radius: 999px;
    background: rgba(59, 130, 246, 0.15);
    border: 1px solid rgba(59, 130, 246, 0.35);
    color: #60a5fa;
    font-size: 0.8rem;
    font-weight: 600;
  }

  .ms-sub {
    margin: 0.35rem 0 0 0;
    font-size: 0.88rem;
    color: var(--text-muted);
  }

  .ms-subtabs {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .subtab-btn {
    padding: 0.5rem 1rem;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-color);
    color: var(--text-muted);
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s;
  }

  .subtab-btn:hover {
    color: var(--text-heading);
    background: rgba(255, 255, 255, 0.07);
  }

  .subtab-btn.active {
    background: var(--primary-color, #3b82f6);
    border-color: var(--primary-color, #3b82f6);
    color: #ffffff;
  }

  .toast-banner {
    padding: 0.75rem 1rem;
    border-radius: 8px;
    background: rgba(16, 185, 129, 0.15);
    border: 1px solid rgba(16, 185, 129, 0.35);
    color: #34d399;
    font-size: 0.88rem;
    font-weight: 500;
  }

  .toast-banner.is-err {
    background: rgba(239, 68, 68, 0.15);
    border-color: rgba(239, 68, 68, 0.35);
    color: #f87171;
  }

  .download-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1.5rem;
    padding: 1rem 1.25rem;
    background: rgba(59, 130, 246, 0.1);
    border: 1px solid rgba(59, 130, 246, 0.3);
    border-radius: 10px;
    flex-wrap: wrap;
  }

  .db-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .db-spinner {
    font-size: 1.5rem;
  }

  .db-title {
    color: #93c5fd;
    font-size: 0.95rem;
  }

  .db-meta {
    font-size: 0.8rem;
    color: var(--text-muted);
    display: block;
    margin-top: 0.15rem;
  }

  .db-right {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex: 1;
    min-width: 260px;
    justify-content: flex-end;
  }

  .db-progress-track {
    flex: 1;
    max-width: 320px;
    height: 8px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 999px;
    overflow: hidden;
  }

  .db-progress-fill {
    height: 100%;
    background: #3b82f6;
    transition: width 0.3s;
  }

  .roles-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 1.25rem;
  }

  .role-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .rc-header {
    display: flex;
    align-items: flex-start;
    gap: 0.85rem;
  }

  .rc-icon {
    font-size: 1.75rem;
  }

  .rc-title {
    margin: 0;
    font-size: 1.05rem;
    color: var(--text-heading);
  }

  .rc-desc {
    font-size: 0.8rem;
    color: var(--text-muted);
    margin-top: 0.2rem;
  }

  .control-row {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-bottom: 0.75rem;
  }

  .ctrl-label {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-muted);
  }

  .control-row select,
  .control-row input,
  .control-row textarea {
    padding: 0.55rem 0.75rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-heading);
    font-size: 0.88rem;
    width: 100%;
    box-sizing: border-box;
  }

  .consensus-box {
    margin-top: 1.5rem;
    padding: 1.25rem;
    border-radius: 12px;
    background: rgba(168, 85, 247, 0.06);
    border: 1px solid rgba(168, 85, 247, 0.3);
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .cb-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .cb-title-group {
    display: flex;
    align-items: flex-start;
    gap: 0.85rem;
  }

  .cb-icon {
    font-size: 1.75rem;
  }

  .cb-title {
    margin: 0;
    color: #c084fc;
    font-size: 1.1rem;
  }

  .cb-sub {
    margin: 0.25rem 0 0 0;
    font-size: 0.85rem;
    color: var(--text-muted);
    line-height: 1.4;
  }

  .switch-label {
    position: relative;
    display: inline-block;
    width: 48px;
    height: 26px;
    flex-shrink: 0;
  }

  .switch-label input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background-color: rgba(255, 255, 255, 0.15);
    border-radius: 34px;
    transition: 0.2s;
  }

  .switch-slider:before {
    position: absolute;
    content: "";
    height: 20px;
    width: 20px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    border-radius: 50%;
    transition: 0.2s;
  }

  input:checked + .switch-slider {
    background-color: #a855f7;
  }

  input:checked + .switch-slider:before {
    transform: translateX(22px);
  }

  .cb-body {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1rem;
    padding-top: 0.5rem;
    border-top: 1px dashed rgba(168, 85, 247, 0.2);
  }

  .roles-actions {
    margin-top: 1.25rem;
    display: flex;
    justify-content: flex-end;
  }

  .catalog-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1.25rem;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .catalog-header-row h3 {
    margin: 0 0 0.25rem 0;
    color: var(--text-heading);
  }

  .catalog-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1.25rem;
  }

  .catalog-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .catalog-card.installed {
    border-color: rgba(16, 185, 129, 0.4);
    background: rgba(16, 185, 129, 0.04);
  }

  .cc-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .cc-name {
    margin: 0;
    font-size: 0.95rem;
    color: var(--text-heading);
  }

  .cc-author {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .cc-kind-badge {
    padding: 0.2rem 0.5rem;
    border-radius: 999px;
    font-size: 0.72rem;
    font-weight: 600;
    background: rgba(59, 130, 246, 0.15);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.3);
  }

  .cc-kind-badge.whisper {
    background: rgba(16, 185, 129, 0.15);
    color: #34d399;
    border-color: rgba(16, 185, 129, 0.3);
  }

  .cc-desc {
    margin: 0;
    font-size: 0.82rem;
    color: var(--text-body);
    line-height: 1.4;
    flex: 1;
  }

  .cc-specs {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .spec-tag {
    font-size: 0.72rem;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-muted);
  }

  .cc-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding-top: 0.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .installed-tag {
    color: #34d399;
    font-size: 0.82rem;
    font-weight: 600;
  }

  .custom-import-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px dashed var(--border-color);
    border-radius: 12px;
    padding: 1.25rem;
  }

  .cic-header h4 {
    margin: 0 0 0.25rem 0;
    color: var(--text-heading);
  }

  .cic-form {
    display: grid;
    grid-template-columns: 2fr 1fr auto;
    gap: 1rem;
    align-items: flex-end;
    margin-top: 1rem;
  }

  .cic-hint {
    margin-top: 0.75rem;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .servers-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1.25rem;
  }

  .servers-header h3 {
    margin: 0 0 0.25rem 0;
    color: var(--text-heading);
  }

  .servers-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 1.25rem;
  }

  .server-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .server-card.online {
    border-color: rgba(16, 185, 129, 0.35);
  }

  .sc-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .sc-name {
    margin: 0;
    font-size: 1.1rem;
    color: var(--text-heading);
  }

  .sc-url {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .sc-status {
    font-size: 0.75rem;
    font-weight: 600;
    color: #ef4444;
  }

  .sc-status.online {
    color: #10b981;
  }

  .sc-models-list {
    margin-top: 0.5rem;
  }

  .sc-models-title {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-weight: 600;
  }

  .sc-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: 0.35rem;
  }

  .model-badge {
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-heading);
  }

  .custom-endpoint-box {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.25rem;
  }

  .custom-endpoint-box h4 {
    margin: 0 0 0.75rem 0;
    color: var(--text-heading);
  }

  .ceb-row {
    display: flex;
    gap: 0.75rem;
  }

  .ceb-row input {
    flex: 1;
    padding: 0.55rem 0.75rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-heading);
  }

  .pg-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.5rem;
  }

  .pg-controls-card,
  .pg-results-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.25rem;
  }

  .pg-controls-card h3,
  .pg-results-card h3 {
    margin: 0 0 0.75rem 0;
    color: var(--text-heading);
  }

  .test-metrics-bar {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-bottom: 1rem;
  }

  .tm-pill {
    padding: 0.25rem 0.65rem;
    border-radius: 999px;
    background: rgba(59, 130, 246, 0.15);
    border: 1px solid rgba(59, 130, 246, 0.3);
    color: #93c5fd;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .test-output-box {
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    max-height: 480px;
    overflow-y: auto;
  }

  .test-output-box pre {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-heading);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .test-error {
    padding: 1rem;
    border-radius: 8px;
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #f87171;
    font-size: 0.88rem;
  }

  .btn {
    padding: 0.55rem 1rem;
    border-radius: 6px;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: filter 0.15s;
  }

  .btn:hover {
    filter: brightness(1.15);
  }

  .btn-primary {
    background: var(--primary-color, #3b82f6);
    color: #fff;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.06);
    border-color: var(--border-color);
    color: var(--text-heading);
  }

  .btn-danger {
    background: rgba(239, 68, 68, 0.15);
    border-color: rgba(239, 68, 68, 0.35);
    color: #f87171;
  }

  .btn-sm {
    padding: 0.35rem 0.75rem;
    font-size: 0.8rem;
  }

  .btn-xs {
    padding: 0.2rem 0.5rem;
    font-size: 0.72rem;
  }

  .w-100 {
    width: 100%;
  }

  .mt-2 { margin-top: 0.5rem; }
  .mt-4 { margin-top: 1rem; }
  .mono { font-family: monospace; }
</style>