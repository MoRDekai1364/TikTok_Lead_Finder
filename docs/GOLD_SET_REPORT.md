# Phase 5 Gold Set Report

> [!WARNING]
> The fixture `fixtures/gold_set/claims.json` is **synthetic** (`"synthetic": true`). Names, institutions, passages and URLs are fictional. This report measures the code-enforced forcing rules and the local model's passage-grounded reasoning offline. It does **not** measure agreement with published fact-checks. See D-009.

Run: `leadfinder eval-gold-set fixtures/gold_set/claims.json` with `BgGPT-Gemma-3-4B-IT-Q4_K_M` (prompt `prompts/verify_v1.txt`). Raw output: `data/phase5_gold_set_eval.json`.

## Results

| Metric | Target | Measured |
| --- | --- | --- |
| Claims | 30 to 50 | 40 |
| Traps | >= 10 | 12 across 6 categories |
| Traps receiving SUPPORTED | 0 | 0 |
| Trap pass rate (not SUPPORTED and expected verdict) | 100% | 12 / 12 |
| Forcing-rule compliance (every verdict backed by qualifying evidence) | 100% | 40 / 40 |
| Recall on CONTRADICTED | >= 70% | 15 / 15 |
| Verdict agreement with fixture labels | reported | 36 / 40 (90%) |
| First-try schema validity on model calls | >= 90% | 28 / 28 |
| `disinformation` labels emitted | 0 | 0 |

## Disagreements

| Id | Expected | Got | Claim |
| --- | --- | --- | --- |
| 17 | ALREADY_CHECKED | CONTRADICTED | България ще приема мигранти от Австрия срещу частично влизане в Шенген. |
| 18 | ALREADY_CHECKED | CONTRADICTED | Въвеждането на еврото автоматично ще удвои цените на основните храни в магазините. |
| 19 | ALREADY_CHECKED | CONTRADICTED | Съюзът задължава българите да ядат брашно от насекоми и щурци без етикетиране. |
| 20 | ALREADY_CHECKED | CONTRADICTED | Украинското зърно, внасяно в България, съдържа опасни нива на пестициди и радиация. |

All four disagreements are claims whose passages come from a fact-check source. The model answered CONTRADICTED where the fixture expects ALREADY_CHECKED. Both outcomes carry a fact-check link, and neither is SUPPORTED. A prompt tweak could close this gap but would need a gold-set re-run.

## Trap categories

| Category | Claims | Passed |
| --- | --- | --- |
| correct_statistic_wrong_source | 2 | 2 |
| fabricated_attribution | 2 | 2 |
| genuine_quote_false_context | 2 | 2 |
| no_retrievable_evidence | 2 | 2 |
| old_video_presented_as_new | 2 | 2 |
| satire_or_parody | 2 | 2 |

## Limits

- Synthetic passages are short and unambiguous, so these numbers are an upper bound on real performance.
- Live retrieval was not recorded. Factcheck.bg returned 403 on re-test (D-008) and a live `verify-video` run on a real claim retrieved no usable passages and correctly returned UNVERIFIABLE with zero evidence strength.
- Agreement with published verdicts, and a replay against recorded HTTP responses from real archives, remain open and are listed in `docs/OWNER_CHECKLIST.md`.
