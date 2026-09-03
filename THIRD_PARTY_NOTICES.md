# Third Party Notices

This application uses the following third-party components:

## Fonts

### Inter Font Family
- **License**: SIL Open Font License 1.1
- **Files**:
  - `assets/fonts/Inter-Medium.ttf`
  - `assets/fonts/Inter-ExtraBold.ttf`
- **Source**: https://github.com/rsms/inter
- **Copyright**: Copyright (c) 2016 The Inter Project Authors

## Sample Images

### carina-nebula.png
- **Title**: "Cosmic Cliffs" in the Carina Nebula (NIRCam Image), reduced to 256x144
- **Credit**: NASA, ESA, CSA, STScI
- **Source**: https://science.nasa.gov/asset/webb/cosmic-cliffs-in-the-carina-nebula-nircam-image/
- **Terms**: NASA content is generally not subject to copyright; used with the credit above

## Major Libraries

### qualetize
- **License**: Unlicense license
- **Purpose**: Tile-based Image Quantization Tool
- **Source**: https://github.com/Aikku93/qualetize

### tiledpalettequant
- **License**: MIT
- **Purpose**: Tile-based image quantizer (alternate engine)
- **Author**: rilden
- **Source**: https://github.com/rilden/tiledpalettequant

### tilepalquant
- **License**: MIT
- **Purpose**: C++ port of tiledpalettequant, ported to Rust as this app's tilepalquant engine
- **Author**: bbbbbr
- **Source**: https://github.com/bbbbbr/tilepalquant

### egui
- **License**: MIT OR Apache-2.0
- **Purpose**: Immediate mode GUI framework
- **Source**: https://github.com/emilk/egui

### image
- **License**: MIT
- **Purpose**: Image processing library
- **Source**: https://github.com/image-rs/image

---

*For a complete list of all dependencies and their licenses, run:*
```sh
cargo install cargo-about
cargo about generate about.hbs
