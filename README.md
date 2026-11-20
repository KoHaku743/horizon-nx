# Horizon NX: Minecraft: Java Edition on the Nintendo Switch

<p>
  <img src="https://img.shields.io/badge/Minecraft-Java%2026.3-5D9C3F" alt="Minecraft Java 26.3">
  <img src="https://img.shields.io/badge/platform-Nintendo%20Switch%20(homebrew)-E60012" alt="Nintendo Switch homebrew">
  <img src="https://img.shields.io/badge/status-playable,%20experimental-F0A030" alt="Playable, experimental">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
</p>

This is a fork of [JohnUzoka/horizon-nx](https://github.com/JohnUzoka/horizon-nx), the Nintendo Switch port of
[Horizon](https://github.com/horizon-rust/horizon): Minecraft: Java Edition 26.3 rewritten from scratch in Rust,
checked against the real game by hundreds of differential tests. Horizon runs natively on the Switch as homebrew,
with no Java and no emulation.

The fork builds the Switch version in the open: the original port's build setup isn't public, so this repository
rebuilds it, fixes what broke on a real console, and adds Microsoft sign-in, server resource packs, Horizon mods and
much faster loading on the console itself.

> **Unofficial.** Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
> This repository contains no Minecraft code, assets or game files: you need your own copy of Minecraft: Java Edition.
> Running homebrew requires a Switch with custom firmware; do it at your own risk.

## Status

Tested on a Switch with Atmosphère, November 2026. Current release: **v1.3.0**.

| | |
|---|---|
| Single player: create and play worlds | ✅ Works |
| Joy-Con and touch screen | ✅ Works: touch in menus and on the keyboard; gyro aiming (off by default) |
| HOME button (leave and come back) | ✅ Works with Nintendo's HOME menu ([custom HOME menus](#the-console-freezes-when-i-press-home) can freeze) |
| Microsoft sign-in on the title screen | ✅ Works (approved by Mojang) |
| Multiplayer, online-mode servers included | ✅ Works (sign in first for online-mode servers) |
| Server resource packs | ✅ Works (Hypixel SkyBlock tested): under a minute the first time, about 7 s after that |
| Clickable chat (server buttons, links) | ✅ Works: open the chat and tap (Hypixel SkyBlock's NPC choices tested) |
| Horizon mods (`.hmod`) | ✅ Works: install them from Options → Mods → Get Mods (or put them in `switch/horizon/horizon-mods/` and enable them in Options → Mods), then restart. Many of the registry's mods need a newer Horizon than this one; Get Mods says which |
| In-game updates | ✅ From this repository's releases (1.0.0 and later) |
| Docked mode | ❔ Not tested yet |
| Frame rate | ✅ Up to 60 fps in handheld mode |
| Loading | ✅ About 15 s to the title screen. The very first start is slower: it compiles shaders and packs the resources into one file in the background (about 20 minutes, once; [Horizon NX Companion](https://github.com/KoHaku743/horizon-nx-companion) packs them on your computer instead) |
| Computer app | ✅ [Horizon NX Companion](https://github.com/KoHaku743/horizon-nx-companion): SD card setup, builds, mods, worlds, logs (macOS tested; Windows and Linux new) |

## What you need

- A Nintendo Switch running **[Atmosphère](https://github.com/Atmosphere-NX/Atmosphere)** custom firmware.
- About **1.5 GB free** on the SD card (the resources, and the same resources packed into one file for fast loading).
- A computer (Windows, macOS or Linux) to prepare the game files, and **your own copy of Minecraft: Java Edition**.
- To play on online servers: the Microsoft account that owns Minecraft: Java Edition.

## Install

### The easy way: Horizon NX Companion

**[Horizon NX Companion](https://github.com/KoHaku743/horizon-nx-companion)** is an app for your computer (macOS,
Windows, Linux) that does steps 1 to 3 for you and the slow part of the first start:

- copies the prepared game files to the SD card and **packs `resources.hzpak` on the computer**, so the Switch
  doesn't spend about 20 minutes on it;
- installs `horizon.nro` from this repository's releases;
- installs mods and resource packs from Horizon's registry, checked on the computer in seconds;
- backs up worlds and copies them between the Switch and Minecraft on your computer, reads the logs, frees up space,
  and cleans macOS's `._` files before ejecting.

Download it from its [releases](https://github.com/KoHaku743/horizon-nx-companion/releases/latest), put the SD
card in the computer (or use hekate's **Tools > USB Tools > SD Card**), and you still need the Horizon launcher from
step 2 once, to prepare the files from your own Minecraft. Then go to [step 4](#4-start-it).

The steps below do the same by hand.

### 1. Get the Switch build

Download `horizon-switch-….zip` from the [latest release](https://github.com/KoHaku743/horizon-nx/releases/latest)
and unzip it to the root of the SD card. It contains:

```
switch/horizon/horizon.nro
switch/horizon/client.txt
```

Builds of every change are also on [GitHub Actions](https://github.com/KoHaku743/horizon-nx/actions/workflows/switch-nro.yml?query=branch%3Aswitch-v010+is%3Asuccess)
(the `horizon-switch-…` file at the bottom of a run; you need to be signed in to GitHub).

`client.txt` makes `horizon.nro` start the game; without it, it starts a dedicated server.

### 2. Prepare the game files on your computer

Horizon reads its data from your own Minecraft, converted once by Horizon's tools:

1. Install the **Horizon launcher** from [playhorizon.dev](https://playhorizon.dev), sign in with your Microsoft
   account, and press **Play** once. It downloads Minecraft 26.3 for your account and prepares it.
2. Find the prepared folder: it's the folder that contains `horizon-resources.json`.
   - macOS / Linux: `find ~ -name horizon-resources.json 2>/dev/null`
   - Windows (PowerShell): `Get-ChildItem $env:USERPROFILE -Recurse -Filter horizon-resources.json -ErrorAction SilentlyContinue`

### 3. Copy everything to the SD card

```
SD card
└── switch/
    └── horizon/
        ├── horizon.nro
        ├── client.txt
        └── resources/        ← the contents of the prepared folder from step 2
            ├── horizon-resources.json
            ├── assets/
            ├── data/
            └── …
```

Copy the *contents* of the prepared folder into `switch/horizon/resources/`, not the folder itself.

**On a Mac**, Finder adds hidden `._` files next to every file it copies to the SD card, and the game can't read
them. Either copy over FTP or MTP (they never create these files), or clean the card before ejecting it:

```sh
dot_clean -m "/Volumes/YOUR SD CARD"
```

### 4. Start it

Horizon needs the memory of a full application, so **don't start it from the Album**. Hold **R** while starting
any game to open the homebrew menu as an application, then choose Horizon. (A title override or a forwarder works too.)

The first start is slow: the graphics driver compiles all of the game's shaders and keeps them on the SD card, and
once the game has loaded, it packs `resources/` into one file, `resources.hzpak` (about 520 MB), in the background
(about 20 minutes; keep playing or leave it at the title screen; with the companion app the pack is already
there). From the next start it loads in about 15 seconds.
Keep the `resources` folder: the game still checks it and reads its largest files from it.

## Microsoft account

On the title screen, **Sign In** (to the right of Realms) opens the Microsoft Account screen:

1. Press **Sign In**. The Switch shows a web address and a code.
2. Open the address on your phone or computer, enter the code, and sign in with the Microsoft account that owns
   Minecraft: Java Edition.
3. The Switch finishes on its own and shows your Minecraft name.

The sign-in is kept in `switch/horizon/account.json`, so you stay signed in after a restart. **Anyone with your SD
card can use that file to sign in as you** until you press **Sign Out**, which deletes it.

When signed in, worlds play under your real account. A world you played before under the offline name starts you
as a new player (as in Minecraft itself); the old character stays saved.

## Settings: `env.txt`

A text file `switch/horizon/env.txt`, next to `horizon.nro`, sets options at startup, one `NAME=value` per line
(`#` starts a comment). Nothing in it is needed to play. Useful ones:

| Setting | What it does |
|---|---|
| `NAK_DEBUG=…` | The graphics shader compiler's optimizations; `-name` turns one off. Only for testing the driver. |
| `MESA_SHADER_CACHE_DISABLE=true` | Compile every shader at every start instead of keeping them on the SD card. |
| `HORIZON_MSA_CLIENT_ID=…` | A different Azure app for the Microsoft sign-in. |
| `HORIZON_NX_CPU_BOOST=0` | Keep normal CPU clocks while loading (they're raised by default). |
| `HORIZON_NX_LINKS=0` | Turn off the SD card links that resource packs and mods need (only to rule them out when something breaks). |
| `HORIZON_NX_PACK=0` | Read the resources folder file by file instead of `resources.hzpak` (slow; only to rule the pack out). |
| `BEVY_HORIZON_CPU_PROFILE=1` | Write `cpu-profile.txt`, where the processor's time goes, every 30 s (slows the game slightly). |

## Troubleshooting

The game's logs are in `switch/horizon/`: `log.txt`, `logs/latest.log`, `panic.txt` and `crash-reports/`.
Atmosphère's own reports are in `atmosphere/crash_reports/`.

### The game closes after loading with "expected value at line 1 column 1"

macOS's hidden `._` files are in the resources folder. Run `dot_clean -m` on the SD card (see
[step 3](#3-copy-everything-to-the-sd-card)), or copy over FTP or MTP.

### The game closes about 10 seconds after the world appears

The log shows `channel 2 lost: … (fifo idle timeout)`: the GPU hung. That happened with versions 1.0.0 and 1.1.0,
built with the public graphics driver, when two of its shader optimizations (`plat`, `ofold`) were on. 1.2.0 uses
release 0.1.0's driver, which runs them without hanging. If it happens anyway, remove `NAK_DEBUG` from `env.txt`.

### Loading is slow every time

The resources pack (`switch/horizon/resources.hzpak`) isn't in use: the log says why on its `[Pack]` line. It's
built once, about 20 minutes after the first start; if you quit before `[Pack] done`, it's built again next time.
Preparing the resources again on your computer makes the game pack them again. Deleting `resources.hzpak` is safe.

### A server's resource pack disconnects me with "Configuration took too long"

The first time with a new pack, the game builds its files (and a mod's data, with mods on); a large pack can take
longer than the server waits. Join again: the second time it's ready. After a join, the pack is bundled into one file
next to it in the background (`.horizon-cache/resourcepacks/*.hzpak`), and later joins apply it in seconds.

### The console freezes when I press HOME

Custom HOME menus (SwitchU, and front ends built on it such as HomeNX and CardsNX) froze the console when HOME
was pressed over Horizon; with Nintendo's own HOME menu it works. Remove or turn off the custom HOME menu.

### The world takes very long to load the first time

The first time a world opens, the driver compiles the terrain's shaders and the world generates around you.
Later starts read the compiled shaders from the SD card (`.mesa/` at the card's root; deleting it is safe).

## Building

The build runs in a container (Docker or Podman, on Linux):

```sh
git clone -b switch-v010 https://github.com/KoHaku743/horizon-nx
cd horizon-nx
packaging/switch/build.sh all     # the toolchain image, the graphics driver (slow, once), then the NRO
```

It clones [bevy-horizon](https://github.com/JohnUzoka/bevy-horizon) (the Switch target, toolchain image and
patched libraries) next to the repository. The result is `target/aarch64-nintendo-horizon/release/horizon.nro`.

The graphics driver comes from [JohnUzoka/mesa-switch](https://github.com/JohnUzoka/mesa-switch) at the revision
release 0.1.0 used (`7789c1d`): the same game built with the public driver ([danfromtico/mesa-switch](https://github.com/danfromtico/mesa-switch)
`606441c`) runs at about 30 fps instead of 60. CI builds JohnUzoka's by default; the "Switch NRO" workflow's
`driver` input picks `public` instead.

Every push to `switch-v010` runs the same build in [GitHub Actions](.github/workflows/switch-nro.yml); its log and
the build's function symbols (for naming the offsets in a `cpu-profile.txt`) are kept on the `ci-logs` branch. To
publish a release, run the "Switch NRO" workflow by hand (**Run workflow**) with a version in `release` (for
example `v1.3.0`): it builds, tags the commit and attaches the zip. Details: [docs/switch.md](docs/switch.md).

### Branches

| Branch | What it is |
|---|---|
| **`switch-v010`** | **The one to use.** The source of the original port's release 0.1.0, with this fork's build setup and fixes. |
| `switch` | The original port's newer development branch. It builds, but on a console it loads 2–3× slower and doesn't run well. |
| `switch-build` | The build setup's first version, on top of `switch`. |
| `pr/*` | Fixes and features offered back to the original port as pull requests (socket sharing, the controller tutorial, the build setup, the app icon, the keyboard's touch, gyro aiming, loading diagnostics on its `switch` branch; Microsoft sign-in on its `switch-release` line). |
| `switch-skip-mac-metadata`, `switch-portable-data-file` | Fixes for macOS `._` files and a data file that works from the SD card; not yet moved to `switch-v010`. |
| `ci-logs` | The CI build logs. |

### What this fork changes

- **A public build:** `packaging/switch/` and the patched graphics, window and network libraries in `third_party/`,
  which the original port built in a private setup.
- **60 fps:** built with release 0.1.0's graphics driver (JohnUzoka/mesa-switch `7789c1d`) instead of the public one.
- **HOME:** the game waits in the background without drawing instead of asking the system to suspend it.
- **Microsoft sign-in on the console** (`crates/horizon-client-render/src/msa.rs`, the title screen's account screen).
- **A faster memory allocator:** the Switch's own (newlib's malloc) took about half of the game's CPU time; Rust's
  blocks now come from a TLSF heap (`crates/horizon-nx/src/allocator/tlsf_heap.rs`, [rlsf](https://github.com/yvt/rlsf)).
- **Resources in one file:** each file opened on the SD card costs about 10 ms, and the resources are 28,000 files.
  `crates/horizon-nx/src/vlinks/pack.rs` packs them into `resources.hzpak` and serves opens, listings and reads from
  it, keeping the folder as the source of truth: loading went from about 3.5 minutes to 15 seconds.
- **Server resource packs in one file too:** a pack's overlay folder is bundled the same way after the first join
  (`.horizon-cache/resourcepacks/<hash>.hzpak`), so joining again applies it in about 7 s instead of 40.
- **Get Mods:** Horizon's mod registry in the game (`crates/horizon-client/src/game/mod_browser.rs`): content mods and
  resource packs, checked against the registry before they're installed.
- **Switch controls:** touch on Horizon's on-screen keyboard (the default on the Switch), gyro aiming
  (`crates/horizon-nx/src/motion.rs`), and server click actions in chat (`ClickEvent.Custom`).
- **Links on the SD card:** resource packs and mods are folders of links into the game files, which the SD card's
  file system (FAT) can't store. `crates/horizon-nx/src/vlinks` keeps each folder's links in a small `.hzlinks`
  file in it and makes the SD card's device follow them, so packs and mods load without copying ~650 MB.

## Credits

- **[Horizon](https://github.com/horizon-rust/horizon)**: Ludovic Marie and the Horizon contributors.
- **The Switch port**: [JohnUzoka](https://github.com/JohnUzoka) ([horizon-nx](https://github.com/JohnUzoka/horizon-nx),
  [bevy-horizon](https://github.com/JohnUzoka/bevy-horizon)).
- **The Switch graphics driver**: [mesa-switch](https://github.com/danfromtico/mesa-switch) by danfromtico and
  NaGaa95, built on Mesa's NVK, with [JohnUzoka's changes](https://github.com/JohnUzoka/mesa-switch).
- **The TLSF allocator**: [rlsf](https://github.com/yvt/rlsf) by yvt.
- **The Switch homebrew toolchain**: [devkitPro](https://devkitpro.org) and [libnx](https://github.com/switchbrew/libnx).
- **This fork**: [KoHaku743](https://github.com/KoHaku743).

## License

MIT, as Horizon: see [LICENSE](LICENSE). Third-party code and its licenses: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
Minecraft is a trademark of Mojang Synergies AB.
