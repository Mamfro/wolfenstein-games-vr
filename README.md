# Wolfenstein VR

Play Wolfenstein: The New Order and Wolfenstein: The Old Blood in VR. Features:
* Your hands and the guns they hold follow your controllers, two guns when dual wielding
* Aim where your controllers point, or where you look
* The ammo counters sit on the guns instead of flat in front of you
* Swing the knife to stab, and throw it along your swing
* Cutscenes keep the horizon level and let you look around
* In The New Order: hotwire panels with your hands, and ammo is picked up as you walk over it

This is a game pack for [Monaka VR](https://github.com/Mamfro/monaka_vr), which runs it.

> **Alpha.** Expect bugs and missing pieces. Until Monaka's API settles, things change often,
> and a pack made for an older Monaka may stop working with a newer one.
>
> **Before you play,** start the game from Steam first. God rays, lens flares, dim shadows and
> screen-space reflections are switched off while VR runs (the game only gets them right in one
> eye).

## Games

| Game | Status | Controls | Aim | HUD |
|---|---|---|---|---|
| **Wolfenstein: The New Order** | Alpha | Motion controls | Head, controller | Dynamic HUD |
| **Wolfenstein: The Old Blood** | Experimental | Motion controls | Head, controller | Dynamic HUD (not yet checked in a headset) |

**Status Meaning**, from most to least finished:
- **Beta**: I'm confident that you can play the whole game this way. Small features/polish and deeper integration may be lacking.
- **Alpha**: I'm somewhat confident you can play the whole game this way, but I haven't played multiple hours at a time.
- **Pre-alpha**: It runs, but probably will be a bad experience. Very rough edges remain.
- **Experimental**: Probably runs, but rough. May crash and will work incorrectly.

**Motion controls**: your VR controllers work as a gamepad, and your in-game hands and what they
hold follow the controllers.

**Aim**: what points your weapon. **Head** means you aim where you look; **Controller** means you
aim where a controller points. Turning with the stick still works on top.

**HUD**: **Dynamic** puts pieces of the HUD on your hands.

Currently only the **Steam** versions are supported.

## Setting up

1. Install **Monaka VR** from its [Releases page](https://github.com/Mamfro/monaka_vr/releases) (see
   its [README](https://github.com/Mamfro/monaka_vr)).
2. Download `wolfenstein-games-vr-<version>.zip` from this repository's
   [Releases page](https://github.com/Mamfro/wolfenstein-games-vr/releases). Don't unzip it.
3. In the Monaka VR window, press **Add game pack...** and pick the zip. Both games appear in the
   list.

Then play as Monaka VR's README says: pick the game, start SteamVR, press **Start VR**. To update,
add the newer zip the same way: it replaces the old one.

## What works

| | The New Order | The Old Blood |
|---|---|---|
| **Aim** | Head, controller | Head, controller |
| **Hands** | Tracked, with both guns when dual wielding | Tracked, with both guns when dual wielding |
| **Melee** | Swing the knife to stab; throw it along your swing | Little tested |
| **Finger tracking** | On a free hand | On a free hand |
| **Cutscenes** | Level horizon, free look; videos on a flat screen | Videos on a flat screen; not otherwise reviewed |
| **Mounted guns and the mech** | The stick aims them, your head looks around | Not reviewed |
| **Hotwiring** | With your hands | Not in this game |

## Known bugs

* Aiming the mech with your hands has not been tried in a headset yet.
* The Old Blood has fallen behind The New Order: most features added since its first version have
  only been checked in The New Order.

## For developers

To build it yourself, clone [Monaka VR](https://github.com/Mamfro/monaka_vr) as `monaka_vr` and
[Monaka SDK](https://github.com/Mamfro/Monaka_SDK) as `monaka_sdk`, and this repository beside them
in a `monaka_adapters` folder, then run `cargo build --release` here:

```text
monaka_sdk\
monaka_vr\
monaka_adapters\
    wolfenstein-games-vr\
```

The DLL lands in `target\release`; **Add game pack...** takes a single DLL too. `tools\Package.ps1`
makes the release zip in `dist\`.

`tools\` also has the scripts used while building this: `Play-WolfTNO.ps1` starts a session with
the usual settings, and `Capture-WolfTNO.ps1` and the files in `tools\harness` run checks on the PC
without a headset. They run Monaka's `monaka_play` from the `monaka_vr` folder beside
`monaka_adapters` (or `MONAKA_ROOT`). To build the DLL straight into Monaka's own `target` (beside
its launcher), copy `cargo-config.example.toml` to `.cargo\config.toml`.

## Licence

Free software under the [GNU General Public License v3.0](LICENSE), like Monaka VR.
