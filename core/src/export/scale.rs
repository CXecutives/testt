//! The colour scale of a score, shared by every place that draws one: ten steps by decile
//! (0-9 ... 90-100), red through orange and yellow to green, clean and not neon (computed in
//! OKLCH). The app's ring (`--p-score-0` ... `--p-score-9` in `ui/src/styles/tokens.css`),
//! the HTML overview and the Excel score cells use this one table; `core/tests/ui_contract.rs`
//! checks that the tokens say the same. Excluded and unscored jobs are not on the scale.

/// A colour as CSS `hsl()` parts: hue in degrees, saturation and lightness in percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hsl {
    pub hue: u16,
    pub saturation: u8,
    pub lightness: u8,
}

const fn hsl(hue: u16, saturation: u8, lightness: u8) -> Hsl {
    Hsl {
        hue,
        saturation,
        lightness,
    }
}

/// Step `n` colours the scores `10 n ..= 10 n + 9` (the last step takes 100 too).
pub const SCORE_SCALE: [Hsl; 10] = [
    hsl(4, 62, 58),
    hsl(13, 69, 58),
    hsl(19, 75, 58),
    hsl(25, 79, 58),
    hsl(33, 80, 58),
    hsl(40, 77, 59),
    hsl(51, 58, 54),
    hsl(68, 43, 49),
    hsl(96, 35, 50),
    hsl(140, 41, 45),
];

/// The step of a score (its decile, 100 in the last one).
#[must_use]
pub const fn score_step(score: u8) -> usize {
    let step = score / 10;
    if step > 9 { 9 } else { step as usize }
}

impl Hsl {
    /// As CSS (`hsl(4 62% 58%)`).
    #[must_use]
    pub fn css(self) -> String {
        format!("hsl({} {}% {}%)", self.hue, self.saturation, self.lightness)
    }

    /// As 0xRRGGBB, rounded like a browser.
    #[must_use]
    pub fn rgb(self) -> u32 {
        let saturation = f64::from(self.saturation) / 100.0;
        let lightness = f64::from(self.lightness) / 100.0;
        let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let sector = f64::from(self.hue) / 60.0;
        let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
        let (red, green, blue) = match self.hue {
            0..60 => (chroma, second, 0.0),
            60..120 => (second, chroma, 0.0),
            120..180 => (0.0, chroma, second),
            180..240 => (0.0, second, chroma),
            240..300 => (second, 0.0, chroma),
            _ => (chroma, 0.0, second),
        };
        let base = lightness - chroma / 2.0;
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "0..=255 by construction"
        )]
        let byte = |channel: f64| ((channel + base) * 255.0).round() as u32;
        (byte(red) << 16) | (byte(green) << 8) | byte(blue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deciles_map_to_steps() {
        assert_eq!(score_step(0), 0);
        assert_eq!(score_step(9), 0);
        assert_eq!(score_step(10), 1);
        assert_eq!(score_step(89), 8);
        assert_eq!(score_step(90), 9);
        assert_eq!(score_step(100), 9);
    }

    #[test]
    fn colours_convert_like_a_browser() {
        // hsl(140 41% 45%) = rgb(68, 162, 99); hsl(4 62% 58%) = rgb(214, 90, 81).
        assert_eq!(SCORE_SCALE[9].rgb(), 0x0044_A263);
        assert_eq!(SCORE_SCALE[0].rgb(), 0x00D6_5A51);
        assert_eq!(SCORE_SCALE[0].css(), "hsl(4 62% 58%)");
    }
}
