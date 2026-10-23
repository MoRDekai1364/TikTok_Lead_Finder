# Phase 2 Bulgarian WER Evaluation & Lexicon Impact Report

## Methodology

- **Dataset**: 12 spoken Bulgarian TikTok clips from the 20-clip Phase 2 batch with human-audited reference transcripts (`fixtures/phase2_wer_references.json`, 1,709 reference words total).
- **Command**: `.\target\release\leadfinder.exe wer-report fixtures/phase2_wer_references.json`
- **Engine**: `whisper.cpp` (`b5454`) with `ggml-small.bin` (`CaptionFirstTranscriber` -> native VTT `found` first, `whisper_cpp` `generated` second).
- **Bulgarian Lexicon & Phonetic/Grammar Pipeline (`config/lexicon.bg.toml` + `crates/providers/src/lexicon.rs`)**:
  - `922,137` inflected Bulgarian wordforms (`bg_wiktionary_lexicon.tsv` + `bg_BG.dic`/`bg_BG.aff` Hunspell affix expansion + `bg_50k.txt` spoken OpenSubtitles frequencies + `bg_mfa_ipa.dict`).
  - `75,930` IPA phonetic transcriptions (`bg_mfa_ipa.dict` + Bulgarian Wiktionary IPA) indexed into `149,148` acoustic/orthographic phonetic transcription keys.
  - `1,241` dialectal, informal, colloquial, and slang entries with standard glosses + IPA transcriptions, plus institutional acronym expansions and renarrated-mood (`преизказно наклонение`) hearsay markers for downstream LLM claim extraction.

## Per-Clip WER Comparison

| Video ID | Ref Words | Baseline `small` WER | Lexicon + Phonetic `small` WER | Delta | Quality Score |
| --- | ---: | ---: | ---: | ---: | ---: |
| `7584058076953627926` | 63 | 0.063 | **0.048** | -0.015 | 0.938 |
| `7697234501155491074` | 427 | 0.145 | **0.119** | -0.026 | 0.955 |
| `7588087577186454806` | 89 | 0.225 | **0.124** | -0.101 | 0.952 |
| `7603789020564376854` | 106 | 0.189 | **0.132** | -0.057 | 0.952 |
| `7564423898557664534` | 74 | 0.122 | 0.135 | +0.013 | 0.931 |
| `7695048513306119457` | 124 | 0.210 | **0.177** | -0.033 | 0.924 |
| `7693938711473523990` | 155 | 0.222 | **0.184** | -0.038 | 0.939 |
| `7694316075466362134` | 16 | 0.188 | **0.188** | 0.000 | 0.870 |
| `7699563776219024662` | 111 | 0.180 | 0.243 | +0.063 | 0.945 |
| `7620485224450805014` | 140 | 0.386 | **0.300** | -0.086 | 0.932 |
| `7692049436964310294` | 303 | 0.328 | **0.328** | 0.000 | 0.941 |
| `7699013266269326614` | 101 | 0.426 | **0.366** | -0.060 | 0.929 |
| **Mean (12 clips)** | **1,709** | **0.224 (22.4%)** | **0.195 (19.5%)** | **-0.029 (-12.9% rel.)** | **0.934** |

## Summary

- Mean WER across the 12 real Bulgarian TikTok clips improved from **22.4%** (`0.224`) to **19.5%** (`0.195`), well under the `0.40` (40%) calibration ceiling.
- All 20 Phase 2 clips completed `ACQUIRE -> TRANSCRIBE -> OCR` with `0` errors (`20/20` `EXTRACTED`); silent/music-only overlay clips (`3/20`) received `transcript_quality = 0.05` and extracted Cyrillic text via `tesseract (bul+eng)` OCR.
