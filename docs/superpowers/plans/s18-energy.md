# Срез 18, поток 2 «Энергия и добыча» — план

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** every MISSING / DIFFERS line of `docs/design/06-audit/energy.md` is DONE in the game or a recorded engine limit.

**Architecture:** logic in `mod/rustcraft/rules` (power, mining, net/energy, new `grow`), wiring in `power.rs`, `generator.rs`, `mining.rs`; content (items, blocks, worldgen, recipes) in `content.rs` and `mod/content`. Visuals through the block-entity renderer (API 71/151) drawn by `power.rs`. Each mechanic gets a self-test scenario in `rules/src/selftest/s18_energy.rs`.

**Tech stack:** Rust (rules crate, Horizon bundle), JSON content mod, `assets/models/export_mc.py` (models without Blender).

Worktree `C:\Claude code projects\rustcraft-s18\energy`, branch `s18/energy`. Checks: `cargo test --release -q` in `mod/rustcraft/rules`; every 3–5 tasks `joint.ps1 try -Mod rustcraft -Path <worktree> -Owner s18-energy`.

## Пределы движка (проверено в SRC)

- Друзы в мире **возможны**: контент-мод может нести свои `worldgen/configured_feature`, `placed_feature` и NeoForge `biome_modifier` `add_features` (SRC `docs/mods.md:458-485`). Делается настоящей генерацией.
- Модель ветряка выше двух блоков нельзя одной моделью: элементы только в −16…32 px (`horizon-assets` `model.rs`, `Element::decode`; `assets/models/README.md`). Верх мачты и ротор рисуются отдельными моделями, сдвинутыми вверх (API 71/151) — это не предел, а способ.
- Сборка бандла проверяется только `joint.ps1 try`; коммиты c817d86…463e912 ещё не прошли try (стенд занят maintenance.txt пакетной сессии).
- (дополняется по ходу)

## Общие файлы: что этот поток добавляет (для интегратора)

- `content.rs`: блоки `rustcraft:druse`, `rustcraft:seed`, предмет `rustcraft:crystal_shard`; шаблон кристалла.
- `rules/src/selftest/mod.rs`: `s18_energy::ALL` в каталоге, методы Bench в блоке «s18 energy».
- `selftest.rs`: реализация этих методов.
- `rules/src/lib.rs`: `pub mod grow;`.
- lang `ru_ru.json` / `en_us.json`: ключи `rustcraft.grow.*`, `rustcraft.structure.*`, `rustcraft.miner.*` (новые), предметы и блоки.
- `hook_block_entity_submit.rs`, `hook_standalone_models.rs`, `anim.rs` (`EXTRA_KEYS`): вызовы рисования кристалла, присосок, ветряка.

## Задачи

Порядок — приоритет из поручения.

### Task 1: предмет «Осколок кристалла» `rustcraft:crystal_shard` и рецепты области
Audit: crystal / solar / battery / miner / vein «Рецепт … осколок — аметист».
- [x] Предмет в `content.rs` (`declare_items`), модель и текстура предмета, имена ru/en.
- [x] Рецепты `crystal`, `solar`, `battery`, `miner`, `vein` берут `rustcraft:crystal_shard` (рецепты других потоков переводит интегратор).
- [x] Самотест: `b.craft` по сетке кристалла даёт кристалл только из осколков мода.
- [x] Commit.

### Task 2: друзы в мире (E5)
Audit: crystal «Осколки выпадают из друз под землёй (E5)».
- [x] Блок `rustcraft:druse` (шаблон аметистового кластера, модель `geode`), лут 2–4 осколка (с шёлковым касанием — сама друза).
- [x] `data/rustcraft/worldgen/configured_feature/druse.json`, `placed_feature/druse.json`, `neoforge/biome_modifier/druse.json` (`add_features`, `#minecraft:is_overworld`, `underground_decoration`).
- [x] Самотест: друза ломается в 2–4 осколка.
- [x] Commit.

### Task 3: выращивание кристаллов (CR2)
Audit: grow — все 4 строки.
- [x] `rules/src/grow.rs`: стадии 0–2 по ходу, 10 минут, 4 ведра (тест: ход за тик, расход, стадия, стоит без раствора, 2–4 осколка).
- [x] Блок `rustcraft:seed` (свойство `stage` 0..2, модели `seed` стадий), ставится кликом осколка по верху бака с раствором.
- [x] Тик: пьёт из бака под собой, растёт; диагноз «Нет раствора в баке».
- [x] Лут: стадия 2 — 2–4 осколка, раньше — 1 осколок.
- [x] Самотест: затравка на баке растёт и пьёт; без раствора стоит с диагнозом.
- [x] Commit.

### Task 4: структура питает вплотную стоящие машины всей отдачей (crystal structure, battery structure)
Audit: structure «Машину вплотную структура питает и без канала»; battery «Вплотную питает машину без магистрали (структура)».
- [x] `net::energy::contact_circuits(groups, around)`: на каждую структуру из 2+ блоков — цепь без предела канала из её блоков и машин вплотную (тест: машина у одного кристалла 2×2 получает 32).
- [x] `power.rs::rescan` добавляет эти цепи.
- [x] Самотесты пары и квадрата: машина вплотную получает всю отдачу.
- [x] Commit.

### Task 5: окно структуры кристаллов и ограничение (E9)
Audit: structure «Окно структуры», «Показывает, что ограничивает: размер, раствор или канал (E9)».
- [x] `power::structure_limit` (тест): Size / Solution / Channel / Shape.
- [x] Кристалл открывает окно (шаблон печи, тип меню `rustcraft:crystal`), лист: кристаллов и форма, отдача, бак раствора, баланс цепи, статус с ограничением.
- [x] Самотест: `b.sheet` у структуры 2×2 с раствором — строки и «Ограничивает размер».
- [x] Commit.

### Task 6: цельная модель структуры, тусклая без раствора; пульс кристалла
Audit: structure «один цельный блок на весь размер», «с раствором светится, без раствора тусклая»; crystal «Пульсирующее свечение».
- [x] Блокстейт кристалла — пустая модель; блок-энтити рисует `crystal` (одиночный, пульс яркости) или `crystalblock` на опорном блоке с размером коробки (тусклый без раствора); остальные члены ничего.
- [x] Сервер пишет `st` (размер), `fed`, `hide` в блок-энтити и шлёт клиентам при смене.
- [x] Самотест: ключи блок-энтити у пары и квадрата.
- [x] Commit.

### Task 7: окна генераторов
Audit: gen_fuel «строка в запасе», «Ход работы»; gen_lava «полоса отдачи и баланс цепи»; battery «Отдаёт 0 из 192».
- [x] `GroupBalance` с запасом и ёмкостью накопителей цепи (тест).
- [x] Окно топливного генератора: «в запасе X из Y Е», полоса «Ход работы».
- [x] Лист лавового: «Даёт X из 40», баланс цепи.
- [x] Лист накопителя: «Отдаёт X из Y Е/с».
- [ ] Самотест: лист лавового генератора и накопителя.
- [x] Commit.

### Task 8: куда складывает добытчик (MN1)
Audit: miner «Куда складывает: сундук → магистраль → блок погрузки».
- [ ] `mining::outlet(chest, net, dock)` (тест порядка).
- [ ] Сундук вплотную — только в него; иначе магистраль; иначе блок погрузки вплотную; иначе «Некуда складывать».
- [ ] Самотест: добытчик с блоком погрузки кладёт в него.
- [ ] Commit.

### Task 9: окно добытчика
Audit: miner «Найдено руды / Осталось», «чипы Куда складывает», «полоса энергии».
- [ ] Подсчёт руды области (найдено = добыто, осталось = найденное сканом вперёд) в правилах (тест).
- [ ] Лист: полоса энергии, чипы «Куда складывает», «Найдено руды», «Осталось».
- [ ] Самотест: лист добытчика.
- [ ] Commit.

### Task 10: модуль стака в добытчике
Audit: miner «Модуль стака».
- [ ] Зависит от потока 4 (`Module::Stack`). Эффект в добытчике: ячейка 256.
- [ ] Commit.

### Task 11: живая жила — режимы, ход, счётчики
Audit: vein «Режимы работы», «Окно: ход работы, Свободных клеток вокруг, Выращено всего».
- [ ] Режим работы в окне жилы и в темпе (тест темпа).
- [ ] Лист: полоса хода, «Свободных клеток вокруг», «Выращено всего».
- [ ] Commit.

### Task 12: присоски `link_*` у кристалла, ветряка и панели
Audit: crystal / solar / windmill «Присоска к магистрали … при подключении».
- [ ] Выгрузка частей `link_<сторона>` отдельными моделями (`export_mc.py`).
- [ ] Сервер пишет маску сторон с корпусом; клиент рисует присоски.
- [ ] Commit.

### Task 13: ветряк в 3 блока высотой
Audit: windmill «Модель в 3 блока высотой».
- [x] Модель в масштабе 1: низ в модели блока, верх мачты и ротор — части, сдвинутые вверх.
- [ ] Commit.

### Task 14: мелочи и самотесты
- [ ] Турбина «ниже по течению»: обосновать или поправить.
- [ ] Самотесты структуры 3×3 и 3×3×3.
- [ ] Окно солнечной панели: решение записать.
- [ ] README: раздел «Что проверить в игре: срез 18» (энергия), строки «Что срезы пока не умеют» этой области (поток 8 правит README — передать текст).
- [ ] Commit.
