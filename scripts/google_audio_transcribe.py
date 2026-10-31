import argparse
import concurrent.futures
import difflib
import json
import os
import re
import subprocess
import sys

if hasattr(sys.stdout, 'reconfigure'):
    sys.stdout.reconfigure(encoding='utf-8')
if hasattr(sys.stderr, 'reconfigure'):
    sys.stderr.reconfigure(encoding='utf-8')

REPAIRS = [
    (r'(?i)\bалкогол\b', 'алкохол'),
    (r'(?i)\bполезай\b', 'полицаи'),
    (r'(?i)\bполицайте\b', 'полицаите'),
    (r'(?i)\bдои доха\b', 'дойдоха'),
    (r'(?i)\bбърза полнош\b', 'Бърза помощ'),
    (r'(?i)\bбър за помощ\b', 'Бърза помощ'),
    (r'(?i)\bпарното нещо\b', 'първото нещо'),
    (r'(?i)\bкота\b', 'колата'),
    (r'(?i)\bкоаа\b', 'кола'),
    (r'(?i)\bе два\b', 'едвам'),
    (r'(?i)\bито и 12\b', '112'),
    (r'(?i)\bи 12\b', '112'),
    (r'(?i)\bвсичко е наряд\b', 'всичко е наред'),
    (r'(?i)\bнуа, нуа\b', '0.0'),
    (r'(?i)\bнуа нуа\b', '0.0'),
    (r'(?i)\bсполиви тест\b', 'с полеви тест'),
    (r'(?i)\bнаркотит\b', 'наркотици'),
    (r'(?i)\bнаркотисти\b', 'наркотици'),
    (r'(?i)\bкълътка\b', 'кратка'),
    (r'(?i)\bнатертена\b', 'натъртена'),
    (r'(?i)\bпотордо\b', 'повторно'),
    (r'(?i)\bвзриви овен\b', 'взриви, освен'),
    (r'(?i)\bдушел\b', 'дошъл'),
    (r'(?i)\bразледането\b', 'разследването'),
    (r'(?i)\bбезпрестрастно\b', 'безпристрастно'),
    (r'(?i)\bрумен бъков\b', 'Румен Бахов'),
    (r'(?i)\bелисавета\b', 'Елизабет'),
    (r'(?i)\bкикам нас\b', 'към нас'),
    (r'(?i)\bследзаве\b', 'слезе'),
    (r'(?i)\bпосмисляване\b', 'по смисъла'),
    (r'(?i)\bтож бе\b', 'всички да сме'),
    (r'(?i)\bсвърчи\b', 'свърши'),
    (r'(?i)\bплътността на спорта на предшествия\b', 'пътнотранспортното произшествие'),
    (r'(?i)\bпът на тъна спорта на правишествия\b', 'пътнотранспортното произшествие'),
    (r'(?i)\bозобахте се\b', 'озовахте се'),
    (r'(?i)\bбиса 10 на минути\b', 'би след десетина минути'),
    (r'(?i)\bакър разбракате\b', 'как разбрахте'),
    (r'(?i)\bпоказвайте ми казаха\b', 'полицаите ми казаха'),
    (r'(?i)\bистория тъм мери, ви ки\b', 'историята, Мира и Вики'),
    (r'(?i)\bбъщам ощещо\b', 'баща му също'),
    (r'(?i)\bкаво че ухте\b', 'какво чухте'),
    (r'(?i)\bса гаснат\b', 'съгласни'),
    (r'(?i)\bсе обера мое приятел\b', 'се обърна моят приятел'),
    (r'(?i)\bот гора на тротоара\b', 'отгоре на тротоара'),
]

def clean_bg(text):
    for pat, rep in REPAIRS:
        text = re.sub(pat, rep, text)
    return text

def ensure_wav(input_path):
    if input_path.lower().endswith('.wav') and os.path.exists(input_path):
        return input_path
    base, _ = os.path.splitext(input_path)
    out_wav = f"{base}_16k.wav"
    if os.path.exists(out_wav) and os.path.getsize(out_wav) > 1000:
        return out_wav
    cmd = [
        'ffmpeg', '-y', '-i', input_path,
        '-vn', '-af', 'highpass=f=100,lowpass=f=3800,afftdn=nf=-25',
        '-ac', '1', '-ar', '16000',
        out_wav
    ]
    try:
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
        return out_wav
    except Exception:
        return input_path

def punctuate_and_divide_bg(raw_text):
    cleaned = clean_bg(raw_text).strip()
    words = cleaned.split()
    if not words:
        return []

    question_triggers = {'ли', 'дали', 'защо', 'как', 'кога', 'къде', 'кой', 'кое', 'чии', 'колко'}
    comma_triggers = {'че', 'но', 'а', 'защото', 'като', 'когато', 'ако', 'докато', 'въпреки'}

    sentences = []
    current_words = []
    has_question = False

    for i, w in enumerate(words):
        low = w.lower().strip('.,!?')
        if low in comma_triggers and current_words and not current_words[-1].endswith(','):
            current_words[-1] = current_words[-1] + ','

        current_words.append(w)
        if low in question_triggers:
            has_question = True

        is_end = False
        if len(current_words) >= 5:
            if i + 1 < len(words):
                next_w = words[i+1].lower().strip('.,!?')
                if next_w in {'но', 'едно', 'след', 'кажете', 'последвайте', 'също', 'така', 'въпросът'}:
                    is_end = True
                elif has_question and low in {'сме', 'всичко', 'ли', 'пари', 'същата', 'те'}:
                    is_end = True
                elif len(current_words) >= 14:
                    is_end = True
            else:
                is_end = True

        if is_end:
            s_text = ' '.join(current_words).strip()
            if s_text:
                s_text = s_text[0].upper() + s_text[1:]
                end_punct = '?' if has_question else '.'
                if not s_text.endswith(('.', '?', '!')):
                    s_text += end_punct
                sentences.append(s_text)
            current_words = []
            has_question = False

    if current_words:
        s_text = ' '.join(current_words).strip()
        if s_text:
            s_text = s_text[0].upper() + s_text[1:]
            end_punct = '?' if has_question else '.'
            if not s_text.endswith(('.', '?', '!')):
                s_text += end_punct
            sentences.append(s_text)

    return sentences

def parse_gemini_timestamped_sentences(gemini_raw_text):
    if not gemini_raw_text or not gemini_raw_text.strip():
        return [], ""

    lines = [l.strip() for l in gemini_raw_text.splitlines() if l.strip()]
    segments = []
    dialogue_lines = []

    pattern_range = re.compile(r'\[(\d{1,2}):(\d{2})\s*-\s*(\d{1,2}):(\d{2})\]\s*(.+)')
    pattern_single = re.compile(r'\[(\d{1,2}):(\d{2})\]\s*(.+)')

    cur_time_ms = 0

    for line in lines:
        m_range = pattern_range.match(line)
        if m_range:
            m1, s1, m2, s2, txt = m_range.groups()
            start_ms = (int(m1) * 60 + int(s1)) * 1000
            end_ms = (int(m2) * 60 + int(s2)) * 1000
            clean_s = clean_bg(txt).strip()
            if clean_s:
                segments.append({
                    "start_ms": start_ms,
                    "end_ms": max(end_ms, start_ms + 1000),
                    "text": clean_s
                })
                dialogue_lines.append(clean_s)
                cur_time_ms = max(end_ms, start_ms + 1000)
            continue

        m_single = pattern_single.match(line)
        if m_single:
            m1, s1, txt = m_single.groups()
            start_ms = (int(m1) * 60 + int(s1)) * 1000
            clean_s = clean_bg(txt).strip()
            if clean_s:
                segments.append({
                    "start_ms": start_ms,
                    "end_ms": start_ms + 4000,
                    "text": clean_s
                })
                dialogue_lines.append(clean_s)
                cur_time_ms = start_ms + 4000
            continue

        divided = punctuate_and_divide_bg(line)
        for s in divided:
            segments.append({
                "start_ms": cur_time_ms,
                "end_ms": cur_time_ms + 3500,
                "text": s
            })
            dialogue_lines.append(s)
            cur_time_ms += 3500

    for i in range(len(segments) - 1):
        if segments[i]["end_ms"] > segments[i+1]["start_ms"] and segments[i+1]["start_ms"] > segments[i]["start_ms"]:
            segments[i]["end_ms"] = segments[i+1]["start_ms"]

    full_dialogue = '\n'.join(dialogue_lines)
    return segments, full_dialogue

def transcribe_with_gemini(wav_path, api_keys_input, hf_token: str | None = None):
    """Transcribe audio with Gemini and optionally attach speaker diarization.

    * ``api_keys_input`` – Gemini API keys (comma‑separated).
    * ``hf_token`` – HuggingFace token for the ``pyannote`` diarization model. If ``None`` diarization is skipped.
    """
    if not api_keys_input:
        return None
    keys = [k.strip() for k in str(api_keys_input).replace('\\n', ',').replace(';', ',').split(',') if k.strip()]
    if not keys:
        return None

    try:
        import urllib.request
        import urllib.error
        import base64
        with open(wav_path, 'rb') as f:
            audio_bytes = f.read()
        b64_audio = base64.b64encode(audio_bytes).decode('utf-8')
    except Exception:
        return None

    prompt_inst = (
        "You are an expert Bulgarian speech transcriber. Transcribe this audio recording verbatim with maximum fidelity.\n\n"
        "CRITICAL SENTENCE DIVISION & PUNCTUATION RULES:\n"
        "1. You MUST divide the speech into natural, complete, grammatical sentences.\n"
        "2. Punctuate every sentence accurately: capitalize the first word, insert commas, and end with a period (.), question mark (?), or exclamation mark (!).\n"
        "3. Output each sentence on its own separate line preceded by its time interval in format:\n"
        "   [MM:SS - MM:SS] Текст на цялото изречение.\n"
        "4. NEVER combine multiple sentences into a single continuous wall of text or long unbroken paragraph.\n"
        "5. Return only the timestamped Bulgarian lines."
    )

    payload = {
        "contents": [{
            "parts": [
                {"text": prompt_inst},
                {"inlineData": {"mimeType": "audio/wav", "data": b64_audio}}
            ]
        }],
        "generationConfig": {"temperature": 0.1, "maxOutputTokens": 8192}
    }
    body_data = json.dumps(payload).encode('utf-8')

    for api_key in keys:
        try:
            url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={api_key}"
            req = urllib.request.Request(url, data=body_data, headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=45) as resp:
                data = json.loads(resp.read().decode('utf-8'))
                candidates = data.get("candidates", [])
                if candidates:
                    parts = candidates[0].get("content", {}).get("parts", [])
                    if parts:
                        text = parts[0].get("text", "").strip()
                        if text:
                            if hf_token:
                                from .diarization_helper import _attach_speakers_to_segments
                                segments, full_dialogue = _attach_speakers_to_segments(wav_path, text, hf_token)
                                return segments, full_dialogue
                            return text
        except urllib.error.HTTPError as he:
            if he.code in (429, 403):
                continue
        except Exception:
            pass
    return None
    if not api_keys_input:
        return None
    keys = [k.strip() for k in str(api_keys_input).replace('\n', ',').replace(';', ',').split(',') if k.strip()]
    if not keys:
        return None

    try:
        import urllib.request
        import urllib.error
        import base64
        with open(wav_path, 'rb') as f:
            audio_bytes = f.read()
        b64_audio = base64.b64encode(audio_bytes).decode('utf-8')
    except Exception:
        return None

    prompt_inst = (
        "You are an expert Bulgarian speech transcriber. Transcribe this audio recording verbatim with maximum fidelity.\n\n"
        "CRITICAL SENTENCE DIVISION & PUNCTUATION RULES:\n"
        "1. You MUST divide the speech into natural, complete, grammatical sentences.\n"
        "2. Punctuate every sentence accurately: capitalize the first word, insert commas, and end with a period (.), question mark (?), or exclamation mark (!).\n"
        "3. Output each sentence on its own separate line preceded by its time interval in format:\n"
        "   [MM:SS - MM:SS] Текст на цялото изречение.\n"
        "4. NEVER combine multiple sentences into a single continuous wall of text or long unbroken paragraph.\n"
        "5. Return only the timestamped Bulgarian lines."
    )

    payload = {
        "contents": [{
            "parts": [
                {"text": prompt_inst},
                {
                    "inlineData": {
                        "mimeType": "audio/wav",
                        "data": b64_audio
                    }
                }
            ]
        }],
        "generationConfig": {
            "temperature": 0.1,
            "maxOutputTokens": 8192
        }
    }
    body_data = json.dumps(payload).encode('utf-8')

    for api_key in keys:
        try:
            import urllib.request
            url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={api_key}"
            req = urllib.request.Request(
                url,
                data=body_data,
                headers={"Content-Type": "application/json"}
            )
            with urllib.request.urlopen(req, timeout=45) as resp:
                data = json.loads(resp.read().decode('utf-8'))
                candidates = data.get("candidates", [])
                if candidates:
                    parts = candidates[0].get("content", {}).get("parts", [])
                    if parts:
                        text = parts[0].get("text", "").strip()
                        if text:
                            return text
        except urllib.error.HTTPError as he:
            if he.code in (429, 403):
                continue
        except Exception:
            pass
    return None

def align_google_words_to_whisper_segments(google_text, whisper_segments):
    if not whisper_segments:
        return []
    g_words = google_text.split()
    if not g_words:
        out = []
        for seg in whisper_segments:
            txt = clean_bg(seg.get('text', '')).strip()
            if not txt:
                continue
            if "offsets" in seg and isinstance(seg["offsets"], dict):
                s_ms = seg["offsets"].get("from", 0)
                e_ms = seg["offsets"].get("to", s_ms + 1000)
            else:
                s_ms = seg.get("start_ms", 0)
                e_ms = seg.get("end_ms", s_ms + 1000)
            out.append({"start_ms": s_ms, "end_ms": e_ms, "text": txt})
        return out

    def normalize_tok(t):
        return re.sub(r'[^\w]', '', t.lower())

    w_all_words = []
    seg_word_indices = []

    for s_idx, seg in enumerate(whisper_segments):
        raw_text = clean_bg(seg.get('text', '')).strip()
        tokens = raw_text.split()
        start_idx = len(w_all_words)
        for tok in tokens:
            w_all_words.append((tok, normalize_tok(tok), s_idx))
        seg_word_indices.append((start_idx, len(w_all_words)))

    w_norm = [item[1] for item in w_all_words]
    g_norm = [normalize_tok(w) for w in g_words]

    matcher = difflib.SequenceMatcher(None, w_norm, g_norm)

    aligned_w_to_g = {}
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == 'equal':
            for offset in range(i2 - i1):
                aligned_w_to_g[i1 + offset] = g_words[j1 + offset]
        elif tag == 'replace':
            for offset in range(min(i2 - i1, j2 - j1)):
                aligned_w_to_g[i1 + offset] = g_words[j1 + offset]

    output_segments = []
    for s_idx, seg in enumerate(whisper_segments):
        s_from, s_to = seg_word_indices[s_idx]
        orig_text = clean_bg(seg.get('text', '')).strip()
        tokens = orig_text.split()

        reconstructed = []
        for t_idx, tok in enumerate(tokens):
            global_idx = s_from + t_idx
            punct_prefix = ''
            punct_suffix = ''
            m_start = re.match(r'^[^\w]+', tok)
            if m_start:
                punct_prefix = m_start.group(0)
            m_end = re.search(r'[^\w]+$', tok)
            if m_end:
                punct_suffix = m_end.group(0)

            if global_idx in aligned_w_to_g:
                g_word = aligned_w_to_g[global_idx]
                clean_w = re.sub(r'^[^\w]+|[^\w]+$', '', tok)
                clean_g = re.sub(r'^[^\w]+|[^\w]+$', '', g_word)
                if clean_w.isupper():
                    clean_g = clean_g.upper()
                elif clean_w and clean_w[0].isupper():
                    clean_g = clean_g.capitalize()
                reconstructed.append(f"{punct_prefix}{clean_g}{punct_suffix}")
            else:
                reconstructed.append(tok)

        res_text = " ".join(reconstructed).strip()
        if res_text and not res_text.endswith(('.', '?', '!', ':', ';', ',')):
            res_text += '.'

        if "offsets" in seg and isinstance(seg["offsets"], dict):
            s_ms = seg["offsets"].get("from", 0)
            e_ms = seg["offsets"].get("to", s_ms + 1000)
        else:
            s_ms = seg.get("start_ms", 0)
            e_ms = seg.get("end_ms", s_ms + 1000)

        if res_text:
            output_segments.append({
                'start_ms': s_ms,
                'end_ms': e_ms,
                'text': res_text
            })

    return output_segments

def transcribe_with_whisper_guided_google(wav_path, whisper_segments):
    g_segs, g_dialogue = transcribe_with_google_web(wav_path)
    if g_dialogue and g_dialogue.strip():
        aligned = align_google_words_to_whisper_segments(g_dialogue, whisper_segments)
        if aligned:
            return aligned, '\n'.join([s['text'] for s in aligned])

    fallback = []
    for seg in whisper_segments:
        txt = clean_bg(seg.get('text', '')).strip()
        if not txt:
            continue
        if "offsets" in seg and isinstance(seg["offsets"], dict):
            s_ms = seg["offsets"].get("from", 0)
            e_ms = seg["offsets"].get("to", s_ms + 1000)
        else:
            s_ms = seg.get("start_ms", 0)
            e_ms = seg.get("end_ms", s_ms + 1000)
        fallback.append({"start_ms": s_ms, "end_ms": e_ms, "text": txt})
    return fallback, '\n'.join([s['text'] for s in fallback])

def detect_acoustic_silence_splits(wav_path):
    cmd = [
        'ffmpeg', '-i', wav_path,
        '-af', 'silencedetect=noise=-30dB:d=0.35',
        '-f', 'null', '-'
    ]
    try:
        proc = subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        lines = proc.stderr.splitlines()
        silence_starts = []
        for l in lines:
            if 'silence_start:' in l:
                parts = l.split('silence_start:')
                if len(parts) > 1:
                    try:
                        silence_starts.append(float(parts[1].split()[0]))
                    except Exception:
                        pass
        return silence_starts
    except Exception:
        return []

def transcribe_with_google_web(wav_path):
    import speech_recognition as sr

    with sr.AudioFile(wav_path) as source:
        total_duration = int(source.DURATION)

    silence_starts = detect_acoustic_silence_splits(wav_path)
    cut_points = [0.0]
    for st in silence_starts:
        if st - cut_points[-1] >= 4.0 and (total_duration - st) >= 2.0:
            cut_points.append(st)
    if cut_points[-1] < total_duration:
        cut_points.append(float(total_duration))

    tasks = []
    for i in range(len(cut_points) - 1):
        off = cut_points[i]
        dur = cut_points[i+1] - off
        if dur > 0.3:
            tasks.append((off, dur))

    if not tasks:
        tasks = [(0.0, float(total_duration))]

    segments = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
        futures = {executor.submit(transcribe_chunk_task, wav_path, off, d): off for off, d in tasks}
        for future in concurrent.futures.as_completed(futures):
            res = future.result()
            if res:
                segments.append(res)

    segments.sort(key=lambda s: s["start_ms"])

    refined_segments = []
    dialogue_lines = []
    for seg in segments:
        div = punctuate_and_divide_bg(seg["text"])
        if len(div) <= 1:
            refined_segments.append(seg)
            dialogue_lines.append(seg["text"])
        else:
            span_ms = seg["end_ms"] - seg["start_ms"]
            part_ms = max(1000, span_ms // len(div))
            cur = seg["start_ms"]
            for s in div:
                refined_segments.append({
                    "start_ms": cur,
                    "end_ms": cur + part_ms,
                    "text": s
                })
                dialogue_lines.append(s)
                cur += part_ms

    return refined_segments, '\n'.join(dialogue_lines)

def transcribe_chunk_task(wav_path, offset, dur):
    import speech_recognition as sr
    recognizer = sr.Recognizer()
    try:
        with sr.AudioFile(wav_path) as source:
            audio_data = recognizer.record(source, duration=dur, offset=offset)
        raw_text = recognizer.recognize_google(audio_data, language="bg-BG")
        cleaned = clean_bg(raw_text).strip()
        if cleaned:
            sentences = punctuate_and_divide_bg(cleaned)
            formatted = ' '.join(sentences)
            return {
                "start_ms": int(offset * 1000),
                "end_ms": int((offset + dur) * 1000),
                "text": formatted
            }
    except Exception:
        pass
    return None

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--audio', type=str, default=None)
    parser.add_argument('--whisper-json', type=str, default=None)
    parser.add_argument('--video-id', type=str, default=None)
    args = parser.parse_args()

    audio_file = args.audio
    if not audio_file and args.video_id:
        cand1 = os.path.join('data', 'media', args.video_id, 'audio_16k.wav')
        cand2 = os.path.join('data', 'media', args.video_id, 'video.mp4')
        if os.path.exists(cand1):
            audio_file = cand1
        elif os.path.exists(cand2):
            audio_file = cand2

    if not audio_file or not os.path.exists(audio_file):
        print(json.dumps({"ok": False, "error": f"Audio file not found: {audio_file}"}))
        sys.exit(1)

    wav_path = ensure_wav(audio_file)

    whisper_segments = []
    if args.whisper_json and os.path.exists(args.whisper_json):
        try:
            with open(args.whisper_json, 'r', encoding='utf-8') as f:
                w_data = json.load(f)
                if isinstance(w_data, list):
                    whisper_segments = w_data
                elif isinstance(w_data, dict):
                    whisper_segments = w_data.get("segments") or w_data.get("transcription") or []
        except Exception:
            pass

    api_key = os.environ.get("GEMINI_API_KEY")
    if api_key:
        gemini_raw = transcribe_with_gemini(wav_path, api_key)
        if gemini_raw and len(gemini_raw.strip()) > 25:
            gem_segs, gem_dialogue = parse_gemini_timestamped_sentences(gemini_raw)
            if gem_segs:
                print(json.dumps({
                    "ok": True,
                    "source": "gemini_multimodal_audio",
                    "corrected_text": gem_dialogue,
                    "segments": gem_segs
                }, ensure_ascii=False))
                sys.exit(0)

    if whisper_segments:
        guided_segs, guided_dialogue = transcribe_with_whisper_guided_google(wav_path, whisper_segments)
        if guided_segs:
            print(json.dumps({
                "ok": True,
                "source": "google_whisper_hybrid",
                "corrected_text": guided_dialogue,
                "segments": guided_segs
            }, ensure_ascii=False))
            sys.exit(0)

    segments, dialogue = transcribe_with_google_web(wav_path)
    print(json.dumps({
        "ok": True,
        "source": "google_web_speech_api",
        "corrected_text": dialogue,
        "segments": segments
    }, ensure_ascii=False))

if __name__ == '__main__':
    main()
