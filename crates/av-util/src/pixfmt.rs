/// Pixel formats for video frames.
///
/// Each variant describes how pixel data is laid out in memory.
/// Use [`PixelFormat::descriptor`] to get detailed properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PixelFormat {
    // ── Planar YUV ──
    /// YUV 4:2:0 planar, 12 bpp.
    Yuv420p,
    /// YUV 4:2:2 planar, 16 bpp.
    Yuv422p,
    /// YUV 4:4:4 planar, 24 bpp.
    Yuv444p,
    /// YUV 4:2:0 planar 10-bit, 15 bpp.
    Yuv420p10Le,
    /// YUV 4:2:0 planar 10-bit BE, 15 bpp.
    Yuv420p10Be,
    /// YUV 4:2:2 planar 10-bit LE, 20 bpp.
    Yuv422p10Le,
    /// YUV 4:4:4 planar 10-bit LE, 30 bpp.
    Yuv444p10Le,

    // ── Planar YUV + Alpha ──
    /// YUVA 4:2:0 planar, 20 bpp.
    Yuva420p,

    // ── NV12 / NV21 semi-planar ──
    /// Y plane + interleaved UV, 12 bpp (common HW format).
    Nv12,
    /// Y plane + interleaved VU, 12 bpp.
    Nv21,

    // ── Packed RGB ──
    /// Packed RGB 8:8:8, 24 bpp.
    Rgb24,
    /// Packed BGR 8:8:8, 24 bpp.
    Bgr24,
    /// Packed RGBA 8:8:8:8, 32 bpp.
    Rgba,
    /// Packed BGRA 8:8:8:8, 32 bpp.
    Bgra,
    /// Packed ARGB 8:8:8:8, 32 bpp.
    Argb,
    /// Packed ABGR 8:8:8:8, 32 bpp.
    Abgr,

    // ── Planar RGB ──
    /// Planar GBR 8-bit, 24 bpp.
    Gbrp,

    // ── Gray ──
    /// Grayscale 8-bit, 8 bpp.
    Gray8,
    /// Grayscale 16-bit LE, 16 bpp.
    Gray16Le,

    // ── Packed YUV ──
    /// Packed YUV 4:2:2, 16 bpp (YUYV).
    Yuyv422,
    /// Packed YUV 4:2:2, 16 bpp (UYVY).
    Uyvy422,

    // ── Hardware surface (opaque) ──
    /// VAAPI hardware surface.
    Vaapi,
    /// D3D11 hardware surface.
    D3d11,
    /// VideoToolbox hardware surface.
    VideoToolbox,
    /// CUDA hardware surface.
    Cuda,
}

/// Descriptor for a pixel format, providing detailed properties.
#[derive(Debug, Clone, Copy)]
pub struct PixelFormatDescriptor {
    /// Human-readable name.
    pub name: &'static str,
    /// Number of components (e.g. 3 for YUV, 4 for RGBA).
    pub nb_components: u8,
    /// Bits per pixel (average for planar formats).
    pub bits_per_pixel: u8,
    /// Number of planes.
    pub nb_planes: u8,
    /// log2 of horizontal chroma subsampling (0 = no subsampling).
    pub log2_chroma_w: u8,
    /// log2 of vertical chroma subsampling (0 = no subsampling).
    pub log2_chroma_h: u8,
    /// True if data is stored in separate planes.
    pub is_planar: bool,
    /// True if format has an alpha channel.
    pub has_alpha: bool,
    /// True if this is a hardware surface format (opaque, no CPU access).
    pub is_hwaccel: bool,
    /// Bits per component (0 for mixed or HW formats).
    pub bits_per_component: u8,
}

impl PixelFormat {
    /// Get the descriptor for this pixel format.
    pub const fn descriptor(&self) -> PixelFormatDescriptor {
        match self {
            // Planar YUV
            Self::Yuv420p => PixelFormatDescriptor {
                name: "yuv420p", nb_components: 3, bits_per_pixel: 12,
                nb_planes: 3, log2_chroma_w: 1, log2_chroma_h: 1,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Yuv422p => PixelFormatDescriptor {
                name: "yuv422p", nb_components: 3, bits_per_pixel: 16,
                nb_planes: 3, log2_chroma_w: 1, log2_chroma_h: 0,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Yuv444p => PixelFormatDescriptor {
                name: "yuv444p", nb_components: 3, bits_per_pixel: 24,
                nb_planes: 3, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Yuv420p10Le => PixelFormatDescriptor {
                name: "yuv420p10le", nb_components: 3, bits_per_pixel: 15,
                nb_planes: 3, log2_chroma_w: 1, log2_chroma_h: 1,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 10,
            },
            Self::Yuv420p10Be => PixelFormatDescriptor {
                name: "yuv420p10be", nb_components: 3, bits_per_pixel: 15,
                nb_planes: 3, log2_chroma_w: 1, log2_chroma_h: 1,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 10,
            },
            Self::Yuv422p10Le => PixelFormatDescriptor {
                name: "yuv422p10le", nb_components: 3, bits_per_pixel: 20,
                nb_planes: 3, log2_chroma_w: 1, log2_chroma_h: 0,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 10,
            },
            Self::Yuv444p10Le => PixelFormatDescriptor {
                name: "yuv444p10le", nb_components: 3, bits_per_pixel: 30,
                nb_planes: 3, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 10,
            },

            // Planar YUV + Alpha
            Self::Yuva420p => PixelFormatDescriptor {
                name: "yuva420p", nb_components: 4, bits_per_pixel: 20,
                nb_planes: 4, log2_chroma_w: 1, log2_chroma_h: 1,
                is_planar: true, has_alpha: true, is_hwaccel: false, bits_per_component: 8,
            },

            // Semi-planar
            Self::Nv12 => PixelFormatDescriptor {
                name: "nv12", nb_components: 3, bits_per_pixel: 12,
                nb_planes: 2, log2_chroma_w: 1, log2_chroma_h: 1,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Nv21 => PixelFormatDescriptor {
                name: "nv21", nb_components: 3, bits_per_pixel: 12,
                nb_planes: 2, log2_chroma_w: 1, log2_chroma_h: 1,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },

            // Packed RGB
            Self::Rgb24 => PixelFormatDescriptor {
                name: "rgb24", nb_components: 3, bits_per_pixel: 24,
                nb_planes: 1, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Bgr24 => PixelFormatDescriptor {
                name: "bgr24", nb_components: 3, bits_per_pixel: 24,
                nb_planes: 1, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Rgba => PixelFormatDescriptor {
                name: "rgba", nb_components: 4, bits_per_pixel: 32,
                nb_planes: 1, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: true, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Bgra => PixelFormatDescriptor {
                name: "bgra", nb_components: 4, bits_per_pixel: 32,
                nb_planes: 1, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: true, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Argb => PixelFormatDescriptor {
                name: "argb", nb_components: 4, bits_per_pixel: 32,
                nb_planes: 1, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: true, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Abgr => PixelFormatDescriptor {
                name: "abgr", nb_components: 4, bits_per_pixel: 32,
                nb_planes: 1, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: true, is_hwaccel: false, bits_per_component: 8,
            },

            // Planar RGB
            Self::Gbrp => PixelFormatDescriptor {
                name: "gbrp", nb_components: 3, bits_per_pixel: 24,
                nb_planes: 3, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: true, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },

            // Gray
            Self::Gray8 => PixelFormatDescriptor {
                name: "gray", nb_components: 1, bits_per_pixel: 8,
                nb_planes: 1, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Gray16Le => PixelFormatDescriptor {
                name: "gray16le", nb_components: 1, bits_per_pixel: 16,
                nb_planes: 1, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: false, bits_per_component: 16,
            },

            // Packed YUV
            Self::Yuyv422 => PixelFormatDescriptor {
                name: "yuyv422", nb_components: 3, bits_per_pixel: 16,
                nb_planes: 1, log2_chroma_w: 1, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },
            Self::Uyvy422 => PixelFormatDescriptor {
                name: "uyvy422", nb_components: 3, bits_per_pixel: 16,
                nb_planes: 1, log2_chroma_w: 1, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: false, bits_per_component: 8,
            },

            // Hardware
            Self::Vaapi => PixelFormatDescriptor {
                name: "vaapi", nb_components: 0, bits_per_pixel: 0,
                nb_planes: 0, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: true, bits_per_component: 0,
            },
            Self::D3d11 => PixelFormatDescriptor {
                name: "d3d11", nb_components: 0, bits_per_pixel: 0,
                nb_planes: 0, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: true, bits_per_component: 0,
            },
            Self::VideoToolbox => PixelFormatDescriptor {
                name: "videotoolbox", nb_components: 0, bits_per_pixel: 0,
                nb_planes: 0, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: true, bits_per_component: 0,
            },
            Self::Cuda => PixelFormatDescriptor {
                name: "cuda", nb_components: 0, bits_per_pixel: 0,
                nb_planes: 0, log2_chroma_w: 0, log2_chroma_h: 0,
                is_planar: false, has_alpha: false, is_hwaccel: true, bits_per_component: 0,
            },
        }
    }

    /// Get the format name as a string.
    pub const fn name(&self) -> &'static str {
        self.descriptor().name
    }

    /// Parse a pixel format from its name string.
    pub fn from_name(name: &str) -> Option<Self> {
        ALL_PIXEL_FORMATS.iter().find(|f| f.name() == name).copied()
    }
}

impl core::fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// All defined pixel formats, for iteration / lookup.
pub const ALL_PIXEL_FORMATS: &[PixelFormat] = &[
    PixelFormat::Yuv420p, PixelFormat::Yuv422p, PixelFormat::Yuv444p,
    PixelFormat::Yuv420p10Le, PixelFormat::Yuv420p10Be,
    PixelFormat::Yuv422p10Le, PixelFormat::Yuv444p10Le,
    PixelFormat::Yuva420p,
    PixelFormat::Nv12, PixelFormat::Nv21,
    PixelFormat::Rgb24, PixelFormat::Bgr24,
    PixelFormat::Rgba, PixelFormat::Bgra, PixelFormat::Argb, PixelFormat::Abgr,
    PixelFormat::Gbrp,
    PixelFormat::Gray8, PixelFormat::Gray16Le,
    PixelFormat::Yuyv422, PixelFormat::Uyvy422,
    PixelFormat::Vaapi, PixelFormat::D3d11, PixelFormat::VideoToolbox, PixelFormat::Cuda,
];

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn yuv420p_descriptor() {
        let d = PixelFormat::Yuv420p.descriptor();
        assert_eq!(d.name, "yuv420p");
        assert_eq!(d.nb_components, 3);
        assert_eq!(d.bits_per_pixel, 12);
        assert_eq!(d.nb_planes, 3);
        assert_eq!(d.log2_chroma_w, 1);
        assert_eq!(d.log2_chroma_h, 1);
        assert!(d.is_planar);
        assert!(!d.has_alpha);
        assert!(!d.is_hwaccel);
        assert_eq!(d.bits_per_component, 8);
    }

    #[test]
    fn rgba_descriptor() {
        let d = PixelFormat::Rgba.descriptor();
        assert_eq!(d.nb_components, 4);
        assert_eq!(d.bits_per_pixel, 32);
        assert!(!d.is_planar);
        assert!(d.has_alpha);
        assert_eq!(d.nb_planes, 1);
    }

    #[test]
    fn nv12_semi_planar() {
        let d = PixelFormat::Nv12.descriptor();
        assert_eq!(d.nb_planes, 2);
        assert!(d.is_planar);
        assert_eq!(d.bits_per_pixel, 12);
    }

    #[test]
    fn ten_bit_yuv() {
        let d = PixelFormat::Yuv420p10Le.descriptor();
        assert_eq!(d.bits_per_component, 10);
        assert_eq!(d.bits_per_pixel, 15);
    }

    #[test]
    fn hw_formats_are_opaque() {
        for fmt in [PixelFormat::Vaapi, PixelFormat::D3d11, PixelFormat::VideoToolbox, PixelFormat::Cuda] {
            let d = fmt.descriptor();
            assert!(d.is_hwaccel);
            assert_eq!(d.nb_components, 0);
            assert_eq!(d.bits_per_pixel, 0);
        }
    }

    #[test]
    fn gray8_single_component() {
        let d = PixelFormat::Gray8.descriptor();
        assert_eq!(d.nb_components, 1);
        assert_eq!(d.bits_per_pixel, 8);
    }

    #[test]
    fn packed_yuv_not_planar() {
        assert!(!PixelFormat::Yuyv422.descriptor().is_planar);
        assert!(!PixelFormat::Uyvy422.descriptor().is_planar);
    }

    #[test]
    fn display_format() {
        assert_eq!(format!("{}", PixelFormat::Yuv420p), "yuv420p");
        assert_eq!(format!("{}", PixelFormat::Rgba), "rgba");
    }

    #[test]
    fn from_name_valid() {
        assert_eq!(PixelFormat::from_name("yuv420p"), Some(PixelFormat::Yuv420p));
        assert_eq!(PixelFormat::from_name("rgba"), Some(PixelFormat::Rgba));
        assert_eq!(PixelFormat::from_name("nv12"), Some(PixelFormat::Nv12));
    }

    #[test]
    fn all_formats_have_unique_names() {
        let mut names: Vec<&str> = ALL_PIXEL_FORMATS.iter().map(|f| f.name()).collect();
        let len_before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), len_before, "duplicate pixel format names found");
    }

    #[test]
    fn all_formats_roundtrip_name() {
        for fmt in ALL_PIXEL_FORMATS {
            let name = fmt.name();
            assert_eq!(PixelFormat::from_name(name), Some(*fmt), "roundtrip failed for {name}");
        }
    }

    // ── Negative ──

    #[test]
    fn from_name_invalid() {
        assert_eq!(PixelFormat::from_name("not_a_format"), None);
        assert_eq!(PixelFormat::from_name(""), None);
    }

    #[test]
    fn from_name_case_sensitive() {
        assert_eq!(PixelFormat::from_name("YUV420P"), None);
    }
}
