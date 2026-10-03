# LEGO Racers RS

A work-in-progress native Rust recreation of LEGO Racers (1999), targeting
Apple Silicon macOS. Uses the original game's data files with a new Rust
runtime. The native presentation backend is being migrated to Bevy 0.19;
loaders, simulation and save data remain engine-independent. Migration verification
is in progress, not a claim that every existing mode is already Bevy-accepted.

**This is not a finished or fidelity-accepted port.** Menus, garage editing,
solo racing, opponents, powerups, circuit progression, time-trial ghosts and
split-screen racing are implemented to varying degrees. Physics, AI, effects,
rendering, audio and original gameplay parity still need work. Scripted smoke
runs do not establish physical-input, audible-output or human gameplay acceptance.

## Build and run

Requires the Rust toolchain and a legally obtained PC copy of the original game.
Game data is **not** provided here.

```sh
cargo build --release -p lrgame --bin lrracers
./target/release/lrracers --play /path/to/LEGO.JAM
```

For a single race, choose **Single Race**, select a track, then confirm your
racer. Use Up/Down and Enter in menus. Enter skips the race introduction;
wait for the countdown before driving. Up accelerates, Down brakes/reverses,
and Left/Right steer. Esc opens the pause menu; R restarts the race. After
all racers finish, Enter opens results; **Race Again** starts another race.

The current native launch/control/pause/restart path has automated keyboard
regressions, and original-asset races reach three-lap results. These checks use
synthetic input and do not establish human-tested handling or whole-game fidelity.

Keep the original `.TUN` music files alongside `LEGO.JAM`. The runtime reads
the archive directly; you do not need to unpack its individual assets.
Music and SFX are on by default: original menu/builder tunes, racing music,
engines, countdown and powerup sounds. Options saves their independent toggles.
Native versioned JSON saves are separate from original `.LRS` saves.

Native keyboard defaults: arrows drive, Space uses a powerup, C cycles the
four original camera views, hold V to look back, Esc pauses, R restarts.
Versus player 2 uses WASD, F for a powerup, Q for camera and E for look-back.
Camera preference can be saved in Options. These keyboard mappings are
provisional native defaults, not a claim of original binding parity.

## Workspace

- `lrjam`: original JAM archive reader and extraction utility.
- `lrformats`: original file-format loaders and conversion tools.
- `lrsim`: vehicle simulation, contacts, race rules, opponents and replay systems.
- `lrgame`: native renderer, menus, garage, racing and audio.
- `assets/native`: small rewrite-owned JSON configuration, not game artwork.

Some tests use your local original archive at
`extracted/Program_Files_Group/LEGO.JAM`. Run tests relevant to the behavior
being changed; asset-backed tests cannot run without your own game data.

## Repository boundaries

Only rewrite source, Rust tests, manifests, configuration and a small native
regression fixture are included. Original executables, decompiled code,
archives, textures, music, screenshots, recordings, saves and local research
artifacts are excluded.

This is an unofficial project, not affiliated with or endorsed by the LEGO
Group or the original game's developers or publishers. Original game content
and trademarks belong to their respective owners. The source license does
not grant rights to original game data.
