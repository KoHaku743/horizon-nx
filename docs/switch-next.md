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
