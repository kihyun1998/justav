use core::fmt;

/// An ordered key-value metadata dictionary.
///
/// Preserves insertion order. Keys are case-sensitive strings.
/// Used for container metadata (title, artist, etc.) and codec options.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Dict {
    entries: Vec<(String, String)>,
}

impl Dict {
    /// Create an empty dictionary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a key-value pair. Replaces existing value if key exists.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    /// Get the value for a key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// Remove a key. Returns the value if it existed.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }

    /// Returns true if the key exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Iterate over key-value pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Copy all entries from `other` into `self`. Existing keys are overwritten.
    pub fn merge(&mut self, other: &Dict) {
        for (k, v) in &other.entries {
            self.set(k.clone(), v.clone());
        }
    }

    /// Parse "key1=val1;key2=val2" format into a Dict.
    pub fn parse(input: &str, pair_sep: char, kv_sep: char) -> Self {
        let mut dict = Self::new();
        for pair in input.split(pair_sep) {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            if let Some((k, v)) = pair.split_once(kv_sep) {
                dict.set(k.trim(), v.trim());
            }
        }
        dict
    }
}

impl fmt::Debug for Dict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

impl fmt::Display for Dict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, (k, v)) in self.entries.iter().enumerate() {
            if i > 0 {
                write!(f, "; ")?;
            }
            write!(f, "{k}={v}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn set_and_get() {
        let mut d = Dict::new();
        d.set("title", "Hello");
        assert_eq!(d.get("title"), Some("Hello"));
    }

    #[test]
    fn set_overwrites() {
        let mut d = Dict::new();
        d.set("k", "v1");
        d.set("k", "v2");
        assert_eq!(d.get("k"), Some("v2"));
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn remove_existing() {
        let mut d = Dict::new();
        d.set("k", "v");
        assert_eq!(d.remove("k"), Some("v".into()));
        assert!(d.is_empty());
    }

    #[test]
    fn iteration_order() {
        let mut d = Dict::new();
        d.set("b", "2");
        d.set("a", "1");
        let keys: Vec<&str> = d.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["b", "a"]);
    }

    #[test]
    fn merge() {
        let mut a = Dict::new();
        a.set("x", "1");
        let mut b = Dict::new();
        b.set("x", "2");
        b.set("y", "3");
        a.merge(&b);
        assert_eq!(a.get("x"), Some("2"));
        assert_eq!(a.get("y"), Some("3"));
    }

    #[test]
    fn parse_semicolon_eq() {
        let d = Dict::parse("title=Hello; artist=World", ';', '=');
        assert_eq!(d.get("title"), Some("Hello"));
        assert_eq!(d.get("artist"), Some("World"));
    }

    #[test]
    fn display_format() {
        let mut d = Dict::new();
        d.set("a", "1");
        d.set("b", "2");
        assert_eq!(format!("{d}"), "a=1; b=2");
    }

    #[test]
    fn contains_key() {
        let mut d = Dict::new();
        d.set("k", "v");
        assert!(d.contains_key("k"));
        assert!(!d.contains_key("missing"));
    }

    #[test]
    fn clear() {
        let mut d = Dict::new();
        d.set("a", "1");
        d.clear();
        assert!(d.is_empty());
    }

    // ── Negative ──

    #[test]
    fn get_missing_key() {
        let d = Dict::new();
        assert_eq!(d.get("missing"), None);
    }

    #[test]
    fn remove_missing_key() {
        let mut d = Dict::new();
        assert_eq!(d.remove("missing"), None);
    }

    #[test]
    fn parse_empty() {
        let d = Dict::parse("", ';', '=');
        assert!(d.is_empty());
    }

    #[test]
    fn parse_no_value() {
        // Entries without '=' are silently skipped.
        let d = Dict::parse("novalue;k=v", ';', '=');
        assert_eq!(d.len(), 1);
        assert_eq!(d.get("k"), Some("v"));
    }
}
