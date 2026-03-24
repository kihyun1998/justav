use crate::error::{Error, Result};
use crate::pixfmt::PixelFormat;

/// Default line size alignment (common for SIMD: 32 bytes).
pub const DEFAULT_ALIGN: u32 = 32;

/// Compute the line size (stride) in bytes for a single plane.
///
/// `width` is the image width in pixels. `plane` is the plane index (0 for Y,
/// 1/2 for chroma in YUV). `align` is the byte alignment (must be power of 2).
pub fn image_line_size(format: PixelFormat, width: u32, plane: usize, align: u32) -> Result<u32> {
    if width == 0 {
        return Err(Error::InvalidArgument("width must be > 0".into()));
    }
    let desc = format.descriptor();
    if desc.is_hwaccel {
        return Err(Error::Unsupported("cannot compute line size for HW format".into()));
    }
    if plane >= desc.nb_planes as usize {
        return Err(Error::InvalidArgument(format!(
            "plane {plane} out of range for {}", desc.name
        )));
    }

    // Effective width for this plane (chroma may be subsampled).
    let plane_width = if plane > 0 && desc.nb_components > 1 {
        (width + (1 << desc.log2_chroma_w) - 1) >> desc.log2_chroma_w
    } else {
        width
    };

    let bytes_per_component = if desc.bits_per_component > 8 { 2u32 } else { 1u32 };

    // For packed formats (nb_planes == 1), all components are in plane 0.
    let components_in_plane = if desc.nb_planes == 1 {
        desc.nb_components as u32
    } else {
        1 // Each plane has one component for planar formats.
    };

    let linesize = plane_width * components_in_plane * bytes_per_component;

    // Align to boundary.
    let align = align.max(1);
    Ok((linesize + align - 1) & !(align - 1))
}

/// Compute the total buffer size needed for an image.
///
/// Returns the sum of all planes' sizes (height * linesize per plane).
pub fn image_buffer_size(format: PixelFormat, width: u32, height: u32, align: u32) -> Result<usize> {
    if width == 0 || height == 0 {
        return Err(Error::InvalidArgument("dimensions must be > 0".into()));
    }
    let desc = format.descriptor();
    if desc.is_hwaccel {
        return Err(Error::Unsupported("cannot compute buffer size for HW format".into()));
    }

    let mut total: usize = 0;
    for plane in 0..desc.nb_planes as usize {
        let linesize = image_line_size(format, width, plane, align)? as usize;
        let plane_height = if plane > 0 && desc.nb_components > 1 {
            ((height + (1 << desc.log2_chroma_h) - 1) >> desc.log2_chroma_h) as usize
        } else {
            height as usize
        };
        total += linesize * plane_height;
    }
    Ok(total)
}

/// Fill a plane buffer with black (zero for YUV luma, 128 for chroma, 0 for RGB).
///
/// `data` is the plane buffer, `linesize` is the byte stride, `height` is
/// the number of rows, `plane` is the plane index, `is_yuv` selects the
/// fill value for chroma planes.
pub fn fill_plane_black(data: &mut [u8], linesize: u32, height: u32, plane: usize, is_yuv: bool) {
    let fill = if is_yuv && plane > 0 { 128u8 } else { 0u8 };
    let linesize = linesize as usize;
    for row in 0..height as usize {
        let start = row * linesize;
        let end = start + linesize;
        if end <= data.len() {
            data[start..end].fill(fill);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn linesize_yuv420p_luma() {
        // 1920px, 1 byte/component, align 32 → 1920 aligned to 32 = 1920
        let ls = image_line_size(PixelFormat::Yuv420p, 1920, 0, 32).unwrap();
        assert_eq!(ls, 1920); // 1920 is already aligned to 32
    }

    #[test]
    fn linesize_yuv420p_chroma() {
        // 1920px >> 1 = 960, align 32 → 960
        let ls = image_line_size(PixelFormat::Yuv420p, 1920, 1, 32).unwrap();
        assert_eq!(ls, 960); // 960 is aligned to 32
    }

    #[test]
    fn linesize_rgb24() {
        // 1920 * 3 = 5760, align 32 → 5760 (already aligned)
        let ls = image_line_size(PixelFormat::Rgb24, 1920, 0, 32).unwrap();
        assert_eq!(ls, 5760);
    }

    #[test]
    fn linesize_rgba() {
        // 1920 * 4 = 7680
        let ls = image_line_size(PixelFormat::Rgba, 1920, 0, 32).unwrap();
        assert_eq!(ls, 7680);
    }

    #[test]
    fn linesize_alignment() {
        // 1921px yuv420p, align 32 → 1921 aligned up to 1952
        let ls = image_line_size(PixelFormat::Yuv420p, 1921, 0, 32).unwrap();
        assert_eq!(ls % 32, 0);
        assert!(ls >= 1921);
    }

    #[test]
    fn linesize_10bit() {
        // yuv420p10le: 1920 * 2 bytes = 3840, align 32
        let ls = image_line_size(PixelFormat::Yuv420p10Le, 1920, 0, 32).unwrap();
        assert_eq!(ls, 3840);
    }

    #[test]
    fn buffer_size_yuv420p() {
        // 1920x1080 yuv420p: Y=1920*1080 + U=960*540 + V=960*540
        let size = image_buffer_size(PixelFormat::Yuv420p, 1920, 1080, 1).unwrap();
        assert_eq!(size, 1920 * 1080 + 960 * 540 * 2);
    }

    #[test]
    fn buffer_size_rgb24() {
        let size = image_buffer_size(PixelFormat::Rgb24, 100, 100, 1).unwrap();
        assert_eq!(size, 100 * 3 * 100);
    }

    #[test]
    fn fill_black_yuv_luma() {
        let mut data = vec![255u8; 8];
        fill_plane_black(&mut data, 4, 2, 0, true);
        assert!(data.iter().all(|&b| b == 0));
    }

    #[test]
    fn fill_black_yuv_chroma() {
        let mut data = vec![0u8; 4];
        fill_plane_black(&mut data, 2, 2, 1, true);
        assert!(data.iter().all(|&b| b == 128));
    }

    #[test]
    fn fill_black_rgb() {
        let mut data = vec![255u8; 12];
        fill_plane_black(&mut data, 4, 3, 0, false);
        assert!(data.iter().all(|&b| b == 0));
    }

    // ── Negative ──

    #[test]
    fn linesize_zero_width() {
        assert!(image_line_size(PixelFormat::Yuv420p, 0, 0, 32).is_err());
    }

    #[test]
    fn linesize_hw_format() {
        assert!(image_line_size(PixelFormat::Vaapi, 1920, 0, 32).is_err());
    }

    #[test]
    fn linesize_plane_out_of_range() {
        assert!(image_line_size(PixelFormat::Yuv420p, 1920, 5, 32).is_err());
    }

    #[test]
    fn buffer_size_zero_dims() {
        assert!(image_buffer_size(PixelFormat::Yuv420p, 0, 1080, 1).is_err());
        assert!(image_buffer_size(PixelFormat::Yuv420p, 1920, 0, 1).is_err());
    }

    #[test]
    fn buffer_size_hw_format() {
        assert!(image_buffer_size(PixelFormat::D3d11, 1920, 1080, 1).is_err());
    }

    // ── Edge ──

    #[test]
    fn linesize_align_1() {
        // No alignment padding.
        let ls = image_line_size(PixelFormat::Yuv420p, 1921, 0, 1).unwrap();
        assert_eq!(ls, 1921);
    }

    #[test]
    fn odd_dimensions_yuv420p() {
        // 1921x1081 yuv420p — chroma width = ceil(1921/2) = 961
        let ls = image_line_size(PixelFormat::Yuv420p, 1921, 1, 1).unwrap();
        assert_eq!(ls, 961);
    }

    #[test]
    fn gray8_single_plane() {
        let ls = image_line_size(PixelFormat::Gray8, 100, 0, 1).unwrap();
        assert_eq!(ls, 100);
        let size = image_buffer_size(PixelFormat::Gray8, 100, 100, 1).unwrap();
        assert_eq!(size, 10000);
    }
}
