use crate::error::{Error, Result};
use crate::samplefmt::SampleFormat;

/// A FIFO queue specialized for audio samples.
///
/// Stores audio data and tracks the number of queued samples.
/// Supports interleaved and planar formats. Internally uses a ring buffer
/// per plane.
pub struct AudioFifo {
    planes: Vec<Vec<u8>>,
    read_pos: usize,
    write_pos: usize,
    /// Number of samples currently queued.
    queued: u32,
    /// Maximum samples the FIFO can hold.
    capacity: u32,
    format: SampleFormat,
    channels: u16,
    bytes_per_sample_per_plane: usize,
}

impl AudioFifo {
    /// Create a new audio FIFO.
    ///
    /// `capacity` is in samples (not bytes).
    pub fn new(format: SampleFormat, channels: u16, capacity: u32) -> Result<Self> {
        if channels == 0 {
            return Err(Error::InvalidArgument("channels must be > 0".into()));
        }
        if capacity == 0 {
            return Err(Error::InvalidArgument("capacity must be > 0".into()));
        }

        let nb_planes = if format.is_planar() { channels as usize } else { 1 };
        let bytes_per_sample_per_plane = if format.is_planar() {
            format.bytes_per_sample()
        } else {
            format.bytes_per_sample() * channels as usize
        };
        let plane_bytes = bytes_per_sample_per_plane * capacity as usize;

        Ok(Self {
            planes: vec![vec![0u8; plane_bytes]; nb_planes],
            read_pos: 0,
            write_pos: 0,
            queued: 0,
            capacity,
            format,
            channels,
            bytes_per_sample_per_plane,
        })
    }

    /// Number of samples available to read.
    pub fn queued_samples(&self) -> u32 {
        self.queued
    }

    /// Number of samples of free space.
    pub fn available_space(&self) -> u32 {
        self.capacity - self.queued
    }

    /// Maximum capacity in samples.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// True if no samples are queued.
    pub fn is_empty(&self) -> bool {
        self.queued == 0
    }

    /// Write samples into the FIFO.
    ///
    /// `data` is a slice of plane buffers: 1 for interleaved, `channels` for planar.
    /// `nb_samples` is how many samples to write from each plane.
    pub fn write(&mut self, data: &[&[u8]], nb_samples: u32) -> Result<()> {
        if nb_samples == 0 {
            return Ok(());
        }
        if nb_samples > self.available_space() {
            return Err(Error::NoMemory);
        }

        let expected_planes = self.planes.len();
        if data.len() < expected_planes {
            return Err(Error::InvalidArgument(format!(
                "expected {} planes, got {}", expected_planes, data.len()
            )));
        }

        let bytes_to_write = nb_samples as usize * self.bytes_per_sample_per_plane;
        let cap_bytes = self.capacity as usize * self.bytes_per_sample_per_plane;

        for (i, plane) in self.planes.iter_mut().enumerate() {
            if data[i].len() < bytes_to_write {
                return Err(Error::InvalidArgument(format!(
                    "plane {i} data too small: {} < {bytes_to_write}", data[i].len()
                )));
            }
            let mut src_off = 0;
            let mut dst_off = self.write_pos * self.bytes_per_sample_per_plane;
            let mut remaining = bytes_to_write;

            while remaining > 0 {
                let chunk = remaining.min(cap_bytes - dst_off);
                plane[dst_off..dst_off + chunk]
                    .copy_from_slice(&data[i][src_off..src_off + chunk]);
                src_off += chunk;
                dst_off = (dst_off + chunk) % cap_bytes;
                remaining -= chunk;
            }
        }

        self.write_pos = (self.write_pos + nb_samples as usize) % self.capacity as usize;
        self.queued += nb_samples;
        Ok(())
    }

    /// Read samples from the FIFO.
    ///
    /// `data` is a mutable slice of plane buffers to fill.
    /// `nb_samples` is how many samples to read.
    pub fn read(&mut self, data: &mut [&mut [u8]], nb_samples: u32) -> Result<()> {
        if nb_samples == 0 {
            return Ok(());
        }
        if nb_samples > self.queued {
            return Err(Error::Eof);
        }

        let expected_planes = self.planes.len();
        if data.len() < expected_planes {
            return Err(Error::InvalidArgument(format!(
                "expected {} planes, got {}", expected_planes, data.len()
            )));
        }

        let bytes_to_read = nb_samples as usize * self.bytes_per_sample_per_plane;
        let cap_bytes = self.capacity as usize * self.bytes_per_sample_per_plane;

        for (i, plane) in self.planes.iter().enumerate() {
            if data[i].len() < bytes_to_read {
                return Err(Error::InvalidArgument(format!(
                    "plane {i} buffer too small: {} < {bytes_to_read}", data[i].len()
                )));
            }
            let mut dst_off = 0;
            let mut src_off = self.read_pos * self.bytes_per_sample_per_plane;
            let mut remaining = bytes_to_read;

            while remaining > 0 {
                let chunk = remaining.min(cap_bytes - src_off);
                data[i][dst_off..dst_off + chunk]
                    .copy_from_slice(&plane[src_off..src_off + chunk]);
                dst_off += chunk;
                src_off = (src_off + chunk) % cap_bytes;
                remaining -= chunk;
            }
        }

        self.read_pos = (self.read_pos + nb_samples as usize) % self.capacity as usize;
        self.queued -= nb_samples;
        Ok(())
    }

    /// Discard `nb_samples` from the read side.
    pub fn drain(&mut self, nb_samples: u32) -> Result<()> {
        if nb_samples > self.queued {
            return Err(Error::Eof);
        }
        self.read_pos = (self.read_pos + nb_samples as usize) % self.capacity as usize;
        self.queued -= nb_samples;
        Ok(())
    }

    /// Reset to empty state.
    pub fn reset(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.queued = 0;
    }

    /// The sample format of this FIFO.
    pub fn format(&self) -> SampleFormat {
        self.format
    }

    /// The number of channels.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl std::fmt::Debug for AudioFifo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioFifo")
            .field("format", &self.format)
            .field("channels", &self.channels)
            .field("capacity", &self.capacity)
            .field("queued", &self.queued)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn new_interleaved() {
        let af = AudioFifo::new(SampleFormat::S16, 2, 1024).unwrap();
        assert_eq!(af.capacity(), 1024);
        assert_eq!(af.queued_samples(), 0);
        assert_eq!(af.available_space(), 1024);
        assert!(af.is_empty());
        assert_eq!(af.channels(), 2);
        assert_eq!(af.format(), SampleFormat::S16);
    }

    #[test]
    fn write_and_read_interleaved() {
        let mut af = AudioFifo::new(SampleFormat::S16, 2, 1024).unwrap();

        // Write 100 samples (s16, 2ch = 400 bytes).
        let src = vec![42u8; 400];
        af.write(&[&src], 100).unwrap();
        assert_eq!(af.queued_samples(), 100);

        // Read them back.
        let mut dst = vec![0u8; 400];
        af.read(&mut [dst.as_mut_slice()], 100).unwrap();
        assert_eq!(dst[0], 42);
        assert!(af.is_empty());
    }

    #[test]
    fn write_and_read_planar() {
        let mut af = AudioFifo::new(SampleFormat::F32p, 2, 512).unwrap();

        let src_l = vec![1u8; 512 * 4]; // 512 samples * 4 bytes
        let src_r = vec![2u8; 512 * 4];
        af.write(&[&src_l, &src_r], 512).unwrap();
        assert_eq!(af.queued_samples(), 512);

        let mut dst_l = vec![0u8; 512 * 4];
        let mut dst_r = vec![0u8; 512 * 4];
        af.read(&mut [dst_l.as_mut_slice(), dst_r.as_mut_slice()], 512).unwrap();
        assert_eq!(dst_l[0], 1);
        assert_eq!(dst_r[0], 2);
    }

    #[test]
    fn partial_read() {
        let mut af = AudioFifo::new(SampleFormat::S16, 1, 1024).unwrap();
        let src = vec![0xABu8; 2048]; // 1024 samples * 2 bytes
        af.write(&[&src], 1024).unwrap();

        // Read first 100.
        let mut dst = vec![0u8; 200];
        af.read(&mut [dst.as_mut_slice()], 100).unwrap();
        assert_eq!(af.queued_samples(), 924);
        assert_eq!(dst[0], 0xAB);
    }

    #[test]
    fn wrap_around() {
        let mut af = AudioFifo::new(SampleFormat::U8, 1, 4).unwrap();

        // Write 3, read 2, write 3 → forces wrap.
        let src = vec![1, 2, 3];
        af.write(&[&src], 3).unwrap();
        let mut dst = vec![0u8; 2];
        af.read(&mut [dst.as_mut_slice()], 2).unwrap();
        assert_eq!(dst, [1, 2]);

        let src2 = vec![4, 5, 6];
        af.write(&[&src2], 3).unwrap();
        assert_eq!(af.queued_samples(), 4); // 1 remaining + 3 new

        let mut dst2 = vec![0u8; 4];
        af.read(&mut [dst2.as_mut_slice()], 4).unwrap();
        assert_eq!(dst2, [3, 4, 5, 6]);
    }

    #[test]
    fn drain_samples() {
        let mut af = AudioFifo::new(SampleFormat::S16, 1, 100).unwrap();
        let src = vec![0u8; 200];
        af.write(&[&src], 100).unwrap();
        af.drain(50).unwrap();
        assert_eq!(af.queued_samples(), 50);
    }

    #[test]
    fn reset_clears() {
        let mut af = AudioFifo::new(SampleFormat::S16, 1, 100).unwrap();
        let src = vec![0u8; 200];
        af.write(&[&src], 100).unwrap();
        af.reset();
        assert!(af.is_empty());
        assert_eq!(af.available_space(), 100);
    }

    // ── Negative ──

    #[test]
    fn new_zero_channels() {
        assert!(AudioFifo::new(SampleFormat::S16, 0, 100).is_err());
    }

    #[test]
    fn new_zero_capacity() {
        assert!(AudioFifo::new(SampleFormat::S16, 2, 0).is_err());
    }

    #[test]
    fn write_overflow() {
        let mut af = AudioFifo::new(SampleFormat::U8, 1, 4).unwrap();
        let src = vec![0u8; 4];
        af.write(&[&src], 4).unwrap();
        assert!(af.write(&[&[0u8]], 1).is_err());
    }

    #[test]
    fn read_underflow() {
        let mut af = AudioFifo::new(SampleFormat::U8, 1, 4).unwrap();
        let mut dst = vec![0u8; 1];
        assert!(af.read(&mut [dst.as_mut_slice()], 1).is_err());
    }

    #[test]
    fn write_wrong_plane_count() {
        let mut af = AudioFifo::new(SampleFormat::F32p, 2, 100).unwrap();
        let src = vec![0u8; 400];
        assert!(af.write(&[&src], 100).is_err()); // needs 2 planes
    }

    #[test]
    fn drain_underflow() {
        let mut af = AudioFifo::new(SampleFormat::U8, 1, 4).unwrap();
        assert!(af.drain(1).is_err());
    }

    // ── Edge ──

    #[test]
    fn write_zero_samples() {
        let mut af = AudioFifo::new(SampleFormat::S16, 2, 100).unwrap();
        af.write(&[&[]], 0).unwrap(); // no-op
        assert!(af.is_empty());
    }

    #[test]
    fn read_zero_samples() {
        let mut af = AudioFifo::new(SampleFormat::S16, 2, 100).unwrap();
        af.read(&mut [&mut []], 0).unwrap(); // no-op
    }
}
