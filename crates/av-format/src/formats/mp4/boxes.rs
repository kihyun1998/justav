use av_util::error::Result;
use crate::io::IOContext;

/// A parsed MP4 box header.
#[derive(Debug, Clone)]
pub struct BoxHeader {
    /// Four-character box type (e.g. "ftyp", "moov").
    pub box_type: [u8; 4],
    /// Total box size including header (0 = extends to EOF).
    pub size: u64,
    /// Offset where the box payload starts.
    pub payload_offset: u64,
    /// Payload size (size - header_size).
    pub payload_size: u64,
}

impl BoxHeader {
    /// Four-char type as string.
    pub fn type_str(&self) -> &str {
        std::str::from_utf8(&self.box_type).unwrap_or("????")
    }

    /// Check if this box matches a given type.
    pub fn is_type(&self, t: &[u8; 4]) -> bool {
        self.box_type == *t
    }
}

/// Read a box header from the current IO position.
pub fn read_box_header(io: &mut IOContext) -> Result<BoxHeader> {
    let start = io.position()?;
    let size32 = io.read_u32_be()?;
    let mut box_type = [0u8; 4];
    io.read_exact(&mut box_type)?;

    let (size, header_size) = if size32 == 1 {
        // 64-bit extended size.
        let size64 = io.read_u64_be()?;
        (size64, 16u64)
    } else if size32 == 0 {
        // Box extends to end of file.
        let remaining = if io.size > 0 {
            io.size as u64 - start
        } else {
            0 // Unknown size.
        };
        (remaining, 8u64)
    } else {
        (size32 as u64, 8u64)
    };

    let payload_offset = start + header_size;
    let payload_size = size.saturating_sub(header_size);

    Ok(BoxHeader {
        box_type,
        size,
        payload_offset,
        payload_size,
    })
}

/// Skip the payload of a box (advance to the next box).
pub fn skip_box(io: &mut IOContext, header: &BoxHeader) -> Result<()> {
    let end = header.payload_offset + header.payload_size;
    io.seek(end)?;
    Ok(())
}

/// Read a full box header (version + flags, used by many MP4 boxes).
pub fn read_full_box_header(io: &mut IOContext) -> Result<(u8, u32)> {
    let vf = io.read_u32_be()?;
    let version = (vf >> 24) as u8;
    let flags = vf & 0x00FFFFFF;
    Ok((version, flags))
}

/// Container boxes that hold other boxes.
pub fn is_container_box(box_type: &[u8; 4]) -> bool {
    matches!(
        box_type,
        b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" | b"dinf"
        | b"edts" | b"udta" | b"mvex" | b"moof" | b"traf"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_box(io: &mut IOContext, box_type: &[u8; 4], payload: &[u8]) {
        let size = (8 + payload.len()) as u32;
        io.write_u32_be(size).unwrap();
        io.write_all(box_type).unwrap();
        io.write_all(payload).unwrap();
    }

    #[test]
    fn read_basic_box() {
        let mut io = IOContext::memory_writer();
        write_box(&mut io, b"ftyp", b"isom\x00\x00\x00\x00");
        io.seek(0).unwrap();

        let h = read_box_header(&mut io).unwrap();
        assert_eq!(h.type_str(), "ftyp");
        assert_eq!(h.size, 16);
        assert_eq!(h.payload_size, 8);
        assert_eq!(h.payload_offset, 8);
    }

    #[test]
    fn read_two_boxes() {
        let mut io = IOContext::memory_writer();
        write_box(&mut io, b"ftyp", b"isom");
        write_box(&mut io, b"moov", b"data");
        io.seek(0).unwrap();

        let h1 = read_box_header(&mut io).unwrap();
        assert!(h1.is_type(b"ftyp"));
        skip_box(&mut io, &h1).unwrap();

        let h2 = read_box_header(&mut io).unwrap();
        assert!(h2.is_type(b"moov"));
    }

    #[test]
    fn container_box_detection() {
        assert!(is_container_box(b"moov"));
        assert!(is_container_box(b"trak"));
        assert!(is_container_box(b"stbl"));
        assert!(!is_container_box(b"ftyp"));
        assert!(!is_container_box(b"mdat"));
    }

    #[test]
    fn read_box_too_short() {
        let mut io = IOContext::from_memory(vec![0, 0, 0]);
        assert!(read_box_header(&mut io).is_err());
    }

    #[test]
    fn full_box_header() {
        let mut io = IOContext::memory_writer();
        // version=1, flags=0x000001
        io.write_u32_be(0x01000001).unwrap();
        io.seek(0).unwrap();

        let (version, flags) = read_full_box_header(&mut io).unwrap();
        assert_eq!(version, 1);
        assert_eq!(flags, 1);
    }

    #[test]
    fn read_extended_size_box() {
        let mut io = IOContext::memory_writer();
        io.write_u32_be(1).unwrap(); // size=1 means extended
        io.write_all(b"mdat").unwrap();
        io.write_u64_be(24).unwrap(); // extended size = 24 (16 header + 8 payload)
        io.write_all(b"payloadX").unwrap();
        io.seek(0).unwrap();

        let h = read_box_header(&mut io).unwrap();
        assert!(h.is_type(b"mdat"));
        assert_eq!(h.size, 24);
        assert_eq!(h.payload_size, 8); // 24 - 16
        assert_eq!(h.payload_offset, 16);
    }

    #[test]
    fn read_zero_size_box() {
        // size=0 means "extends to EOF".
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_be_bytes()); // size=0
        data.extend_from_slice(b"mdat");
        data.extend_from_slice(&[0xAA; 100]); // payload

        let mut io = IOContext::from_memory(data.clone());
        let h = read_box_header(&mut io).unwrap();
        assert!(h.is_type(b"mdat"));
        assert_eq!(h.size, data.len() as u64); // extends to EOF
    }

    #[test]
    fn skip_box_advances_position() {
        let mut io = IOContext::memory_writer();
        write_box(&mut io, b"free", &[0u8; 20]);
        write_box(&mut io, b"mdat", &[0xFF; 10]);
        io.seek(0).unwrap();

        let h1 = read_box_header(&mut io).unwrap();
        assert!(h1.is_type(b"free"));
        skip_box(&mut io, &h1).unwrap();
        assert_eq!(io.position().unwrap(), 28); // 8+20

        let h2 = read_box_header(&mut io).unwrap();
        assert!(h2.is_type(b"mdat"));
    }
}
