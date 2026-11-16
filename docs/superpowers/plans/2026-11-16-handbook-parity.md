# Срез 18 «Игра по справочнику» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every MISSING and DIFFERS line of the handbook audit becomes DONE in the game. The only exceptions are engine limits, each recorded with evidence.

**Architecture:** Eight parallel streams, one per subsystem. Each stream works in its own git worktree on branch `s18/<stream>` from `feat/handbook-parity` and writes its own detailed TDD plan from its audit table. Logic goes into the rules crate with unit tests, game wiring into the bundle, and each mechanic gets an in-world self-test. An integrator merges the streams one by one, checks each merge on the joint bench, then runs bots, screenshots and LetsCam recordings, and a second audit.

**Tech Stack:** Rust (Horizon native mod, horizon-mixins), the content mod (JSON: recipes, models, lang), PowerShell (dev.ps1, joint.ps1), Python (asset generators), the HTML handbook (JS).

**Spec:** [docs/superpowers/specs/2026-11-16-handbook-parity-design.md](../specs/2026-11-16-handbook-parity-design.md)

## Global Constraints

- Horizon is pinned to `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3. Horizon itself is not changed.
- The game follows the handbook (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`). The handbook changes only for the spec's contradictions and for an engine limit that has been checked in `SRC`.
- Keys: **Z** magnet, **B** backpack, **;** night vision. Every new key is first checked against all pack mods. Taken: PCT V K Y U J I H C; LetsCam P N K F9 F10; JEI R U; worldmap M H E; betterf3 X V M F; the game O G and its defaults.
- Armour sockets: 1 / 2 / 2. Suit capacity: +50% battery. Enchantments are applied at the anvil.
- Text and docs are in Russian per `CONTRIBUTING.md`. Commit messages are in English. Commit email: `128125070+Betreazen@users.noreply.github.com`. Every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- No Mojang assets in git. The owner's account (Betreazen) is never used in tests. Clients run muted (master volume 0 in options.txt). The instances `pack` and `pack-build` are never touched.
- The shared stage receives only the integration branch, at the end. A stream's checks always use `joint.ps1 try -Mod rustcraft -Path <worktree>`, which never publishes.
- The launcher builds one thing at a time: tries queue up. Do not kill another session's build.
- `--trust-native-code` only through `mod\dev.ps1` and `joint.ps1` (the owner's permission for RustCraft).

## Files

- `docs/design/06-audit/<area>.md` — the work list (read only; the second audit writes new files).
- `docs/superpowers/plans/s18-<stream>.md` — each stream's detailed plan and checklist. The stream creates it and keeps it current.
- The files each stream owns are listed in the spec, in the "Потоки" table.

---

### Task 0: Infrastructure (integrator) — done before the streams start

- [x] Add `-Path DIR` to `joint.ps1 try`: checks a worktree, never publishes (horizon-joint-dev 3e9b388).
- [x] Branch `feat/handbook-parity` with the spec and the audit tables (f1fe9b6).
- [ ] Worktrees `C:\Claude code projects\rustcraft-s18\<stream>` on branches `s18/<stream>` for the 8 streams.
- [ ] Each worktree has its own `CARGO_TARGET_DIR` (`<worktree>\target`), so the rules crates don't block each other.

### Tasks 1–8: Streams (one agent per stream, in parallel)

Streams: `pipe` (1), `energy` (2), `machine` (3), `module` (4), `craft-storage` (5), `rail-worker` (6), `base-gear` (7), `handbook` (8).

Each stream:

- [ ] **Step 1: Read.** Its audit table, the spec, `CONTRIBUTING.md`, `mod/README.md`, the relevant sections of `docs/design/02-spec.md`, and `SRC/docs/native-mods.md` for every engine API it relies on (custom screens: API 111; drawing over a menu: API 78; block-entity drawing: API 71/151; player layers: API 149; keys).
- [ ] **Step 2: Detailed plan.** Write `docs/superpowers/plans/s18-<stream>.md` with the superpowers:writing-plans skill. Every MISSING/DIFFERS line of the table becomes a task with files, a rules-crate test, wiring code, a self-test and a commit. Commit the plan.
- [ ] **Step 3: Execute with TDD.**
  - Each task: a failing test in `mod/rustcraft/rules` → `cargo test` (RED) → implementation → GREEN → wiring in the bundle → an in-world self-test scenario → commit. Mark the task done in its plan.
  - Every 3–5 tasks: `joint.ps1 try -Mod rustcraft -Path <worktree> -Owner s18-<stream>` must pass with every self-test, RC + PCT + SC.
- [ ] **Step 4: Engine limits.** Confirm each one in `SRC`, record the evidence in the stream plan, and build the closest real alternative.
- [ ] **Step 5: Hand off.**
  - All lines of the table are DONE or recorded exceptions.
  - The stream's section "Что проверить в игре: срез 18" is in `mod/README.md` (visual checks).
  - The last `try` is green.
  - Report to the integrator: commits, tests, exceptions, what bots/screenshots must check.

Shared files are only extended (append within the stream's own block, never rewrite another stream's lines): `content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, `rules/src/modules.rs`, lang files, `horizon-mixin.toml`.

Cross-stream contracts (from the spec):

- Stream 3 provides the tier mechanism: `rustcraft_rules::tiers` (tier items, the U2 swap rule with contents kept) and its bundle side. Streams 1 and 6 use it. Until it is merged, they build their tiers behind the same function names and switch to stream 3's after the merge.
- Stream 4 owns `rules/src/modules.rs`: which modules exist, which blocks accept which modules, and the socket counts. Streams 3 and 5 implement module effects in their machines.
- Stream 2 provides the crystal shard item `rustcraft:crystal_shard`. Other streams' recipes switch to it after stream 2 is merged (the integrator does this during the merge).

### Task 9: Integration (integrator)

- [ ] Merge into `feat/handbook-parity` in this order: 3, 4, 2, 1, then 5, 6, 7, 8 as each is ready. After each merge: resolve conflicts (`content.rs`, lang files), run `cargo test` in rules, then a full `joint.ps1 try -Mod rustcraft -NoPublish` (RC/PCT/SC green).
- [ ] Switch every recipe from the amethyst shard to `rustcraft:crystal_shard`.

### Task 10: Game check (integrator, bots)

- [ ] Release build of the dev server from `feat/handbook-parity`.
- [ ] Bots (offline, horizon-ctl, muted; whitelist off for the duration, then back on):
  - [ ] Survival through the steps of `quickstart` and `progress` using recipes, without `/give`.
  - [ ] For each handbook entry: its mechanic by a bot or a self-test.
- [ ] Screenshots of every block and window (`tools/bench/shot.ps1`), compared against `ui.js`.
- [ ] LetsCam recordings: turret, worker tools, windmill, jetpack and backpack on the player, zipline handle.
- [ ] Fix what the checks find (back into the owning stream's area, a new self-test for each bug).

### Task 11: Second audit and release

- [ ] Re-run the 7-area audit on the new code. Result in `docs/design/06-audit/after/`. Zero MISSING/DIFFERS, except recorded exceptions.
- [ ] Rewrite `mod/README.md` → «Что срезы пока не умеют» to be only the real differences. Add the slice 18 section and CHANGELOG `0.18.0`.
- [ ] Bump the version to 0.18.0 (`mod/content/horizon-mod.toml`, `mod/rustcraft/horizon-mixin.toml`, README clone lines).
- [ ] Final `joint.ps1 try -Mod rustcraft` (publishes on green), tag `v0.18.0`, push, GitHub release, hand-off to the pack session.
