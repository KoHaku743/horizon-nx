# switch-next: the Switch port on today's Horizon

`switch-next` is the Switch port (`switch-v010`, based on Horizon of 2026-10-31) merged with
upstream Horizon of 2026-11-21. `switch-v010` stays the release branch until this one works on a
console.

## Done in the merge

- The Switch platform as it was: `crates/horizon-nx`, `crates/horizon-switch`, the patched crates in
  `third_party/`, `packaging/switch/` and the `Switch NRO` workflow (it builds this branch too).
- The game binary is `horizon-game`'s (`-p horizon-game --bin horizon-client`): upstream moved the
  game's `main` there.
- mimalloc needs mmap, which libnx lacks: `packaging/switch/stubs/{mimalloc,libmimalloc-sys}` stand
  in, over the system heap (horizon-nx's TLSF heap), patched in by `cargo-patch.toml`.
- `std::env::current_exe` is the homebrew loader's argv[0] (the NRO's path) on the Switch:
  `packaging/switch/patch-std.sh` patches the toolchain's std source before `-Z build-std`. Upstream
  names its own file in about fifty places (mods, the launch arguments, the resources beside it).
- `horizon_nx::init` sets `HORIZON_HOME=.` (the NRO's folder is the game's home) and
  `HORIZON_WORKER_THREADS=3` (std has no core count there) unless `env.txt` sets them.
- cpal 0.17, the version bevy-horizon patches.
- Each event-loop pass drains the applet messages (HOME button, focus) and flushes the log
  (`game/runtime/window_handler.rs`); the resources come from their pack (`game.rs`), which is
  built after the first load (`game/world/world_rest.rs`).
- Graphics presets stay within the render distance cap (12).

## Ported since the merge

- Credits: Horizon NX's people open the game's credits on the Switch; the README names KoHaku743.
- Switch defaults (Cloud Distance 32, Cutout Leaves off), the CPU boost while a world opens.
- Taps on Horizon's on-screen keyboard; gyro aiming and its modes; the Microsoft sign-in on the
  title screen (beside Realms) and the saved account resuming at start.
- Folder syncs skipped (the SD card driver opens no folder as a file), which failed every
  advancement and statistics save.
- Already upstream, nothing to port: Nintendo button letters, chat's custom click actions, a mod
  browser. Not ported: the in-game updater (not wanted), server pack overlays (upstream reads packs
  in layers now, without a merged folder: measure first).

## To port onto upstream's code (from switch-v010)

Each was dropped where upstream rewrote the file, and comes back as its own commit:

| Feature | switch-v010 commits | Where it goes now |
|---|---|---|
| Nintendo button letters (A confirms), the pad on the title from the first frame | fb2490c022, 3f1a509df5 | `game/interface/pad/`, `pad_menu.rs`, `startup_menu.rs`, `horizon-client-input/src/gamepad.rs` (check what upstream does first) |
| On-screen keyboard: taps on its keys, Horizon's keyboard by default | c43bc59da6, 5a4d6ebf47, 7e0c2dd1e3 | `osk.rs`, `game/interface/pad_menu.rs`, `gamepad.rs` |
| Gyro aiming and its modes | 0b45d9956d, 80ecdf7854 | `gamepad.rs`, `game/interface/pad/`, `settings.rs`, `options_ui.rs` |
| Microsoft sign-in on the title screen | 0fb85785ca | `title.rs`, `game.rs` (`msa.rs` is still in horizon-client-render) |
| In-game updater from the fork's releases | d194acce58, cab57f3ed8, 1305e38100 | `switch_update.rs` (out of the tree for now), `title.rs` |
| Simulation distance capped at 12 | c1dd855fca | `game/interface/dedicated/` |
| Switch defaults: Cloud Distance 32, Cutout Leaves off | dca7144c68, 23298dd775 | `settings.rs` |
| Server and resource pack overlays packed | 70c0d646b8 | `game/world/packs/` |
| The CPU boost while a world opens | bd3a059103 | `game.rs` |
| `args.txt`, `client.txt` (the default is the game now, not a server) | dd018e0483 | `entry.rs` |
| Chat: ClickEvent.Custom, pre-1.21.5 names, click logging | c71810b3cb, 1dbbad1ffa, 8a05caa7ee | check upstream first: it may have them |
| Get Mods | 78f363304e and later | upstream has its own mod browser now: make it work on the Switch instead |
| Diagnostics: draws per frame, chunk job times, world settings, SD card time per step | 605601c327, 9d249f2a70, d5db2ac3f3, c1a09664bd | `gfx.rs`, `streaming.rs`, `game.rs` |
| John's worldgen, density, noise and meshing optimizations, sunrise fan, water strips | 2015977dfe ... 680cabdba2, 8e4c1e770d, 8aa8fe856d | upstream's versions for now; measure on a console first |

## Mods with code

Upstream runs mod code with wasmtime, built with Pulley (its interpreter, no JIT), and can run it in a
separate process. On the Switch it has to run in the game's own process; whether wasmtime builds for
`aarch64-nintendo-horizon` is the first thing to find out once the game itself runs.

## Ported in the second pass

- The in-game updater (`switch_update.rs`): a release build checks this fork's newest **full** release
  once a launch (drafts and pre-releases are skipped) and offers it in Realms' place on the title
  screen. A test build (no `HORIZON_SWITCH_VERSION`) never checks, so it isn't offered an older release.
- Horizon extras on in the Switch build: Horizon Settings and its mod and resource pack browser.
- Performance defaults: Menu Background Blur 0 (5-6 passes, up to 18 ms of a frame), the glowing
  entities' outline at half resolution (~10 ms at full).

## Third pass

- Get Mods installs on the Switch: the install lock is in-process only there (no file locks in
  libnx, no launcher beside the game). Its page has Mods and Resource Packs tabs again, as the
  1.3.0 page did; a registry resource pack is checked like a mod and goes to `resourcepacks/`.
- HD rumble: the Switch plays each vibration as shaped HD rumble (`Rumble::hd`, `horizon_nx::rumble`)
  instead of gilrs' flat full-strength pulse, and Controller Settings has a Vibration Strength slider
  for every controller.
- Still to look at: in a normal world 1.3.0 held 60 fps with 2-3 render passes a frame; this build
  runs about 40, with the server thread and the newer passes on the same three cores.

## Fourth pass

- Mods screen: on the Switch the footer's left button turns the selected mod off or on (no folder
  to open, no launcher to choose); the change waits for a restart.
- HD rumble a quarter stronger than the first tuning.
- Entity and particle culling on by default on the Switch. A normal world (RUN 46, ~38 fps) was
  GPU-bound: world 3-6 ms, entity skinning 2-3 ms, HUD 1.5-2.9 ms, animated sprites 1-1.7 ms.

## Fifth pass

- Flying stutter: over 10 s the SD card spent up to 4.5 s in calls, 3.2 s of it in 486 writes of
  3.9 MiB, and reads waited behind them (1.3-2.7 MiB/s). Half of it was the region header (8 KiB)
  rewritten for every chunk: `RegionFile::batch` writes it once per region save, and frees the
  batch's old sectors only after that header is on disk. Still seen: about one short-lived 2 MiB
  thread a second (not yet traced).
- A tap on the touch screen in play no longer takes the sticks from the controller.
