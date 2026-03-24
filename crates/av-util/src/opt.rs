use crate::error::{Error, Result};

/// The type of an option value.
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

impl std::fmt::Display for OptionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
        }
    }
}

/// Describes a single configurable option.
#[derive(Debug, Clone)]
pub struct OptionDef {
    /// Option name (key).
    pub name: String,
    /// Human-readable description.
    pub help: String,
    /// Default value.
    pub default: OptionValue,
    /// Minimum value (for Int/Float).
    pub min: Option<f64>,
    /// Maximum value (for Int/Float).
    pub max: Option<f64>,
}

/// A collection of option definitions and their current values.
#[derive(Debug, Clone)]
pub struct Options {
    defs: Vec<OptionDef>,
    values: Vec<OptionValue>,
}

impl Options {
    /// Create a new empty option set.
    pub fn new() -> Self {
        Self {
            defs: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Register an option definition. Sets the value to the default.
    pub fn register(&mut self, def: OptionDef) {
        let value = def.default.clone();
        self.defs.push(def);
        self.values.push(value);
    }

    /// Set an option by name, parsing the string value to the correct type.
    pub fn set(&mut self, name: &str, value: &str) -> Result<()> {
        let idx = self.find_index(name)?;
        let def = &self.defs[idx];
        let parsed = Self::parse_value(value, &def.default)?;
        Self::validate_range(&parsed, def)?;
        self.values[idx] = parsed;
        Ok(())
    }

    /// Set an option with a typed value directly.
    pub fn set_value(&mut self, name: &str, value: OptionValue) -> Result<()> {
        let idx = self.find_index(name)?;
        let def = &self.defs[idx];
        // Check type matches.
        if std::mem::discriminant(&value) != std::mem::discriminant(&def.default) {
            return Err(Error::InvalidArgument(format!(
                "option '{}': type mismatch", name
            )));
        }
        Self::validate_range(&value, def)?;
        self.values[idx] = value;
        Ok(())
    }

    /// Get the current value of an option.
    pub fn get(&self, name: &str) -> Result<&OptionValue> {
        let idx = self.find_index(name)?;
        Ok(&self.values[idx])
    }

    /// Get as i64.
    pub fn get_int(&self, name: &str) -> Result<i64> {
        match self.get(name)? {
            OptionValue::Int(v) => Ok(*v),
            _ => Err(Error::InvalidArgument(format!("option '{name}' is not an int"))),
        }
    }

    /// Get as f64.
    pub fn get_float(&self, name: &str) -> Result<f64> {
        match self.get(name)? {
            OptionValue::Float(v) => Ok(*v),
            _ => Err(Error::InvalidArgument(format!("option '{name}' is not a float"))),
        }
    }

    /// Get as string.
    pub fn get_string(&self, name: &str) -> Result<&str> {
        match self.get(name)? {
            OptionValue::String(v) => Ok(v),
            _ => Err(Error::InvalidArgument(format!("option '{name}' is not a string"))),
        }
    }

    /// Get as bool.
    pub fn get_bool(&self, name: &str) -> Result<bool> {
        match self.get(name)? {
            OptionValue::Bool(v) => Ok(*v),
            _ => Err(Error::InvalidArgument(format!("option '{name}' is not a bool"))),
        }
    }

    /// Reset all options to their defaults.
    pub fn reset_defaults(&mut self) {
        for (i, def) in self.defs.iter().enumerate() {
            self.values[i] = def.default.clone();
        }
    }

    /// Number of registered options.
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// Returns true if no options are registered.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Iterate over (name, current_value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &OptionValue)> {
        self.defs.iter().zip(self.values.iter()).map(|(d, v)| (d.name.as_str(), v))
    }

    // ── Internal helpers ──

    fn find_index(&self, name: &str) -> Result<usize> {
        self.defs
            .iter()
            .position(|d| d.name == name)
            .ok_or_else(|| Error::NotFound(format!("option '{name}' not found")))
    }

    fn parse_value(s: &str, template: &OptionValue) -> Result<OptionValue> {
        match template {
            OptionValue::Int(_) => {
                let v = s.parse::<i64>().map_err(|_| {
                    Error::InvalidArgument(format!("cannot parse '{s}' as integer"))
                })?;
                Ok(OptionValue::Int(v))
            }
            OptionValue::Float(_) => {
                let v = s.parse::<f64>().map_err(|_| {
                    Error::InvalidArgument(format!("cannot parse '{s}' as float"))
                })?;
                Ok(OptionValue::Float(v))
            }
            OptionValue::String(_) => Ok(OptionValue::String(s.to_string())),
            OptionValue::Bool(_) => {
                let v = match s {
                    "1" | "true" | "yes" | "on" => true,
                    "0" | "false" | "no" | "off" => false,
                    _ => return Err(Error::InvalidArgument(format!("cannot parse '{s}' as bool"))),
                };
                Ok(OptionValue::Bool(v))
            }
        }
    }

    fn validate_range(value: &OptionValue, def: &OptionDef) -> Result<()> {
        let numeric = match value {
            OptionValue::Int(v) => Some(*v as f64),
            OptionValue::Float(v) => Some(*v),
            _ => None,
        };
        if let Some(val) = numeric {
            if let Some(min) = def.min && val < min {
                return Err(Error::InvalidArgument(format!(
                    "option '{}': value {val} below minimum {min}", def.name
                )));
            }
            if let Some(max) = def.max && val > max {
                return Err(Error::InvalidArgument(format!(
                    "option '{}': value {val} above maximum {max}", def.name
                )));
            }
        }
        Ok(())
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_opts() -> Options {
        let mut opts = Options::new();
        opts.register(OptionDef {
            name: "bitrate".into(),
            help: "target bitrate".into(),
            default: OptionValue::Int(128000),
            min: Some(1000.0),
            max: Some(10_000_000.0),
        });
        opts.register(OptionDef {
            name: "quality".into(),
            help: "quality factor".into(),
            default: OptionValue::Float(0.8),
            min: Some(0.0),
            max: Some(1.0),
        });
        opts.register(OptionDef {
            name: "title".into(),
            help: "stream title".into(),
            default: OptionValue::String(String::new()),
            min: None,
            max: None,
        });
        opts.register(OptionDef {
            name: "verbose".into(),
            help: "enable verbose logging".into(),
            default: OptionValue::Bool(false),
            min: None,
            max: None,
        });
        opts
    }

    // ── Positive ──

    #[test]
    fn default_values() {
        let opts = sample_opts();
        assert_eq!(opts.get_int("bitrate").unwrap(), 128000);
        assert!((opts.get_float("quality").unwrap() - 0.8).abs() < 1e-9);
        assert_eq!(opts.get_string("title").unwrap(), "");
        assert!(!opts.get_bool("verbose").unwrap());
    }

    #[test]
    fn set_int_from_string() {
        let mut opts = sample_opts();
        opts.set("bitrate", "256000").unwrap();
        assert_eq!(opts.get_int("bitrate").unwrap(), 256000);
    }

    #[test]
    fn set_float_from_string() {
        let mut opts = sample_opts();
        opts.set("quality", "0.5").unwrap();
        assert!((opts.get_float("quality").unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn set_string_from_string() {
        let mut opts = sample_opts();
        opts.set("title", "My Video").unwrap();
        assert_eq!(opts.get_string("title").unwrap(), "My Video");
    }

    #[test]
    fn set_bool_variants() {
        let mut opts = sample_opts();
        for truthy in &["1", "true", "yes", "on"] {
            opts.set("verbose", truthy).unwrap();
            assert!(opts.get_bool("verbose").unwrap());
        }
        for falsy in &["0", "false", "no", "off"] {
            opts.set("verbose", falsy).unwrap();
            assert!(!opts.get_bool("verbose").unwrap());
        }
    }

    #[test]
    fn set_value_typed() {
        let mut opts = sample_opts();
        opts.set_value("bitrate", OptionValue::Int(500000)).unwrap();
        assert_eq!(opts.get_int("bitrate").unwrap(), 500000);
    }

    #[test]
    fn reset_defaults() {
        let mut opts = sample_opts();
        opts.set("bitrate", "999999").unwrap();
        opts.reset_defaults();
        assert_eq!(opts.get_int("bitrate").unwrap(), 128000);
    }

    #[test]
    fn iterate_options() {
        let opts = sample_opts();
        let names: Vec<&str> = opts.iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["bitrate", "quality", "title", "verbose"]);
    }

    #[test]
    fn len_and_empty() {
        let opts = sample_opts();
        assert_eq!(opts.len(), 4);
        assert!(!opts.is_empty());
        assert!(Options::new().is_empty());
    }

    // ── Negative ──

    #[test]
    fn get_nonexistent() {
        let opts = sample_opts();
        assert!(opts.get("missing").is_err());
    }

    #[test]
    fn set_nonexistent() {
        let mut opts = sample_opts();
        assert!(opts.set("missing", "42").is_err());
    }

    #[test]
    fn set_int_parse_error() {
        let mut opts = sample_opts();
        assert!(opts.set("bitrate", "not_a_number").is_err());
    }

    #[test]
    fn set_bool_parse_error() {
        let mut opts = sample_opts();
        assert!(opts.set("verbose", "maybe").is_err());
    }

    #[test]
    fn set_below_min() {
        let mut opts = sample_opts();
        assert!(opts.set("bitrate", "500").is_err()); // min is 1000
    }

    #[test]
    fn set_above_max() {
        let mut opts = sample_opts();
        assert!(opts.set("quality", "1.5").is_err()); // max is 1.0
    }

    #[test]
    fn set_value_type_mismatch() {
        let mut opts = sample_opts();
        assert!(opts.set_value("bitrate", OptionValue::String("wrong".into())).is_err());
    }

    #[test]
    fn get_wrong_type() {
        let opts = sample_opts();
        assert!(opts.get_int("title").is_err()); // title is String, not Int
        assert!(opts.get_string("bitrate").is_err()); // bitrate is Int, not String
    }
}
