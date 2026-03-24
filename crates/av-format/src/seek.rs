/// Seek flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFlag {
    /// Seek to the nearest keyframe before the target.
    Backward,
    /// Seek to any frame (not just keyframes).
    Any,
    /// Seek by byte position instead of timestamp.
    Byte,
}

/// An index entry for fast seeking.
#[derive(Debug, Clone, Copy)]
pub struct IndexEntry {
    /// Timestamp in stream time_base units.
    pub timestamp: i64,
    /// Byte position in the file.
    pub pos: u64,
    /// Size of the packet at this position.
    pub size: u32,
    /// True if this is a keyframe.
    pub keyframe: bool,
}

/// A seek index for a single stream.
#[derive(Debug, Clone, Default)]
pub struct SeekIndex {
    entries: Vec<IndexEntry>,
}

impl SeekIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry to the index.
    pub fn add(&mut self, entry: IndexEntry) {
        self.entries.push(entry);
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find the index entry closest to `timestamp` (binary search).
    /// If `keyframe_only` is true, only returns keyframe entries.
    pub fn find(&self, timestamp: i64, keyframe_only: bool) -> Option<&IndexEntry> {
        if self.entries.is_empty() {
            return None;
        }

        // Binary search for the closest entry.
        let idx = self.entries
            .binary_search_by_key(&timestamp, |e| e.timestamp)
            .unwrap_or_else(|i| i.saturating_sub(1));

        if keyframe_only {
            // Walk backward to find the nearest keyframe.
            for i in (0..=idx).rev() {
                if self.entries[i].keyframe {
                    return Some(&self.entries[i]);
                }
            }
            None
        } else {
            self.entries.get(idx)
        }
    }

    /// Find by byte position.
    pub fn find_by_pos(&self, pos: u64) -> Option<&IndexEntry> {
        self.entries.iter().rev().find(|e| e.pos <= pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_index() -> SeekIndex {
        let mut idx = SeekIndex::new();
        idx.add(IndexEntry { timestamp: 0, pos: 0, size: 1000, keyframe: true });
        idx.add(IndexEntry { timestamp: 1000, pos: 5000, size: 500, keyframe: false });
        idx.add(IndexEntry { timestamp: 2000, pos: 10000, size: 1000, keyframe: true });
        idx.add(IndexEntry { timestamp: 3000, pos: 15000, size: 800, keyframe: false });
        idx.add(IndexEntry { timestamp: 4000, pos: 20000, size: 1000, keyframe: true });
        idx
    }

    // ── Positive ──

    #[test]
    fn find_exact_timestamp() {
        let idx = make_index();
        let entry = idx.find(2000, false).unwrap();
        assert_eq!(entry.timestamp, 2000);
        assert_eq!(entry.pos, 10000);
    }

    #[test]
    fn find_between_timestamps() {
        let idx = make_index();
        // 1500 is between 1000 and 2000, should return 1000.
        let entry = idx.find(1500, false).unwrap();
        assert_eq!(entry.timestamp, 1000);
    }

    #[test]
    fn find_keyframe_only() {
        let idx = make_index();
        // 3000 is not a keyframe; nearest keyframe before it is 2000.
        let entry = idx.find(3000, true).unwrap();
        assert_eq!(entry.timestamp, 2000);
        assert!(entry.keyframe);
    }

    #[test]
    fn find_first_entry() {
        let idx = make_index();
        let entry = idx.find(0, false).unwrap();
        assert_eq!(entry.timestamp, 0);
    }

    #[test]
    fn find_by_pos() {
        let idx = make_index();
        let entry = idx.find_by_pos(12000).unwrap();
        assert_eq!(entry.pos, 10000); // Closest ≤ 12000.
    }

    // ── Negative / Edge ──

    #[test]
    fn find_empty_index() {
        let idx = SeekIndex::new();
        assert!(idx.find(0, false).is_none());
    }

    #[test]
    fn find_keyframe_none_before() {
        let mut idx = SeekIndex::new();
        idx.add(IndexEntry { timestamp: 100, pos: 0, size: 100, keyframe: false });
        // No keyframes at all.
        assert!(idx.find(100, true).is_none());
    }

    #[test]
    fn len_and_empty() {
        let idx = make_index();
        assert_eq!(idx.len(), 5);
        assert!(!idx.is_empty());
        assert!(SeekIndex::new().is_empty());
    }
}
