# Phase 2 OCR Engine Benchmark (`bul+eng`)

## Methodology

- **Dataset**: 20 real Bulgarian TikTok clips (`311` extracted 3-second keyframes at `720p` width, stored in `data/media/<video_id>/frames/`).
- **Command**: `.\target\release\leadfinder.exe benchmark-ocr --limit 20` (raw output saved in `data/phase2_ocr_benchmark.json`).
- **Candidates evaluated**:
  1. `tesseract (bul+eng)` — Tesseract `5.4.0` via `-l bul+eng --psm 6 -c tessedit_create_tsv=1` with `tessdata_fast` (`bul.traineddata` + `eng.traineddata`), cross-frame deduplicated at normalized Jaro/Levenshtein similarity `0.85`.
  2. `paddle_onnx (RapidOCR)` — PaddleOCR ONNX runtime (`rapidocr_onnxruntime`), cross-frame deduplicated at `0.85`.

## Measured Results

| Engine | Clips | Total Frames | Total Elapsed (ms) | Mean Latency (ms/frame) | Deduped Lines | Cyrillic Lines | Cyrillic Share | Mean Confidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `tesseract (bul+eng)` | 20 | 311 | 61,467 | **197.6 ms** | **248** | **234** | **94.4%** | 0.695 |
| `paddle_onnx (RapidOCR)` | 20 | 311 | 489,184 | 1,572.9 ms | 233 | 33 | 14.2% | 0.845 |

## Decision (`D-005`)

**Selected default OCR engine**: `tesseract` (`bul+eng`).

- **8.0x faster** per frame (`197.6 ms` vs `1,572.9 ms`).
- **7.1x higher Bulgarian Cyrillic recall** (`234` vs `33` Cyrillic overlay lines across the 20 clips), because `tesseract` ships native `bul+eng` recognition weights whereas default `RapidOCR` ONNX bundles a Latin/CJK recognition head unless a separate custom Slavic ONNX model + dictionary is supplied.
- Zero Python runtime requirement in production builds: `tesseract.exe` + `bul.traineddata` + `eng.traineddata` are self-contained native files verified and installed automatically by `leadfinder setup-deps`.
