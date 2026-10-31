export function escapeHtml(raw: string): string {
    return raw
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}

const BG_MONTHS =
    'януари|февруари|март|април|май|юни|юли|август|септември|октомври|ноември|декември|' +
    'ян\\.|февр\\.|апр\\.|авг\\.|септ\\.|окт\\.|ноем\\.|дек\\.';

const DATE_PATTERN = new RegExp(
    `(\\b\\d{1,2}\\s+(?:${BG_MONTHS})(?:\\s+\\d{4})?(?:\\s*г\\.)?|` +
        `\\b(?:${BG_MONTHS})\\s+\\d{4}(?:\\s*г\\.)?|` +
        `\\b\\d{1,2}[./]\\d{1,2}[./](?:19|20)\\d{2}(?:\\s*г\\.)?|` +
        `\\b(?:19|20)\\d{2}-\\d{2}-\\d{2}\\b|` +
        `\\b(?:19|20)\\d{2}\\s*[–—-]\\s*(?:19|20)\\d{2}(?:\\s*г\\.)?|` +
        `\\b(?:19|20)\\d{2}\\s*г\\.|` +
        `(?<=\\b(?:през|от|до|след|преди|към|за)\\s)(?:19|20)\\d{2}\\b)`,
    'gi'
);

const ID_PATTERN =
    /\b(?:ЕИК\s*:?\s*\d{9,13}|БУЛСТАТ\s*:?\s*\d{9,13}|(?:адм\.\s*|гр\.\s*|нак\.\s*|търг\.\s*)?дело\s*№?\s*[\d/A-Za-zА-Яа-я-]+|Решение\s*№\s*[\w/-]+|Договор\s*№\s*[\w/-]+|Акт\s*(?:14|15|16)\b)/gi;

const MONEY_PATTERN =
    /\b\d[\d\s.,]*(?:млн\.|млрд\.|хил\.)?\s*(?:лв\.?|лева|евро|EUR|BGN|USD|\$|€|%)(?!\w)/gi;

const ENTITY_PATTERN =
    /(?:[„"«][^„"“»\n]{2,55}[“"»]\s*(?:ЕООД|ООД|ЕАД|АД|ДЗЗД)|\b[А-ЯA-Z][\wА-Яа-я-]{1,28}(?:\s+[А-ЯA-Zа-яa-z][\wА-Яа-я-]{1,28}){0,3}\s+(?:ЕООД|ООД|ЕАД|АД|ДЗЗД)\b|\b(?:КПКОНПИ|КПК|Сметна(?:та)?\s+палата|Търговски(?:я|ят)?\s+регистър|Агенция\s+по\s+вписванията|БНБ|ВАС|ВКС|СЕС|КЗК|КФН|ДАНС|ГДБОП|ДНСК|МРРБ|АПИ|НКЖИ|АОП|ЦАИС\s*ЕОП|СЕМ|ЦИК|НАП|НОИ|Прокуратура(?:та)?|Европейска(?:та)?\s+прокуратура|EPPO|OFAC|Магнитски|OpenSanctions|Интерпол|ООН)\b)/g;

function applyOutsideTags(html: string, regex: RegExp, className: string): string {
    const parts = html.split(/(<[^>]+>)/g);
    for (let i = 0; i < parts.length; i += 2) {
        if (!parts[i]) continue;
        parts[i] = parts[i].replace(regex, (m) => `<mark class="${className}">${m}</mark>`);
    }
    return parts.join('');
}

export function highlightInlineMarkdown(raw: string, enableHighlights = true): string {
    let s = escapeHtml(raw);
    s = s.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
    s = s.replace(
        /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
        '<a href="$2" target="_blank" rel="noopener noreferrer" class="report-ext-link">$1</a>'
    );
    s = s.replace(
        /(?<!href="|">)(https?:\/\/[^\s<)]+)/g,
        '<a href="$1" target="_blank" rel="noopener noreferrer" class="report-ext-link">$1</a>'
    );
    if (!enableHighlights) {
        return s;
    }
    s = applyOutsideTags(s, DATE_PATTERN, 'hl-date');
    s = applyOutsideTags(s, ID_PATTERN, 'hl-id');
    s = applyOutsideTags(s, MONEY_PATTERN, 'hl-money');
    s = applyOutsideTags(s, ENTITY_PATTERN, 'hl-entity');
    return s;
}

export function highlightPlainTextToHtml(raw: string, enableHighlights = true): string {
    if (!raw) return '';
    return highlightInlineMarkdown(raw, enableHighlights).replace(/\n/g, '<br/>');
}
