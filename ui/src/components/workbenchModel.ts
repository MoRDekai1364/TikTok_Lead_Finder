import type { Case, Claim } from './workbenchData';

export type Hit = { id: number; platform: string; url: string; author: string; date: string; kind: 'exact' | 'near' | 'paraphrase'; sim: number; snippet: string; include: boolean; claim: number };
export type Repost = { id: number; url: string; author: string; date: string; method: string; conf: number; include: boolean };
export type Query = { platform: string; q: string; on: boolean };
export type Source = { id: number; url: string; title: string; role: string; claims: number[]; why: string; quote: string; score: number; include: boolean; archive: string };
export type EntityCheck = { src: string; result: string };
export type Entity = { id: number; name: string; type: string; summary: string; found: boolean; checks: EntityCheck[]; sanctions: string };
export type Block = { id: number; key: string; label: string; kind: 'text' | 'long' | 'select' | 'list'; value: string; list: string[]; options: string[] };
export type ReportSet = { TCS: Block[]; TT: Block[]; FC: Block[] };

export const roles = ['Първоизточник', 'Същото твърдение (разпространение)', 'Официален документ', 'Предишна проверка', 'Контекст', 'Архив'];
export const platformsAll = ['TikTok', 'Facebook', 'Instagram', 'Telegram', 'X', 'Reddit', 'Bluesky', 'Snapchat', 'YouTube'];
export const ratings = ['Misleading', 'False', 'Unsubstantiated', 'Out of Scope', 'Not Misinformation'];
export const topics = ['Climate', 'Health', 'Civic', 'Conspiracy Theory', 'Politics', 'Science', 'Economy'];

export type Taxonomy = {
  ratings: string[];
  topics: string[];
  claimKinds: string[];
  claimTypes: string[];
  harms: string[];
};

export const defaultTaxonomy: Taxonomy = {
  ratings: ['Misleading', 'False', 'Unsubstantiated', 'Out of Scope', 'Not Misinformation', 'Satire', 'Scam'],
  topics: ['Climate', 'Health', 'Civic', 'Conspiracy Theory', 'Politics', 'Science', 'Economy', 'Elections'],
  claimKinds: ['New Claim', 'Known Claim', 'Trend'],
  claimTypes: ['твърдение', 'число', 'цитат', 'прогноза', 'причинно-следствена връзка', 'заплаха'],
  harms: ['Low', 'Medium', 'High']
};

const rng = (seed: number | string) => () => {
  let s = typeof seed === 'number' ? seed : Number(String(seed).replace(/\D/g, '').slice(-6)) || 42;
  s |= 0;
  s = (s + 0x6d2b79f5) | 0;
  let t = Math.imul(s ^ (s >>> 15), 1 | s);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};

const handles = ['milena_k', 'bg_patriot77', 'novini24', 'the_real_stan', 'dnes_bg', 'vesti_live', 'petar.d', 'istinata', 'info_front', 'zdravko_n', 'kanal_bg', 'sofia_voice'];

const urlFor = (p: string, a: string, n: number): string => {
  const id = 1000000000 + n * 7919;
  if (p === 'TikTok') return `https://www.tiktok.com/@${a}/video/${7300000000000000000 + n * 104729}`;
  if (p === 'Facebook') return `https://www.facebook.com/${a}/posts/${id}`;
  if (p === 'X') return `https://x.com/${a}/status/${1700000000000000000 + n * 104729}`;
  if (p === 'Telegram') return `https://t.me/${a}/${1000 + n * 13}`;
  if (p === 'Reddit') return `https://www.reddit.com/r/bulgaria/comments/${(id).toString(36)}`;
  if (p === 'Bluesky') return `https://bsky.app/profile/${a}.bsky.social/post/${id.toString(36)}`;
  if (p === 'YouTube') return `https://www.youtube.com/watch?v=${id.toString(36)}`;
  if (p === 'Instagram') return `https://www.instagram.com/reel/${id.toString(36)}`;
  return `https://www.snapchat.com/spotlight/${id.toString(36)}`;
};

const isoDate = (r: () => number): string => {
  const start = Date.UTC(2025, 8, 1);
  const end = Date.UTC(2026, 10, 10);
  const d = new Date(start + r() * (end - start));
  return d.toISOString().slice(0, 10);
};

export const makeHit = (c: Case, platform: string, seed: number | string, id: number, claimIdx?: number): Hit => {
  const seedNum = typeof seed === 'number' ? seed : Number(String(seed).replace(/\D/g, '').slice(-6)) || 42;
  const r = rng(seedNum * 31 + id);
  const a = handles[Math.floor(r() * handles.length)];
  const k = r();
  const kind: Hit['kind'] = k < 0.6 ? 'exact' : k < 0.85 ? 'near' : 'paraphrase';
  const sim = kind === 'exact' ? 0.97 + r() * 0.03 : kind === 'near' ? 0.85 + r() * 0.11 : 0.7 + r() * 0.14;
  const ci = claimIdx ?? (c.claims.length > 0 ? Math.floor(r() * c.claims.length) : 0);
  const text = c.claims[ci]?.text ?? c.title;
  const snippet = kind === 'exact' ? text : kind === 'near' ? text.replace(/\.$/, '') + ' …' : text.split(' ').slice(0, 9).join(' ') + ' …';
  return { id, platform, url: urlFor(platform, a, id), author: platform === 'TikTok' || platform === 'X' || platform === 'Telegram' ? `@${a}` : a, date: isoDate(r), kind, sim: Math.round(sim * 100) / 100, snippet, include: true, claim: ci };
};

export const buildHits = (c: Case): Hit[] => {
  const out: Hit[] = [];
  let id = 1;
  for (const [p, n] of Object.entries(c.platforms)) {
    for (let i = 0; i < n; i++) out.push(makeHit(c, p, c.id, id++));
  }
  return out.sort((a, b) => a.date.localeCompare(b.date));
};

export const buildReposts = (c: Case): Repost[] => {
  const idNum = typeof c.id === 'number' ? c.id : Number(String(c.id).replace(/\D/g, '').slice(-6)) || 42;
  const r = rng(idNum * 97);
  const methods = ['Същ звук', 'Същ надпис', 'Визуален отпечатък', 'Stitch / Duet'];
  const n = 3 + Math.floor(r() * 3);
  return Array.from({ length: n }, (_, i) => {
    const a = handles[Math.floor(r() * handles.length)];
    return { id: i + 1, url: urlFor('TikTok', a, 500 + i), author: `@${a}`, date: isoDate(r), method: methods[Math.floor(r() * methods.length)], conf: Math.round((0.6 + r() * 0.39) * 100) / 100, include: true };
  });
};

const prefix: Record<string, string> = { TikTok: 'site:tiktok.com', Facebook: 'site:facebook.com', X: 'site:x.com', Telegram: 'site:t.me', Instagram: 'site:instagram.com', Reddit: 'site:reddit.com', Bluesky: 'site:bsky.app', Snapchat: 'site:snapchat.com', YouTube: 'site:youtube.com' };

export const queryFor = (c: Case, p: string): string => {
  const first = c.claims[0]?.text || c.title || 'твърдение';
  const words = Array.from(new Set(first.replace(/[.,„“"]/g, '').split(' ').filter((w) => w.length >= 5))).slice(0, 4);
  const phrase = first.split(' ').slice(0, 8).join(' ').replace(/[.,]/g, '');
  return `${prefix[p]} ("${phrase}" OR (${words.join(' AND ')}))`;
};

export const buildQueries = (c: Case): Query[] => platformsAll.map((p) => ({ platform: p, q: queryFor(c, p), on: p in c.platforms }));

const roleFor = (text: string): string => {
  if (/state\.gov|nasa|wmo|whitehouse|unhcr|ema\.|nih\.gov|nhs|bda\.bg|foreignassistance/.test(text)) return roles[2];
  if (/factcheck|politifact|afp/.test(text)) return roles[3];
  if (/news-front|rebornslim|страница на продукта|оригиналната/.test(text)) return roles[0];
  return roles[4];
};

export const makeSource = (c: Case, text: string, why: string, id: number): Source => {
  const sep = text.indexOf(':');
  const dom = sep > 0 ? text.slice(0, sep).trim() : text;
  const title = sep > 0 ? text.slice(sep + 1).trim() : text;
  const linked: number[] = [];
  c.claims.forEach((cl: Claim, i) => {
    if (cl.evidence.some((e) => dom.includes(e.split('.')[0]) || e.includes(dom.split('.')[0]))) linked.push(i);
  });
  const quote = linked.length ? c.claims[linked[0]].checks[0].c : 'Релевантен контекст';
  const r = rng(typeof c.id === 'number' ? id * 13 + c.id : `${id}_${c.id}`);
  return { id, url: dom.includes('.') ? `https://${dom}/` : dom, title, role: roleFor(text), claims: linked, why, quote, score: Math.round((0.72 + r() * 0.27) * 100) / 100, include: true, archive: 'Не е архивиран' };
};

export const entityMap: Record<string | number, Entity[]> = {
  1: [
    { id: 1, name: 'Егон Чолакян', type: 'лице', summary: 'Българско лице, представящо се като автор на климатичен модел. Няма рецензирани публикации за модела.', found: true, checks: [{ src: 'Wikidata', result: 'Няма запис' }, { src: 'Google Scholar', result: 'Няма публикации по климатично моделиране' }, { src: 'Новини', result: '3 интервюта без научно становище' }], sanctions: 'Няма съвпадение (0 от 6 списъка)' },
    { id: 2, name: 'Световна метеорологична организация', type: 'организация', summary: 'Специализирана агенция на ООН. Публикува годишните доклади за глобалната температура.', found: true, checks: [{ src: 'Wikidata', result: 'Q5555' }, { src: 'wmo.int', result: 'Съобщение за 2024 г.' }], sanctions: 'Не е приложимо' }
  ],
  2: [
    { id: 1, name: 'Сара Перин', type: 'лице', summary: 'Няма информация за лице с това име, свързано с разпределяне на средства.', found: false, checks: [{ src: 'Wikidata', result: 'Няма запис' }, { src: 'Търговски регистър', result: 'Няма съвпадение' }, { src: 'Новини', result: '0 резултата' }, { src: 'OpenSanctions', result: 'Няма съвпадение' }], sanctions: 'Няма съвпадение (0 от 6 списъка)' },
    { id: 2, name: 'Доналд Тръмп', type: 'лице', summary: 'Президент на САЩ. Подписал указ за преглед на федералното финансиране за НПО.', found: true, checks: [{ src: 'whitehouse.gov', result: 'Указ от януари 2025 г.' }, { src: 'msn.com', result: 'Отразяване в медиите' }], sanctions: 'Не е приложимо' }
  ],
  3: [
    { id: 1, name: 'Reborn Slim', type: 'продукт', summary: 'Хранителна добавка, продавана през собствен сайт. Няма клинични изпитвания.', found: true, checks: [{ src: 'rebornslim.com', result: 'Търговска страница' }, { src: 'EMA / БАБХ', result: 'Не фигурира като лекарство' }, { src: 'Whois', result: 'Домейн регистриран преди 7 месеца' }], sanctions: 'Не е приложимо' }
  ],
  4: [
    { id: 1, name: 'Държавен департамент на САЩ', type: 'организация', summary: 'Външнополитическото ведомство на САЩ. Официално определя Путин като президент на Русия.', found: true, checks: [{ src: 'state.gov', result: 'Брифинги от 4 януари и 1 февруари 2024 г.' }, { src: 'YouTube', result: 'Официален канал с оригиналното видео' }], sanctions: 'Не е приложимо' },
    { id: 2, name: 'Владимир Путин', type: 'лице', summary: 'Президент на Руската федерация.', found: true, checks: [{ src: 'Wikidata', result: 'Q7747' }, { src: 'OpenSanctions', result: 'Съвпадение: ЕС, Великобритания, САЩ' }], sanctions: 'Съвпадение в 3 списъка (достоверност 100%)' }
  ],
  5: [
    { id: 1, name: 'bgr.news-front.info', type: 'медия', summary: 'Сайт, разпространяващ материали от руската платформа News Front.', found: true, checks: [{ src: 'Whois', result: 'Регистрация извън ЕС' }, { src: 'EUvsDisinfo', result: 'Споменат в докладите' }], sanctions: 'Не е приложимо' },
    { id: 2, name: 'UNHCR', type: 'организация', summary: 'Агенция на ООН за бежанците; публикува данни за украинските бежанци в ЕС.', found: true, checks: [{ src: 'unhcr.org', result: 'Актуални данни' }], sanctions: 'Не е приложимо' }
  ],
  6: [
    { id: 1, name: 'Laxotabs', type: 'продукт', summary: 'Продукт, рекламиран срещу запек. Не фигурира в регистъра на лекарствата.', found: true, checks: [{ src: 'БДА регистър', result: 'Няма запис' }, { src: 'Онлайн магазин', result: 'Единствен продавач' }], sanctions: 'Не е приложимо' }
  ]
};

const b = (id: number, key: string, label: string, kind: Block['kind'], value = '', options: string[] = [], list: string[] = []): Block => ({ id, key, label, kind, value, list, options });

export const buildReport = (c: Case, hits: Hit[], sources: Source[], queries: Query[], tax?: Taxonomy): ReportSet => {
  const t = tax ?? defaultTaxonomy;
  const examples = hits.filter((h) => h.include).slice(0, 8).map((h) => h.url);
  const srcUrls = sources.filter((s) => s.include).map((s) => s.url);
  const qs = queries.filter((q) => q.on).map((q) => q.q).join('\n');
  const total = hits.filter((h) => h.include).length;
  const claimText = c.claims.map((cl) => cl.text).join(' ');
  const kw = c.ocr.map((o) => '#' + o.text.replace(/\s+/g, '')).join(' ');
  return {
    TCS: [
      b(1, 'date', 'Дата', 'text', c.date),
      b(2, 'task', 'Връзка към задачата', 'text', c.taskUrl),
      b(3, 'rating', 'Оценка', 'select', c.rating, t.ratings),
      b(4, 'topic', 'Тема', 'select', c.topic, t.topics),
      b(5, 'kind', 'Вид', 'select', c.kind, t.claimKinds),
      b(6, 'claim', 'CLAIM', 'long', claimText),
      b(7, 'notesEn', 'NOTES (EN)', 'long', c.notesEn),
      b(8, 'notesBg', 'NOTES (BG)', 'long', c.notesBg),
      b(9, 'sources', 'Източници', 'list', '', [], srcUrls)
    ],
    TT: [
      b(1, 'headEn', '1. Headline (EN)', 'long', c.title),
      b(2, 'headBg', 'Headline (BG)', 'long', c.title),
      b(3, 'region', '2. Region/Country', 'text', 'Bulgaria'),
      b(4, 'harm', '3. Harm (Low/Medium/High)', 'select', c.harm, t.harms),
      b(5, 'examples', '4. Examples', 'list', '', [], examples),
      b(6, 'falsif', '5. Link to Falsification Source Debunking Claim', 'list', '', [], srcUrls),
      b(7, 'qty', '6.Quantity on Platform (High or Low)', 'select', total >= 10 ? 'High' : 'Low', ['High', 'Low']),
      b(8, 'keywords', '7.Keywords/Hashtags', 'text', kw),
      b(9, 'false', '8. Verifiably False?', 'select', c.claims.some((cl) => cl.v === 'CONTRADICTED') ? 'Yes' : 'No', ['Yes', 'No']),
      b(10, 'bool', '9. Boolean Queries used to find the trend', 'long', qs),
      b(11, 'explanation', 'Please include short explanation', 'long', c.notesEn || c.notesBg)
    ],
    FC: [
      b(1, 'url', 'URL', 'text', hits[0]?.url ?? ''),
      b(2, 'caption', 'Caption', 'text', c.topic === 'Conspiracy Theory' ? 'Conspiracy' : c.topic),
      b(3, 'headline', 'Headline', 'long', `There is NO proof that: ${c.claims[0]?.text ?? c.title}`),
      b(4, 'lead', 'Debunk Lead', 'long', `Is there evidence that ${(c.claims[0]?.text ?? c.title).replace(/\.$/, '')}? No, that's not true: ${c.notesEn.split('. ')[0] || ''}.`),
      b(5, 'appeared', 'Къде се е появило', 'long', `The claim appeared in a TikTok video (archived here) published on ${c.date}:`),
      b(6, 'newclaim', '1. New Claim or Trend', 'text', c.claims[0]?.text ?? c.title),
      b(7, 'region', '2. Region/Country', 'text', 'Bulgaria'),
      b(8, 'harm', '3. Harm (Low/Medium/High)', 'select', c.harm, t.harms),
      b(9, 'examples', '4. Examples', 'list', '', [], examples),
      b(10, 'falsif', '5. Link to Falsification Source Debunking Claim', 'list', '', [], srcUrls),
      b(11, 'qty', '6.Quantity on Platform (High or Low)', 'select', total >= 10 ? 'High' : 'Low', ['High', 'Low']),
      b(12, 'keywords', '7.Keywords/Hashtags', 'text', kw),
      b(13, 'false', '8. Verifiably False?', 'select', c.claims.some((cl) => cl.v === 'CONTRADICTED') ? 'Yes' : 'No', ['Yes', 'No']),
      b(14, 'bool', '9. Boolean Queries used to find the trend', 'long', qs),
      b(15, 'explanation', 'Please include short explanation', 'long', c.notesEn || c.notesBg)
    ]
  };
};

export function formatDocText(tpl: 'TCS' | 'TT' | 'FC', blocks: Block[]): string {
  const map: Record<string, Block> = {};
  const standardKeys = new Set(['date', 'task', 'rating', 'topic', 'kind', 'claim', 'notesEn', 'notesBg', 'sources', 'headEn', 'headBg', 'region', 'harm', 'examples', 'falsif', 'qty', 'keywords', 'false', 'bool', 'explanation', 'url', 'caption', 'headline', 'lead', 'appeared', 'newclaim']);
  const custom = blocks.filter((b) => !standardKeys.has(b.key));
  blocks.forEach((b) => (map[b.key] = b));

  if (tpl === 'TCS') {
    const d = map['date']?.value?.trim() || '';
    const task = map['task']?.value?.trim() || '';
    const r = map['rating']?.value?.trim() || 'Misleading';
    const top = map['topic']?.value?.trim() || 'Health';
    const k = map['kind']?.value?.trim() || 'New Claim';
    const cl = map['claim']?.value?.trim() || '';
    const en = map['notesEn']?.value?.trim() || '';
    const bg = map['notesBg']?.value?.trim() || '';
    const src = (map['sources']?.list || []).filter(Boolean);

    let doc = `${d}\n${task}\n${r} – ${top} – ${k}\nCLAIM\n${cl}\nNOTES\n${en}`;
    if (bg) doc += `\n\n${bg}`;
    if (src.length) doc += `\n\n${src.join('\n')}`;
    if (custom.length) {
      doc += '\n\n' + custom.map((c) => `${c.label.toUpperCase()}\n${c.kind === 'list' ? c.list.join('\n') : c.value}`).join('\n\n');
    }
    return doc;
  }

  if (tpl === 'TT') {
    const he = map['headEn']?.value?.trim() || '';
    const hb = map['headBg']?.value?.trim() || '';
    const reg = map['region']?.value?.trim() || 'Bulgaria';
    const hm = map['harm']?.value?.trim() || 'Low';
    const ex = (map['examples']?.list || []).filter(Boolean);
    const fl = (map['falsif']?.list || []).filter(Boolean);
    const qt = map['qty']?.value?.trim() || 'Low';
    const kw = map['keywords']?.value?.trim() || '';
    const vf = map['false']?.value?.trim() || 'No';
    const bq = map['bool']?.value?.trim() || '';
    const exp = map['explanation']?.value?.trim() || '';

    let doc = `1. Headline: ${he}`;
    if (hb && hb !== he) doc += `\n${hb}`;
    doc += `\n2. Region/Country: ${reg}\n3. Harm (Low/Medium/High): ${hm}\n4. Examples:\n${ex.join('\n')}\n\n5. Link to Falsification Source Debunking Claim:\n${fl.join('\n')}\n\n6.Quantity on Platform (High or Low): ${qt}\n7.Keywords/Hashtags: ${kw}\n8. Verifiably False? ${vf}\n9. Boolean Queries used to find the trend: ${bq}\nPlease include short explanation:\n${exp}`;
    if (custom.length) {
      doc += '\n\n' + custom.map((c) => `${c.label}\n${c.kind === 'list' ? c.list.join('\n') : c.value}`).join('\n\n');
    }
    return doc;
  }

  const url = map['url']?.value?.trim() || '';
  const cap = map['caption']?.value?.trim() || '';
  const hl = map['headline']?.value?.trim() || '';
  const ld = map['lead']?.value?.trim() || '';
  const app = map['appeared']?.value?.trim() || '';
  const nc = map['newclaim']?.value?.trim() || '';
  const reg = map['region']?.value?.trim() || 'Bulgaria';
  const hm = map['harm']?.value?.trim() || 'Low';
  const ex = (map['examples']?.list || []).filter(Boolean);
  const fl = (map['falsif']?.list || []).filter(Boolean);
  const qt = map['qty']?.value?.trim() || 'Low';
  const kw = map['keywords']?.value?.trim() || '';
  const vf = map['false']?.value?.trim() || 'No';
  const bq = map['bool']?.value?.trim() || '';
  const exp = map['explanation']?.value?.trim() || '';

  let doc = `Headline: \n${hl}\nCaption: ${cap}\n${ld}\n\n${app}\n\n1. Headline: ${hl}\n2. Region/Country: ${reg}\n3. Harm (Low/Medium/High): ${hm}\n4. Examples:\n${ex.join('\n')}\n\n5. Link to Falsification Source Debunking Claim:\n${fl.join('\n')}\n\n6.Quantity on Platform (High or Low): ${qt}\n7.Keywords/Hashtags: ${kw}\n8. Verifiably False? ${vf}\n9. Boolean Queries used to find the trend: ${bq}\nPlease include short explanation:\n${exp}`;
  if (custom.length) {
    doc += '\n\n' + custom.map((c) => `${c.label}\n${c.kind === 'list' ? c.list.join('\n') : c.value}`).join('\n\n');
  }
  return doc;
}
