# Qualetize GUI

<p align="center">
  <img width="240" height="240" src="https://raw.githubusercontent.com/ulalume/qualetize_gui/main/docs/icon.png" alt="app icon">
</p>

**Qualetize GUI** is an unofficial GUI frontend for [Qualetize (by Aikku93)](https://github.com/Aikku93/qualetize/).
It provides an intuitive interface for tile-based image conversion for retro consoles.
This tool is designed for _Genesis_, _GBA/NDS_ graphics and can be adapted for custom formats.

![Screenshot](https://raw.githubusercontent.com/ulalume/qualetize_gui/main/docs/screenshot.png)

## Installation

### App Download (Windows/ macOS/ Linux)

Download the latest app from [GitHub Releases](https://github.com/ulalume/qualetize_gui/releases/latest).

### Cargo Installation

You can also install using Cargo:

```sh
cargo install --git https://github.com/ulalume/qualetize_gui
qualetize_gui
```

## Usage

1. Drag & drop the image you want to convert
2. Adjust parameters while previewing the result
3. Export the image

## Features

- Instant preview updates when parameters are changed
- Color adjustment tools
- Display palettes
- Save and load custom settings presets

## Settings Management

You can save and load your custom settings combinations.

### Settings File Format

Settings files use JSON format (`.qset` extension) containing:

- Qualetize parameters (tile size, palettes, dithering, etc.)
- Color correction values (brightness, contrast, gamma, etc.)
- Version information for compatibility

Example settings file structure can be found in `examples/genesis.qset`.

## Licence

This project is licensed under MIT License.

Third-party components are used under their respective licenses. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for details.
