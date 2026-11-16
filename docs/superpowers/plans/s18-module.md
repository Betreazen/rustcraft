# Срез 18, поток 4 «Модули» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** every MISSING / DIFFERS line of `docs/design/06-audit/module.md` becomes DONE, or a recorded engine limit, or is handed to the stream that owns the block (with the rule ready here).

**Architecture:** `rules/src/modules.rs` is the one source of truth: which modules exist, which block takes which, how many sockets a tier has, and the numbers of each module's effect (pure functions with unit tests). Streams 3 (machines), 5 (craft and storage), 1 (node), 6 (workers), 7 (backpack) call these functions in their blocks. This stream also owns the module items, their recipes, and the turret's module effects.

**Tech Stack:** Rust (rules crate, `cargo test --release -q` in `mod/rustcraft/rules`), JSON content mod, `assets/models/icons_mc.py` + `export_mc.py` for item textures.

**Spec:** [docs/superpowers/specs/2026-11-16-handbook-parity-design.md](../specs/2026-11-16-handbook-parity-design.md), master plan [2026-11-16-handbook-parity.md](2026-11-16-handbook-parity.md).

## Global Constraints

- Horizon pinned to `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3. Horizon is not changed.
- The game follows the handbook (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`).
- Shared files (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, lang json): only add own blocks of lines.
- Text in Russian per `CONTRIBUTING.md`, en_us too; commits in English, email `128125070+Betreazen@users.noreply.github.com`, ending `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Recipes use `minecraft:amethyst_shard` for the shard until stream 2 is merged (the integrator switches them).
- Check: `pwsh -NoProfile -File "C:\Claude code projects\horizon-joint-dev\joint.ps1" try -Mod rustcraft -Path "C:\Claude code projects\rustcraft-s18\module" -Owner s18-module`.

---

## Контракт для потоков 3 и 5

Всё в `rustcraft_rules::modules` (файл `mod/rustcraft/rules/src/modules.rs`).

**Модули (`Module`).** К прежним 25 добавлены: `Capacity` (`rustcraft:module_capacity`, теперь в `ITEMS`, вставляется в гнёзда окон), `Stack` / `Stack2` / `Stack3` (`rustcraft:module_stack`, `module_stack_2`, `module_stack_3`: искросталь, кристаллит, звёздная сталь), `Yield` (`rustcraft:module_yield`), `Line` (`rustcraft:module_line`).

**Кто что берёт (`Holder` + `takes`).** Наборы точно по `data.js` (`MODS(...)` у каждой записи):

| Holder | Блоки | Модули |
|---|---|---|
| `Processor` | печь, дробилка, плавильня | скорость, экономия, ёмкость, стак, выход |
| `Pump` | насос | скорость, экономия |
| `Crafter` | автокрафтер | скорость, экономия, ёмкость, стак |
| `Mixer` | смеситель | скорость, экономия, ёмкость, стак |
| `Placer` | установщик, разрушитель | скорость, экономия |
| `Battery` | накопитель | ёмкость накопителя, отдача |
| `Node` | узел | ускорение линии |
| `Miner` | добытчик | скорость, экономия, разведка, площадь, хранилище, стак |
| `Terminal` | терминал склада | стак, книга рецептов |
| `Backpack` | рюкзак | стак |
| `Anchor` | якорь базы | радиус, экономия |
| `Butcher` | забойщик | экономия, радиус |
| `Turret` | турель | 3 эффекта, 4 поддержки, скорость, экономия, радиус |
| `Worker` | шасси (любой инструмент) | улучшения батареи и скорости, хранилище, радиус, коридор, высота, лава, застройка |
| `Util(u)` | машины среза 12 | см. `Util` ниже |

`Util`: утилизатор, зарядник, зачаровательный автомат, археологический стол, строительный автомат — скорость, экономия; дезинтегратор — скорость, экономия, выход; ремонтный стенд — скорость, экономия, ёмкость; кухонный комбайн, варочный куб — скорость, экономия, ёмкость, стак; рыболовная станция — экономия, хранилище; кормушка, стригаль, пасечный сборщик — экономия, радиус; живая жила — скорость, экономия, редкость, выбор руды, пласт. Зарядная плита, конденсатор опыта, голопроектор, портальный док — **модулей нет** (`Holder::has_sockets()` = false: окно гнёзд не рисует).

Что из модулей шасси работает с каким инструментом: `tool_uses(tool: worker::Tool, m: Module) -> bool` (пила, жатка, захват, ковш — радиус, хранилище; бур — коридор, высота, лава, застройка, хранилище; картограф — радиус; улучшения — всем).

**Гнёзда.** `sockets(tier: u8) -> usize` — 3 / 3 / 4 / 4 для ступеней 1–4 (A5); `MAX_SOCKETS = 4` (раскладка окна); `chassis_sockets(tier) -> usize` — шасси ступеней 2 / 3 / 4: 3 / 5 / 7; `MAX_CHASSIS_SOCKETS = 7`. `Holder::has_sockets(self) -> bool`.

**Действие (числа справочника, A9 — одинаковые складываются полностью):**

- `factors(slots, t) -> (work, energy)` — скорость ×2 / ×2,5, экономия ×0,85 / ×0,6 за модуль (без режима; режим умножается отдельно).
- `paced_ticks(step_ticks, slots, t) -> u64` — тики шага машины после модулей: `round(step / work)`, не меньше 1.
- `capacity_slots(slots) -> usize` — +2 ячейки очереди ингредиентов за модуль ёмкости; `capacity_pages(slots) -> usize` — +1 страница рецептов (автокрафтер, комбайн, варочный куб).
- `stack_tier(slots) -> u8` — старшая ступень модуля стака в гнёздах (0 — нет); `machine_stack(slots, base_max) -> u32` — стак ячейки машины/добытчика: 64 → 256 при любом модуле стака (16 → 64, 1 → 1); `backpack_stack(slots, base_max) -> u32` — как склад, но не больше 1024; склад — `storage::STACK_BY_MODULE[stack_tier]` (поле `Store::stack_module`).
- `yield_chance(slots) -> f64` — 0,15 за модуль выхода, не больше 1 (печь, дробилка, плавильня); `yield_lucky(slots, roll: f64) -> bool` — `roll < chance`. Дезинтегратор: `utility::disassembly::share_percent(count(slots, Yield))` — 50 / 60 / 70, не выше 75.
- `storage_factor(slots) -> usize` — ×2 за модуль хранилища (добытчик, рыболовная станция, трюм работника).
- `line_factor(slots) -> f64` — пропускная способность участка узла: 1 + 0,5 за модуль.
- `height_step(module: bool, target: Option<i32>, y: i32) -> i32` — модуль высоты бура: на сколько (−1/0/+1) сменить уровень следующего рельса; без модуля или без заданной высоты 0.

## Задачи

### Task 1: наборы модулей, гнёзда, новые предметы (приоритет — их читают потоки 3 и 5)

**Files:** `mod/rustcraft/rules/src/modules.rs`; `mod/rustcraft/content.rs` (модуль ёмкости регистрируется через `ITEMS`, свой блок строк); `mod/content/assets/rustcraft/{models/item,items,textures/item}/module_{stack,stack_2,stack_3,yield,line}.*`; `assets/models/icons_mc.py`; рецепты `mod/content/data/rustcraft/recipe/module_{stack,stack_2,stack_3,yield,line}.json`; lang ru/en; `rules/src/machines/sheet.rs` только строка `Kind::holder` для новых держателей не трогается (поток 3).

- [ ] Тест RED: `each_holder_takes_exactly_the_handbook_set` (таблица выше, для каждого держателя полный список и «не берёт» остальное), `sockets_by_tier`, `stack_tiers_and_machine_stack`, `yield_chance_stacks`, `capacity_slots_and_pages`, `factors_with_economy_slow_work`, `tool_uses`.
- [ ] GREEN: новые варианты `Module`, `Holder`, функции контракта.
- [ ] Предметы: иконки из `icons.js` (`stack`, `yield`, `line`) в `icons_mc.py`, `py -I assets/models/export_mc.py`, модели и `items/*.json`, имена ru/en.
- [ ] Рецепты по `data.js`: стак `b b/ c /b b` (сплав a2 / a3 / a4 + сундук), выход `axa/xsx/axa` (кристаллит, золото, осколок), линия (искросталь, воронка, осколок).
- [ ] `cargo test`, коммит.

### Task 2: рецепты пласта, высоты, лавы, застройки, книги рецептов по `data.js`

- [ ] пласт `asa/bdb/asa` (a4, осколок, кисточка, алмаз); высота `aca/rsr/aca` (a3, компас, редстоун, осколок); лава `axa/xsx/axa` (a3, обсидиан); застройка (a3, выбрасыватель); книга `aba/tst/aba` (a3, книга, верстак, осколок).
- [ ] Тест крейта правил: рецепт читается и совпадает с `data.js` (`recipes_match_the_handbook` — разбор JSON рецептов модулей из `mod/content`).
- [ ] Коммит.

### Task 3: дезинтегратор берёт модуль выхода

- [ ] `Holder::Util(Disintegrator)` берёт `Yield`, не `Output` (Task 1). `utility.rs:525` считает `Module::Yield`.
- [ ] Самотест `module_disintegrator_yield` (дезинтегратор с модулем выхода отдаёт 60%).
- [ ] Коммит.

### Task 4: экономия ×0,85 работы в машинах среза 12

- [ ] `utility.rs::paced` берёт `modules::paced_ticks` и `factors` (экономия замедляет шаг).
- [ ] Самотест: утилизатор с модулем экономии работает медленнее шага на 1/0,85.
- [ ] Коммит.

### Task 5: модуль хранилища ×2

- [ ] Правило `storage_factor` (Task 1). Добытчик 9 → 18 → 36; трюм работника ×2; буфер рыболовной станции ×2 — подключают потоки 2, 6, 5 (окна больше 3 рядов — их файлы). Здесь: функции `mining::slots_of` и `Worker::hold_size` переходят на `storage_factor`, если окно и блок-энтити это держат; иначе — передать владельцу.

### Task 6: модуль высоты — правило

- [ ] `height_step` с тестами; бур и поле высоты в окне шасси подключает поток 6.

### Task 7: живая жила — редстоун не редкий, цена выбора руды

- [ ] `utility/vein.rs`: редстоун из «редких» убрать (справочник: золото, лазурит, кварц, изумруд, алмаз, древние обломки); `PICK_FACTOR` → 1 (справочник: цена блока не меняется, дорог сам модуль). Тесты в `vein.rs`.

### Task 8: замедление турели ровно 20 / 40 / 60 %

- [ ] Модификатор атрибута `minecraft:movement_speed` моба (`add_multiplied_total` −0,2 × n, не больше −0,6) на 3 с вместо эффекта игры с шагом 15%. Правило `turret::slow_fraction`; провод в `base.rs` в функции эффектов турели.
- [ ] Самотест `turret_slowness_exact`.

### Task 9: луч лечения турели

- [ ] Частицы от турели к лечимому игроку при каждом лечении (`/particle` или API частиц). Правило `turret::beam_points(from, to, step)`.

### Task 10: прочие строки таблицы — передать владельцам с готовым правилом

Печь/дробилка/плавильня (гнёзда, ёмкость, стак, выход) — поток 3; смеситель/автокрафтер/комбайн/куб (ёмкость, стак) — 3/5; ремонтный стенд, рыболовная станция — 5; терминал (стак) — 5; рюкзак (стак) — 7; узел (линия) — 1; 4-е гнездо и ступени — 3; шасси 5/7 — 6; выбор руды/пласта в окне жилы — 5. Отметить здесь, чьё и что готово.

## Пределы движка

(пока нет)
