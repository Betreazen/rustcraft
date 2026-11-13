# Что может мод для Horizon: исследование осуществимости RustCraft

Дата: 2026-11-10. Метод: только чтение исходников и документации. Ничего не собиралось и не запускалось, поэтому ни один вывод не проверен в игре.

## Как читать документ

Каждое утверждение помечено источником:

- **[И]** проверено в исходниках движка: указан файл и строка.
- **[Д]** сказано в документации движка: указан файл и строка. В игре не проверялось.
- **[П]** предположение или вывод автора, прямого подтверждения нет.

Оценки трудоёмкости грубые, получены чтением кода: **S** (дни), **M** (1–2 недели), **L** (3–6 недель), **XL** (дольше или с неясным исходом).

Сокращения путей:

| Сокращение | Полный путь |
| --- | --- |
| `SRC` | `%APPDATA%\horizon-launcher\native\source\7e4e421224743469035ba0c34fc21e569e64a17d` |
| `GAME` | `SRC\crates\horizon-client\src\game` |
| `MIX` | `%APPDATA%\horizon-launcher\cargo\git\checkouts\horizon-mixins-71247e19d95fed41\5a64af9` |
| `POC` | `C:\Claude code projects\minecraft-site-pr\mods\path_of_craft_tree` |

## Где лежат исходники движка локально

| Что | Где |
| --- | --- |
| Исходники Horizon на коммите `7e4e421` (нетронутые, с каталогом `docs\`) | `%APPDATA%\horizon-launcher\native\source\7e4e421224743469035ba0c34fc21e569e64a17d` |
| Исходники Nightly `fdc87ae` (база `nightly-2026-11-08`) | `%APPDATA%\horizon-launcher\native\source\fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca` |
| Исходники с уже применёнными миксинами последней сборки. Меняются каждой сборкой, как эталон не годятся | `%APPDATA%\horizon-launcher\native\stage` |
| Инструмент horizon-mixins (README и `docs\`) | `%APPDATA%\horizon-launcher\cargo\git\checkouts\horizon-mixins-71247e19d95fed41\5a64af9` |
| Каталог именованных хуков | `SRC\native-hooks.json` (432 хука, 177 хелперов) |
| Хуки без номера API (161 фрагмент) | `SRC\tools\native-hook-fragments\` |
| Лог последней сборки | `%APPDATA%\horizon-launcher\native\build.log` |

В `%LOCALAPPDATA%\Horizon` исходников движка нет: там только готовые программы и каталог `crates\horizon-prepare`.

Главные документы: `SRC\docs\native-mods.md` (522 КБ, полный справочник хуков), `SRC\docs\mods.md` (контент-моды: блоки, предметы, меню), `SRC\docs\modpacks.md:190-297` (реестр блоков), `SRC\docs\api\mod-files.md`, `MIX\README.md`, `MIX\docs\mixin-api.md`, `MIX\docs\spongepowered-feature-gaps.md`.

Версия API хуков: на `7e4e421` это 413 [И] (`GAME\native_hooks.rs:1175`), на Nightly `fdc87ae` 411 [И] (строка 1170 того же файла в каталоге `fdc87ae…`).

## Главное в двух абзацах

Исходное допущение «мод не умеет добавлять блоки, предметы и сущности» неверно. В Horizon есть готовый слой для технических модов, сделанный по образцу NeoForge: свои блоки и предметы из кода, состояния блока, формы коллизии, block entity с сохранением и тиками, слоты, баки для жидкостей, буфер энергии, свои типы меню и экраны, свои пакеты, прогрузка чанков, отрисовка block entity и своих моделей, хуки генерации и спавна. Мод `path_of_craft_tree` этим слоем просто не пользовался.

Настоящие ограничения другие. Нет своих классов сущностей: тип сущности можно создать только по четырём ванильным шаблонам, вагонетки среди них нет. Нет дополнительных слотов инвентаря игрока. Нет своих жидкостей в мире и вёдер с ними. Нет наклона камеры больше 10 градусов. Реестр состояний блоков ограничен 2^20. Block entity рисуется только в 64 блоках от игрока. Текстуры и модели мода живут в отдельном контент-моде, а не в бандле. Серверный движок собирается только на той же ОС, где работает сервер. Публичного примера мода с машинами в официальном репозитории нет, поэтому весь «машинный» API не обкатан.

## 1. Новые блоки

**Вывод: есть готовый механизм.** Свой блок, свои состояния, форма не в целый куб, block entity с сохранением, тики трёх видов. Ограничения: блок всегда копирует поведение ванильного шаблона, реестр состояний конечен.

### Регистрация

- Блок объявляется в коде хуком `native_blocks(blocks)` или файлом контент-мода `data/<id>/horizon/block/*.json`. Формат один и тот же: `horizon_mods::content::Block` [И] (`GAME\native_hooks.rs:3220-3238`; структура в `SRC\crates\horizon-mods\src\content.rs:290-327`).
- Хук спрашивают один раз при старте на каждой стороне, до загрузки ресурсов [И] (`GAME\native_hooks.rs:3231-3233`).
- Блок обязан назвать `template`: ванильный блок, чей класс поведения, свойства состояния и формы он наследует [Д] (`SRC\docs\modpacks.md:209-219`). Произвольного «класса блока» с собственными методами нет. Поведение сверх шаблона добавляется хуками ниже.
- Настраиваются: твёрдость, взрывоустойчивость, свет, инструмент, трение, цвет на карте, звуки, `random_ticks`, `render_shape`, `occludes` [И] (`content.rs:77-95`).

### Состояния

- Свои свойства состояния: `state_properties` типа `boolean` или `enum`, плюс `default_state` [Д] (`SRC\docs\mods.md:143-166`).
- Состояния блока равны состояниям шаблона, умноженным на комбинации объявленных свойств [Д] (`mods.md:145-151`).
- **Предел: все состояния всех блоков мира должны уместиться в 2^20 (1 048 576).** Ваниль занимает 35 723. Превышение отклоняет загрузку контента [Д] (`mods.md:165-166`, `modpacks.md:281-284`).
- Состояние при установке задаёт `native_state_for_placement` [И] (`native_hooks.rs:1776`), реакцию на соседей `native_update_shape` [И] (`native_hooks.rs:1658`), выживание блока `native_can_survive` [Д] (`native-mods.md:909-918`).
- Свойство своего блока меняет `native_set_block_property`, блок целиком `native_set_block` [Д] (`native-mods.md:230-232`, `260-264`).

### Форма коллизии не в целый куб

- `shapes` (контур прицела) и `collision_shapes` (коллизия движения) задаются таблицей по свойствам состояния. Ящики в координатах блока, не больше 16 на форму [Д] (`mods.md:194-224`), проверка предела [И] (`content.rs:186-192`).
- Пустая коллизия: `"collision_shapes": { "default": [] }` [Д] (`mods.md:211`).
- Отдельные формы для игрока, предмета и моба: `outline_context_shapes`, `collision_context_shapes`; `interaction_shapes` меняет грань попадания [Д] (`mods.md:581-609`).
- **Форма зависит только от состояния блока.** От мира, соседей, позиции и данных block entity она зависеть не может [Д] (`native-mods.md:5105-5110`).
- Готовый шаблон соединяющегося в 6 сторон блока: `minecraft:chorus_plant` с `connects_to` и `thickness` 2–14 пикселей; контур и коллизия строятся как у `PipeBlock`: ядро и рукав к каждой соединённой стороне [Д] (`mods.md:281-294`).

### Block entity, сохранение, тики

- `native_block_entity(block, entity)` даёт блоку block entity при любом шаблоне [И] (`native_hooks.rs:3292-3306`). Движок сам удаляет её при разрушении блока; поршень такой блок не двигает.
- Данные: `id`, `nbt` (сохраняется с чанком), `items` (слоты), `data` (числа для меню, не сохраняются) [И] (`SRC\crates\horizon-client-render\src\chunk\block_entity.rs:111-129`).
- Тик на сервере: `native_block_entity_ticks` включает, `native_block_entity_tick(pos, entity)` выполняет в ванильном порядке [И] (`native_hooks.rs:3319`, `3526-3538`). На время своего тика block entity изъята из мира.
- Клиенту уходит то, что вернул `native_block_entity_update_tag`; повторная отправка по `native_block_entity_sync(pos)` [И] (`native_hooks.rs:3548-3564`), [Д] (`native-mods.md:830-834`).
- Тики самого блока: запланированный `native_tick_block` [И] (`native_hooks.rs:1736`), случайный `native_random_tick` [Д] (`native-mods.md:315-330`), планирование через `native_schedule_block_tick` [Д] (`native-mods.md:307-309`).
- Редстоун: `native_block_signal`, `native_is_signal_source`, `native_neighbor_signal`, компаратор `native_analog_output` [Д] (`native-mods.md:353-356`, `891-893`, `1032`).
- Соседнюю block entity своего мода меняет `native_with_mod_block_entity` [Д] (`native-mods.md:230-232`).
- Привязанные к миру данные вне block entity: `AttachmentType<T>` на `Level`, `Chunk`, `BlockEntity`, `Entity`, `Player`, с кодеком и синхронизацией клиентам [Д] (`native-mods.md:4972-5053`).

### Реестр и синхронизация клиент↔сервер

- Ванильные id состояний не сдвигаются. Состояния модов дописываются после ванильных, моды идут в порядке id, поэтому одинаковый набор модов даёт одинаковые id. Сохранения хранят имена блоков, id на диск не попадают [Д] (`modpacks.md:260-261`).
- Реестр по сети не передаётся. Он строится на каждой стороне из одного и того же кода и файлов [Д] (`native-mods.md:1223`).
- Клиента с другим набором блоков, предметов, эффектов, типов сущностей или меню сервер не пускает и называет причину [Д] (`mods.md:104-105`, `modpacks.md:294-295`).
- Контент-моды с `side = "both"` дополнительно сверяются по id, версии и SHA-256 файла на канале `horizon:mod_list` [Д] (`mods.md:107-114`).
- **Native-моды в этот список не входят** [Д] (`mods.md:113-114`). Версию кода RustCraft придётся сверять своим приветствием, как делает `path_of_craft_tree` [И] (`POC\mixins\path_of_craft_tree\glue.rs:22`, `33-34`).
- Мир, открытый без мода: поведение для блоков не определено, в документации «спросим позже» [Д] (`modpacks.md:296-297`). Риск потери блоков при запуске сервера без мода.

## 2. Предметы, рецепты, теги, лут

**Вывод: есть готовый механизм.**

- Предметы: `native_items(items)` или `data/<id>/horizon/item/*.json` [И] (`native_hooks.rs:3239-3248`; структура `content.rs:374-388`). Предмет копирует компоненты ванильного шаблона, любой компонент 26.3 можно задать или убрать [Д] (`mods.md:226-240`).
- **Поведение, привязанное к id ванильного предмета, не копируется**: копия трезубца не бросается [Д] (`mods.md:311-314`). Поведение своего предмета пишется хуками: `native_use_item`, `native_use_item_on`, `native_inventory_tick`, `native_on_equip`, `native_hurt_enemy`, использование с удержанием `native_use_duration` и `native_release_using` [Д] (`native-mods.md:211-214`, `358-361`, `1009`, `1187`).
- Свои типы компонентов: `native_component_types` [И] (`native_hooks.rs:3216-3219`), [Д] (`mods.md:250-255`).
- Предмет-контейнер энергии, жидкости и предметов: `native_item_energy_buffer`, `native_item_fluid_tank`, `native_item_container` (до 256 слотов) [И] (`native_hooks.rs:3487-3525`).
- Рецепты крафта и плавки: обычные файлы 26.3 в `data/<ns>/recipe/` контент-мода [Д] (`modpacks.md:192-207`). Из кода их можно подать через `native_data_pack_files`: ключи `recipe/…`, `tags/…`, `loot_table/…`, `advancement/…` [И] (`native_hooks.rs:2511`), [Д] (`native-mods.md:1108`).
- Рецепты своих типов для машин (`mymod:crusher`): файл остаётся загруженным, мод читает его через `native_recipes_of_type`, `native_recipe_ingredient`, `native_recipe_result`. Игра их сама не крафтит и в книге рецептов не показывает [Д] (`native-mods.md:344-348`). Клиентам рецепты доставляет `native_datapack_sync` [Д] (`native-mods.md:1145`).
- Удаление и замена чужих рецептов: `recipe_filter` [Д] (`native-mods.md:590-619`).
- Теги: файлы в своём пространстве имён, добавки к ванильным `data/minecraft/tags/` и общим `data/c/tags/` [Д] (`mods.md:397-431`).
- Лут: свои таблицы файлами, изменение чужих `native_loot_table_load`, правка выпавшего `native_loot_drops`, свои функции и условия лута [Д] (`native-mods.md:637-660`, `3659-3683`).
- Вкладки творческого инвентаря: `data/<id>/horizon/creative_tab/*.json` [Д] (`mods.md:296-309`).
- Достижения со своими триггерами: `native_trigger_criterion` [Д] (`native-mods.md:3887-3914`).

## 3. Сущности, вагонетки, монорельс

**Вывод: своего класса сущности нет. Монорельс возможен только через инъекции в код вагонеток (оценка L–XL) или обходом через моба-сиденье (оценка M–L, риск рывков).**

### Что есть

- Тип сущности объявляется только по шаблонам `minecraft:snowball`, `minecraft:villager`, `minecraft:wandering_trader`, `minecraft:zombie` [И] (`content.rs:414-419`, проверка `content.rs:907-909`). Вагонетки, лодки и «пустой» сущности среди шаблонов нет.
- Живой тип проверен только юнит-тестами: «в запущенной игре не пробовали» [Д] (`mods.md:366-367`). Бросаемый тип на выделенном сервере «не проверен» [Д] (`mods.md:342-343`).
- Сущности в движке лежат в типизированных списках (`self.carts`, `self.boats`, `self.mobs`, `self.cushions`), вид вагонетки задан перечислением `CartKind` [И] (`GAME\minecarts.rs:30`, `GAME\native_mounts.rs:26-37`). Общего реестра сущностей с пользовательским тиком нет.
- Серверные хелперы: `native_summon` (любая сущность по NBT), `native_move_entity` и `native_set_display` для display-сущностей с плавным переходом на клиенте [Д] (`native-mods.md:3699-3716`); мобы `native_spawn_mob`, `native_teleport_mob`, `native_push_mob` [Д] (`native-mods.md:3584-3600`); скорость `entity.set_velocity` (для игрока возвращает false) [И] (`GAME\native_entity_motion.rs:61-85`).
- Посадка: `native_mount(entity, vehicle)`, `native_dismount`, `native_eject_passengers`. Сажают игрока или моба на лодку, вагонетку или моба. Событие `native_entity_mount` можно отменить [И] (`GAME\native_travel.rs:156`, `179`, `209`), [Д] (`native-mods.md:2962-2969`, `1199`).
- Перенос транспорта с седоками: `native_teleport_vehicle(dimension, id, pos)`; правка движения `native_vehicle_moved_v2` (можно изменить `to` или отменить) [И] (`native_travel.rs:112`, `GAME\native_hooks.rs:7556`, вызов в `GAME\lan_vehicles.rs:150`).
- Свои цели ИИ для моба: `native_register_goals` [Д] (`native-mods.md:3604-3655`).

### Как устроены ванильные вагонетки и рельсы

| Что | Где |
| --- | --- |
| Вагонетки целиком: движение по рельсам и склонам, сход с пути, толкание, варианты, сохранение, отрисовка | `GAME\minecarts.rs` (3539 строк), описание в строках 1–6 |
| Структура вагонетки: позиция, скорость, `no_gravity`, `custom_display` (свой блок внутри), `custom_offset` | `minecarts.rs:97-173` |
| Один тик вагонетки | `tick_cart`, `minecarts.rs:1299` |
| Движение по рельсу, старая и новая физика | `move_along_track` (`:752`), `move_along_track_new` (`:1020`) |
| Сход с рельс | `come_off_track`, `minecarts.rs:1236` |
| Цикл по вагонеткам без седока | `tick_carts_now`, `minecarts.rs:1806-1878` |
| Тик вагонетки с игроком, посадка игрока на `SEAT` выше вагонетки | `tick_cart_riding`, `minecarts.rs:1945-1992` |
| Вагонетки игроков сервера | `GAME\lan_vehicles.rs:1-11` |
| Интерполяция вагонетки на удалённом клиенте | `GAME\remote.rs:308`, `319` |
| Отрисовка вагонетки и блока внутри | `cart_geometry`, `minecarts.rs:2609` |
| Блоки рельсов: формы, соединения, питание | `SRC\crates\horizon-world\src\level\rails.rs` (756 строк): `RailShape` (`:17`), `rail_place` (`:228`), `rail_neighbor_changed` (`:334`) |
| Мобы-седоки | `GAME\passengers.rs` |
| Подушка-сиденье (сущность, привязанная к блоку) | `GAME\cushions.rs` |

Важные детали:

- Вагонетку с игроком тикает не общий цикл, а код седока: в `tick_carts_now` она пропускается [И] (`minecarts.rs:1822-1827`). Поэтому хук `native_entity_tick_pre` её не слышит [Д] (`native-mods.md:1237`). Единая точка для своей физики: `tick_cart`, её вызывают оба пути [И] (`minecarts.rs:1842`, `1966`).
- На сервере вагонетка игрока движется самим сервером по нажатиям игрока [И] (`lan_vehicles.rs:8-9`). Значит, своя физика нужна в первую очередь на сервере, клиент интерполирует.
- В начале каждого тика вагонетки применяется гравитация [И] (`minecarts.rs:1316-1317`). Рельс ищется как блок под вагонеткой [И] (`minecarts.rs:1318-1328`). Езды по стене и потолку в ванильной модели нет.

### Варианты монорельса

**Вариант А: инъекция в `tick_cart`.** [П]

- Cancellable HEAD в `horizon_client::game::minecarts::Game<'_>::tick_cart`: для вагонеток RustCraft выполнять свою физику вдоль рельса на любой грани, для остальных пропускать в ванильный код. Пометить свои вагонетки можно полем `custom_display` или вложением `Holder::Entity`.
- Отмена с возвратом значения поддержана: хелпер возвращает `ControlFlow<R>`, где `R` тип результата цели (у `tick_cart` это `bool`) [Д] (`MIX\docs\advanced-injections.md:18`, `125-128`).
- Инъекции в сырые цели работают: `path_of_craft_tree` уже ставит HEAD на `boats::tick_boat` и `motion::MotionWorld::local_tick` [И] (`POC\mixins\path_of_craft_tree\horizon-mixin.toml:1687`, `1654`).
- Дополнительно понадобятся: своя высота сиденья вместо `SEAT` [И] (`minecarts.rs:1969`), своя отрисовка вместо `cart_geometry`, интерполяция на удалённом клиенте.
- Цена: привязка к приватному коду, который меняется от коммита к коммиту. Оценка L–XL.

**Вариант Б: моб-сиденье и display-сущность.** [П]

- Невидимый моб без ИИ (`NativeSpawn { no_ai, invisible, silent, no_gravity }`), игрок посажен через `native_mount`, сервер каждый тик двигает его `native_teleport_vehicle`. Внешний вид рисует `native_mob_submit` или `item_display`.
- Только документированные хелперы, сырых целей нет.
- Риски: рывки из-за телепортации каждый тик; античит выделенного сервера видит состояние `riding` [Д] (`SRC\docs\anticheat.md:74`); моб остаётся мобом (урон, толкание, выгрузка). Оценка M–L, нужен прототип плавности.

### Посадка игрока и камера

- Камеру можно поставить в любую точку: `native_camera_pose(partial, position, yaw, pitch)` [И] (`native_hooks.rs:3125-3139`). **Крена в этом хуке нет.**
- Крен есть только в `native_camera_shake`, и он ограничен 10 градусами [Д] (`native-mods.md:3932-3937`), [И] (`native_hooks.rs:12327-12332`).
- Переворот камеры вниз головой возможен только инъекцией RETURN в функцию, которая строит матрицу тряски [П].
- Модель сидящего игрока движок рисует вертикально. Перерисовать игрока целиком можно через `native_player_submit` с `drawn = true` [Д] (`native-mods.md:3851-3853`), это отдельная большая работа.
- **Рекомендация [П]:** на потолке делать подвесную вагонетку (игрок висит под рельсом, головой вверх), на стене ехать вдоль стены в вертикальном положении. Тогда крен камеры и переворот модели не нужны.

## 4. Модели и рендер

**Вывод: есть готовый механизм для блоков, предметов и block entity. Свой рендер моба и слои на игроке есть. glTF нет.**

### Форматы

- Блоки и предметы: ванильные JSON-модели 26.3 с `elements`, блокстейты с `variants` и `multipart` [И] (`SRC\crates\horizon-assets\src\blockstate.rs:45`, `93-118`; `SRC\crates\horizon-assets\src\bake.rs:223`).
- Загрузчики NeoForge: `neoforge:composite` и `neoforge:obj` (модели OBJ с MTL) [И] (`SRC\crates\horizon-assets\src\model.rs:320`, `332`; `bake.rs:304`).
- glTF, GeckoLib, Blockbench-формат: в коде движка не найдены [И] (поиск по `SRC\crates`: совпадения только с OBJ и в анализаторе чужих jar).
- Модели сущностей: слои в коде, как `createBodyLayer()` в NeoForge. `native_layer_definitions` регистрирует `LayerDefinition` из `MeshDefinition` и `CubeListBuilder`, `native_bake_layer` печёт дерево `ModelPart`, `native_draw_model_part` рисует [И] (`native_hooks.rs:3262-3272`; `GAME\native_block_entity_draw.rs:727`, `750`), [Д] (`native-mods.md:3858-3883`). Номер API не присвоен, нужна привязка к коммиту.
- Связанные текстуры без кода: правила OptiFine CTM в `assets/<id>/optifine/ctm/` [Д] (`native-mods.md:4427-4428`).

### Свои модели и текстуры

- Ресурсы лежат в контент-моде: `assets/<id>/blockstates`, `models`, `textures`, `lang`, `sounds.json` [Д] (`modpacks.md:192-207`). Блоки из `native_blocks` получают эти файлы, если включён контент-мод с тем же id; иначе выглядят как шаблон [Д] (`native-mods.md:1223`).
- В самом бандле каталог `data/` вкомпилируется в игру (`native_data!`) и ресурсами игры не становится [Д] (`native-mods.md:709-719`).
- Модели-детали без блока: `native_standalone_models` и `native_draw_model` [И] (`native_hooks.rs:3249-3261`; `native_block_entity_draw.rs:699`).
- Текстуры GUI: `assets/<ns>/textures/gui`, рисуются `native_sprite` [Д] (`native-mods.md:234-236`). Картинка из кода: `native_mod_image(width, height, rgba)` [И] (`native_hooks.rs:10854`).
- Лимит атласа блоков: 16 384 слоя, ваниль занимает 1 250 [Д] (`modpacks.md:287-288`).

### Произвольная геометрия блока

Два пути, их можно сочетать:

1. **Модель в меше чанка.** JSON или OBJ-модель, выбор деталей через `multipart` по состояниям. Рисуется на любой дальности, стоит дёшево. Геометрия зависит только от состояния блока.
2. **Отрисовка block entity.** `native_block_entity_submit` раз в кадр для каждой block entity в поле зрения [И] (`native_hooks.rs:3595-3624`). Примитивы: `native_draw_quad`, `native_draw_cube`, `native_draw_block`, `native_draw_model`, `native_draw_item`, `native_draw_text` [И] (`native_block_entity_draw.rs:551`, `582`, `630`, `663`, `699`, `776`).
   - `draw.keep()` кэширует геометрию до изменения состояния, данных или света, без вызовов мода каждый кадр [И] (`native_block_entity_draw.rs:210`), [Д] (`native-mods.md:1023-1028`).
   - `draw.spin(...)` вращает деталь силами движка [И] (`native_block_entity_draw.rs:228`).
   - **Рисуется только в пределах 64 блоков** [Д] (`native-mods.md:920-922`), [И] (`native_hooks.rs:3597`).
   - `render_shape: "ENTITYBLOCK_ANIMATED"` убирает модель блока из меша чанка [Д] (`mods.md:131-135`).

Применение:

- **Труба с 4 каналами и соединениями в 6 сторон.** Полное кодирование «канал × сторона» в состоянии даёт 2^24 комбинаций и не помещается в предел 2^20 [П]. Рабочая схема [П]: в состоянии 6 булевых «есть соединение со стороны» и 4 булевых «канал присутствует» (1024 состояния), модель через `multipart`, форма из ядра и шести рукавов (7 ящиков из 16 допустимых). Подробности по каналам на каждой стороне хранить в block entity и дорисовывать вблизи через `native_block_entity_submit` с `keep()`.
- **Рельс на стене и потолке.** Блок со свойством грани и свойством формы, пустая коллизия, модель с поворотами в блокстейте, выживание через `native_can_survive` [П].
- **Жидкость в баке.** `native_draw_fluid_box` рисует воду и лаву; свои жидкости мод рисует сам [И] (`GAME\native_fluid_render.rs:13-22`).

### Анимации, сущности, броня

- Анимация block entity: поза по `partial` и `game_time` в `native_block_entity_submit`, либо `spin`.
- Свой рендер моба: `native_mob_submit(mob, draw, drawn)` с отключением ванильной модели [И] (`native_hooks.rs:3673`), [Д] (`native-mods.md:3836-3850`).
- Игрок: `native_player_submit` (заменить модель) и `native_mob_layer` (добавить геометрию поверх). Требуют включить `native_player_render_listened` [И] (`native_hooks.rs:3696`, `3714`), [Д] (`native-mods.md:1243`).
- **Геометрия слоя крепится к системе координат сущности, а не к костям модели.** Положение головы и цикл шагов доступны числами, анимацию конечностей мод считает сам [Д] (`native-mods.md:3854-3857`).
- Броня с ванильной формой: предмет с компонентом `equippable` и своим `asset_id`. Движок читает `assets/<ns>/equipment/*.json` из всех пространств имён, включая контент-моды; текстуры `textures/entity/equipment/humanoid/` и `humanoid_leggings/` [И] (`GAME\armor_stands.rs:126`, `147-159`).
- Рука от первого лица: `native_render_hand` умеет только скрыть руку, нарисовать замену нельзя [Д] (`native-mods.md:999`).
- Геометрия в мире вне блоков (трос крюка, подсветка сети): `native_world_primitives` (линии, ящики, текст) [Д] (`native-mods.md:4458-4473`) и `native_world_submit` (текстурированные квады) [И] (`native_hooks.rs:3650-3657`; `GAME\native_world_draw.rs:13-22`).
- Свой проход рендера с wgpu: `native_world_drawn` [Д] (`native-mods.md:4432-4454`).

## 5. Интерфейсы

**Вывод: есть готовый механизм. Свои экраны, контейнеры со слотами, HUD, схема сети.**

### Контейнер машины со слотами

Цепочка:

1. Тип меню в контент-моде: `data/<id>/horizon/menu/<name>.json` с числом слотов 1–256, числом data-слотов 0–256 и списком слотов-результатов [Д] (`mods.md:373-393`), [И] (`content.rs:473-485`).
2. Контейнер блока: `native_block_entity_container` возвращает `ModContainer` (слоты, доступ по граням, входы и выходы для воронок, имя меню) [И] (`native_hooks.rs:3349-3364`; `block_entity.rs:252-258`). Без своего типа меню доступны 11 ванильных меню [И] (`block_entity.rs:295-307`).
3. Раскладка экрана: `native_screen` возвращает размер, координаты слотов и позицию инвентаря [И] (`native_hooks.rs:3746`; `SRC\crates\horizon-client\src\mod_api.rs:398-412`).
4. Рисование: `native_screen_draw` своими текстурами, клики `native_screen_click`, подсказки `native_screen_tooltip` [Д] (`native-mods.md:234-240`).
5. Кнопки на сервере: `native_menu_button` и `native_menu_button_v2` (с предметом на курсоре) [Д] (`native-mods.md:238-240`, `828`).
6. Правила слота: `native_menu_slot_rules` (что принимает и сколько вмещает) [Д] (`native-mods.md:4617-4627`).
7. Shift-клик: `native_menu_quick_move`. В документации прямо назван случай терминала хранилища, чьи предметы лежат в других блоках [Д] (`native-mods.md:4638-4657`).
8. Данные при открытии (уровень машины, режим): `native_menu_extra_data`, до 32 КиБ [Д] (`native-mods.md:4599-4613`).
9. Приём от воронок: `native_container_accepts` [И] (`native_hooks.rs:3565-3594`).

Ограничения:

- Data-слоты 16-битные [Д] (`mods.md:386`).
- Клиент не предсказывает клики в своём меню: результат приходит от сервера [Д] (`mods.md:392-393`).
- Меню открывается при использовании блока, только если шаблон блока сам открывает меню (сундук, печь) [И] (комментарий `native_hooks.rs:3352-3357`). Проверка в игре нужна: неясно, осталось ли требование после API 129.
- Слотов-фантомов для фильтров нет [П]. Фильтр делается кнопками: клик на экране, `native_menu_button` на сервере, список в `nbt`, иконки через `native_item`.
- Поле ввода текста: `GAME\native_text_field.rs` (фрагмент `screen_text_field`, без номера API) [И].

### Свой экран без блока

- `native_mod_screen_open(mod_id)` открывает экран поверх мира. Мод рисует его в `native_screen_render_post` и получает мышь, колесо и клавиши через хуки экрана [И] (`native_hooks.rs:11036`), [Д] (`native-mods.md:1125`).
- Так сделано дерево в `path_of_craft_tree`: открытие [И] (`POC\…\glue.rs:681`), проверка «экран наш» [И] (`POC\…\ui.rs:37-39`), рисование `native_fill`, `native_quad`, `native_item` [И] (`ui.rs:67`, `93`, `106`), хуки мыши [И] (`POC\…\horizon-mixin.toml:238-280`). По спецификации мода дерево из 1078 узлов собрано и окно работает (`C:\Claude code projects\minecraft-site-pr\docs\superpowers\specs\2026-11-10-path-of-craft-tree-design.md:525`).
- Серверное меню на ванильном сундуке без клиентского кода: `native_open_menu`. Клики ничего не двигают, мод обрабатывает их сам [Д] (`native-mods.md:4393-4410`).
- Объёмная сцена из блоков внутри экрана (предпросмотр мультиблока): `native_draw_scene` [Д] (`native-mods.md:4333-4344`).

### HUD, мини-карта, схема сети

- HUD: `native_hud(batch, corners)` [И] (`native_hooks.rs:2642`); слои вокруг ванильных `native_gui_layer_pre` и `native_gui_layer_post` [Д] (`native-mods.md:1203`); поверх всего `native_gui_render_post` [И] (`native_hooks.rs:2652`).
- Мини-карта движка с настройками мода: `native_minimap` [И] (`native_hooks.rs:3212-3215`; `mod_api.rs:357-367`).
- Своя карта или схема: снимки клиентского мира `native_client_chunks` для фоновых потоков [Д] (`native-mods.md:4241-4270`), вывод картинки через `native_mod_image`, либо рисование примитивами, как в дереве `path_of_craft_tree`.
- Подсказка предмета: `native_tooltip`; полоска под предметом (заряд): `native_item_bar` [Д] (`native-mods.md:207-209`, `877-881`).

## 6. Сеть

**Вывод: есть готовый механизм.**

- Байты между серверной и клиентской половиной мода идут по одному каналу `horizon:mod_net` [Д] (`native-mods.md:4316-4329`).
- Отправка: `native_net_send_to_player(player, channel, data)` на сервере, `native_net_send_to_server(channel, data)` на клиенте [И] (`GAME\native_net.rs:174`, `230`).
- Приём: `native_net_from_client` и `native_net_from_server` [И] (`native_hooks.rs:8397`).
- Пределы: 30 000 байт на сообщение, 32 сообщения за тик в каждую сторону на игрока. Имя канала `mod_id:name` [Д] (`native-mods.md:4321-4325`).
- Каждое сообщение слышат все моды, канал надо проверять. Данные клиента ничем не проверяются [Д] (`native-mods.md:4326-4327`).
- Сообщение клиента доходит до хука на следующем тике, ответ уходит в конце тика [Д] (`native-mods.md:4328-4329`).
- Автоматическая синхронизация данных: вложения с `sync: true` [Д] (`native-mods.md:5027-5039`) и update tag у block entity.
- Чтение любых пакетов сервера на клиенте: `native_client_packet`; отправка произвольного пакета: `native_send_packet` [Д] (`native-mods.md:7`, `4370-4380`).

Как в `path_of_craft_tree`: каналы `path_of_craft_tree:hello`, `allocate`, `toggle`, `respec`, `tree`, `state` [И] (`glue.rs:22-27`); приём на сервере через хук `net_from_client` [И] (`horizon-mixin.toml:66-77`); отправка [И] (`glue.rs:330`, `701-718`); сервер отключает клиента без приветствия за 15 секунд [И] (`glue.rs:33-34`).

## 7. Жидкости

**Вывод: баки и трубы с водой и лавой — готовый механизм. Свои жидкости только внутри баков. Своей жидкости в мире и ведра с ней нет.**

- Баки на block entity: `native_block_entity_fluids` объявляет до 16 баков, ёмкость до 1 000 000 мБ, доступ по граням. Содержимое в `nbt` под ключом `fluids` [И] (`native_hooks.rs:3365-3379`; `block_entity.rs:265-273`).
- Операции: `native_fluid_tanks`, `native_fluid_insert`, `native_fluid_extract`, `native_move_fluids`. Работают и с котлом [Д] (`native-mods.md:841-854`).
- Свои жидкости: любой id вне `minecraft`, одинаковые id сливаются. Мод сам рисует их в block entity [Д] (`native-mods.md:4120-4125`).
- **«В мире только ванильные жидкости: нет ни источников, ни текучих блоков жидкости мода, ни вёдер с ней»** [Д] (`native-mods.md:4124-4125`).
- Фильтр бака: `native_block_entity_accepts_fluid` [И] (`native_hooks.rs:3380-3407`).
- Предмет-контейнер жидкости: `native_item_fluid_tank` [И] (`native_hooks.rs:3487-3497`).
- Течение в точке: `native_fluid_flow` (водяное колесо) [Д] (`native-mods.md:4113-4116`).
- События: растекание `native_block_spread`, ведро `native_fill_bucket` и `native_bucket_empty`, изменение уровня `native_fluid_level_change` [Д] (`native-mods.md:250-254`, `1318`, `1412`).
- Насос, забирающий воду и лаву из мира: прочитать блок через `level.block_state`, убрать источник `native_set_block`. Готового хелпера «выкачать источник» не найдено [П].

## 8. Игрок

| Задумка | Вывод | Доказательства |
| --- | --- | --- |
| Дополнительные слоты инвентаря | **Не найдено, риск.** Инвентарь игрока фиксирован: 41 слот | [Д] `native-mods.md:1189`. Хуков расширения в `GAME\native_inventories.rs` и `SRC\native-hooks.json` нет [И] |
| Рюкзак | Есть механизм: предмет со слотами до 256, содержимое в компоненте `minecraft:container` | [И] `native_hooks.rs:3514-3525`; `GAME\native_item_container.rs:1-9`. Экран рюкзака пишется самим модом [П] |
| Размер стака | Есть механизм: `native_modify_default_components`, `max_stack_size` до `i32::MAX`. Клиент без мода покажет не больше 99 | [Д] `native-mods.md:4661-4678` |
| Полёт в стиле творческого | Есть механизм: `native_set_flight(uuid, allow, flying)`, скорость `native_set_flying_speed`, включение и выключение слышит `native_player_toggle_flight`. Разрешение не сохраняется и сбрасывается при возрождении и смене режима | [И] `native_travel.rs:269`; [Д] `native-mods.md:2970-2976`, `3949-3954`, `1251` |
| Реактивная тяга | Есть механизм на клиенте: `native_push_player([x,y,z])`, каждая компонента не больше 4 блоков за тик. Сервер этого толчка не знает, античит видит необъяснённое движение | [И] `GAME\native_motion.rs:30`; [Д] `native-mods.md:3943-3948` |
| Урон от падения | `native_living_fall` меняет или отменяет | [Д] `native-mods.md:1213` |
| Притягивание крюком | Через тот же `native_push_player` каждый тик. Серверу задать скорость игрока нельзя: `set_velocity` для игрока возвращает false | [И] `native_entity_motion.rs:61-64`; [Д] `native-mods.md:3948` |
| Снаряд крюка | Тип сущности по шаблону `minecraft:snowball` со своим предметом; точку попадания даёт `native_projectile_impact`, попадание можно отменить | [Д] `mods.md:330-347`, `native-mods.md:1197` |
| Трос крюка | `native_world_primitives.line` или `native_world_submit` | [Д] `native-mods.md:4458-4473`; [И] `native_hooks.rs:3650-3657` |
| Атрибуты | `native_attribute`, `native_set_attribute_base`; модификаторы через компонент `attribute_modifiers` предмета. Номер API не присвоен | [И] `GAME\native_player_state.rs:315`, `334`; [Д] `mods.md:232-240` |
| Эффекты | `native_add_effect`, `native_remove_effect`; свои эффекты объявляются в контент-моде, поведение хуком `native_apply_effect_tick` | [И] `native_player_state.rs:267`, `277`; [Д] `mods.md:316-328`, `native-mods.md:856-864` |
| Свои клавиши | Есть механизм: мод перечисляет клавиши в `NativeMod.keys`, игрок переназначает их в Options > Mods, выбор читает `native_mod_key`, нажатия приходят в `native_key(key, pressed)` | [И] `mod_api.rs:416-425`; `native_hooks.rs:2832`, `11249-11260`. В `path_of_craft_tree`: `glue.rs:28`, `horizon-mixin.toml:192` |
| Клавиши взаимодействия | `native_interaction_key` (атака, использование, выбор блока) можно отменить; ввод движения меняет `native_movement_input` | [Д] `native-mods.md:1201`; [И] `native_hooks.rs:2943` |
| Тик игрока | `native_player_tick` на сервере, `native_inventory_tick` для каждого предмета | [Д] `native-mods.md:293-296`, `358-361` |

Античит: на выделенном сервере работает предсказание движения, его можно выключить (`enabled = false`) [Д] (`SRC\docs\anticheat.md:13`). Джетпак и крюк на клиентских толчках потребуют либо настройки античита, либо режима `native_set_flight` [П].

## 9. Мир

| Задумка | Вывод | Доказательства |
| --- | --- | --- |
| Свои руды | Есть готовый механизм без кода: `worldgen/feature` и `placed_feature` в контент-моде, подключение к ванильным биомам файлом `neoforge/biome_modifier` с `add_features` | [Д] `mods.md:458-484`; [И] `SRC\crates\horizon-client\src\mod_content_biomes.rs:146`, тест `SRC\crates\horizon-client\src\mod_content_tests.rs:3652-3664` |
| Своя генерация чанка | `native_chunk_generate`. Работает на потоках генерации, хелперы игры там недоступны. Номер API не присвоен | [И] `SRC\crates\horizon-world\src\worldgen\native_generate.rs:132`; [Д] `native-mods.md:3370-3383` |
| Свои биомы, шум | Не переносятся | [Д] `modpacks.md:279-280` |
| Спавн мобов в области | `native_potential_spawns(dimension, category, pos, entries, canceled)`: убрать записи или очистить список. **Вызывается только при загруженном trait-моде** | [И] `native_hooks.rs:6339`; `GAME\natural_spawning.rs:456-459`; [Д] `native-mods.md:1742` |
| Спавн, другие точки | `native_mob_spawned` (слушает), `native_entity_join_level` (можно отменить любую сущность, тоже за проверкой trait-мода) | [И] `native_hooks.rs:8750`; `GAME\native_entities.rs:473`; [Д] `native-mods.md:755-762` |
| Прогрузка чанков | Есть готовый механизм: `native_force_chunk(owner, [x,z], hold)` и `native_force_chunk_in` для названного измерения. Чанк грузится с кольцом 5×5 и тикает | [И] `GAME\native_chunk_tickets.rs:22`, `32`; [Д] `native-mods.md:4297-4314` |
| Тики в непрогруженных чанках | **Не найдено.** Block entity тикает только в загруженном чанке | [П], по `native_hooks.rs:3526-3528` и `minecarts.rs:1808-1809` |

Ограничения прогрузки чанков [Д] (`native-mods.md:4301-4314`):

- Билеты не сохраняются. Список надо хранить самим и брать заново в `native_level_load`.
- Незер и Энд открываются, только когда туда кто-то вошёл; билет до перезапуска там не действует, пока уровень не откроется.
- Выделенный сервер с паузой при пустом сервере перестаёт тикать и удерживаемые чанки.
- Рассчитано на десятки чанков, не на тысячи.

Обход для механизмов вне прогрузки [П]: считать сеть (энергия, предметы, жидкости) как граф в памяти мода, хранить его во вложении `Holder::Level` или своих файлах и тикать из `native_level_tick_post`. Чанки нужны только для видимых эффектов. При загрузке чанка (`native_chunk_load`, API 57) догонять состояние по разнице игрового времени.

Массовые правки и чтение: `native_edit_blocks`, `native_read_columns`, шаблоны структур `native_structure_template`, поломка блока машиной `native_machine_destroy_block`, использование предмета от имени машины `native_use_as_player` [Д] (`native-mods.md:4161-4200`, `4222-4239`, `4272-4293`).

## 10. Производительность и ограничения

### Стоимость хуков

- Хелпер миксина вкомпилирован в тело целевой функции как локальная функция [Д] (`MIX\README.md:65-74`). Отдельного слоя вызова нет [П].
- Trait-мод: на каждой точке хука обход списка `mods()` с динамическим вызовом [И] (`mod_api.rs:857-865`).
- Дорогие хуки включаются по требованию флагом `listened`: тики сущностей, экипировка мобов, рендер игроков, попадания снарядов [И] (`GAME\native_ticks.rs:58-74`), [Д] (`native-mods.md:1237`, `1241`, `1243`, `1197`).
- Часто вызываемые хуки, где документация требует краткости: `native_update_shape` (не меньше 6 раз на `setBlock`), `native_random_tick`, `native_block_signal`, `native_fire_odds`, `native_block_entity_submit`, `native_menu_slot_rules` [Д] (`native-mods.md:310-313`, `318`, `354`, `1270`, `929-930`, `4622-4624`).
- Единственный замер: `path_of_craft_tree` на `7e4e421` дал средний тик 4,3 мс против 2,92 мс без мода. Сборки отличались коммитом и профилем, долю мода выделить нельзя [И] (спецификация `…\2026-11-10-path-of-craft-tree-design.md:504-511`).
- Профиль `dev` собирается примерно в 6 раз быстрее, логика игры идёт до двух раз медленнее [Д] (`native-mods.md:1796-1799`, `1878-1885`).

Вывод для RustCraft [П]: не делать тикающей каждую трубу. Тикать контроллер сети, геометрию кэшировать через `keep()`.

### Ловушка `mods().is_empty()`

`mod_api::mods()` содержит только trait-моды [И] (`mod_api.rs:857-865`). У бандла из одних миксинов список пуст, и часть движка молчит. Места проверки [И]:

| Файл и строка | Что отключено без trait-мода |
| --- | --- |
| `GAME\natural_spawning.rs:458` | `native_potential_spawns` |
| `GAME\native_entities.rs:473` | `native_entity_join_level` |
| `GAME\native_world_draw.rs:130-134` | `native_world_submit`, если не подключён `world_drawn` |
| `GAME\lan.rs:1435` | синхронизация вложений клиентам |
| `GAME\machines.rs:995`, `1113` | `native_inventory_move_item` (воронки) |
| `GAME\item_draw.rs:30` | свойства моделей предметов |
| `GAME\lan.rs:2289`, `2305`, `2319`, `3355` | приседание, кик, ввод и позы игроков |
| `GAME\lan_stations.rs:85` | хук наковальни |

Обход, уже применённый в `path_of_craft_tree`: HEAD прямо в настоящую функцию [И] (`POC\…\breed_listens.rs`, `anvil_listens.rs`; `horizon-mixin.toml:517`, `528`).

Альтернатива: сделать RustCraft trait-модом (пакет `[api]` с `hooks()`), тогда все проверки снимаются [Д] (`native-mods.md:4683-4707`). Раздел документации помечен «draft». Риск [П]: в общей сборке с `path_of_craft_tree` заговорят хуки, которые у него молчали, нужна регрессионная проверка.

### Виды инъекций и запреты

Есть [Д] (`MIX\README.md:5-8`, `MIX\docs\spongepowered-feature-gaps.md:14-32`): HEAD, BEFORE_CALL, AFTER_CALL, AFTER_ASSIGN, RETURN, MODIFY_ARG, MODIFY_ARGS, MODIFY_CONSTANT, MODIFY_VARIABLE, MODIFY_EXPRESSION_VALUE, WRAP_OPERATION, OVERWRITE; отмена (`cancellable`) у HEAD и внутренних точек; захват локальных переменных; мосты к полям и методам; состояние `[[state]]` в структурах; модули `[[module]]`; свои крейты `[[crate]]`; шаги `[[post_build]]`.

Нет и запрещено [Д]:

- REDIRECT, TAIL, выбор отдельного `return`, точки FIELD, NEW, JUMP (`spongepowered-feature-gaps.md:44-50`).
- Хелпер: ровно одна приватная синхронная безопасная функция без атрибутов и обобщённых параметров (`MIX\docs\mixin-api.md:35-41`).
- BEFORE_CALL только на целом выражении-операторе, простом `let` или хвосте блока (`mixin-api.md:88-90`).
- Инъекции, переписывающие тело, отклоняются в функциях с замыканиями, `async`, `?`, `unsafe`, в обобщённых и `const` функциях. HEAD допускает замыкания и `?` (`mixin-api.md:90-99`, `116-120`). Опыт `path_of_craft_tree` это подтверждает: MODIFY_EXPRESSION_VALUE был отклонён в функции с замыканиями.
- Отсутствующая цель всегда ошибка, необязательных целей нет (`spongepowered-feature-gaps.md:95`).
- OVERWRITE конфликтует с любой другой инъекцией в ту же функцию (`MIX\README.md:74-76`).
- Нет условий «только клиент» или «только сервер»: код компилируется в обе стороны, сторону проверяют в рантайме (`spongepowered-feature-gaps.md:94`).
- `include_bytes!` в модулях мода отклоняется, вместо него `native_data!` [И] (`POC\lib.rs`, комментарий перед `mod shipped_tree`).
- Свой крейт мода не может зависеть от `horizon-client` и от path- или git-крейтов. Зависимости только из `Cargo.lock` движка [Д] (`native-mods.md:554-573`).
- Файл `.hnmod`: не больше 512 файлов и 64 МиБ; для сборки из папки пределов нет [Д] (`native-mods.md:721-724`).
- Инъекция в сырую цель привязывает мод к коммиту: такая функция «может измениться в любом коммите» [Д] (`native-mods.md:4906-4907`). Именованные хуки стабильны.
- Мод не изолирован, выполняется с правами игрока [Д] (`native-mods.md:17-20`).

### Состояние, конфиги, данные без пересборки

| Что | Механизм | Доказательства |
| --- | --- | --- |
| Состояние в памяти | Статические `LazyLock<Mutex<…>>` в модуле | [И] `POC\…\glue.rs:84-93` |
| Поля в структурах движка | `[[state]]` | [Д] `MIX\README.md:154-157` |
| Данные мира | Вложения (уровень в `data/minecraft/horizon_attachments.dat`), `nbt` block entity | [Д] `native-mods.md:5010-5025` |
| Свои файлы в мире | `native_world_dir()`, запись через временный файл | [И] `glue.rs:207`, `218` |
| Конфиги | `Game::native_config_dir()`; `native_register_configs` с автоперечитыванием раз в секунду и серверным конфигом на мир | [И] `native_hooks.rs:11305-11314`, `2558`; [Д] `native-mods.md:1760` |
| Настройки игрока | `native_set_setting`, `native_setting` | [И] `native_hooks.rs:11295-11304` |
| Данные сборки | `native_data!` (в игру), `native_built_data` (файлы шага после сборки) | [Д] `native-mods.md:709-719`, `736-753` |
| Свои папки данных | `native_add_server_reload_listeners`, перечитываются при `/reload` | [Д] `native-mods.md:1766` |

Без пересборки меняются: конфиги; рецепты, лут, теги и достижения (файлы и `/reload`); текстуры, модели и звуки контент-мода (переключение применяется сразу) [Д] (`mods.md:24-29`). Новые блоки, предметы, типы меню и сущностей требуют перезапуска, изменение кода требует пересборки. В `path_of_craft_tree` дерево перечитывается с диска только в сборке `dev` [И] (`glue.rs:16-18`, `POC\dev.ps1:107-110`).

### Несколько модов вместе

- Все native-моды компилируются в один движок [Д] (`native-mods.md:2099-2102`).
- Моды должны закрепить один коммит Horizon, иначе нужен `--allow-version-mismatch`. Сборка серверного движка при несовместимости падает, ничего не пропуская [Д] (`native-mods.md:121-132`).
- Порядок инъекций в одной точке: приоритет по убыванию, затем id мода, затем id инъекции. Лаунчер пишет предупреждения в `build.log` [Д] (`native-mods.md:513-538`).
- Имена модулей уникальны, иначе `E_MODULE_CONFLICT` [Д] (`native-mods.md:126-127`). В `path_of_craft_tree` модули с префиксом `pct_` [И] (`horizon-mixin.toml:11-63`).
- У двух включённых контент-модов не может быть общего пространства имён [Д] (`mods.md:32-33`).
- Зависимости между trait-модами: `[dependencies]` с semver, включая необязательные [Д] (`native-mods.md:4711-4732`). Это путь для интерфейса «опыт владельцу механизма» из спецификации `path_of_craft_tree`.

## 11. Сборка и распространение

**Вывод: клиент и сервер собираются раздельно одной командой каждый. Ресурсы едут отдельным контент-модом. Сборку запускает только владелец.**

- Клиент: `horizon-launcher native-build <бандл> --instance <имя> --version <тег> --profile <профиль>`. Результат: версия `native-<коммит>-<ключ>` в `%APPDATA%\horizon-launcher\versions`, экземпляр переводится на неё [Д] (`native-mods.md:1817-1819`, `1906-1913`), [И] (`POC\dev.ps1:95-98`).
- Сервер: `horizon-launcher native-build <папка> --server-output <DIR> --version <тег>`. Результат: `horizon-engine`, подготовленные ресурсы, `native-data/`, `native-mods.json`. Подключение: `horizon-server --dir SERVER config set engine.path=…` [Д] (`native-mods.md:2097-2113`), [И] (`dev.ps1:81-86`).
- **Кросс-компиляции нет**: серверный движок работает только на той же ОС и архитектуре, где собран [Д] (`native-mods.md:2106-2107`). Для сервера на Linux сборка нужна на Linux.
- Проверки совместимости клиента и серверного движка нет («no compatibility handshake yet») [Д] (`native-mods.md:2115-2118`).
- Обновление Horizon не пересобирает закреплённый `engine.path`: новую сборку делают вручную [Д] (`native-mods.md:2115-2116`).
- Первая сборка скачивает Rust и занимает 20–30 минут; полная сборка крейтов 1,5–3,5 минуты; дальше пересобирается только изменённое [Д] (`native-mods.md:1802-1803`, `1896-1897`). Тяжёлый код выгодно держать в своём крейте мода: он не пересобирается при правке хелперов [Д] (`native-mods.md:550-553`).
- Windows: Smart App Control ломает сборку ошибкой 4551 [Д] (`native-mods.md:1856-1860`).
- **Флаг `--trust-native-code` обязателен для обеих команд и является решением владельца.** В документации прямо: «an agent must not supply that consent» [Д] (`native-mods.md:1819-1820`, `2118-2119`). `dev.ps1` передаёт этот флаг [И] (`dev.ps1:84-85`, `96-97`), поэтому запускать скрипт должен владелец.

### Ресурсы

- Текстуры, модели, блокстейты, языки и звуки лежат в контент-моде: `.hmod` или папка с `horizon-mod.toml` в каталоге `horizon-mods/` папки игры [Д] (`mods.md:17-21`), [И] (`GAME\mods.rs:94-104`).
- Контент-моды по умолчанию выключены. Включение хранится в `horizon-mods.txt` строкой `mod.<id>:true` [Д] (`mods.md:13`, `22-23`).
- Манифест для обеих сторон: `side = "both"`, `capabilities = ["registries"]`, `api = "0.2"` [Д] (`mods.md:84-94`, `112-113`).
- Сервер сверяет `.hmod` клиента по id, версии и SHA-256, поэтому файл должен быть побайтно одинаковым [Д] (`mods.md:107-110`).
- Для разработки: параметр игры `--dev-mod <путь>` [И] (`SRC\crates\horizon-client\src\game.rs:912`); сборщик `cargo horizon-mod build` и `check` [Д] (`mods.md:39-63`).
- Один `.hmod` с кодом и ресурсами описан форматом, но реестр по-прежнему публикует `.hnmod`, «пока лаунчеры и игра не читают native `.hmod`» [Д] (`SRC\docs\api\mod-files.md:26-33`). Надёжный вариант [П]: два артефакта, бандл с кодом и контент-мод с ресурсами под тем же id.
- Где выделенный сервер ищет `horizon-mods/`, по исходникам не подтверждено [П]: вероятно, в папке сервера. В локальных папках серверов (`%LOCALAPPDATA%\Horizon\server`, `…\dev\server`) каталога `horizon-mods` сейчас нет [И], контент-моды в проекте ещё не использовались.

## Таблица рисков для RustCraft

| Задумка | Во что упирается | Обходной путь | Цена |
| --- | --- | --- | --- |
| Монорельс с вагонетками на стенах и потолке | Нет своего класса сущности; вагонетка ищет рельс под собой и получает гравитацию каждый тик | А: cancellable HEAD в `minecarts::tick_cart` со своей физикой, свои сиденье и отрисовка. Б: моб-сиденье с `native_mount` и `native_teleport_vehicle` | **Самая дорогая часть.** А: L–XL, привязка к коммиту. Б: M–L, риск рывков. Нужен прототип до проектирования |
| Камера и поза игрока вниз головой | Крен камеры ограничен 10°, модель седока вертикальна | Подвесная вагонетка под потолочным рельсом, игрок головой вверх | S, если принять дизайн; иначе L |
| Вагонетка без игроков рядом | Сущности тикают только в загруженных чанках; билеты не сохраняются, рассчитаны на десятки | `native_force_chunk` на вагонетку, список во вложении уровня, восстановление в `native_level_load` | M; ограничить число активных составов |
| Труба с 4 каналами в блоке | Предел 2^20 состояний; форма зависит только от состояния; block entity рисуется до 64 блоков | 1024 состояния (6 сторон и 4 канала), модель `multipart`, детали через block entity с `keep()` | M |
| Длинные трубопроводы и производительность | Тик block entity на каждый блок; хуки формы вызываются очень часто | Сеть как граф в памяти мода, тикает контроллер | M, решается архитектурой |
| Машины с интерфейсами | Меню открывается только у шаблона, который сам открывает меню (не проверено); нет предсказания кликов; data-слоты 16 бит | Шаблон-контейнер; крупные числа через extra data, update tag или сообщения мода | M на первую машину, S на следующие |
| Фильтры в интерфейсе | Слотов-фантомов нет | Кнопки и `native_menu_button`, список в `nbt` | S–M |
| Энергия | Буфер `i32` | Готовый `native_block_entity_energy` или свой учёт в графе сети | S |
| Склад-мультиблок | Контейнер не больше 256 слотов | Счётчики в `nbt` или вложении, свой экран, shift-клик через `native_menu_quick_move` | M–L |
| Работа механизмов в непрогруженных чанках | Тиков там нет | Симуляция сети вне чанков, догон по времени при загрузке | M |
| Свои жидкости | Нет источников в мире и вёдер | Жидкости только в баках и трубах; ёмкость как предмет с `native_item_fluid_tank` | S, если принять ограничение. Жидкость в мире: XL |
| Насос для воды и лавы | Готового хелпера нет | Чтение блока и `native_set_block` | S |
| Джетпак | Клиентский толчок не виден серверу; разрешение полёта не сохраняется | `native_set_flight` (творческий полёт с расходом топлива) или `native_push_player` с настройкой античита | S (полёт), M (тяга) |
| Крюк-гарпун | Сервер не задаёт скорость игрока; своего снаряда нет, только копия снежка | Снаряд по шаблону снежка, `native_projectile_impact`, сообщение клиенту, клиентский `native_push_player`, трос через `native_world_primitives` | M |
| Броня | Слои не привязаны к костям модели | Ванильная форма через `equippable` и equipment-ассеты; объём через `native_mob_layer` с ручной анимацией | S (ванильная форма), M (объёмная) |
| Дополнительные слоты игрока | Механизма нет | Рюкзак-предмет (`native_item_container`) или вложение игрока со своим экраном | M; настоящих слотов экипировки не будет |
| Свои руды | — | Контент-мод: feature и biome modifier | S |
| Запрет спавна в области | Хук молчит без trait-мода | Сделать RustCraft trait-модом или HEAD в функцию спавна | S–M |
| Синхронизация данных клиентам | Синхронизация вложений выключена без trait-мода | Trait-мод, либо update tag и свои сообщения | S |
| Совместная сборка с `path_of_craft_tree` | Один движок, общий коммит; trait-мод меняет поведение молчавших хуков | Один закреплённый коммит; регрессия `path_of_craft_tree` | S–M |
| Обновления Horizon | Сырые цели ломаются между коммитами; часть нужных функций без номера API | Держаться именованных хуков, сырые цели собрать в одном месте | Постоянная цена сопровождения |
| Распространение | Два артефакта (код и ресурсы); `.hmod` побайтно одинаков; нет кросс-компиляции; нет проверки совместимости движков | Доставка через pack-sync; серверный движок собирать на ОС сервера; своё приветствие с версией | M |
| Зрелость API | В `horizon-rust/horizon-mods` нет native-мода с блоками и машинами (список каталога `native/` на GitHub на 2026-11-10: coords-hud, fps-overlay, keystrokes, minimap, recipes, smp, voicechat-server, zoom). Сущности по шаблонам «в игре не пробовали» | Вертикальный срез до полного проекта: блок, block entity, экран, труба, сохранение, выделенный сервер | **Главный неизвестный риск** |

## Что проверить прототипом в первую очередь

1. Блок из `native_blocks` с контент-модом ресурсов: модель, состояния, форма, вход клиента на выделенный сервер.
2. Block entity с тиком, слотами, своим типом меню и экраном на шаблоне, который не открывает меню.
3. Вагонетка: оба варианта (инъекция в `tick_cart` и моб-сиденье) на стене и под потолком; оценить плавность у седока и у наблюдателя.
4. Дальность и стоимость отрисовки 500–1000 труб: меш чанка против block entity с `keep()`.
5. `native_force_chunk` после перезапуска сервера и при паузе пустого сервера.
6. Trait-мод в одной сборке с `path_of_craft_tree`: что изменилось в его поведении.
