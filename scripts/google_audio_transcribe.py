import argparse
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
        '-vn', '-ac', '1', '-ar', '16000',
        out_wav
    ]
    try:
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
        return out_wav
    except Exception:
        return input_path

def transcribe_with_gemini(wav_path, api_key):
    try:
        import urllib.request
        import base64
        with open(wav_path, 'rb') as f:
            audio_bytes = f.read()
        b64_audio = base64.b64encode(audio_bytes).decode('utf-8')
        url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={api_key}"
        payload = {
            "contents": [{
                "parts": [
                    {"text": "Transcribe this Bulgarian audio verbatim. Format the output in flowing paragraphs with bold timestamps:\n**MM:SS**\nReturn only the transcribed Bulgarian text."},
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
        req = urllib.request.Request(
            url,
            data=json.dumps(payload).encode('utf-8'),
            headers={"Content-Type": "application/json"}
        )
        with urllib.request.urlopen(req, timeout=45) as resp:
            data = json.loads(resp.read().decode('utf-8'))
            candidates = data.get("candidates", [])
            if candidates:
                parts = candidates[0].get("content", {}).get("parts", [])
                if parts:
                    return parts[0].get("text", "").strip()
    except Exception:
        pass
    return None

def transcribe_chunk_task(wav_path, offset, dur):
    import speech_recognition as sr
    recognizer = sr.Recognizer()
    try:
        with sr.AudioFile(wav_path) as source:
            audio_data = recognizer.record(source, duration=dur, offset=offset)
        raw_text = recognizer.recognize_google(audio_data, language="bg-BG")
        cleaned = clean_bg(raw_text).strip()
        if cleaned:
            return {
                "start_ms": offset * 1000,
                "end_ms": (offset + dur) * 1000,
                "text": cleaned
            }
    except Exception:
        pass
    return None

def transcribe_with_google_web(wav_path):
    import speech_recognition as sr
    import concurrent.futures

    with sr.AudioFile(wav_path) as source:
        total_duration = int(source.DURATION)

    chunk_sec = 15
    tasks = []
    for offset in range(0, total_duration, chunk_sec):
        dur = min(chunk_sec, total_duration - offset)
        if dur > 0:
            tasks.append((offset, dur))

    segments = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
        futures = {executor.submit(transcribe_chunk_task, wav_path, off, d): off for off, d in tasks}
        for future in concurrent.futures.as_completed(futures):
            res = future.result()
            if res:
                segments.append(res)

    segments.sort(key=lambda s: s["start_ms"])
    return segments

def format_segments_to_dialogue(segments):
    lines = []
    cur_bucket = -1

    for seg in segments:
        sec = seg['start_ms'] // 1000
        bucket = (sec // 45) * 45
        if bucket != cur_bucket:
            cur_bucket = bucket
            m = sec // 60
            s = sec % 60
            lines.append(f'**{m:02d}:{s:02d}**')
        
        t = seg.get('text', '').strip()
        if not t:
            continue
        lines.append(t)

    return '\n'.join(lines)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--audio', type=str, default=None)
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
    api_key = os.environ.get("GEMINI_API_KEY")

    if api_key:
        gemini_dialogue = transcribe_with_gemini(wav_path, api_key)
        if gemini_dialogue and len(gemini_dialogue) > 40:
            print(json.dumps({
                "ok": True,
                "source": "gemini_multimodal_audio",
                "corrected_text": clean_bg(gemini_dialogue),
                "segments": []
            }, ensure_ascii=False))
            sys.exit(0)

    segments = transcribe_with_google_web(wav_path)
    dialogue = format_segments_to_dialogue(segments)

    print(json.dumps({
        "ok": True,
        "source": "google_web_speech_api",
        "corrected_text": dialogue,
        "segments": segments
    }, ensure_ascii=False))

if __name__ == '__main__':
    main()
