# Qualetize GUI

<p align="center">
  <img width="240" height="240" src="https://raw.githubusercontent.com/ulalume/qualetize_gui/main/docs/icon.png" alt="app icon">
</p>

**Qualetize GUI** is an unofficial GUI frontend for [Qualetize (by Aikku93)](https://github.com/Aikku93/qualetize/).
It provides an intuitive interface for tile-based image conversion for retro consoles.
This tool is designed for _Genesis_, _GBA/NDS_ graphics and can be adapted for custom formats.

![Screenshot](https://raw.githubusercontent.com/ulalume/qualetize_gui/main/docs/screenshot.png)

## Installation

There are three ways:

### 1. App Download (Windows/ macOS/ Linux)

Download the latest app from [GitHub Releases](https://github.com/ulalume/qualetize_gui/releases/latest).

> **Note:** The app is not currently notarized.\
> On some systems you may encounter warnings or issues when downloading
> or launching it.\
> If that happens, please try the installation method below.

### 2. Cargo Build

You can also build an application bundle yourself using Cargo:

```sh
git clone --recursive https://github.com/ulalume/qualetize_gui
cd qualetize_gui

cargo bundle --release

# for Windows
cargo bundle --release --target x86_64-pc-windows-gnu
```

This will generate an application bundle (e.g. `.app` on macOS).
It has only been tested on macOS. If you encounter issues, please use `cargo build --release` instead.

> **Note (Windows):**\
> Building with `x86_64-pc-windows-msvc` will fail.\
> Please specify `--target x86_64-pc-windows-gnu` instead.

> **Note (Linux):**\
> Additionally, on Linux, building has only been tested with the GNU toolchain.

### 3. Cargo Installation

You can also install using Cargo:

```sh
cargo install --git https://github.com/ulalume/qualetize_gui

# for Windows
cargo install --git https://github.com/ulalume/qualetize_gui --target x86_64-pc-windows-gnu

# Launch app
qualetize_gui
```

## Features

- Two quantization engines: Qualetize and a Rust port of tiledpalettequant
- Instant preview updates when parameters are changed
- Color adjustment tools
- Display palettes (hover to see palette/index and RGBA/hex)
- Reorder palette colors
- Tile reduce post-pass (blurred MSE + flips)
- Custom per-channel quantization levels (Genesis preset uses the real hardware brightness steps)

## Licence

This project is licensed under MIT License.

Third-party components are used under their respective licenses. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for details.
