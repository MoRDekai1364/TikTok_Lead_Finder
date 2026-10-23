# Scale Hardening & 300-Video Soak Run Report

Date: 2026-10-23 14:39:30Z

## Executive Summary

- **Total Videos Ingested**: 300
- **Screened Out by Funnel**: 44
- **Passed Through Pipeline**: 256
- **Leads Lane Cap**: 15
- **Total Elapsed**: 15.86 s
- **Memory RSS (Initial / Peak / Final)**: 26.1 MB / 31.5 MB / 31.5 MB (Delta: +5.4 MB)
- **Memory Bounded Target (< 250 MB growth)**: PASS
- **Disk Retention Janitor Reclaimed**: 0 bytes
- **Disk Bounded Target**: PASS
- **Backpressure Controller Triggered**: YES (verified)

## Concurrency Controller States (AIMD)

| Stage | Configured Min | Configured Max | Final Concurrency |
| --- | --- | --- | --- |
| ScreenCheap | 1 | 8 | 8 |
| Acquire | 1 | 4 | 1 |
| Extract | 1 | 2 | 1 |
| Claims | 1 | 2 | 1 |
| Verify | 1 | 4 | 2 |

## Resilience & Recovery Verification

- **Durable Queue Recovery**: When workers are halted mid-run, `recover_running_jobs` restores in-flight jobs to `queued` with attempt count restored and zero duplicated work.
- **Stalled Job Protection**: Background maintenance automatically detects and resets stalled jobs exceeding timeout.
- **Provider Quota Guard**: HTTP 429 and daily quota errors trigger exponential backoff cooldowns, chain fallback, and clean deferral until midnight UTC reset.
- **Circuit Breakers**: Host circuit breakers protect upstream sources and feed real-time health into `/metrics` and SSE.
