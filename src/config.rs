use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read configuration: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid configuration: {0}")]
    Invalid(&'static str),
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub hotkeys: Hotkeys,
    pub camera: CameraConfig,
    pub flashlight: Flashlight,
    pub appearance: Appearance,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Hotkeys {
    pub activate: String,
}
impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            activate: "Ctrl+Alt+Z".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CameraConfig {
    pub min_scale: f64,
    pub max_scale: f64,
    pub wheel_factor: f64,
    pub zoom_half_life_ms: f64,
    pub pan_half_life_ms: f64,
}
impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            min_scale: 0.5,
            max_scale: 16.0,
            wheel_factor: 1.2,
            zoom_half_life_ms: 90.0,
            pan_half_life_ms: 120.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Flashlight {
    pub radius_px: f64,
    pub min_radius_px: f64,
    pub max_radius_px: f64,
    pub outside_brightness: f64,
}
impl Default for Flashlight {
    fn default() -> Self {
        Self {
            radius_px: 240.0,
            min_radius_px: 40.0,
            max_radius_px: 1200.0,
            outside_brightness: 0.28,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    #[default]
    Linear,
    Nearest,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub filter: Filter,
}

impl Config {
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(input)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let c = &self.camera;
        if !in_range(c.min_scale, 0.01, 1.0)
            || !in_range(c.max_scale, 1.0, 1024.0)
            || !in_range(c.wheel_factor, 1.001, 4.0)
            || !in_range(c.zoom_half_life_ms, 1.0, 2000.0)
            || !in_range(c.pan_half_life_ms, 0.0, 2000.0)
        {
            return Err(ConfigError::Invalid(
                "camera: scale must bracket 1 (0.01..1024), wheel factor 1.001..4, zoom half-life 1..2000 ms, pan half-life 0..2000 ms",
            ));
        }
        let f = &self.flashlight;
        if !in_range(f.min_radius_px, 1.0, 32768.0)
            || !in_range(f.max_radius_px, f.min_radius_px, 32768.0)
            || !in_range(f.radius_px, f.min_radius_px, f.max_radius_px)
            || !in_range(f.outside_brightness, 0.0, 1.0)
        {
            return Err(ConfigError::Invalid(
                "flashlight: ordered positive radii <=32768 and brightness 0..1 required",
            ));
        }
        validate_shortcut(&self.hotkeys.activate)?;
        Ok(())
    }
}

fn validate_shortcut(shortcut: &str) -> Result<(), ConfigError> {
    let parts: Vec<_> = shortcut.split('+').collect();
    let invalid = || {
        ConfigError::Invalid(
            "shortcut: use modifiers Ctrl/Alt/Shift/Super and one A-Z, 0-9 or F1-F24 key (e.g. Ctrl+Alt+Z)",
        )
    };
    if parts.is_empty() || parts.len() > 5 {
        return Err(invalid());
    }
    let mut modifiers = 0;
    for part in &parts[..parts.len() - 1] {
        let bit = match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => 1,
            "alt" => 2,
            "shift" => 4,
            "super" => 8,
            _ => return Err(invalid()),
        };
        if modifiers & bit != 0 {
            return Err(invalid());
        }
        modifiers |= bit;
    }
    let key = parts[parts.len() - 1].to_ascii_uppercase();
    let valid = (key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric())
        || key
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n));
    if !valid {
        return Err(invalid());
    }
    Ok(())
}

fn in_range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

pub fn path() -> Result<PathBuf, ConfigError> {
    let base = std::env::var_os("APPDATA").ok_or(ConfigError::Invalid(
        "APPDATA is unavailable; using in-memory defaults",
    ))?;
    Ok(PathBuf::from(base).join("Zoomer").join("config.toml"))
}

/// Missing configuration uses defaults in memory; files are never created or overwritten.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(input) => Config::parse(&input),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_partial_config() {
        Config::default().validate().unwrap();
        let c = Config::parse("[appearance]\nfilter = 'nearest'").unwrap();
        assert_eq!(c.appearance.filter, Filter::Nearest);
        assert_eq!(c.camera.max_scale, 16.0);
        Config::parse(include_str!("../config.example.toml")).unwrap();
    }
    #[test]
    fn rejects_nonfinite_and_bad_ranges() {
        for input in [
            "[camera]\nmin_scale=nan",
            "[camera]\nmax_scale=inf",
            "[camera]\nzoom_half_life_ms=0",
            "[camera]\nwheel_factor=1",
            "[flashlight]\nradius_px=-1",
            "[flashlight]\noutside_brightness=1.1",
            "[flashlight]\nmax_radius_px=10",
            "[appearance]\nfilter='cubic'",
            "unknown=1",
            "[camera]\nmin_scale=2",
        ] {
            assert!(Config::parse(input).is_err(), "{input}");
        }
    }
    #[test]
    fn malformed_config_is_not_rewritten() {
        let directory =
            std::env::temp_dir().join(format!("zoomer-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let p = directory.join("config.toml");
        std::fs::write(&p, "[broken").unwrap();
        assert!(load(&p).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "[broken");
        std::fs::remove_file(p).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn shortcut_validation_is_portable_and_bounded() {
        for key in ["Ctrl+Alt+Z", "Shift+F24", "Super+1", "control+alt+x"] {
            validate_shortcut(key).unwrap();
        }
        for key in [
            "",
            "Ctrl+",
            "Ctrl+Ctrl+Z",
            "Ctrl+Z+Alt",
            "F25",
            "Alt+😃",
            "Ctrl+Alt+Unknown",
        ] {
            assert!(validate_shortcut(key).is_err(), "{key}");
        }
    }
}
