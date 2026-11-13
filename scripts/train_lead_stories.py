import sys
import os
import re
import glob
import json
import hashlib
import zipfile
import xml.etree.ElementTree as ET

sys.stdout.reconfigure(encoding='utf-8')

TRAINING_DIR = os.path.normpath(os.path.join(os.path.dirname(__file__), '..', 'docs', 'training_data', 'LEAD STORIES'))
DATA_DIR = os.path.normpath(os.path.join(os.path.dirname(__file__), '..', 'data'))
TRAINING_OUT_DIR = os.path.join(DATA_DIR, 'training')
FIXTURES_DIR = os.path.normpath(os.path.join(os.path.dirname(__file__), '..', 'fixtures'))

os.makedirs(TRAINING_OUT_DIR, exist_ok=True)

VERDICT_MAP_ORDERED = [
    ('not misinformation',   'SUPPORTED'),
    ('factually accurate',   'SUPPORTED'),
    ('true',                 'SUPPORTED'),
    ('misleading',           'CONTRADICTED'),
    ('misinformation',       'CONTRADICTED'),
    ('false',                'CONTRADICTED'),
    ('unconfirmed',          'UNVERIFIABLE'),
    ('unsubstantiated',      'UNVERIFIABLE'),
    ('inconclusive',         'UNVERIFIABLE'),
    ('disputed',             'UNVERIFIABLE'),
    ('out of scope',         'NOT_CHECKABLE'),
    ('no fact-checkable',    'NOT_CHECKABLE'),
    ('opinion',              'NOT_CHECKABLE'),
    ('satire',               'NOT_CHECKABLE'),
    ('not related',          'NOT_CHECKABLE'),
]

LABEL_MAP = {
    'False':          'false_claim',
    'Misleading':     'misleading',
    'Unsubstantiated':'unverifiable',
    'Inconclusive':   'unverifiable',
    'True':           'supported',
    'Out of Scope':   'satire_or_opinion',
    'CONTRADICTED':   'false_claim',
    'SUPPORTED':      'supported',
    'UNVERIFIABLE':   'unverifiable',
    'NOT_CHECKABLE':  'satire_or_opinion',
}

CATEGORY_KW = {
    'elections':       ['election', 'civic', 'избори', 'вот', 'гласуване', 'парламент', 'president', 'президент', 'premier', 'министър-председател'],
    'health':          ['health', 'covid', 'vaccine', 'ваксин', 'здраве', 'virus', 'вирус', 'медицин', 'болест', 'disease', 'cancer', 'cancer', 'рак'],
    'conspiracy':      ['conspiracy', 'illuminati', 'теория', 'конспирация', 'chemtrail', 'зионист', 'nwo', 'deep state', 'тайна'],
    'aigc':            ['deepfake', 'generated', 'изкуствен интелект', 'aigc', 'ai generated', 'ai image', 'ai video', 'нейронна', 'neural'],
    'climate':         ['climate', 'климат', 'global warming', 'greenhouse', 'мъгла', 'спрей', 'chemtrail'],
    'crisis':          ['war', 'война', 'ukraine', 'украйна', 'nato', 'нато', 'протест', 'riot', 'military', 'военен', 'газа', 'gaza', 'израел'],
    'science':         ['science', 'наука', 'nasa', 'experiment', 'physics', 'биология', 'луна', 'moon', 'superconductor', 'earth', 'земя'],
    'financial':       ['bank', 'банк', 'инфлация', 'inflation', 'savings', 'спестявания', 'crypto', 'евро', 'euro', 'лев', 'бюджет', 'budget'],
    'repurposed_media':['repurposed', 'old footage', 'стар клип', 'archival', 'out of context', 'footage from', 'clip from'],
    'consumer':        ['product', 'продукт', 'pepsi', 'coca-cola', 'food', 'храна', 'consumer', 'напитка', 'drink', 'brand'],
    'religion':        ['сатанизъм', 'satanic', 'religion', 'религия', 'church', 'черква', 'папа', 'pope', 'ислям', 'islam', 'allah', 'бог', 'god'],
    'history':         ['ancient', 'история', 'исторически', 'medieval', 'средновековн', 'генчис', 'genghis', 'цар', 'цивилизация'],
    'politics':        ['политик', 'minister', 'министър', 'парти', 'party', 'коалиция', 'dep', 'депутат', 'premier', 'правителство', 'government'],
}

URL_RE = re.compile(r'https?://[^\s\)\]\>\"\'\,\u00a0]+')
NBSP = '\u00a0'

SYSTEM_PROMPT = (
    'You are a Bulgarian TikTok fact-checker trained by Lead Stories. '
    'Evaluate the given claim using the provided sources and return a structured verdict. '
    'Rating options: False (CONTRADICTED), Misleading (CONTRADICTED), '
    'Unsubstantiated/Inconclusive (UNVERIFIABLE), True (SUPPORTED), Out of Scope (NOT_CHECKABLE).'
)


def extract_docx_text(path):
    try:
        with zipfile.ZipFile(path) as z:
            if 'word/document.xml' not in z.namelist():
                return ''
            xml_content = z.read('word/document.xml')
            tree = ET.fromstring(xml_content)
            ns = '{http://schemas.openxmlformats.org/wordprocessingml/2006/main}'
            paragraphs = []
            for p in tree.iter(f'{ns}p'):
                para = ''.join(node.text or '' for node in p.iter(f'{ns}t'))
                paragraphs.append(para)
            return '\n'.join(paragraphs)
    except Exception:
        return ''


def extract_pdf_text(path):
    try:
        import pypdf
        reader = pypdf.PdfReader(path)
        return '\n'.join(p.extract_text() or '' for p in reader.pages)
    except Exception:
        return ''


def detect_category(text):
    low = text.lower()
    for cat, kws in CATEGORY_KW.items():
        if any(kw in low for kw in kws):
            return cat
    return 'other'


def normalize_verdict(raw):
    low = (raw or '').lower().strip().replace(NBSP, ' ')
    for key, v in VERDICT_MAP_ORDERED:
        if key in low:
            return v
    return 'CONTRADICTED'


def normalize_label(rating_raw, verdict):
    r = (rating_raw or '').strip()
    for k, v in LABEL_MAP.items():
        if k.lower() == r.lower():
            return v
    return LABEL_MAP.get(verdict, 'false_claim')


def collect_urls(text):
    tiktok = []
    sources = []
    for u in URL_RE.findall(text):
        u = u.rstrip('.,;)')
        if 'tiktok.com' in u or 'facebook.com' in u:
            tiktok.append(u)
        elif u.startswith('http') and 'tcs-sg.bytelemon' not in u:
            sources.append(u)
    return list(dict.fromkeys(tiktok)), list(dict.fromkeys(sources))


def split_bilingual(text):
    lines = text.splitlines()
    en_lines, bg_lines = [], []
    for line in lines:
        if any('\u0400' <= c <= '\u04FF' for c in line):
            bg_lines.append(line.strip())
        elif line.strip():
            en_lines.append(line.strip())
    return ' '.join(en_lines).strip(), ' '.join(bg_lines).strip()


def empty_record():
    return {
        'claim_text_en': '',
        'claim_text_bg': '',
        'verdict': 'CONTRADICTED',
        'label': 'false_claim',
        'category': 'other',
        'notes': '',
        'sources': [],
        'tiktok_urls': [],
        'boolean_queries': '',
        'verifiably_false': True,
    }


def parse_new_claim_format(text):
    rec = empty_record()

    claim_match = re.search(r'New Claim or Trend\s*:[ \xa0]*(.*?)(?=\n(?:Region|Language|Rating|Category|Notes)\s*[:|\xa0]|\Z)', text, re.IGNORECASE | re.DOTALL)
    if claim_match:
        claim_body = claim_match.group(1).strip()
        rec['claim_text_en'], rec['claim_text_bg'] = split_bilingual(claim_body)

    rating_match = re.search(r'Rating\s*:[ \xa0]*(.*?)(?:\n|$)', text, re.IGNORECASE)
    if rating_match:
        rating_raw = rating_match.group(1).replace(NBSP, ' ').strip()
        rec['verdict'] = normalize_verdict(rating_raw)
        rec['label'] = normalize_label(rating_raw, rec['verdict'])

    cat_match = re.search(r'Category\s*:[ \xa0]*(.*?)(?:\n|$)', text, re.IGNORECASE)
    if cat_match:
        cat_raw = cat_match.group(1).replace(NBSP, ' ').strip().lower()
        for cat_key, kws in CATEGORY_KW.items():
            if any(kw in cat_raw for kw in kws):
                rec['category'] = cat_key
                break
        if rec['category'] == 'other':
            rec['category'] = cat_raw[:40] if cat_raw else 'other'

    notes_match = re.search(r'Notes\s*:\s*\n(.*?)(?=\nLinks? to Falsification|Keywords?/Hashtags?|Quantity|\Z)', text, re.IGNORECASE | re.DOTALL)
    if notes_match:
        rec['notes'] = notes_match.group(1).strip()[:1200]

    sources_match = re.search(r'Links? to Falsification[^:]*:(.*?)(?=\nKeywords?/Hashtags?|Quantity|\Z)', text, re.IGNORECASE | re.DOTALL)
    if sources_match:
        _, sources = collect_urls(sources_match.group(1))
        rec['sources'] = sources[:8]

    kw_match = re.search(r'Keywords?/Hashtags?\s*:\s*(.*?)(?:\n|$)', text, re.IGNORECASE)
    if kw_match:
        rec['boolean_queries'] = kw_match.group(1).replace(NBSP, ' ').strip()

    tiktok, _ = collect_urls(text)
    rec['tiktok_urls'] = tiktok[:6]

    if rec['category'] == 'other':
        rec['category'] = detect_category(rec['claim_text_en'] + ' ' + rec['claim_text_bg'])

    return rec


def parse_numbered_trend_format(text):
    rec = empty_record()

    headline_match = re.search(r'1\.[ \xa0]*Headline\s*:?[ \xa0]*(.*?)(?=\n(?:2\.|3\.|Region|Harm)|\Z)', text, re.IGNORECASE | re.DOTALL)
    if headline_match:
        body = headline_match.group(1).strip()
        rec['claim_text_en'], rec['claim_text_bg'] = split_bilingual(body)

    harm_match = re.search(r'3\.[ \xa0]*Harm[^:]*:[ \xa0]*(High|Medium|Low|Moderate)', text, re.IGNORECASE)
    if harm_match:
        h = harm_match.group(1).lower()
        if h == 'high':
            rec['label'] = 'false_claim'
        elif h in ('medium', 'moderate'):
            rec['label'] = 'misleading'
        else:
            rec['label'] = 'missing_context'

    falsif_match = re.search(r'5\.[ \xa0]*Link[^:]*:(.*?)(?=\n6\.|8\.|\Z)', text, re.IGNORECASE | re.DOTALL)
    if falsif_match:
        _, sources = collect_urls(falsif_match.group(1))
        rec['sources'] = sources[:8]

    verif_match = re.search(r'8\.[ \xa0]*Verifiably[^?]*\??\s*:?[ \xa0]*(Yes|No)', text, re.IGNORECASE)
    if verif_match:
        if verif_match.group(1).strip().lower() == 'no':
            rec['verdict'] = 'UNVERIFIABLE'
            rec['label'] = 'unverifiable'
            rec['verifiably_false'] = False

    bool_match = re.search(r'9\.[ \xa0]*Boolean[^:]*:[ \xa0]*(.*?)(?:\n|$)', text, re.IGNORECASE)
    if bool_match:
        rec['boolean_queries'] = bool_match.group(1).replace(NBSP, ' ').strip()

    tiktok, other = collect_urls(text)
    rec['tiktok_urls'] = tiktok[:6]
    if not rec['sources']:
        rec['sources'] = other[:6]

    notes_lines = []
    for line in text.splitlines():
        if not re.match(r'^\d+\.', line) and not any(re.match(pat, line, re.I) for pat in [r'^https?://', r'^Headline', r'^Region', r'^Harm', r'^Example', r'^Link']):
            if line.strip() and len(line.strip()) > 20:
                notes_lines.append(line.strip())
    rec['notes'] = ' '.join(notes_lines[:10])[:1200]

    if rec['category'] == 'other':
        rec['category'] = detect_category(rec['claim_text_en'] + ' ' + rec['claim_text_bg'])

    return rec


def parse_tcs_shift_format(text):
    rec = empty_record()

    lines = [l.strip() for l in text.splitlines()]
    claim_lines, notes_lines, url_lines = [], [], []
    in_claim = in_notes = in_url = False

    for line in lines:
        if not line:
            continue
        low = line.lower()

        if 'tcs-sg.bytelemon.com' in low:
            continue

        v = None
        for key, verdict in VERDICT_MAP_ORDERED:
            if key in low and len(line) < 80:
                v = verdict
                break
        if v and not claim_lines and not in_claim:
            rec['verdict'] = v
            if 'mislead' in low:
                rec['label'] = 'misleading'
            elif v == 'CONTRADICTED':
                rec['label'] = 'false_claim'
            elif v == 'SUPPORTED':
                rec['label'] = 'supported'
            elif v == 'UNVERIFIABLE':
                rec['label'] = 'unverifiable'
            elif v == 'NOT_CHECKABLE':
                rec['label'] = 'satire_or_opinion'

            cat_part = re.sub(r'^.*(misinformation|not misinformation|misinformaton)\s*[–-]\s*', '', line, flags=re.IGNORECASE).strip()
            if cat_part and len(cat_part) < 50:
                for ck, kws in CATEGORY_KW.items():
                    if any(kw in cat_part.lower() for kw in kws):
                        rec['category'] = ck
                        break
            continue

        if re.match(r'^CLAIM\s*\d*', line, re.IGNORECASE):
            in_claim = True; in_notes = False; in_url = False
            continue
        if re.match(r'^NOTES?$', line, re.IGNORECASE):
            in_claim = False; in_notes = True; in_url = False
            continue
        if re.match(r'^URLS?$', line, re.IGNORECASE):
            in_claim = False; in_notes = False; in_url = True
            continue

        if in_claim:
            claim_lines.append(line)
        elif in_notes:
            notes_lines.append(line)
        elif in_url:
            url_lines.append(line)

    rec['claim_text_en'] = ' '.join(claim_lines).strip()
    rec['notes'] = ' '.join(notes_lines).strip()[:1200]
    tiktok, sources = collect_urls('\n'.join(url_lines + lines))
    rec['tiktok_urls'] = tiktok[:6]
    rec['sources'] = [s for s in sources if 'tcs-sg' not in s][:8]

    if rec['category'] == 'other':
        rec['category'] = detect_category(rec['claim_text_en'] + ' ' + rec['notes'])

    return rec


def parse_fc_article_format(text):
    rec = empty_record()

    headline_match = re.search(r'Headline\s*:[ \xa0]*(.*?)(?:\n|$)', text, re.IGNORECASE)
    if headline_match:
        rec['claim_text_en'] = headline_match.group(1).replace(NBSP, ' ').strip()

    cap_match = re.search(r'Caption\s*:[ \xa0]*(.*?)(?:\n\n|\nIs it|$)', text, re.IGNORECASE | re.DOTALL)
    if cap_match:
        rec['notes'] = cap_match.group(1).replace(NBSP, ' ').strip()[:600]

    tiktok, sources = collect_urls(text)
    rec['tiktok_urls'] = tiktok[:6]
    rec['sources'] = sources[:8]
    rec['category'] = detect_category(rec['claim_text_en'] + ' ' + rec['notes'])

    return rec


def classify_and_parse(text):
    low = text.lower().replace(NBSP, ' ')

    if re.search(r'new claim or trend\s*:', low):
        return 'new_claim', parse_new_claim_format(text)

    if re.search(r'1\.\s*headline', low) and ('falsification source' in low or 'boolean queries' in low or 'verifiably false' in low):
        return 'numbered_trend', parse_numbered_trend_format(text)

    if 'claim 1' in low or ('misinformation' in low and 'tcs-sg' in low):
        return 'tcs_shift', parse_tcs_shift_format(text)

    if re.search(r'headline\s*:', low) and ('caption:' in low or 'the claim appeared in' in low or 'it is not true' in low or 'fact check' in low):
        return 'fc_article', parse_fc_article_format(text)

    return 'unknown', None


def stable_id(filepath, claim_text):
    raw = filepath + '::' + (claim_text or '')[:200]
    return hashlib.md5(raw.encode('utf-8')).hexdigest()[:16]


def build_chatml(rec):
    claim = rec.get('claim_text_en') or rec.get('claim_text_bg') or ''
    if not claim:
        return None
    source_block = '\n'.join(f'- {s}' for s in rec.get('sources', [])[:5])
    user_msg = f'Claim: {claim}'
    if source_block:
        user_msg += f'\n\nSources:\n{source_block}'
    assistant = json.dumps({
        'verdict': rec['verdict'],
        'label': rec['label'],
        'category': rec['category'],
        'reasoning_bg': (rec.get('notes') or f"The claim is rated {rec['verdict']}.")[:600],
        'evidence_strength': 0.88 if rec['verifiably_false'] and rec.get('sources') else 0.4,
    }, ensure_ascii=False)
    return {'messages': [
        {'role': 'system', 'content': SYSTEM_PROMPT},
        {'role': 'user', 'content': user_msg},
        {'role': 'assistant', 'content': assistant},
    ]}


def build_alpaca(rec):
    claim = rec.get('claim_text_en') or rec.get('claim_text_bg') or ''
    if not claim:
        return None
    sources = '; '.join(rec.get('sources', [])[:3])
    output = (
        f"Verdict: {rec['verdict']} | Label: {rec['label']} | Category: {rec['category']}.\n"
        f"Evidence: {(rec.get('notes') or '')[:400]}\nSources: {sources}"
    )
    return {
        'instruction': 'You are a Bulgarian fact-checker. Evaluate the following TikTok claim using the provided sources.',
        'input': claim,
        'output': output,
    }


def build_gold(rec, idx):
    claim = rec.get('claim_text_en') or rec.get('claim_text_bg') or ''
    if not claim or not rec.get('sources'):
        return None
    passages = []
    for j, url in enumerate(rec['sources'][:3]):
        passages.append({
            'passage_id': f'p{j+1}',
            'source_url': url,
            'source_name': 'Lead Stories / Professional Fact-Check',
            'source_kind': 'factcheck',
            'published_at': None,
            'text': (rec.get('notes') or '')[:600],
            'score': 0.92 if j == 0 else 0.72,
        })
    return {
        'id': idx,
        'claim_text': claim,
        'speaker': None,
        'claim_type': 'factual',
        'attribution': [],
        'expected_verdict': rec['verdict'],
        'expected_label': rec['label'],
        'is_trap': False,
        'trap_category': None,
        'date_window': None,
        'passages': passages,
    }


def main():
    print(f'Scanning: {TRAINING_DIR}')

    all_docx = glob.glob(os.path.join(TRAINING_DIR, '**', '*.docx'), recursive=True)
    all_pdfs = glob.glob(os.path.join(TRAINING_DIR, '**', '*.pdf'), recursive=True)

    print(f'  Found {len(all_docx)} .docx files, {len(all_pdfs)} PDF files')

    knowledge = []
    chatml_entries = []
    alpaca_entries = []
    gold_set_entries = []
    errors = skipped = 0
    type_counts = {}
    tiktok_creators = set()
    seed_queries = []

    for path in all_docx:
        text = extract_docx_text(path)
        if not text.strip():
            skipped += 1
            continue

        doc_type, rec = classify_and_parse(text)
        type_counts[doc_type] = type_counts.get(doc_type, 0) + 1

        if rec is None:
            skipped += 1
            continue

        claim = rec.get('claim_text_en') or rec.get('claim_text_bg') or ''
        if not claim:
            skipped += 1
            continue

        rec['id'] = stable_id(path, claim)
        rec['source_file'] = os.path.relpath(path, TRAINING_DIR).replace('\\', '/')
        rec['doc_type'] = doc_type
        knowledge.append(rec)

        for url in rec.get('tiktok_urls', []):
            m = re.search(r'tiktok\.com/@([^/?&\s\u00a0]+)', url)
            if m:
                tiktok_creators.add(m.group(1).lower().lstrip('@'))

        if rec.get('boolean_queries'):
            seed_queries.append(rec['boolean_queries'])

        entry = build_chatml(rec)
        if entry:
            chatml_entries.append(entry)

        a_entry = build_alpaca(rec)
        if a_entry:
            alpaca_entries.append(a_entry)

    training_pdfs = []
    for path in all_pdfs:
        txt = extract_pdf_text(path)
        if txt and len(txt) > 100:
            training_pdfs.append({
                'source_file': os.path.relpath(path, TRAINING_DIR).replace('\\', '/'),
                'content': txt[:10000],
                'type': 'training_guideline',
            })

    gold_idx = 1
    for rec in knowledge:
        if rec.get('sources') and rec['verdict'] != 'UNVERIFIABLE' and gold_idx <= 300:
            g = build_gold(rec, gold_idx)
            if g:
                gold_set_entries.append(g)
                gold_idx += 1

    knowledge_out = os.path.join(DATA_DIR, 'lead_stories_knowledge.json')
    with open(knowledge_out, 'w', encoding='utf-8') as f:
        json.dump({
            'total': len(knowledge),
            'extracted_at': '2026-11-12',
            'source': 'docs/training_data/LEAD STORIES/',
            'factchecks': knowledge,
            'training_guidelines': training_pdfs,
        }, f, ensure_ascii=False, indent=2)

    chatml_out = os.path.join(TRAINING_OUT_DIR, 'lead_stories_train_chatml.jsonl')
    with open(chatml_out, 'w', encoding='utf-8') as f:
        for entry in chatml_entries:
            f.write(json.dumps(entry, ensure_ascii=False) + '\n')

    alpaca_out = os.path.join(TRAINING_OUT_DIR, 'lead_stories_train_alpaca.json')
    with open(alpaca_out, 'w', encoding='utf-8') as f:
        json.dump(alpaca_entries, f, ensure_ascii=False, indent=2)

    gold_out = os.path.join(FIXTURES_DIR, 'lead_stories_gold_set.json')
    with open(gold_out, 'w', encoding='utf-8') as f:
        json.dump({
            'synthetic': False,
            'notice': 'Lead Stories ground truth evaluation set extracted from real 2023-2026 fact-checks in docs/training_data/LEAD STORIES/TCS and LL WORK/',
            'total_claims': len(gold_set_entries),
            'claims': gold_set_entries,
        }, f, ensure_ascii=False, indent=2)

    existing_creators = {'benyta7756', 'petya_lawrence', 'peter_evtimov', 'gogobulrus', 'vstaykov', 'saprotivatabulgaria'}
    creator_lines = []
    for creator in sorted(tiktok_creators):
        clean = re.sub(r'[^a-z0-9_.@-]', '', creator)
        if not clean or clean in existing_creators or len(clean) < 3 or len(clean) > 50:
            continue
        creator_lines.append(f"\n[[seeds]]\nkind = 'creator'\nvalue = '{clean}'\ntopic = 'flagged_creator'")
        existing_creators.add(clean)

    query_lines = []
    seen_queries = set()
    for q in seed_queries:
        q = q.strip().replace("'", '').replace('\xa0', ' ')
        if q and len(q) > 5 and q not in seen_queries:
            query_lines.append(f"\n[[seeds]]\nkind = 'keyword'\nvalue = '{q[:100]}'\ntopic = 'disinformation_trend'")
            seen_queries.add(q)
            if len(query_lines) >= 80:
                break

    all_new_seeds = creator_lines + query_lines
    if all_new_seeds:
        seeds_path = os.path.normpath(os.path.join(os.path.dirname(__file__), '..', 'config', 'seeds.bg.toml'))
        with open(seeds_path, 'r', encoding='utf-8') as f:
            existing = f.read()
        already_in_file = set(re.findall(r"value = '([^']+)'", existing))
        new_to_write = []
        for block in all_new_seeds:
            m = re.search(r"value = '([^']+)'", block)
            if m and m.group(1) not in already_in_file:
                new_to_write.append(block)
                already_in_file.add(m.group(1))
        if new_to_write:
            with open(seeds_path, 'a', encoding='utf-8') as f:
                f.write('\n' + '\n'.join(new_to_write))
            print(f'  Added {len(new_to_write)} new seed entries to config/seeds.bg.toml')
        else:
            print('  Seeds already up to date (no new entries needed)')

    total = len(knowledge)
    contradicted = sum(1 for r in knowledge if r['verdict'] == 'CONTRADICTED')
    supported = sum(1 for r in knowledge if r['verdict'] == 'SUPPORTED')
    unverifiable = sum(1 for r in knowledge if r['verdict'] == 'UNVERIFIABLE')
    not_checkable = sum(1 for r in knowledge if r['verdict'] == 'NOT_CHECKABLE')
    with_sources = sum(1 for r in knowledge if r.get('sources'))
    with_tiktok = sum(1 for r in knowledge if r.get('tiktok_urls'))

    print('\n=== EXTRACTION COMPLETE ===')
    print(f'  Total fact-checks extracted : {total}')
    print(f'  Skipped (empty/unparseable) : {skipped}')
    print(f'  Doc type breakdown:')
    for t, c in sorted(type_counts.items(), key=lambda x: -x[1]):
        print(f'    {t}: {c}')
    print(f'  CONTRADICTED (False/Misleading): {contradicted}')
    print(f'  SUPPORTED (True):                {supported}')
    print(f'  UNVERIFIABLE:                    {unverifiable}')
    print(f'  NOT_CHECKABLE:                   {not_checkable}')
    print(f'  With debunking sources:          {with_sources}')
    print(f'  With TikTok URLs:                {with_tiktok}')
    print(f'  Unique creators harvested:       {len(tiktok_creators)}')
    print(f'  Gold set evaluation entries:     {len(gold_set_entries)}')
    print(f'  ChatML training examples:        {len(chatml_entries)}')
    print(f'  Alpaca training examples:        {len(alpaca_entries)}')
    print(f'  Training PDFs ingested:          {len(training_pdfs)}')
    print()
    print(f'  Outputs:')
    print(f'    {knowledge_out}')
    print(f'    {chatml_out}')
    print(f'    {alpaca_out}')
    print(f'    {gold_out}')


if __name__ == '__main__':
    main()
