Minecraft: Java Edition 26.3 (Horizon, rewritten in Rust) as Nintendo Switch homebrew. Unofficial: you need your own copy of Minecraft: Java Edition and a Switch with Atmosphère. Full instructions: [README](https://github.com/KoHaku743/horizon-nx#readme).

### Install

1. Unzip `horizon-switch-….zip` to the root of the SD card: it gives `switch/horizon/horizon.nro` and `switch/horizon/client.txt`.
2. Prepare the game files from your own Minecraft with the Horizon launcher ([playhorizon.dev](https://playhorizon.dev)) and copy the *contents* of the folder with `horizon-resources.json` into `switch/horizon/resources/` ([how](https://github.com/KoHaku743/horizon-nx#2-prepare-the-game-files-on-your-computer)).
3. Start it as an application: hold R while starting a game, then pick Horizon in the homebrew menu. Not from the Album.

On a Mac, run `dot_clean -m` on the SD card before ejecting it, or copy over FTP/MTP.

Already on 1.0.0 or later? The title screen offers new releases itself (the button left of Sign In, in Realms' place).

### New in 2.2.1

- **Frame generation is back** in the Graphics settings, off by default. It costs GPU time: with a
  light shader pack it can make 20-40 fps look smoother, but one console at 60 fps without shaders
  fell to about 10 fps with it. If the game gets slower, turn it off. Thanks to bovorapha-glitch
  for asking (#9).

### New in 2.2.0

- **Type in your own language.** The on-screen keyboard now opens on the letters of the game's
  language: Russian (and Bulgarian), Ukrainian, Belarusian and Greek get their own layout, with
  every letter of the alphabet. The Layer key (or RB) goes from those letters to Latin ones,
  then symbols, and back. Set it with Options > Language; other languages keep the keyboard as it
  was. Thanks to Rodion0585 for asking for it (#7).
- **Worlds with Cyrillic and other non-Latin names save and show up.** The Switch's file system
  can't keep a name that isn't plain ASCII: it garbled it on saving and then hid the folder, so
  the world vanished from the list. Such names are now stored in an ASCII form (Punycode, the way
  web addresses keep "мир.рф"): the game shows "Мир", while the folder on the card is named
  `xn--o0a0dya`. This covers worlds, resource packs, mods and screenshots. A folder put on the
  card by a computer with a non-ASCII name still can't be seen by the Switch. Thanks to Rodion0585
  for the report and for finding the garbled folders on the card (#7).
- **Hints name the controller's buttons.** "Press Left Shift to Dismount" and the like now name
  the button on your controller (R3), in the tutorial, the creative hotbar's messages and the
  spectator menu too.
- **Frame generation is gone from the Switch's settings.** The Switch's GPU can't keep up with it:
  turned on, the game dropped to about 10 fps. A settings file that had it on now reads off.

### New in 2.1.1

- **Ender pearls go through End gateways.** A pearl thrown into a gateway came out at its far
  exit but hung there and never landed, so it never took you to the outer End; walking in worked.
  It now falls and brings you along, as in Minecraft.
- **Chunks load at full speed in Battery Mode.** Battery Mode halved the chunk workers, and on the
  Switch that left one thread to generate and mesh every chunk while the cores sat half idle: new
  terrain filled in slowly when flying or after a teleport. Battery Mode now keeps both workers
  and still caps the frame rate. To choose smoother frames over faster chunks while new terrain
  loads, set Graphics > Worker threads to 1.

### New in 2.1.0

- **A clearer message when the game files are too old.** Updating from 1.x in the game replaces
  `horizon.nro` but not the game files, and 2.0 needs files prepared by a current Horizon launcher:
  a world that can't open for that reason now says on the Switch what to do (prepare them again on
  the computer, replace `switch/horizon/resources/`, delete `resources.hzpak`).
- **Servers with SRV records connect.** A server whose address points elsewhere by DNS (another
  host or port) refused the connection (os error 111): the Switch now asks its own DNS servers for
  the record, as Minecraft does. If a lookup gets no answer within a few seconds, the game joins
  the address as typed; `HORIZON_NX_SRV=0` in `switch/horizon/env.txt` turns the lookup off.
- **Smoother frames.** The game no longer reads a file from the SD card on every frame (the
  player's name, for the HUD), writes a log line to the SD card for every thread it starts, looks
  for new audio devices every second, or times every frame on the GPU (only Dynamic Resolution
  needs that now). Each of these stalled the frame on the Switch.
- **MCC Island loads.** Its resource pack no longer runs the Switch out of memory: the GUI atlas
  (the menus' and HUD's textures) stays at most 4096x8192 on the Switch. A server pack too big for
  it keeps its font sheets out of the atlas first, drawing their glyphs from the pack as they're
  shown, and only then lowers the resolution of its model textures: MCC Island's menus look a little
  softer than on a computer.
- **Faster in server lobbies.** On MCC Island the lobby went from about 40 to 56-57 fps in handheld
  mode, and Hypixel's holds 60: the held items' data is no longer converted on every frame, the
  scoreboard sidebar is laid out once a tick instead of every frame, animated menu textures are
  copied instead of resized when they already fit, and a pack's glyphs no longer have to be
  uploaded again every frame. Skins download two at a time.
- **Smoother first minute in a new world.** The stronghold search and other background work now
  run below the game's own threads, so the frame rate no longer drops to about 20 right after a
  world opens (thanks to mrrobotroot-prog, #6).
- **Less memory held back.** The game kept up to 176 MiB of freed memory aside for reuse, and a
  console held about 150 MiB of it for whole sessions; it now keeps at most 48 MiB, which leaves
  more room for the world and the GPU.

### New in 2.0.2

- **Splitscreen on the Switch.** A Pro Controller or a second pair of Joy-Con joins as player 2
  (up to 4 players). Turn it on in Options → Horizon Settings → Splitscreen; Setup opens the
  Switch's own controller screen (and its Controllers button opens it again), and a player is
  moved between the two halves there. Each player's rumble plays on their own controller.
- **Faster GPU.** In handheld mode Horizon now runs the GPU at 460.8 MHz, a speed the Switch offers
  games (it was 307.2 MHz). During splitscreen in handheld mode it runs at 614.4 MHz, an
  overclock like sys-clk's: it uses more battery and runs warmer. `HORIZON_NX_SPLIT_GPU_MHZ=0`
  in `switch/horizon/env.txt` turns that off, and `HORIZON_NX_GPU_CLOCKS=system` keeps the
  Switch's own speed. Docked is unchanged.
- **Water draws faster.** An open sea or lake is drawn as strips instead of one square per block
  again (John Uzoka's optimization from 1.x).
- **Gyro aiming** While Attacking / While Using works (only Always turned the view before).
- **No crash when quitting.** The game no longer crashes on its way back to the HOME menu.

### New in 2.0.1

- **Mods switch on and off without a restart.** A content mod like Salvage turned on in Options →
  Mods now works in the next world you open; before, its recipes stayed missing until you
  restarted the game. The Mods screen's Turn On/Turn Off button shows the right state.
- **Mods that run code** (WebAssembly or native, like Zoom) say plainly in Get Mods that the Switch
  can't run them, and aren't listed again after that.
- Microsoft sign-in is unchanged; its app ID now comes from the build's settings instead of the
  source code.

### New in 2.0.0

Rebuilt on today's Horizon (November 2026), so mods made for current Horizon work, along with
everything Horizon added since 1.3.0.

- **Faster and smoother.** About 50-60 fps in a world in handheld mode at stock clocks, a steady
  60 on lighter servers (Hypixel Party Games), and far smaller drops while new chunks load.
  Chunk loading and saving now run behind the game instead of competing with it, and a network
  thread that used most of a processor core in single player now sleeps when it has nothing to do.
  Entity and particle culling are on by default, the menu background blur is off, and glowing
  outlines draw at half resolution.
- **Get Mods installs on the Switch again**, with a **Resource Packs** tab beside Mods. Turn a mod
  off or on from the Mods screen (the button bottom left).
- **HD rumble.** Vibration plays on the Joy-Cons' HD rumble: short ticks for blocks, a thud for
  hits, a roll for explosions. **Vibration Strength** is in Options → Controls → Controller.
- **Tapping the screen to attack** no longer stops your movement.
- **The in-game updater** offers full releases only (never pre-releases), in Realms' place on the
  title screen.
- Switch credits in the game's credits.

The first start of 2.0.0 compiles its shaders again, so it's slow once. In big worlds a GPU
overclock (sys-clk, 614 MHz) helps; on an OLED or other 2019+ Switch that's within the clocks the
console uses docked.

### New in 1.3.0

- **Server resource packs apply in seconds.** Joining a server again with a pack you've used takes about 7 s instead of about 40 (Hypixel's). The first join with a new pack is still slow; the pack is then bundled into one file in the background (a minute or so) and used from the next join, on any server. If the server updates its pack, it is bundled again.
- **Get Mods** (Options → Mods → Get Mods): browse Horizon's mod registry in the game and install content mods and resource packs, each checked against the registry before it's installed. Mods made for a newer Horizon than this one say so instead of installing. Installed ones have a **Remove** button. Restart after installing a mod.
- **Touch on Horizon's keyboard**, which is now the default on the Switch: tap the keys, and tap chat messages while typing. The Switch's own keyboard is still in Options → Controls → Controller → On-Screen Keyboard.
- **Clickable server buttons in chat**, such as Hypixel SkyBlock's NPC choices ("[Yes!]"): open the chat and tap them.
- **Gyro aiming** (Options → Controls → Controller → Gyro Aiming): aim by turning the console or controller, always or only while attacking or using items. Off by default.

### New in 1.2.0

- **60 fps.** Built with the graphics driver release 0.1.0 used ([JohnUzoka/mesa-switch](https://github.com/JohnUzoka/mesa-switch)), which compiles the game's shaders much better: about twice the frame rate of 1.0.0 and 1.1.0.

### New in 1.1.0

- **Much faster loading.** After the first start, the game's resources load in about 15 seconds instead of 2-4 minutes. The first start after installing packs the resources folder into one file, `switch/horizon/resources.hzpak` (about 520 MB), in the background while you play; that takes about 20 minutes, and the next start uses it. The resources folder itself stays as it is.
- **Server resource packs work**, Hypixel SkyBlock included: joining with a pack takes under a minute to apply instead of disconnecting you.
- **Horizon mods (`.hmod`) work**: put them in `switch/horizon/horizon-mods/`, turn them on in Options → Mods, and restart.
- **A faster memory manager**: the console's own was about half of the game's processor time; world data now loads in under 10 seconds instead of over a minute.

### What works

- **Single player** and **multiplayer**, including online-mode servers and their resource packs.
- **Microsoft sign-in on the title screen** (Sign In, right of Realms): enter a code at microsoft.com/link on your phone. Approved by Mojang.
- **Horizon mods** (`.hmod` content mods), installed by hand or from Get Mods.
- Joy-Con (with gyro aiming), touch screen (menus and the keyboard), and HOME (with Nintendo's HOME menu; custom HOME menus such as SwitchU can freeze the console).
- In-game updates from this repository's releases.

### Known issues

- The very first start is slow (shaders compile and are kept on the SD card, and the resources are packed).
- Docked mode isn't tested yet.
- Need the SD card space: the resources pack is about 520 MB on top of the resources folder, and each server's resource pack is bundled once more (Hypixel's: about 20 MB). Deleting `resources.hzpak` makes the game use the folder again (and pack it again); `HORIZON_NX_PACK=0` in `env.txt` turns packing off.
