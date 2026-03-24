use crate::error::{Error, Result};
use crate::samplefmt::SampleFormat;

/// Allocate a buffer for audio samples.
///
/// For interleaved formats, returns a single buffer.
/// For planar formats, returns one buffer per channel.
///
/// Each buffer is zeroed (silence).
pub fn alloc(
    format: SampleFormat,
    channels: u16,
    nb_samples: u32,
) -> Result<Vec<Vec<u8>>> {
    if channels == 0 {
        return Err(Error::InvalidArgument("channels must be > 0".into()));
    }
    if nb_samples == 0 {
        // Empty frame — valid but zero-length buffers.
        if format.is_planar() {
            return Ok(vec![Vec::new(); channels as usize]);
        } else {
            return Ok(vec![Vec::new()]);
        }
    }

    if format.is_planar() {
        let plane_size = format.buffer_size(nb_samples, channels);
        Ok(vec![vec![0u8; plane_size]; channels as usize])
    } else {
        let total_size = format.buffer_size(nb_samples, channels);
        Ok(vec![vec![0u8; total_size]])
    }
}

/// Copy samples from `src` to `dst`.
///
/// Both must be the same format, channels, and have at least `nb_samples`
/// worth of data. For planar, copies each plane independently.
pub fn copy(
    dst: &mut [Vec<u8>],
    src: &[Vec<u8>],
    format: SampleFormat,
    channels: u16,
    nb_samples: u32,
) -> Result<()> {
    if channels == 0 {
        return Err(Error::InvalidArgument("channels must be > 0".into()));
    }

    let expected_planes = if format.is_planar() { channels as usize } else { 1 };
    if dst.len() < expected_planes || src.len() < expected_planes {
        return Err(Error::InvalidArgument(format!(
            "expected {expected_planes} planes, got dst={} src={}", dst.len(), src.len()
        )));
    }

    let bytes_per_plane = format.buffer_size(nb_samples, channels);

    for (i, (dst_plane, src_plane)) in dst.iter_mut().zip(src.iter()).enumerate().take(expected_planes) {
        if src_plane.len() < bytes_per_plane {
            return Err(Error::InvalidArgument(format!(
                "src plane {i} too small: {} < {bytes_per_plane}", src_plane.len()
            )));
        }
        if dst_plane.len() < bytes_per_plane {
            return Err(Error::InvalidArgument(format!(
                "dst plane {i} too small: {} < {bytes_per_plane}", dst_plane.len()
            )));
        }
        dst_plane[..bytes_per_plane].copy_from_slice(&src_plane[..bytes_per_plane]);
    }
    Ok(())
}

/// Fill sample buffers with silence.
///
/// For integer formats, silence is 0x00. For unsigned 8-bit, silence is 0x80.
pub fn silence(
    data: &mut [Vec<u8>],
    format: SampleFormat,
    channels: u16,
    nb_samples: u32,
) -> Result<()> {
    if channels == 0 {
        return Err(Error::InvalidArgument("channels must be > 0".into()));
    }

    let expected_planes = if format.is_planar() { channels as usize } else { 1 };
    if data.len() < expected_planes {
        return Err(Error::InvalidArgument(format!(
            "expected {expected_planes} planes, got {}", data.len()
        )));
    }

    let fill_byte: u8 = match format {
        SampleFormat::U8 | SampleFormat::U8p => 0x80, // unsigned 8-bit silence = midpoint
        _ => 0x00, // signed int and float silence = zero
    };

    let bytes_per_plane = format.buffer_size(nb_samples, channels);

    for plane in data.iter_mut().take(expected_planes) {
        let len = bytes_per_plane.min(plane.len());
        plane[..len].fill(fill_byte);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn alloc_interleaved_s16() {
        let bufs = alloc(SampleFormat::S16, 2, 1024).unwrap();
        assert_eq!(bufs.len(), 1); // single interleaved buffer
        assert_eq!(bufs[0].len(), 1024 * 2 * 2); // 1024 samples * 2 channels * 2 bytes
    }

    #[test]
    fn alloc_planar_f32() {
        let bufs = alloc(SampleFormat::F32p, 2, 1024).unwrap();
        assert_eq!(bufs.len(), 2); // one buffer per channel
        assert_eq!(bufs[0].len(), 1024 * 4); // 1024 samples * 4 bytes
        assert_eq!(bufs[1].len(), 1024 * 4);
    }

    #[test]
    fn alloc_zeroed() {
        let bufs = alloc(SampleFormat::S16, 1, 100).unwrap();
        assert!(bufs[0].iter().all(|&b| b == 0));
    }

    #[test]
    fn copy_interleaved() {
        let src = alloc(SampleFormat::S16, 2, 512).unwrap();
        let mut dst = alloc(SampleFormat::S16, 2, 512).unwrap();
        // Put some data in src
        let mut src = src;
        src[0][0] = 42;
        src[0][100] = 99;
        copy(&mut dst, &src, SampleFormat::S16, 2, 512).unwrap();
        assert_eq!(dst[0][0], 42);
        assert_eq!(dst[0][100], 99);
    }

    #[test]
    fn copy_planar() {
        let mut src = alloc(SampleFormat::S16p, 2, 256).unwrap();
        src[0][0] = 10;
        src[1][0] = 20;
        let mut dst = alloc(SampleFormat::S16p, 2, 256).unwrap();
        copy(&mut dst, &src, SampleFormat::S16p, 2, 256).unwrap();
        assert_eq!(dst[0][0], 10);
        assert_eq!(dst[1][0], 20);
    }

    #[test]
    fn silence_s16() {
        let mut bufs = alloc(SampleFormat::S16, 2, 512).unwrap();
        bufs[0].fill(0xFF); // dirty it
        silence(&mut bufs, SampleFormat::S16, 2, 512).unwrap();
        assert!(bufs[0].iter().all(|&b| b == 0));
    }

    #[test]
    fn silence_u8_is_0x80() {
        let mut bufs = alloc(SampleFormat::U8, 1, 100).unwrap();
        silence(&mut bufs, SampleFormat::U8, 1, 100).unwrap();
        assert!(bufs[0].iter().all(|&b| b == 0x80));
    }

    #[test]
    fn silence_u8p_is_0x80() {
        let mut bufs = alloc(SampleFormat::U8p, 2, 100).unwrap();
        silence(&mut bufs, SampleFormat::U8p, 2, 100).unwrap();
        assert!(bufs[0].iter().all(|&b| b == 0x80));
        assert!(bufs[1].iter().all(|&b| b == 0x80));
    }

    #[test]
    fn silence_f32_is_zero() {
        let mut bufs = alloc(SampleFormat::F32, 2, 100).unwrap();
        bufs[0].fill(0xFF);
        silence(&mut bufs, SampleFormat::F32, 2, 100).unwrap();
        assert!(bufs[0].iter().all(|&b| b == 0));
    }

    // ── Negative ──

    #[test]
    fn alloc_zero_channels() {
        assert!(alloc(SampleFormat::S16, 0, 1024).is_err());
    }

    #[test]
    fn copy_wrong_plane_count() {
        let src = alloc(SampleFormat::S16p, 2, 100).unwrap();
        let mut dst = vec![vec![0u8; 200]]; // only 1 plane, need 2
        assert!(copy(&mut dst, &src, SampleFormat::S16p, 2, 100).is_err());
    }

    #[test]
    fn copy_src_too_small() {
        let src = vec![vec![0u8; 10]]; // too small
        let mut dst = alloc(SampleFormat::S16, 2, 100).unwrap();
        assert!(copy(&mut dst, &src, SampleFormat::S16, 2, 100).is_err());
    }

    #[test]
    fn silence_zero_channels() {
        let mut bufs = vec![vec![0u8; 100]];
        assert!(silence(&mut bufs, SampleFormat::S16, 0, 100).is_err());
    }

    #[test]
    fn silence_not_enough_planes() {
        let mut bufs = vec![vec![0u8; 100]]; // 1 plane
        assert!(silence(&mut bufs, SampleFormat::S16p, 2, 100).is_err()); // needs 2
    }

    // ── Edge ──

    #[test]
    fn alloc_zero_samples() {
        let bufs = alloc(SampleFormat::S16, 2, 0).unwrap();
        assert_eq!(bufs.len(), 1);
        assert!(bufs[0].is_empty());
    }

    #[test]
    fn alloc_zero_samples_planar() {
        let bufs = alloc(SampleFormat::F32p, 2, 0).unwrap();
        assert_eq!(bufs.len(), 2);
        assert!(bufs[0].is_empty());
        assert!(bufs[1].is_empty());
    }

    #[test]
    fn alloc_mono() {
        let bufs = alloc(SampleFormat::S16, 1, 1024).unwrap();
        assert_eq!(bufs[0].len(), 1024 * 2);
    }
}
