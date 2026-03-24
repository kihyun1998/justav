use av_util::buffer::Buffer;
use av_util::dict::Dict;

/// Container-level metadata.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    /// Key-value tags (title, artist, album, etc.).
    pub tags: Dict,
    /// Chapter list.
    pub chapters: Vec<Chapter>,
    /// Attached files (album art, fonts, etc.).
    pub attachments: Vec<Attachment>,
}

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A chapter with time range and title.
#[derive(Debug, Clone)]
pub struct Chapter {
    /// Unique chapter ID.
    pub id: u64,
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
    /// Chapter title.
    pub title: String,
    /// Chapter-level metadata.
    pub metadata: Dict,
}

/// An attachment (e.g. album art, font file).
#[derive(Debug, Clone)]
pub struct Attachment {
    /// Filename (e.g. "cover.jpg").
    pub filename: String,
    /// MIME type (e.g. "image/jpeg").
    pub mime_type: String,
    /// The file data.
    pub data: Buffer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_default() {
        let m = Metadata::new();
        assert!(m.tags.is_empty());
        assert!(m.chapters.is_empty());
        assert!(m.attachments.is_empty());
    }

    #[test]
    fn add_chapter() {
        let mut m = Metadata::new();
        m.chapters.push(Chapter {
            id: 1,
            start_ms: 0,
            end_ms: 60_000,
            title: "Intro".into(),
            metadata: Dict::new(),
        });
        assert_eq!(m.chapters.len(), 1);
        assert_eq!(m.chapters[0].title, "Intro");
    }

    #[test]
    fn add_attachment() {
        let mut m = Metadata::new();
        m.attachments.push(Attachment {
            filename: "cover.jpg".into(),
            mime_type: "image/jpeg".into(),
            data: Buffer::from_vec(vec![0xFF, 0xD8, 0xFF]),
        });
        assert_eq!(m.attachments.len(), 1);
        assert_eq!(m.attachments[0].mime_type, "image/jpeg");
    }

    #[test]
    fn tags() {
        let mut m = Metadata::new();
        m.tags.set("title", "My Video");
        m.tags.set("artist", "justav");
        assert_eq!(m.tags.get("title"), Some("My Video"));
    }

    // ── Negative / Edge ──

    #[test]
    fn empty_chapter_title() {
        let mut m = Metadata::new();
        m.chapters.push(Chapter {
            id: 1,
            start_ms: 0,
            end_ms: 1000,
            title: String::new(),
            metadata: Dict::new(),
        });
        assert!(m.chapters[0].title.is_empty());
    }

    #[test]
    fn chapter_start_after_end() {
        // Invalid timing — the struct accepts it (no validation at this level).
        let ch = Chapter {
            id: 1,
            start_ms: 5000,
            end_ms: 1000, // start > end
            title: "Bad".into(),
            metadata: Dict::new(),
        };
        assert!(ch.start_ms > ch.end_ms);
    }

    #[test]
    fn empty_attachment() {
        let mut m = Metadata::new();
        m.attachments.push(Attachment {
            filename: String::new(),
            mime_type: String::new(),
            data: Buffer::from_vec(vec![]),
        });
        assert!(m.attachments[0].data.is_empty());
    }

    #[test]
    fn multiple_chapters_ordering() {
        let mut m = Metadata::new();
        for i in 0..5 {
            m.chapters.push(Chapter {
                id: i,
                start_ms: i * 10_000,
                end_ms: (i + 1) * 10_000,
                title: format!("Chapter {i}"),
                metadata: Dict::new(),
            });
        }
        assert_eq!(m.chapters.len(), 5);
        assert_eq!(m.chapters[2].title, "Chapter 2");
        assert_eq!(m.chapters[4].end_ms, 50_000);
    }

    #[test]
    fn metadata_clone() {
        let mut m = Metadata::new();
        m.tags.set("key", "val");
        m.chapters.push(Chapter {
            id: 1, start_ms: 0, end_ms: 1000, title: "Ch1".into(), metadata: Dict::new(),
        });
        let m2 = m.clone();
        assert_eq!(m2.tags.get("key"), Some("val"));
        assert_eq!(m2.chapters.len(), 1);
    }
}
