# Безопасность · Security

RustCraft — нативный мод Horizon. Его код компилируется прямо в игру и в сервер и работает с правами пользователя, без песочницы. Поэтому лаунчер просит подтвердить доверие к моду (`--trust-native-code`). Перед установкой прочитайте код или ставьте мод только из этого репозитория по тегу релиза.

Что мод делает за пределами игры:

- пишет только в папку мира: содержимое рюкзаков в `<мир>/data/rustcraft_backpacks.dat`; всё остальное (каналы, машины, склад) хранится в данных блоков и мира самой игрой;
- когда администратор запускает самотест (`rustcraft_selftest`), кладёт его итог в `selftest/rustcraft.json` рядом с папкой мира;
- читает таблицу мобов SoulsCraft из папки сервера, если она есть, чтобы турели знали их свойства;
- не открывает сетевых соединений: клиент и сервер обмениваются сообщениями через соединение самой игры.

## Как сообщить об уязвимости

Не открывайте публичный issue. Используйте [приватное сообщение об уязвимости](https://github.com/Betreazen/rustcraft/security/advisories/new) на GitHub. Ответ — в течение нескольких дней. Исправление выйдет в ближайшем релизе с благодарностью в списке изменений, если вы не против.

---

RustCraft is a native Horizon mod: it is compiled into the game and the server and runs with your rights, unsandboxed. It writes only inside the world folder (backpack contents) and, when an admin runs the self-test, a result file beside it; it opens no network connections of its own. Report vulnerabilities privately through [GitHub security advisories](https://github.com/Betreazen/rustcraft/security/advisories/new), not in public issues.
