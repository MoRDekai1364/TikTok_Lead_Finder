# Phase 6 Funnel Report

> [!WARNING]
> `fixtures/phase6_replay.json` is a **synthetic** replay set: 300 generated videos with generated claims, verdicts and keep labels. It is not recorded TikTok data and the labels are not the owner's. The numbers below check the mechanics of screening, clustering, scoring, lanes and the cap. Precision at 25 is not an accuracy estimate. See D-011.

Run: `leadfinder replay-funnel fixtures/phase6_replay.json`. Raw output: `data/phase6_replay_result.json`.

## Screening

| Gate | Videos dropped |
| --- | --- |
| below_min_views | 44 |
| below_min_views_per_hour | 12 |
| duplicate_phash | 12 |
| not_relevant | 29 |
| **passed** | 203 of 300 |

## Lanes after ranking

| Lane | Videos |
| --- | --- |
| leads | 25 |
| leads_queued | 47 |
| clustered | 69 |
| already_checked | 25 |
| low_quality | 37 |
| failed | 0 |
| screened_out | 0 |

## Acceptance

| Check | Target | Measured |
| --- | --- | --- |
| Videos in the Leads lane | <= 25 | 25 |
| Overflow held behind the cap | queued, pullable | 47 in `leads_queued` |
| Seeded narratives (10 groups of 3 to 6 videos) | each in one cluster | 10 of 10 in one cluster, 0 split, 0 merged with another narrative |
| Seeded perceptual-hash copies (12) | dropped before extraction | 12 of 12 dropped |
| Precision at 25 against labelled fixture | reported | 23 of 25 = 0.92 (synthetic labels) |

## Known weaknesses

- 14 unrelated videos were absorbed into seeded narrative clusters because the generated claims share one sentence template. Real claims vary more, but over-merging risk remains. The threshold is `funnel.cluster_similarity_threshold` in `config/default.toml`.
- 52 clusters hold more than one video and the largest holds 8, mostly template collisions in the fixture.
- The search budget gate reads `provider_usage` rows named `search`. Nothing writes them yet, so the gate never trips until Phase 7 adds search accounting.
- Creator expansion has eligibility only: leadfinder expansion-candidates lists creators with at least 3 flagged videos in 30 days and depth below 2. Fetching their videos is not wired because automated discovery is NO-GO (D-004).
