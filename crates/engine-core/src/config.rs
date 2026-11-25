//! Typed RON engine configuration with per-module sections.

use std::collections::BTreeMap;
use std::path::Path;

use ron::value::RawValue;

use crate::error::ConfigError;

/// Maximum accepted config file size (1 MiB).
pub const MAX_CONFIG_BYTES: u64 = 1_048_576;

/// The `engine` section of the config.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EngineSection {
    /// Application name.
    pub app_name: String,
    /// Tracing filter directive.
    pub log_filter: String,
    /// Number of frames kept in frame-timing history.
    pub frame_history: usize,
}

impl Default for EngineSection {
    fn default() -> Self {
        Self {
            app_name: "engine-app".to_owned(),
            log_filter: "info".to_owned(),
            frame_history: 120,
        }
    }
}

/// Whole engine configuration.
#[derive(Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EngineConfig {
    /// Engine-level settings.
    pub engine: EngineSection,
    /// Raw per-module sections, parsed lazily by each module.
    pub modules: BTreeMap<String, Box<RawValue>>,
}

impl std::fmt::Debug for EngineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineConfig")
            .field("engine", &self.engine)
            .field("modules", &self.modules.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl EngineConfig {
    /// Parse from RON text.
    pub fn from_ron_str(text: &str) -> Result<Self, ConfigError> {
        ron::from_str::<EngineConfig>(text).map_err(ConfigError::Parse)
    }

    /// Load from a file, rejecting files over [`MAX_CONFIG_BYTES`] before reading.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let io = |source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        };
        let size = std::fs::metadata(path).map_err(io)?.len();
        if size > MAX_CONFIG_BYTES {
            return Err(ConfigError::TooLarge {
                path: path.to_path_buf(),
                size,
                limit: MAX_CONFIG_BYTES,
            });
        }
        let text = std::fs::read_to_string(path).map_err(io)?;
        Self::from_ron_str(&text)
    }

    /// View of the named module's section (possibly absent).
    pub fn module_config(&self, name: &str) -> ModuleConfig<'_> {
        match self.modules.get_key_value(name) {
            Some((k, v)) => ModuleConfig {
                name: k.as_str(),
                section: Some(v),
            },
            None => ModuleConfig {
                name: "",
                section: None,
            },
        }
    }

    /// Names of all module sections present.
    pub fn module_section_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.modules.keys().map(String::as_str)
    }
}

/// A module's view of its config section.
#[derive(Clone, Copy)]
pub struct ModuleConfig<'a> {
    name: &'a str,
    section: Option<&'a RawValue>,
}

impl<'a> ModuleConfig<'a> {
    /// A config with no section.
    pub fn empty(name: &'a str) -> Self {
        Self {
            name,
            section: None,
        }
    }

    /// Module name.
    pub fn name(&self) -> &'a str {
        self.name
    }

    /// Whether a section exists.
    pub fn is_present(&self) -> bool {
        self.section.is_some()
    }

    /// Parse the section; `None` if absent.
    pub fn parse<T: serde::de::DeserializeOwned>(&self) -> Result<Option<T>, ConfigError> {
        self.section
            .map(|raw| raw.into_rust::<T>())
            .transpose()
            .map_err(|source| ConfigError::ModuleSection {
                module: self.name.to_owned(),
                source,
            })
    }

    /// Parse the section, or `T::default()` if absent.
    pub fn parse_or_default<T: serde::de::DeserializeOwned + Default>(
        &self,
    ) -> Result<T, ConfigError> {
        Ok(self.parse::<T>()?.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq, serde::Deserialize)]
    #[serde(default)]
    struct AudioCfg {
        volume: f32,
        muted: bool,
    }

    #[test]
    fn config_parses_engine_and_modules() {
        let c = EngineConfig::from_ron_str(
            r#"(engine: (app_name: "demo", log_filter: "debug", frame_history: 8), modules: { "audio": (volume: 0.5, muted: false) })"#,
        )
        .unwrap();
        assert_eq!(c.engine.app_name, "demo");
        assert_eq!(c.engine.log_filter, "debug");
        assert_eq!(c.engine.frame_history, 8);
        assert_eq!(
            c.module_config("audio").parse::<AudioCfg>().unwrap(),
            Some(AudioCfg {
                volume: 0.5,
                muted: false
            })
        );
        assert_eq!(c.module_section_names().collect::<Vec<_>>(), ["audio"]);
        assert!(format!("{c:?}").contains("audio"));
    }

    #[test]
    fn config_defaults() {
        let c = EngineConfig::from_ron_str("()").unwrap();
        assert_eq!(c.engine.app_name, "engine-app");
        assert_eq!(c.engine.log_filter, "info");
        assert_eq!(c.engine.frame_history, 120);
        assert!(c.modules.is_empty());
    }

    #[test]
    fn config_missing_module_section() {
        let c = EngineConfig::from_ron_str("()").unwrap();
        let m = c.module_config("absent");
        assert!(!m.is_present());
        assert_eq!(m.parse::<AudioCfg>().unwrap(), None);
        assert_eq!(
            m.parse_or_default::<AudioCfg>().unwrap(),
            AudioCfg::default()
        );
        assert_eq!(ModuleConfig::empty("x").name(), "x");
    }

    #[test]
    fn config_bad_ron() {
        assert!(matches!(
            EngineConfig::from_ron_str("(engine: (app_name: 5"),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn config_unknown_field() {
        assert!(matches!(
            EngineConfig::from_ron_str("(engine: (bogus: 1))"),
            Err(ConfigError::Parse(_))
        ));
        assert!(matches!(
            EngineConfig::from_ron_str("(nope: 1)"),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn config_bad_module_section() {
        let c = EngineConfig::from_ron_str(r#"(modules: { "audio": (volume: "loud") })"#).unwrap();
        match c.module_config("audio").parse::<AudioCfg>() {
            Err(ConfigError::ModuleSection { module, .. }) => assert_eq!(module, "audio"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn config_load_too_large() {
        let path = std::env::temp_dir().join(format!("engine-core-big-{}.ron", std::process::id()));
        std::fs::write(&path, vec![b' '; (MAX_CONFIG_BYTES + 1) as usize]).unwrap();
        let r = EngineConfig::load(&path);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(r, Err(ConfigError::TooLarge { .. })));
    }

    #[test]
    fn config_load_ok() {
        let path = std::env::temp_dir().join(format!("engine-core-ok-{}.ron", std::process::id()));
        std::fs::write(&path, "(engine: (app_name: \"f\"))").unwrap();
        let r = EngineConfig::load(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(r.unwrap().engine.app_name, "f");
    }

    #[test]
    fn config_load_missing_file() {
        let path = std::env::temp_dir().join("engine-core-definitely-missing.ron");
        assert!(matches!(
            EngineConfig::load(&path),
            Err(ConfigError::Io { .. })
        ));
    }
}
