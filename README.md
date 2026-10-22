<p align="center">
  <img src="docs/assets/rerac-logo.png" alt="ReRAC2" width="480">
</p>

<h1 align="center">ReRAC2</h1>

<p align="center">An unofficial native PC port of Ratchet & Clank: Going Commando (2003).</p>

<p align="center">
  <a href="https://discord.gg/v2Ek44kdyu">Discord</a> ·
  <a href="https://re-rac.github.io">Website</a> ·
  <a href="https://github.com/re-rac/rerac">ReRAC (the first game)</a>
</p>

## Please read first

> [!IMPORTANT]
> This repository is for developing ReRAC2, not for playing it. There is no release yet; players will install and
> run the game through the [ReRAC Launcher](https://github.com/re-rac/rerac-launcher), like the first game.

> [!WARNING]
> You need your own copy of the game: no game assets are included here. ReRAC2 supports one disc, the North American
> release (SCUS-97268). ReRAC2 is an unofficial fan project and is not affiliated with or endorsed by Sony Interactive
> Entertainment or Insomniac Games.

## About

ReRAC2 rebuilds Ratchet & Clank: Going Commando (PlayStation 2) as a native PC game, written in Rust on the Bevy
engine. The game's systems are ported and run natively, with no emulator in between, and the goal is a port that
looks, sounds and plays like the original. It grew out of [ReRAC](https://github.com/re-rac/rerac), the port of the
first game, and shares its engine.

## Status

Early. The game boots to its main menu, and New Game starts on Aranos, the first planet. The other planets are next.

## Building from source

You need Rust, installed through [rustup](https://rustup.rs), and an image (`.iso`) of your own disc. macOS on Apple
Silicon is the only platform tested so far. Run every command from the repository root.

First, copy the game's files from your disc into `extracted/`. This builds `rerac2-extract`, checks that your disc is
supported, and verifies every file:

```sh
cargo xtask regen-data --iso "<path to your disc image>.iso"
```

Then build and run the game:

```sh
cargo dev
```

The first build takes a few minutes; later rebuilds take seconds. Tests run through `cargo xtask`:

```sh
cargo xtask test-quick        # fast unit tests
cargo xtask test-job formats  # unit tests plus one area's integration tests
cargo xtask test-full         # the full suite
```

More detail is in [`docs/workflows/`](docs/workflows/), for example
[`game-data.md`](docs/workflows/game-data.md) for the extractor and
[`dev-switches.md`](docs/workflows/dev-switches.md) for debugging options. What is still missing is in the
[gap register](docs/plan/gaps.md).

## Repository layout

| Folder | Contents |
|---|---|
| `crates/` | The game itself: file formats, game logic, the engine (`rerac2`) and the extractor (`rerac2-extract`) |
| `tools/` | Development tools that never ship, including `cargo xtask` |
| `docs/` | Developer documentation: format specs, plans and workflows |

## License

ReRAC2 is released under the ISC license. See [LICENSE](LICENSE).
