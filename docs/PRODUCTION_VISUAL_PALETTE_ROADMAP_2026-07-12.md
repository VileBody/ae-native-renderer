# Production visual palette: audit and reproduction roadmap

Дата проверки: 2026-07-12.

## Короткий вывод

`visual_palette_catalog.md` неполон как инвентаризация production-палитры. Он хорошо описывает старый публичный срез из трех семейств, но не отражает два добавленных 5th-template рендера, фактический legacy-пайплайн, фон `solid_strobe` и целую систему F1-F5 hooks.

В production сейчас обнаружено:

- 7 `subtitles_mode`;
- 6 визуально разных семейств субтитров;
- 3 типа фона (`footage`, `solid`, `solid_strobe`);
- 5 hook-категорий с 28 базовыми cases в `/bigtest`;
- отдельная библиотека semantic effect styles, для которой в просмотренных production artifacts пока нет подтвержденных `effectStyleId`-применений.

Таким образом, три исходно воспроизведенных семейства - это только часть всей монтажной палитры. `legacy_blocks` подтвержден production-аудитом, но исключен из активного Rust roadmap по продуктовому решению.

## Состояние после Rust/JSON-итерации

Старый вывод выше описывает исходную точку аудита. За текущую итерацию закрыт первый production-контур:

- введены versioned request/response/manifest schemas для `JSON -> Rust -> artifacts`;
- `render-cli json` поддерживает `validate`, `inspect`, `render`, stdin и machine-readable stdout;
- `extract-jsx-request` извлекает четыре статических JSX input-массива, Brat/Trendy word timings и F3 palette IDs;
- unknown, approximate и not-implemented operations больше не исчезают молча;
- `outputSpec.frames` рендерит точные номера кадров без сокращения длительности композиции, а frame-locked comparator запрещает temporal offset и resize;
- Brat и Trendy lowering проходят screenshot-проверки на тех же кадрах реальных S3 jobs;
- Brat получил native tracking/leading/full justification, sourceRect centering, Difference/Add, Gaussian Blur и production CC Image Wipe по BPM;
- Trendy получил native base tracking, fill/stroke copies, gradient/shadow, Montserrat Bold и F3 subsets: Lightness Invert shutter, Geometry2/Minimax snap и highlight-selective 4x8 analog grid;
- `solid`/`solid_strobe`, Brat word reveal, F3 `flash_on_cuts`, Invert, Gaussian Blur, CC Image Wipe и blend modes `Normal`/`Add`/`Difference` представлены в общем IR/effect stack;
- простой main-comp audio layer поддерживает AE `startTime`, trim, fade envelope и проверяемый MP4 mux; production Brat `startTime=-53` закреплен E2E-тестом;
- deterministic manifest включает content hashes служебных artifacts и aggregate hash всех PNG frames;
- CLI E2E проверяют stdin/stdout, exit codes, MP4 и одинаковый manifest в разных roots.

Это еще не означает full parity: production jobs возвращают `partial`, пока нужны точные precomp/premult/color-management semantics, Trendy Tracking Amount animator, Motion Blur/Optics Compensation и оставшиеся F3/F1/F2/F4/F5 операции. Multi-track/SFX/TTS audio также пока вне простого mux-среза.

## Откуда взяты данные

Проверка сделана не по локальным примерам, а по production:

1. Loki: `worker-build` и orchestrator hook markers, окна с 2026-01-01 по 2026-07-12.
2. Production Postgres: generation runtime events и статусы версий.
3. Timeweb S3: реальные `output.mp4`, job archives и сгенерированные `render.jsx`.
4. Production `main`: генераторы Brat/Trendy и исходники F1-F5 JSX.
5. Текущий Rust renderer: `crates/render-cli/src/bin/visual_repro_v2.rs`.

Счетчики Loki ниже означают события в логах и могут включать повторные попытки. Они используются как доказательство реального запуска режима, а не как billing-статистика уникальных роликов.

## Реальные subtitle modes

| Mode | Визуальное семейство | Loki `stage2_start` | Текущий статус Rust |
|---|---|---:|---|
| `legacy_blocks` | Legacy, 7 macro-blocks | 18 | Out of scope; только явное распознавание старых jobs |
| `impulse_2nd` | Impulse | 77 | Есть full side-by-side, приблизительно |
| `scenes_3rd` | Scenes TYPE_1..TYPE_6 | 224 | Есть frame-locked snippets и flash |
| `scenes_3rd_single_step` | Тот же renderer, другой planner | 0 в сохраненном окне | Нужен contract test, не новый renderer |
| `template_4th` | Tape | 36 | Есть full side-by-side, приблизительно |
| `trendy_5th` | Trendy | 70 | Native approximate subtitles + gradient + calibrated analog + partial shutter/snap; exact tracking animator/Motion Blur/Optics pending |
| `brat_5th` | Brat | 241 | Native subtitles + strobe + Difference/Add + CC Image Wipe + production-timed audio mux; final raster/precomp parity pending |

Итого: 7 mode IDs, но 6 визуально разных renderer families, потому что `scenes_3rd_single_step` использует визуальную механику `scenes_3rd`.

Production DB дополнительно подтверждает успешные публичные рендеры: Tape 435, Impulse 426, Scenes 365, Brat 30, Trendy 14. Это другой счетчик и другой временной срез, поэтому его не следует складывать с Loki.

## Что уже действительно воспроизведено

| Семейство | Что есть | Остаточный риск |
|---|---|---|
| Impulse | Полный Rust-vs-AE side-by-side, PSNR 30.441 | Метрика еще не означает точную text/effect parity |
| Tape / `template_4th` | Полный side-by-side, PSNR 33.599 | Нужна фиксация покадровых переходов и декодирования |
| Scenes | TYPE_1..TYPE_6, flash, frame-locked comparisons | Это палитра фрагментов, а не полная матрица production jobs |
| Scenes single-step | Общая механика Scenes | Не проверен входной planner contract |
| Legacy | Только AE output/contact | Out of scope; native renderer не планируется |
| Trendy | JSON extraction, base tracking -55, Montserrat fix, fill/stroke, gradient/shadow, calibrated analog stack, Lightness Invert shutter и Geometry2/Minimax snap subsets | Нет Tracking Amount animator, Motion Blur и Optics Compensation |
| Brat | JSON extraction, word reveal, tracking/leading/full justification, strobe, Difference/Add, Gaussian/Minimax/shadow, CC Image Wipe, flash-on-cuts и production-timed audio mux | Остались raster/precomp/premult/color-management расхождения и full-length validation |
| F1-F5 hooks | IDs сохраняются в `visualOps`; F3 flash/analog/shutter/snap native subsets lowered | Остальные primitives/devices в основном еще `not_implemented` |

Последний общий frame-locked монтаж существующих примеров:

`target/visual_palette_compare_subtitles_repro_v11_point_effect_stack/all_rust_vs_ae_frame_locked.mp4`

## Почему Brat, Trendy и hooks исчезали раньше

Текущий standalone renderer извлекает из `render.jsx` статические структуры `projectSpec`, `compsSpec`, `footage_layers` и `text_layers`.

У production Brat и Trendy:

- `projectSpec.subtitlesMode` выставлен корректно;
- `text_layers = []`;
- слои субтитров создаются позже императивным injected JSX;
- F1-F5 аналогично мутируют уже созданную AE-композицию injected JSX-кодом.

Следовательно, добавление очередного `match_name` в effect parser не закрывало проблему: Rust сначала должен был получить сами динамически созданные layers, precomps, shapes, masks, keyframes и audio operations.

Реализованный первый контракт:

1. Request сохраняет JSX inputs top-level и добавляет нормализованный `visualOps`.
2. Для старых archives extractor распознает Brat/Trendy/F3 injected blocks без выполнения JavaScript в Rust.
3. Неизвестная injected operation дает явный capability report и может заблокировать render через policy.
4. Следующий production-шаг - писать тот же request JSON рядом с JSX прямо в build worker, чтобы extractor оставался migration path, а не основным parser.

## Недостающие subtitle families

### P1. Brat 5th

Уже реализовано: word timings, lowercase, Arial Narrow, полные Cyrillic glyph contours, 2 слова x 4 строки с odd-tail merge, word jump reveal, tracking `-20`, leading `130`, layer scale `80%`, full justification, sourceRect centering, Minimax/Gaussian/shadow, strobe solids, Difference text over B/W background, Add flash layers, production CC Image Wipe по BPM и F3 `flash_on_cuts` 25% -> 0 за 0.6 s.

На чистых CC Image Wipe кадрах `14/15/17/19` текущий frame-locked результат: `MAE 1.698`, `PSNR 34.976`, `SSIM 0.920`. На основном контрольном наборе текстовый bbox Rust `195,767,690x187` против AE `195,768,691x176`.

Осталось:

- убрать остаточные 11 px расхождения raster bbox по высоте и проверить glyph hinting/baseline на всех строках;
- воспроизвести точную precomp boundary, premultiplied alpha и color-management semantics AE;
- проверить full-length MP4 + audio output, а не только sparse PNG и synthetic audio E2E;
- ускорить blur/glow/full-length render и собрать полный frame-locked side-by-side.

### P1. Trendy 5th

Уже реализовано приблизительно: uppercase layer per word, next-word exposure, Montserrat Bold, font-size fit, base tracking `-55`, 100x400 scale, cap-like Y offset, отдельный black stroke-under-fill layer, white-to-gray alpha gradient, Drop Shadow и F3 analog stack (`Posterize Time`, red scanline/wave, two Glows). Исправлен rasterizer bug, который съедал stems у `B/P` в Montserrat Bold.

F3 дополнительно воспроизводит шесть shutter chunks по `0.1001001 s`, Lightness Invert на четных chunks, анимированный horizontal Gaussian как временную замену Motion Blur и cut-relative snap layers с Geometry2/Minimax. Для клипнутых shutter highlights реализована измеренная по AE 4x8 CRT-сетка: на кадре `24` red mean/std Rust `158.7/29.6` против AE `159.9/28.7`; frame `MAE` снизился с `32.24` до `16.66`.

Осталось:

- Tracking Amount animator `7 -> -1` с AE Bezier easing; base tracking `-55` уже native;
- точный sourceRect/cap baseline fit для кириллицы и длинных слов;
- единый TextDocument stroke-under-fill вместо двух raster layers;
- точные Sapphire `S_Gradient`/`S_DropShadow` параметры;
- настоящий ADBE Motion Blur для shutter/snap и Optics Compensation для snap; Invert, Geometry2, Minimax и временный Gaussian уже native;
- полный frame-locked side-by-side по всем словам/cuts; multi-track audio нужен только для jobs, где он реально присутствует.

### Out of scope. Legacy blocks

Это не один простой subtitle preset, а последовательность из 7 macro-blocks:

1. `INTRO_ZOOM`
2. `WALTZ_BRIDGE`
3. `SOLO_PHOTO`
4. `BABY_BUILD`
5. `GLITCH_CRESCENDO`
6. `DUAL_TRUTH`
7. `FINALE`

В исходниках подтверждены word reveal animators, adjustment layers, Geometry2 gestures, Motion Blur, Turbulent Displace, Posterize Time, Minimax, Box Blur, отдельный `Mine` precomp, две его копии, outline/fill dual layer, fade tails и transform animation.

Native-реализация этих блоков не входит в активный roadmap. Старый mode остается распознаваемым, чтобы API не отбрасывал его молча и возвращал явный `not_implemented` для архивных jobs.

## Production backgrounds

По generation runtime events:

| Background | Использований |
|---|---:|
| `footage` | 243 |
| `solid` | 46 |
| `solid_strobe` | 13 |

Для solid background встречались green 27, black 12, white 7. `solid_strobe` уже подтвержден внутри реального Brat job pack и должен стать отдельным reusable timeline primitive.

## F1-F5: фактически использованная монтажная палитра

| Категория | Loki markers | Содержание |
|---|---:|---|
| F1 Sound | 9 | Audio hook, ducking, опциональный subtitle, light + transition combo |
| F2 Object | 44 | Shape до drop, light в drop, transition после drop |
| F3 Effect | 125 | Hooks, transitions и extras |
| F4 Motion | 42 | Gesture overlay, BPM/drop-relative timing |
| F5 Cognition | 48 | TTS, ducking, subtitle injection и effect combo |

### F2 shapes

Все пять реально встречались в production:

- `square` - 11;
- `star1` - 10;
- `ellipse` - 9;
- `rhomb` - 7;
- `star2` - 7.

### F3 effect primitives

Hooks:

- `hook_light`;
- `shutter_effect`;
- `flash_slow_shutter`;
- `negative_zoom` встречается в production logs, хотя отсутствует в текущем видимом picker.

Transitions:

- `snap_wipe`;
- `minimax`;
- `invert_flash`;
- `extract_flash`;
- `flash_on_cuts`;
- `layer_shake`.

Active extras:

- `xerox`;
- `analog_glitch`;
- `neon_extract`;
- `old_camera`.

Historical/retired assets `pixel_grain` и `warm_map` следует поддерживать только при необходимости обратной совместимости со старыми archives.

### F4 motion devices

- `head` - 12;
- `pinch` - 11;
- `holdfinger` - 8;
- `tap` - 6;
- `swipe` - 5.

### F5 cognition devices

- `punchline` - 13;
- `question_to_track` - 13;
- `inverse_lyric` - 9;
- `lyric_echo` - 8;
- `missing_word` - 5.

F5 нельзя считать только визуальным overlay: production flow также создает TTS, приглушает track, клонирует стиль subtitle и затем собирает light/transition combo.

## Roadmap

### P0. Зафиксировать ground truth

1. Заменить старый трехсемейный inventory на 7 modes / 6 renderer families.
2. Завести machine-readable manifest: job id, S3 keys, mode, hook config, fps, resolution, duration, drop/cut timings и ожидаемые capabilities.
3. Зафиксировать один полный golden на каждое активное subtitle family, кроме out-of-scope Legacy, и минимум один golden на каждый F1-F5 primitive/device.
4. Использовать существующую production `/bigtest` matrix из 28 cases как основу hook battery.
5. Генерировать frame-locked side-by-side на одном и том же timestamp: Rust слева, AE справа. Не сравнивать соседние shots или разные моменты.
6. Добавить contact sheets вокруг каждого in/out/drop/cut и автоматический unsupported-capability report.

Definition of done: ни один production job не проходит с молча отброшенным layer/effect/injected block.

### P1. Закрыть оставшиеся subtitle families

Порядок:

1. Brat - первым, потому что он чаще всего встречался в последних Loki logs и открывает precomp/box-text/strobe primitives.
2. Trendy - вторым, чтобы закрыть tracking, non-uniform text scaling и Sapphire effects.
3. Scenes single-step - contract/golden test без отдельного renderer fork.

Definition of done для каждого семейства:

- fps, resolution, frame count и audio fragment совпадают;
- subtitle in/out не расходится более чем на один frame;
- bbox, baseline, line breaks, fill/stroke и reveal проверены на transition frames;
- есть полный frame-locked side-by-side и отдельные diffs для subtitle-only и footage-only;
- golden запускается повторяемо одной командой.

### P2. Реализовать F3 как общий effect kernel

F3 надо делать раньше остальных hook-категорий, потому что F1, F2 и F5 переиспользуют его light/transition операции.

Порядок:

1. `flash_on_cuts` готов native subset; далее `invert_flash`, `extract_flash`, `minimax`.
2. `snap_wipe` имеет Geometry2/Minimax subset; далее Motion Blur, Optics Compensation и `layer_shake`.
3. `shutter_effect` имеет точные chunks/Invert и Gaussian approximation; далее Motion Blur, `hook_light`, `flash_slow_shutter`, затем historical `negative_zoom` hook.
4. `analog_glitch` имеет calibrated posterize/CRT/glow stack; далее `xerox`, `neon_extract`, `old_camera`.

Ожидаемые общие primitives: adjustment layers, Gaussian blur, CC Image Wipe, Levels/Invert/Extract, repeated Minimax, temporal trails, mattes/masks, blend modes, transform sampling и cut-relative windows.

### P3. Собрать composite hooks

1. F2: пять vector shape overlays + F3 light/transition chain.
2. F4: пять gesture overlays, contour paths, transforms и BPM scaling относительно drop.
3. F1: audio insert, duck envelope, optional subtitle, F3 chain.
4. F5: TTS lifecycle, track ducking, subtitle-style clone/injection и F3 chain.

Definition of done: 28-case battery дает отдельные AE/Rust outputs, frame-locked comparisons, audio-envelope diff и сводный отчет без unsupported operations.

### P4. Semantic effect-style library

Старый каталог отдельно описывает декларативные stacks:

- BCC Lens Blur;
- Curves;
- Geometry2;
- `S_BlurMotion`;
- Deep Glow;
- Directional Blur;
- `ftg_al16_default_v1`;
- `txt_soft_v1`;
- `txt_punch_v1`;
- `txt_drop_v1`.

В просмотренных production artifacts не найдено подтвержденного применения через `effectStyleId`. Поэтому это реальный, но более низкий приоритет, чем F1-F5, которые точно использовались. Для этой библиотеки нужно специально сгенерировать маленькие AE goldens на checkerboard/text/footage, а не ждать случайный production job.

### P5. Parity hardening

- AE text metrics: glyph raster, sourceRect, tracking, paragraph justification, cap-height alignment;
- cubic Bezier temporal easing;
- generic source-frame timing без per-video remap catalog;
- полный precomp graph, collapse behavior, masks/mattes и blend modes;
- premultiplied alpha и color-management contract;
- subtitle-only, effects-only, footage-only и final-composite diffs;
- CI regression matrix по 5 активным families и 28 hook cases; Legacy в нее не входит.

## Уже скачанные production goldens

| Family | Job id | Локальные artifacts |
|---|---|---|
| Impulse | `e73f911f128c47e0b8369628e4a1e202` | `out/visual_tool_examples_s3/impulse_2nd/` |
| Scenes | `0069eaf78a5b4032b258d6b660743273` | `out/visual_tool_examples_s3/scenes_3rd/` |
| Tape | `06425e5a4d9d4836b67337bc31fac029` | `out/visual_tool_examples_s3/template_4th/` |
| Brat | `a15d4c02d67843b787402bb27aeb5830` | `out/visual_tool_examples_s3/brat_5th/` |
| Trendy | `9ef2717145c04318927ca738f5882541` | `out/visual_tool_examples_s3/trendy_5th/` |
| Legacy | `60215839b239420ab933419a3a6f79b0` | `out/visual_tool_examples_s3/legacy_blocks/` |

Для Scenes также сохранены три full-palette job packs в `out/visual_tool_examples_s3/scenes_full_palette/`.

## Рекомендуемая следующая итерация

JSON boundary, sparse exact-frame rendering, Brat/Trendy lowerers и первый F3 vertical slice уже готовы. Следующий срез, без Legacy:

1. добавить Tracking Amount animator и единый TextDocument fill/stroke для Trendy;
2. реализовать настоящий Motion Blur и Optics Compensation, затем закрыть `invert_flash`, `extract_flash`, `minimax`, `layer_shake` и light hooks;
3. довести Brat/Trendy precomp, premultiplied alpha, sourceRect/raster и color-management parity;
4. отрендерить оба полных production jobs с audio и собрать frame-locked side-by-side на всех in/out/cut точках;
5. собрать F2/F4 и multi-track/SFX/TTS-зависимые F1/F5 hooks поверх общего F3 kernel;
6. научить production worker писать request JSON рядом с JSX и оптимизировать blur/glow для release/full-length рендера.

До exact parity каждый неперенесенный элемент продолжает явно возвращаться в `capabilities.not_implemented` или `capabilities.approximate`.
