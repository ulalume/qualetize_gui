//! The RGB color the algorithm works in, and the metrics over it.
//!
//! Everything is `f64` and in the sRGB byte range 0..=255: the optimization
//! moves colors by fractions of a step, so the values only become integers
//! again when they are snapped to the target format's levels.

/// One color, channels in 0.0..=255.0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Rgb {
    pub const fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b }
    }

    pub fn from_u8(rgb: [u8; 3]) -> Self {
        Self::new(rgb[0] as f64, rgb[1] as f64, rgb[2] as f64)
    }

    /// The channels truncated to bytes, as the output palette takes them.
    /// Values that went through a level table are whole numbers already.
    pub fn to_u8(self) -> [u8; 3] {
        [self.r as u8, self.g as u8, self.b as u8]
    }

    pub fn add(&mut self, other: Self) {
        self.r += other.r;
        self.g += other.g;
        self.b += other.b;
    }

    pub fn subtract(&mut self, other: Self) {
        self.r -= other.r;
        self.g -= other.g;
        self.b -= other.b;
    }

    pub fn scale(&mut self, factor: f64) {
        self.r *= factor;
        self.g *= factor;
        self.b *= factor;
    }

    pub fn clamp(&mut self, low: f64, high: f64) {
        self.r = self.r.clamp(low, high);
        self.g = self.g.clamp(low, high);
        self.b = self.b.clamp(low, high);
    }

    /// Every channel squared. The algorithm's stand-in for a linear light
    /// space.
    pub fn to_linear(self) -> Self {
        Self::new(to_linear(self.r), to_linear(self.g), to_linear(self.b))
    }

    /// The inverse of [`Rgb::to_linear`].
    pub fn to_srgb(self) -> Self {
        Self::new(self.r.sqrt(), self.g.sqrt(), self.b.sqrt())
    }
}

pub fn to_linear(x: f64) -> f64 {
    x * x
}

/// Weighted squared difference of two colors, weights 2, 4 and 1 on R, G and B.
pub fn color_distance(a: Rgb, b: Rgb) -> f64 {
    (2.0 * ((a.r - b.r) * (a.r - b.r)))
        + (4.0 * ((a.g - b.g) * (a.g - b.g)))
        + ((a.b - b.b) * (a.b - b.b))
}

/// Perceived brightness, measured on the linearized channels. Only the
/// ordering matters: it sorts the dither candidates of one pixel.
pub fn brightness(color: Rgb) -> f64 {
    let mut sum = 0.0;
    sum += 0.299 * to_linear(color.r);
    sum += 0.587 * to_linear(color.g);
    sum += 0.114 * to_linear(color.b);
    sum
}

/// Move `color` a fraction `alpha` of the way towards `target`.
///
/// `alpha` is `f32` because the reference implementation holds it in a
/// `float`, and the widening happens after `1 - alpha` is computed.
pub fn move_color_closer(color: &mut Rgb, target: Rgb, alpha: f32) {
    let keep = f64::from(1.0 - alpha);
    let take = f64::from(alpha);
    color.r = (keep * color.r) + (take * target.r);
    color.g = (keep * color.g) + (take * target.g);
    color.b = (keep * color.b) + (take * target.b);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_weights_green_most_and_blue_least() {
        let black = Rgb::new(0.0, 0.0, 0.0);
        assert_eq!(color_distance(black, Rgb::new(10.0, 0.0, 0.0)), 200.0);
        assert_eq!(color_distance(black, Rgb::new(0.0, 10.0, 0.0)), 400.0);
        assert_eq!(color_distance(black, Rgb::new(0.0, 0.0, 10.0)), 100.0);
    }

    #[test]
    fn distance_is_zero_for_equal_colors() {
        let color = Rgb::new(12.5, 200.0, 7.0);
        assert_eq!(color_distance(color, color), 0.0);
    }

    #[test]
    fn brightness_ranks_white_above_the_primaries_above_black() {
        let white = brightness(Rgb::new(255.0, 255.0, 255.0));
        let green = brightness(Rgb::new(0.0, 255.0, 0.0));
        let blue = brightness(Rgb::new(0.0, 0.0, 255.0));
        assert!(white > green && green > blue && blue > brightness(Rgb::default()));
    }

    #[test]
    fn moving_closer_interpolates_towards_the_target() {
        let mut color = Rgb::new(0.0, 100.0, 200.0);
        move_color_closer(&mut color, Rgb::new(100.0, 100.0, 100.0), 0.5);
        assert_eq!(color, Rgb::new(50.0, 100.0, 150.0));
    }

    #[test]
    fn an_alpha_of_one_lands_on_the_target() {
        let target = Rgb::new(3.0, 5.0, 7.0);
        let mut color = Rgb::new(200.0, 100.0, 0.0);
        move_color_closer(&mut color, target, 1.0);
        assert_eq!(color, target);
    }

    #[test]
    fn linear_and_srgb_round_trip() {
        let color = Rgb::new(16.0, 64.0, 255.0);
        assert_eq!(color.to_linear(), Rgb::new(256.0, 4096.0, 65025.0));
        assert_eq!(color.to_linear().to_srgb(), color);
    }
}
