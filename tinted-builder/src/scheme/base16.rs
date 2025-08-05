use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashMap, fmt};

pub use crate::scheme::color::Color;

use crate::scheme::color::{
    adjust_brightness_and_saturation, generate_grayish_gradient, invert_lightness, tint_color,
};
use crate::{utils::slugify, SchemeSystem, SchemeVariant};

pub(crate) const REQUIRED_BASE16_PALETTE_KEYS: [&str; 16] = [
    "base00", "base01", "base02", "base03", "base04", "base05", "base06", "base07", "base08",
    "base09", "base0A", "base0B", "base0C", "base0D", "base0E", "base0F",
];

pub(crate) const REQUIRED_BASE24_PALETTE_KEYS: [&str; 24] = [
    "base00", "base01", "base02", "base03", "base04", "base05", "base06", "base07", "base08",
    "base09", "base0A", "base0B", "base0C", "base0D", "base0E", "base0F", "base10", "base11",
    "base12", "base13", "base14", "base15", "base16", "base17",
];

pub(crate) const OPT_BASE16_PALETTE_KEYS: [(&str, &str); 7] = [
    ("opt08", "base08"),
    ("opt09", "base09"),
    ("opt0A", "base0A"),
    ("opt0B", "base0B"),
    ("opt0C", "base0C"),
    ("opt0D", "base0D"),
    ("opt0E", "base0E"),
];

pub(crate) const REQUIRED_ANSI8_PALETTE_KEYS: [(&str, &str); 8] = [
    ("ansi0", "base00"),
    ("ansi1", "base08"),
    ("ansi2", "base0B"),
    ("ansi3", "base0A"),
    ("ansi4", "base0D"),
    ("ansi5", "base0E"),
    ("ansi6", "base0C"),
    ("ansi7", "base05"),
];

#[derive(Deserialize, Serialize)]
struct SchemeWrapper {
    pub(crate) system: SchemeSystem,
    pub(crate) name: String,
    pub(crate) slug: Option<String>,
    pub(crate) author: String,
    pub(crate) description: Option<String>,
    pub(crate) variant: Option<SchemeVariant>,
    pub(crate) bright: Option<f32>,
    pub(crate) saturation: Option<f32>,
    pub(crate) palette: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Base16Scheme {
    pub system: SchemeSystem,
    pub name: String,
    pub slug: String,
    pub author: String,
    pub description: Option<String>,
    pub variant: SchemeVariant,
    pub bright: Option<f32>,
    pub saturation: Option<f32>,
    pub palette: HashMap<String, Color>,
    pub provided_palette_keys: Vec<String>,
}

/// Generates optional colors (opt08 to opt0E) for the palette if they are not present.
fn generate_optional_colors<D: serde::de::Error>(
    palette: &mut HashMap<String, Color>,
    brgt_adj: f32,
    sat_adj: f32,
) -> Result<(), D> {
    let opt_pairs = &OPT_BASE16_PALETTE_KEYS[..];

    for (opt_key, base_key) in opt_pairs {
        if !palette.contains_key(&opt_key.to_string()) {
            let base_color = palette
                .get(&base_key.to_string())
                .ok_or(D::custom(format!(
                    "Missing base key {} for generating {}",
                    base_key, opt_key
                )))?
                .clone();
            let new_color = adjust_brightness_and_saturation(&base_color, brgt_adj, sat_adj)
                .map_err(D::custom)?;
            palette.insert(opt_key.to_string(), new_color);
        }
    }
    Ok(())
}

impl fmt::Display for Base16Scheme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "author: \"{}\"", self.author)?;
        if let Some(ref desc) = self.description {
            writeln!(f, "description: \"{}\"", desc)?;
        }
        writeln!(f, "name: \"{}\"", self.name)?;
        writeln!(f, "slug: \"{}\"", self.slug)?;
        writeln!(f, "system: \"{}\"", self.system)?;
        writeln!(f, "variant: \"{}\"", self.variant)?;
        if let Some(bright) = self.bright {
            writeln!(f, "bright: {}", bright)?;
        }
        if let Some(saturation) = self.saturation {
            writeln!(f, "saturation: {}", saturation)?;
        }
        writeln!(f, "palette:")?;

        let mut palette_vec: Vec<(String, Color)> = self
            .palette
            .clone()
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .filter(|(k, _)| self.provided_palette_keys.contains(k))
            .collect();
        palette_vec.sort_by_key(|k| k.0.clone());

        for (key, value) in palette_vec {
            writeln!(f, "  {}: \"{}\"", key, value)?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Base16Scheme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wrapper = SchemeWrapper::deserialize(deserializer)?;
        let slug = wrapper
            .slug
            .map_or(slugify(&wrapper.name), |slug| slugify(&slug));
        let variant = wrapper.variant.unwrap_or(SchemeVariant::Dark);
        let brgt_adj = wrapper.bright.unwrap_or(if variant == SchemeVariant::Dark {
            0.1
        } else {
            -0.1
        });
        let sat_adj = wrapper
            .saturation
            .unwrap_or(if variant == SchemeVariant::Dark {
                -0.2
            } else {
                0.2
            });

        let provided_palette_keys: Vec<String> = wrapper.palette.keys().cloned().collect();
        let mut palette = HashMap::new();

        match wrapper.system {
            SchemeSystem::Ansi8 => {
                let contains_all_keys = REQUIRED_ANSI8_PALETTE_KEYS
                    .iter()
                    .all(|(ansi_key, _)| wrapper.palette.contains_key(*ansi_key));

                if !contains_all_keys {
                    return Err(serde::de::Error::custom(format!(
                        "{} scheme does not contain the required palette properties",
                        wrapper.system
                    )));
                }

                for &(ansi_key, base_key) in &REQUIRED_ANSI8_PALETTE_KEYS {
                    if let Some(hex) = wrapper.palette.get(ansi_key) {
                        let color = Color::new(hex.clone())
                            .map_err(|e| serde::de::Error::custom(e.to_string()))?;
                        palette.insert(base_key.to_string(), color);
                    }
                }

                // Generate base07
                let base00 = palette
                    .get("base00")
                    .ok_or(serde::de::Error::custom("Missing base00"))?
                    .clone();
                let base07 = invert_lightness(&base00).map_err(serde::de::Error::custom)?;
                palette.insert("base07".to_string(), base07);

                // Generate base01 to base04 from gradient between base00 and base05
                let base05 = palette
                    .get("base05")
                    .ok_or(serde::de::Error::custom("Missing base05"))?
                    .clone();
                let gradient01_05 = generate_grayish_gradient(&base00, &base05, 6)
                    .map_err(serde::de::Error::custom)?;
                palette.insert("base01".to_string(), gradient01_05[1].clone());
                palette.insert("base02".to_string(), gradient01_05[2].clone());
                palette.insert("base03".to_string(), gradient01_05[3].clone());
                palette.insert("base04".to_string(), gradient01_05[4].clone());

                // Generate base06 from gradient between base05 and base07m
                let base07 = palette
                    .get("base07")
                    .ok_or(serde::de::Error::custom("Missing base07"))?
                    .clone();
                let gradient05_07 = generate_grayish_gradient(&base05, &base07, 3)
                    .map_err(serde::de::Error::custom)?;
                palette.insert("base06".to_string(), gradient05_07[1].clone());

                // Generate base09 (orange: tint between red and yellow)
                let base08 = palette
                    .get("base08")
                    .ok_or(serde::de::Error::custom("Missing base08"))?
                    .clone();
                let base0a = palette
                    .get("base0A")
                    .ok_or(serde::de::Error::custom("Missing base0A"))?
                    .clone();
                let base09 = tint_color(&base08, &base0a, 0.5).map_err(serde::de::Error::custom)?;
                palette.insert("base09".to_string(), base09);

                // Generate base0F (brown: tint between red and green)
                let base0b = palette
                    .get("base0B")
                    .ok_or(serde::de::Error::custom("Missing base0B"))?
                    .clone();
                let base0f = tint_color(&base08, &base0b, 0.5).map_err(serde::de::Error::custom)?;
                palette.insert("base0f".to_string(), base0f);

                // Generate opt08 to opt0E
                generate_optional_colors(&mut palette, brgt_adj, sat_adj)?;

                Ok(Base16Scheme {
                    name: wrapper.name,
                    slug,
                    system: wrapper.system, // Retain Ansi8 for serialization
                    author: wrapper.author,
                    description: wrapper.description,
                    variant,
                    bright: wrapper.bright,
                    saturation: wrapper.saturation,
                    palette,
                    provided_palette_keys: provided_palette_keys,
                })
            }
            SchemeSystem::Base16 => {
                let contains_all_keys = REQUIRED_BASE16_PALETTE_KEYS
                    .iter()
                    .all(|&key| wrapper.palette.contains_key(key));

                if !contains_all_keys {
                    return Err(serde::de::Error::custom(format!(
                        "{} scheme does not contain the required palette properties",
                        wrapper.system
                    )));
                }

                let palette_result: Result<HashMap<String, Color>, _> = wrapper
                    .palette
                    .into_iter()
                    .map(|(key, value)| {
                        Color::new(value)
                            .map_err(|e| serde::de::Error::custom(e.to_string()))
                            .map(|color| (key, color))
                    })
                    .collect();

                let mut palette = palette_result?;

                // Generate opt08 to opt0E
                generate_optional_colors(&mut palette, brgt_adj, sat_adj)?;

                Ok(Base16Scheme {
                    name: wrapper.name,
                    slug,
                    system: wrapper.system,
                    author: wrapper.author,
                    description: wrapper.description,
                    variant,
                    bright: wrapper.bright,
                    saturation: wrapper.saturation,
                    palette,
                    provided_palette_keys: provided_palette_keys,
                })
            }
            SchemeSystem::Base24 => {
                let contains_all_keys = REQUIRED_BASE24_PALETTE_KEYS
                    .iter()
                    .all(|&key| wrapper.palette.contains_key(key));

                if !contains_all_keys {
                    return Err(serde::de::Error::custom(format!(
                        "{} scheme does not contain the required palette properties",
                        wrapper.system
                    )));
                }

                let palette_result: Result<HashMap<String, Color>, _> = wrapper
                    .palette
                    .into_iter()
                    .map(|(key, value)| {
                        Color::new(value)
                            .map_err(|e| serde::de::Error::custom(e.to_string()))
                            .map(|color| (key, color))
                    })
                    .collect();

                Ok(Base16Scheme {
                    name: wrapper.name,
                    slug,
                    system: wrapper.system,
                    author: wrapper.author,
                    description: wrapper.description,
                    variant,
                    bright: wrapper.bright,
                    saturation: wrapper.saturation,
                    palette: palette_result?,
                    provided_palette_keys: provided_palette_keys,
                })
            }
            SchemeSystem::List | SchemeSystem::ListBase16 | SchemeSystem::ListBase24 => {
                Err(serde::de::Error::custom(format!(
                    "{} is not a valid Scheme system for a specific scheme",
                    wrapper.system
                )))
            }
        }
    }
}

impl Serialize for Base16Scheme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Scheme", 7)?;
        state.serialize_field("system", &self.system)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("slug", &self.slug)?;
        state.serialize_field("author", &self.author)?;
        if let Some(description) = &self.description {
            state.serialize_field("description", description)?;
        }
        state.serialize_field("variant", &self.variant)?;
        if let Some(bright) = &self.bright {
            state.serialize_field("bright", bright)?;
        }
        if let Some(saturation) = &self.saturation {
            state.serialize_field("saturation", saturation)?;
        }

        // Collect and sort the palette by key, including only provided keys
        let mut sorted_palette: Vec<(&String, &Color)> = self
            .palette
            .iter()
            .filter(|(k, _)| self.provided_palette_keys.contains(k))
            .collect();
        sorted_palette.sort_by(|a, b| a.0.cmp(b.0));

        // Serialize the filtered palette as a map within the struct
        state.serialize_field("palette", &SortedPalette(sorted_palette))?;

        state.end()
    }
}

// Helper struct for serializing sorted palette
struct SortedPalette<'a>(Vec<(&'a String, &'a Color)>);

impl<'a> Serialize for SortedPalette<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, format!("#{}", &value.to_hex()).as_str())?;
        }
        map.end()
    }
}
