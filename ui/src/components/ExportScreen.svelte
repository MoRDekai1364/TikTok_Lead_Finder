<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchVideos } from '../lib/api';
  import type { Language } from '../lib/i18n';
  import { translations } from '../lib/i18n';
  import type { LeadCard } from '../lib/types';

  let { currentLang }: { currentLang: Language } = $props();
  const t = $derived(translations[currentLang]);

  let keptLeads = $state<LeadCard[]>([]);
  let exportFormat = $state<'markdown' | 'csv' | 'json'>('markdown');
  let exportContent = $state('');

  onMount(async () => {
    try {
      const all = await fetchVideos('leads', 500);
      keptLeads = all.filter((c) => c.lane === 'leads' || c.lane === 'kept');
      generateExport();
    } catch {}
  });

  $effect(() => {
    if (keptLeads.length >= 0 && exportFormat) {
      generateExport();
    }
  });

  function generateExport() {
    if (exportFormat === 'json') {
      exportContent = JSON.stringify(keptLeads, null, 2);
    } else if (exportFormat === 'csv') {
      const headers = ['id', 'creator', 'views', 'verdict', 'top_claim', 'lead_score', 'reason'];
      const rows = keptLeads.map((c) => [
        `"${c.id}"`,
        `"${c.creator_handle || ''}"`,
        c.views,
        `"${c.top_claim_verdict || ''}"`,
        `"${(c.top_claim_text || '').replace(/"/g, '""')}"`,
        c.lead_score.toFixed(3),
        `"${(c.reason_flagged || '').replace(/"/g, '""')}"`,
      ]);
      exportContent = [headers.join(','), ...rows.map((r) => r.join(','))].join('\n');
    } else {
      const lines = [
        `# TikTok Lead Finder - Kept Leads Export`,
        `Date: ${new Date().toISOString()}`,
        `Total Leads: ${keptLeads.length}\n`,
      ];
      keptLeads.forEach((c, idx) => {
        lines.push(`## ${idx + 1}. ${c.top_claim_text || 'Video ' + c.id}`);
        lines.push(`- **Creator**: @${c.creator_handle || 'unknown'} (${c.views.toLocaleString()} views)`);
        lines.push(`- **Verdict**: ${c.top_claim_verdict || 'NEW'}`);
        lines.push(`- **Lead Score**: ${c.lead_score.toFixed(3)}`);
        lines.push(`- **Flagged Reason**: ${c.reason_flagged}`);
        lines.push(`- **URL**: https://www.tiktok.com/@${c.creator_handle || 'user'}/video/${c.id}\n`);
      });
      exportContent = lines.join('\n');
    }
  }

  function handleDownload() {
    const ext = exportFormat === 'markdown' ? 'md' : exportFormat;
    const blob = new Blob([exportContent], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `leadfinder_export_${new Date().toISOString().substring(0, 10)}.${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

<div class="screen-container">
  <div class="screen-header">
    <h2>{t.exportHeading}</h2>
  </div>

  <div class="export-controls">
    <div class="format-buttons">
      <button class="btn" class:btn-primary={exportFormat === 'markdown'} onclick={() => (exportFormat = 'markdown')}>
        Markdown (.md)
      </button>
      <button class="btn" class:btn-primary={exportFormat === 'csv'} onclick={() => (exportFormat = 'csv')}>
        CSV (.csv)
      </button>
      <button class="btn" class:btn-primary={exportFormat === 'json'} onclick={() => (exportFormat = 'json')}>
        JSON (.json)
      </button>
    </div>

    <button class="btn btn-primary" onclick={handleDownload} disabled={keptLeads.length === 0}>
      ⬇ {currentLang === 'bg' ? 'Изтегли файла' : 'Download file'}
    </button>
  </div>

  <div class="preview-card">
    <div class="preview-header">
      <span>{currentLang === 'bg' ? 'Преглед на съдържанието' : 'Export preview'} ({keptLeads.length} {currentLang === 'bg' ? 'записа' : 'items'})</span>
    </div>
    <pre class="export-preview">{exportContent}</pre>
  </div>
</div>

<style>
  .screen-container {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem;
    background: var(--bg-canvas);
  }

  .screen-header {
    margin-bottom: 1.25rem;
  }

  .screen-header h2 {
    margin: 0;
    font-size: 1.2rem;
    color: var(--text-heading);
  }

  .export-controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
  }

  .format-buttons {
    display: flex;
    gap: 0.5rem;
  }

  .preview-card {
    background: var(--surface-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    overflow: hidden;
  }

  .preview-header {
    padding: 0.65rem 1rem;
    background: var(--card-bg);
    border-bottom: 1px solid var(--border-color);
    font-size: 0.82rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .export-preview {
    margin: 0;
    padding: 1rem;
    font-size: 0.78rem;
    line-height: 1.45;
    background: var(--input-bg);
    color: var(--text-body);
    max-height: 480px;
    overflow-y: auto;
    font-family: monospace;
  }
</style>
