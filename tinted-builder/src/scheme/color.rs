use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::TintedBuilderError;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Color {
    pub hex: (String, String, String),
    pub rgb: (u8, u8, u8),
    pub dec: (f32, f32, f32),
    pub hsl: (f32, f32, f32),
}

impl Color {
    pub fn new(hex_color: String) -> Result<Color, TintedBuilderError> {
        let hex_full = process_hex_input(&hex_color).ok_or(TintedBuilderError::HexInputFormat)?;
        let hex: (String, String, String) = (
            hex_full[0..2].to_lowercase(),
            hex_full[2..4].to_lowercase(),
            hex_full[4..6].to_lowercase(),
        );
        let rgb = hex_to_rgb(&hex)?;
        let dec: (f32, f32, f32) = (
            rgb.0 as f32 / 255.0,
            rgb.1 as f32 / 255.0,
            rgb.2 as f32 / 255.0,
        );
        let hsl = rgb_to_hsl(&rgb)?;

        Ok(Color { hex, rgb, dec, hsl })
    }

    pub fn to_hex(&self) -> String {
        format!("{}{}{}", &self.hex.0, &self.hex.1, &self.hex.2)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{}", &self.to_hex())
    }
}

fn process_hex_input(input: &str) -> Option<String> {
    // Check and process the hash prefix
    let hex_str = input.strip_prefix('#').unwrap_or(input);

    match hex_str.len() {
        // Convert 3-length hex to 6-length by duplicating each character
        3 => {
            if hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
                Some(
                    hex_str
                        .chars()
                        .flat_map(|c| std::iter::repeat(c).take(2))
                        .collect(),
                )
            } else {
                None // Contains invalid characters
            }
        }
        // Validate the 6-length hex value
        6 => {
            if hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
                Some(hex_str.to_string())
            } else {
                None // Contains invalid characters
            }
        }
        // Invalid length
        _ => None,
    }
}

fn hex_to_rgb(hex: &(String, String, String)) -> Result<(u8, u8, u8), TintedBuilderError> {
    let r = u8::from_str_radix(hex.0.as_str(), 16)?;
    let g = u8::from_str_radix(hex.1.as_str(), 16)?;
    let b = u8::from_str_radix(hex.2.as_str(), 16)?;

    Ok((r, g, b))
}

// Convert RGB tuple to HSL tuple
fn rgb_to_hsl(rgb: &(u8, u8, u8)) -> Result<(f32, f32, f32), TintedBuilderError> {
    let r = rgb.0 as f32 / 255.0;
    let g = rgb.1 as f32 / 255.0;
    let b = rgb.2 as f32 / 255.0;

    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let l = (max + min) / 2.0;
    let mut h = 0.0;
    let mut s = 0.0;

    if delta != 0.0 {
        s = if l > 0.5 {
            delta / (2.0 - max - min)
        } else {
            delta / (max + min)
        };

        if max == r {
            h = (g - b) / delta + if g < b { 6.0 } else { 0.0 };
        } else if max == g {
            h = (b - r) / delta + 2.0;
        } else if max == b {
            h = (r - g) / delta + 4.0;
        }
        h *= 60.0;
    }

    Ok((h, s, l))
}

// Convert HSL tuple to RGB tuple
fn hsl_to_rgb(hsl: &(f32, f32, f32)) -> (u8, u8, u8) {
    let h = hsl.0 / 360.0;
    let s = hsl.1;
    let l = hsl.2;

    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
        let g = hue_to_rgb(p, q, h);
        let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
        (r, g, b)
    };

    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

// Helper function for HSL to RGB conversion
fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
    let mut t = t;
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 0.5 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

// Adjust color hue, saturation, and lightness
pub fn adjust_hsl(
    color: &Color,
    hue_adj: f32,
    sat_adj: f32,
    light_adj: f32,
) -> Result<Color, TintedBuilderError> {
    let adjusted_hsl = (
        ((color.hsl.0 + hue_adj) % 360.0 + 360.0) % 360.0,
        (color.hsl.1 + ((1.0 - color.hsl.1) * sat_adj)).clamp(0.0, 1.0),
        (color.hsl.2 + ((1.0 - color.hsl.2) * light_adj)).clamp(0.0, 1.0),
    );
    let adjusted_rgb = hsl_to_rgb(&adjusted_hsl);
    Color::new(format!(
        "{:02x}{:02x}{:02x}",
        adjusted_rgb.0, adjusted_rgb.1, adjusted_rgb.2
    ))
}

// Tint a source color with a tint color at a given level
pub fn tint_color(source: &Color, tint: &Color, level: f32) -> Result<Color, TintedBuilderError> {
    if level < 0.0 || level > 1.0 {
        return Err(TintedBuilderError::InvalidTintLevel(
            "Level must be between 0.0 and 1.0".to_string(),
        ));
    }
    let t = level;
    let mut delta_h = tint.hsl.0 - source.hsl.0;
    if delta_h > 180.0 {
        delta_h -= 360.0;
    } else if delta_h < -180.0 {
        delta_h += 360.0;
    }
    let mut h = source.hsl.0 + t * delta_h;
    if h < 0.0 {
        h += 360.0;
    } else if h >= 360.0 {
        h -= 360.0;
    }
    let s = source.hsl.1 + t * (tint.hsl.1 - source.hsl.1);
    let l = source.hsl.2 + t * (tint.hsl.2 - source.hsl.2);
    let interp_hsl = (h, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    let interp_rgb = hsl_to_rgb(&interp_hsl);
    let hex_full = format!(
        "{:02x}{:02x}{:02x}",
        interp_rgb.0, interp_rgb.1, interp_rgb.2
    );
    Color::new(hex_full)
}

// Generate n lightness gradients colors between two colors
pub fn generate_lightness_gradient(
    color1: &Color,
    color2: &Color,
    n: usize,
) -> Result<Vec<Color>, TintedBuilderError> {
    if n == 0 {
        return Ok(vec![]);
    }
    let mut colors = vec![];
    for i in 0..n {
        let t = if n == 1 {
            0.0
        } else {
            (i as f32) / ((n - 1) as f32)
        };
        let mut delta_h = color2.hsl.0 - color1.hsl.0;
        if delta_h > 180.0 {
            delta_h -= 360.0;
        } else if delta_h < -180.0 {
            delta_h += 360.0;
        }
        let mut h = color1.hsl.0 + t * delta_h;
        if h < 0.0 {
            h += 360.0;
        } else if h >= 360.0 {
            h -= 360.0;
        }
        let s = (color2.hsl.1 - color1.hsl.1) * 0.5;
        let l = color1.hsl.2 + t * (color2.hsl.2 - color1.hsl.2);
        let interp_hsl = (h, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
        let interp_rgb = hsl_to_rgb(&interp_hsl);
        let hex_full = format!(
            "{:02x}{:02x}{:02x}",
            interp_rgb.0, interp_rgb.1, interp_rgb.2
        );
        colors.push(Color::new(hex_full)?);
    }
    Ok(colors)
}

// Invert the lightness of a color to generate a complementary color
pub fn invert_lightness(color: &Color) -> Result<Color, TintedBuilderError> {
    let bright_l = 1.0 - color.hsl.2;
    let bright_hsl = (color.hsl.0, color.hsl.1, bright_l.clamp(0.0, 1.0));
    let bright_rgb = hsl_to_rgb(&bright_hsl);
    let hex_full = format!(
        "{:02x}{:02x}{:02x}",
        bright_rgb.0, bright_rgb.1, bright_rgb.2
    );
    Color::new(hex_full)
}
