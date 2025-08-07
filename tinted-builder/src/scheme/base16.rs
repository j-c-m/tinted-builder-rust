use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashMap, fmt};

pub use crate::scheme::color::Color;

use crate::scheme::color::{adjust_hsl, generate_lightness_gradient, invert_lightness, tint_color};
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

pub(crate) const BRIGHT_BASE16_PALETTE_KEYS: [(&str, &str); 8] = [
    ("bright08", "base08"),
    ("bright09", "base09"),
    ("bright0A", "base0A"),
    ("bright0B", "base0B"),
    ("bright0C", "base0C"),
    ("bright0D", "base0D"),
    ("bright0E", "base0E"),
    ("bright0F", "base0F"),
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
    pub(crate) bright_adj_hsl: Option<BrightAdjHSL>,
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
    pub bright_adj_hsl: Option<BrightAdjHSL>,
    pub palette: HashMap<String, Color>,
    pub provided_palette_keys: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BrightAdjHSL {
    hue: f32,
    saturation: f32,
    lightness: f32,
}

/// Generates bright colors (bright08 to bright0F) for the palette if they are not present.
fn generate_bright_colors<D: serde::de::Error>(
    palette: &mut HashMap<String, Color>,
    hue_adj: f32,
    sat_adj: f32,
    light_adj: f32,
) -> Result<(), D> {
    let bt_pairs = &BRIGHT_BASE16_PALETTE_KEYS[..];

    for (bt_key, base_key) in bt_pairs {
        if !palette.contains_key(&bt_key.to_string()) {
            let base_color = palette
                .get(&base_key.to_string())
                .ok_or(D::custom(format!(
                    "Missing base key {} for generating {}",
                    base_key, bt_key
                )))?
                .clone();
            let new_color =
                adjust_hsl(&base_color, hue_adj, sat_adj, light_adj).map_err(D::custom)?;
            palette.insert(bt_key.to_string(), new_color);
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
        if let Some(adj) = &self.bright_adj_hsl {
            writeln!(f, "bright_adj_hsl:")?;
            writeln!(f, "  hue: {}", adj.hue)?;
            writeln!(f, "  saturation: {}", adj.saturation)?;
            writeln!(f, "  lightness: {}", adj.lightness)?;
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
        let hue_adj = wrapper.bright_adj_hsl.as_ref().map_or(0.0, |adj| adj.hue);
        let sat_adj = wrapper.bright_adj_hsl.as_ref().map_or(
            if variant == SchemeVariant::Dark {
                0.0
            } else {
                0.25
            },
            |adj| adj.saturation,
        );
        let light_adj = wrapper.bright_adj_hsl.as_ref().map_or(
            if variant == SchemeVariant::Dark {
                0.25
            } else {
                -0.25
            },
            |adj| adj.lightness,
        );

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
                let gradient01_05 = generate_lightness_gradient(&base00, &base05, 6)
                    .map_err(serde::de::Error::custom)?;
                palette.insert("base01".to_string(), gradient01_05[1].clone());
                palette.insert("base02".to_string(), gradient01_05[2].clone());
                palette.insert("base03".to_string(), gradient01_05[3].clone());
                palette.insert("base04".to_string(), gradient01_05[4].clone());

                // Generate base06 from gradient between base05 and base07
                let base07 = palette
                    .get("base07")
                    .ok_or(serde::de::Error::custom("Missing base07"))?
                    .clone();
                let gradient05_07 = generate_lightness_gradient(&base05, &base07, 3)
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

                // Generate base0F (brown: tint between base09 (orange) and black)
                let base09 = palette
                    .get("base09")
                    .ok_or(serde::de::Error::custom("Missing base09"))?
                    .clone();
                let base0f = tint_color(
                    &base09,
                    &Color::new("#000000".to_string()).map_err(serde::de::Error::custom)?,
                    0.2,
                )
                .map_err(serde::de::Error::custom)?;
                palette.insert("base0F".to_string(), base0f);

                // Generate bright08 to bright0F
                generate_bright_colors(&mut palette, hue_adj, sat_adj, light_adj)?;

                Ok(Base16Scheme {
                    name: wrapper.name,
                    slug,
                    system: wrapper.system, // Retain Ansi8 for serialization
                    author: wrapper.author,
                    description: wrapper.description,
                    variant,
                    bright_adj_hsl: wrapper.bright_adj_hsl,
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

                // Generate bright08 to bright0F
                generate_bright_colors(&mut palette, hue_adj, sat_adj, light_adj)?;

                Ok(Base16Scheme {
                    name: wrapper.name,
                    slug,
                    system: wrapper.system,
                    author: wrapper.author,
                    description: wrapper.description,
                    variant,
                    bright_adj_hsl: wrapper.bright_adj_hsl,
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
                    bright_adj_hsl: wrapper.bright_adj_hsl,
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
        if let Some(bright_adj_hsl) = &self.bright_adj_hsl {
            state.serialize_field("bright_adj_hsl", bright_adj_hsl)?;
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
