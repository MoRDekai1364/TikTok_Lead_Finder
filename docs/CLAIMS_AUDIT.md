# Phase 4 Claims Audit

Generated from `data/claims_review_after_filters.json`, which is exported from the `claims` table after `extract-claims-batch --limit 20` on the 20 stored Bulgarian TikTok clips.

## Method and limits

- Reviewer: the implementing agent, not the owner. Plan Phase 4 requires an owner review of 50 claims. That review is still open.
- The corpus yields only **39 claims**, not 50. Several clips are music, ads or very short, so they produce none after filtering.
- A claim is marked OK when it is a faithful, standalone, meaningful statement taken from the clip's own text, with a sensible type. It is BAD for lyrics, rhetorical questions or slogans, garbled ASR where the meaning is lost, OCR noise, or fragments.
- Attribution actor errors are noted in the comment column but a correct claim with a wrong actor is still marked OK only when the claim text itself is faithful.

## Result

| Metric | Target | Measured | Status |
| --- | --- | --- | --- |
| Claims reviewed | 50 | 39 | NOT MET (corpus too small) |
| Correct extractions (agent review) | >= 80% | **26 / 39 = 66.7%** | NOT MET |
| Owner review | required | not done | OPEN |

Before the quality filters the same corpus produced 64 claims with a visibly worse noise rate (lyrics, OCR garbage, fragments, hashtag lists). The filters in `config/default.toml` `[claim_quality]` removed 25 claims. They did not reach the 80% target.

Remaining error classes: garbled ASR (6), nostalgic or rhetorical copy (5), lyrics or banter (2), wrong or unsupported actor (2, overlapping). These come from Whisper `small` on noisy or dialect speech and from the 4B model, not from the extraction code.

## Claim-by-claim review

| # | Video | Type | Actor | Claim | Review | Note |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `7584058076953627926` | attribution | Европейската комисия/said | Европейската комисия потвърди, процесът е необратим, независимо кое управлява, независимо какво искат хората. | OK | faithful quote-style claim |
| 2 | `7584058076953627926` | factual | - | Правителството пада. Протести по улиците. А решението вече е взето. | OK | checkable assertion in video |
| 3 | `7584058076953627926` | factual | - | България влиза в Еврозоната на 1 януари 2026 година. | OK | dated factual claim |
| 4 | `7584058076953627926` | factual | - | Еврото идва. Въпросът е само един, кои всъщност взема решението. | BAD | rhetorical statement, not a claim |
| 5 | `7692049436964310294` | attribution | Радев/said | Некога не съм казвали, не сме казвали, че ще намалим цените с 15%. | OK | denial attributed to the speaker in the clip |
| 6 | `7695048513306119457` | attribution | Китай и като Щатите/said | Търмто обесни, че ако се въведе цифров долар, свободата на американците ще бъде взе. | BAD | garbled ASR; actor 'Китай и като Щатите' is wrong |
| 7 | `7695048513306119457` | attribution | Европейският съюз/proposed | Темата беше подхвална тощо в 2019 година, но 2027 година започна стартирането на този проект, който означава пълен контрол. | BAD | garbled ASR; actor 'Европейският съюз' not supported by transcript |
| 8 | `7695048513306119457` | attribution | Емил Ангелов 🇧🇬🇧🇬🇧🇬🇧🇬/said | В момента редица държави от Европа, Словения, Словакия, Унгария, сега в момента Австрия и Швеция се борят за вписването на кеше на парите в брой в конституция. | OK | claim faithful; actor is the creator, not a third party |
| 9 | `7620485224450805014` | attribution | Слави Василев/said | Казвам го в ефир, без да преувеличавам, че хората могат да забележат това, което казваме така или иначе. Така че ефектите на това България да влезне в еврозоната сега се ... | OK | claim faithful; actor is the speaker's own statement |
| 10 | `7620485224450805014` | attribution | Слави Василев/said | Аз със сметката за ток, което плащаме, по чити толкова в евро, колкото съм плащал във лева. | OK | claim faithful despite dialect spelling |
| 11 | `7693938711473523990` | attribution | Рада Лайкова/said | Оважаями граждани, ако търсите виновника, погляднете към Брюксел. | BAD | rhetorical blame, no checkable content |
| 12 | `7693938711473523990` | attribution | Рада Лайкова/said | България е шампионат на Европа по поскъпва ненажилищата. 14% и отгоре за последната година. | OK | statistical claim, meaning recoverable |
| 13 | `7693938711473523990` | attribution | Рада Лайкова/said | Първо парите. Поглянете моята страна България. Влязахме в Еврозоната, а на народа беше отказан референдум. | OK | factual claim about referendum |
| 14 | `7588087577186454806` | factual | - | Без шум, стои четиридесет години история. | BAD | nostalgic advertising copy |
| 15 | `7588087577186454806` | factual | - | С този лев сме купували първия си хляб. | BAD | nostalgic advertising copy |
| 16 | `7588087577186454806` | factual | - | С него сме получавали първите си джобни. | BAD | nostalgic advertising copy |
| 17 | `7564423898557664534` | attribution | Кристин Лагард/said | Кристин Лагард обяви: „Край на колебанията — време е България да се присъедини към еврото.“ | OK | quote with named speaker |
| 18 | `7627561583845100822` | attribution | Костадин Костадинов/said | Не, не, не ще ви кажа защото, защото курсът, което ние ще направиш бъде едно камено, да няма никаква рязък. | BAD | garbled ASR, meaning lost |
| 19 | `7627561583845100822` | attribution | Костадин Костадинов/said | Курсът го определя нашата банка тощата. От там на сет не вече, когато имаме вълтен борд и се договорим со срещната страна. | BAD | garbled ASR, meaning lost |
| 20 | `7627561583845100822` | attribution | Костадин Костадинов/said | Ние ще възстановим нова вълта. Макар ще звучи доста странно, но хем ще възстановим български илиев. Хем на практика той ще бъде нова вълта. | OK | prediction by named speaker |
| 21 | `7627561583845100822` | attribution | Костадин Костадинов/said | В този случай, ами ние определим и съм изплюнасте на вълтата, защото ние направиш да са задем. 2 евро е горе до 4 лева. Така. | BAD | garbled ASR, meaning lost |
| 22 | `7600093189558652182` | factual | - | няма ми си пари, баратонки, няма ми! | BAD | song lyric |
| 23 | `7685833348014509334` | factual | - | Протестите срещу управлението на Радев ЗАПОЧНАХА! | OK | factual claim |
| 24 | `7685833348014509334` | factual | - | С всеки изминал ден проблемите се трупат, цените летят нагоре, народът обеднява, а властта все едно я няма! | OK | borderline, general claim about prices |
| 25 | `7694316075466362134` | attribution | Господин Ганев/said | Господин Ганев, според правилника, за да говорите от място трябва да имате физически увреждане, неумствени. | OK | procedural claim about parliamentary rules |
| 26 | `7690626350159318305` | factual | - | Да, веднага правим гръцки лок. | BAD | banter or lyric, not a claim |
| 27 | `7699563776219024662` | attribution | ПИК/said | По информация на ПИК, тя е била изблъскана от охранител при опит да зададе въпрос на Михаила Додсова и е паднала на тротоара. | OK | reported claim with media source |
| 28 | `7699563776219024662` | attribution | БЛИЦ/said | Журналистката Соня Кулутоклиева е в Болница след инцидент пред централата на прогресивна България в София. | OK | reported claim with media source |
| 29 | `7699563776219024662` | attribution | ПИК/said | Медията съобщава за травма на рамото и мозъчно сътресение. | OK | reported claim with media source |
| 30 | `7697234501155491074` | attribution | Чикагото/said | Чикагото заяви, че Илияна Йотова ще стане президент. | OK | reported statement |
| 31 | `7697234501155491074` | attribution | Чикагото/said | Но според него, резултатът вече бил предопределен и нямало да зави си от волята на българските граждани. | OK | reported statement |
| 32 | `7697234501155491074` | attribution | Чикагото/said | В разговора на меси имената на Андрей Гюров и Джордж Сорос. | OK | reported statement |
| 33 | `7697234501155491074` | attribution | Чикагото/said | Отправи политически обвинения и използва квалификации, които предизвикаха остър реакция. | OK | reported statement |
| 34 | `7697234501155491074` | factual | - | Водещата прекрати интервюто с категоричната позиция. Няма да терпим подобно отношение. | OK | factual claim about interview |
| 35 | `7699013266269326614` | factual | - | 20. 20 декора. 20 е такива чистите декора. Без пъти. 20 декора. | BAD | garbled ASR fragment |
| 36 | `7699013266269326614` | factual | - | 20. 20% ДДС на цялата продукция, което ще е хвърлим там. | OK | statistical claim |
| 37 | `7699013266269326614` | factual | - | Съм платил 20% ДДС. На всички материали и сорев ни, които са в Каралтап. Торове препарати. Капково полети лен. Нафта 20%. ДДС акцист близо 20%. Гърция 6%. ДДС на торове и... | OK | statistical claim |
| 38 | `7699013266269326614` | factual | - | Именно затова създаваме Българската обединена народна кооперация и разширяваме магазините „ХОРА ОТ НАРОДА“ – за да осигуряваме пазар на родните производители и да изгражд... | OK | factual claim |
| 39 | `7698066283052436769` | opinion | SIMONA/said | SIMONA казва, че Цветозар Захариев е много добър изпълнител, страшно талантлив и има огромно бъдеще. | OK | opinion correctly typed and low check-worthiness |

## Other Phase 4 acceptance measurements

| Metric | Target | Measured | Source |
| --- | --- | --- | --- |
| First-try JSON validity across the 20 clips | >= 90% | 95% (19 of 20, one repaired retry, 0 failed) | `data/phase4_claims_batch_20.json` |
| Attribution trap cases split into asserted and attributed parts | 100% | 100% (8 of 8) | `data/phase4_attribution_traps_eval.json` |
| People resolved | informational | 50 resolved, 12 Wikidata public figures | `people` table |
