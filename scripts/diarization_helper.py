import re
import json
from typing import List, Dict, Tuple

def _parse_gemini_output(gemini_text: str) -> List[Dict]:
    """Parse Gemini timestamped output into a list of segment dicts.
    Expected line format: "[MM:SS - MM:SS] Текст..."
    Returns list of dicts with ``start_ms``, ``end_ms`` and ``text``.
    """
    segments = []
    pattern = re.compile(r"\[(\d{2}):(\d{2})\s*-\s*(\d{2}):(\d{2})\]\s*(.+)")
    for line in gemini_text.splitlines():
        line = line.strip()
        if not line:
            continue
        m = pattern.match(line)
        if not m:
            # fallback – treat whole line as text with unknown timestamps
            continue
        sh, sm, eh, em, txt = m.groups()
        start_ms = (int(sh) * 60 + int(sm)) * 1000
        end_ms = (int(eh) * 60 + int(em)) * 1000
        segments.append({"start_ms": start_ms, "end_ms": end_ms, "text": txt.strip()})
    return segments

def _attach_speakers_to_segments(wav_path: str, gemini_text: str, hf_token: str) -> Tuple[List[Dict], str]:
    """Run pyannote speaker diarization on ``wav_path`` and merge with Gemini output.

    Returns a tuple ``(segments, full_dialogue)`` where ``segments`` is a list of
    dicts ``{"start_ms": int, "end_ms": int, "text": str, "speaker": str}``.
    ``full_dialogue`` is the concatenated text of all segments (without timestamps).
    """
    # Load diarization pipeline
    from pyannote.audio import Pipeline
    pipeline = Pipeline.from_pretrained(
        "pyannote/speaker-diarization-3.1",
        use_auth_token=hf_token,
    )
    diarization = pipeline(wav_path)
    # Convert diarization to a list of (start_ms, end_ms, speaker)
    diar_segments = []
    for turn, _, speaker in diarization.itertracks(yield_label=True):
        diar_segments.append({
            "start_ms": int(turn.start * 1000),
            "end_ms": int(turn.end * 1000),
            "speaker": speaker,
        })
    # Parse Gemini output
    gemini_segments = _parse_gemini_output(gemini_text)
    # Merge speaker info
    merged = []
    for seg in gemini_segments:
        # Find diarization segment that overlaps the Gemini segment start
        speaker = None
        for d in diar_segments:
            if d["start_ms"] <= seg["start_ms"] < d["end_ms"]:
                speaker = d["speaker"]
                break
        merged.append({
            "start_ms": seg["start_ms"],
            "end_ms": seg["end_ms"],
            "text": seg["text"],
            "speaker": speaker,
        })
    full_dialogue = "\n".join([s["text"] for s in merged])
    return merged, full_dialogue
