use av_codec::packet::Packet;
use av_util::error::{Error, Result};

use crate::demux::Demuxer;
use crate::io::IOContext;
use crate::metadata::Metadata;
use crate::mux::Muxer;
use crate::stream::Stream;

/// High-level context for reading a container.
pub struct InputContext {
    io: IOContext,
    demuxer: Box<dyn Demuxer>,
    /// Streams discovered in the header.
    pub streams: Vec<Stream>,
    /// Container metadata.
    pub metadata: Metadata,
    opened: bool,
}

impl InputContext {
    /// Create an input context with the given I/O and demuxer.
    pub fn new(io: IOContext, demuxer: Box<dyn Demuxer>) -> Self {
        Self {
            io,
            demuxer,
            streams: Vec::new(),
            metadata: Metadata::new(),
            opened: false,
        }
    }

    /// Read the container header, populating streams and metadata.
    pub fn open(&mut self) -> Result<()> {
        if self.opened {
            return Err(Error::InvalidState("already opened".into()));
        }
        let header = self.demuxer.read_header(&mut self.io)?;
        self.streams = header.streams;
        self.metadata = header.metadata;
        self.opened = true;
        Ok(())
    }

    /// Read the next packet from the container.
    pub fn read_packet(&mut self) -> Result<Packet> {
        if !self.opened {
            return Err(Error::InvalidState("not opened".into()));
        }
        self.demuxer.read_packet(&mut self.io)
    }

    /// Number of streams.
    pub fn nb_streams(&self) -> usize {
        self.streams.len()
    }

    /// Get the demuxer format name.
    pub fn format_name(&self) -> &str {
        self.demuxer.name()
    }
}

/// High-level context for writing a container.
pub struct OutputContext {
    io: IOContext,
    muxer: Box<dyn Muxer>,
    /// Streams to be written.
    pub streams: Vec<Stream>,
    header_written: bool,
    trailer_written: bool,
}

impl OutputContext {
    /// Create an output context with the given I/O and muxer.
    pub fn new(io: IOContext, muxer: Box<dyn Muxer>) -> Self {
        Self {
            io,
            muxer,
            streams: Vec::new(),
            header_written: false,
            trailer_written: false,
        }
    }

    /// Add a stream to the output.
    pub fn add_stream(&mut self, stream: Stream) {
        self.streams.push(stream);
    }

    /// Write the container header. Must be called before `write_packet`.
    pub fn write_header(&mut self) -> Result<()> {
        if self.header_written {
            return Err(Error::InvalidState("header already written".into()));
        }
        if self.streams.is_empty() {
            return Err(Error::InvalidArgument("no streams added".into()));
        }
        self.muxer.write_header(&mut self.io, &self.streams)?;
        self.header_written = true;
        Ok(())
    }

    /// Write a packet to the container.
    pub fn write_packet(&mut self, packet: &Packet) -> Result<()> {
        if !self.header_written {
            return Err(Error::InvalidState("header not written".into()));
        }
        if self.trailer_written {
            return Err(Error::InvalidState("trailer already written".into()));
        }
        self.muxer.write_packet(&mut self.io, packet)
    }

    /// Write the container trailer (finalize).
    pub fn write_trailer(&mut self) -> Result<()> {
        if !self.header_written {
            return Err(Error::InvalidState("header not written".into()));
        }
        if self.trailer_written {
            return Err(Error::InvalidState("trailer already written".into()));
        }
        self.muxer.write_trailer(&mut self.io)?;
        self.trailer_written = true;
        Ok(())
    }

    /// Consume the context and return the underlying I/O.
    pub fn into_io(self) -> IOContext {
        self.io
    }

    /// Get the muxer format name.
    pub fn format_name(&self) -> &str {
        self.muxer.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::wav::{WavDemuxer, WavMuxer};
    use av_codec::codec::CodecId;
    use av_codec::codec_par::CodecParameters;
    use av_codec::packet::Packet;
    use av_util::buffer::Buffer;

    fn audio_stream() -> Stream {
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();
        Stream::new(0, params)
    }

    #[test]
    fn send_traits() {
        fn assert_send<T: Send>() {}
        assert_send::<InputContext>();
        assert_send::<OutputContext>();
    }

    // ── InputContext state machine ──

    #[test]
    fn input_read_before_open() {
        let io = IOContext::from_memory(vec![0u8; 100]);
        let mut ctx = InputContext::new(io, Box::new(WavDemuxer::new()));
        assert!(ctx.read_packet().is_err());
    }

    #[test]
    fn input_double_open() {
        // Build a valid WAV first.
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(audio_stream());
        out.write_header().unwrap();
        out.write_packet(&Packet::new(Buffer::from_vec(vec![0u8; 20]))).unwrap();
        out.write_trailer().unwrap();
        let wav = out.into_io().into_vec().unwrap();

        let mut ctx = InputContext::new(IOContext::from_memory(wav), Box::new(WavDemuxer::new()));
        ctx.open().unwrap();
        assert!(ctx.open().is_err()); // already opened
    }

    // ── OutputContext state machine ──

    #[test]
    fn output_write_packet_before_header() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));
        out.add_stream(audio_stream());
        let pkt = Packet::new(Buffer::from_vec(vec![0u8; 10]));
        assert!(out.write_packet(&pkt).is_err());
    }

    #[test]
    fn output_write_trailer_before_header() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));
        out.add_stream(audio_stream());
        assert!(out.write_trailer().is_err());
    }

    #[test]
    fn output_double_header() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));
        out.add_stream(audio_stream());
        out.write_header().unwrap();
        assert!(out.write_header().is_err());
    }

    #[test]
    fn output_write_packet_after_trailer() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));
        out.add_stream(audio_stream());
        out.write_header().unwrap();
        out.write_trailer().unwrap();
        let pkt = Packet::new(Buffer::from_vec(vec![0u8; 10]));
        assert!(out.write_packet(&pkt).is_err());
    }

    #[test]
    fn output_double_trailer() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));
        out.add_stream(audio_stream());
        out.write_header().unwrap();
        out.write_trailer().unwrap();
        assert!(out.write_trailer().is_err());
    }

    #[test]
    fn output_no_streams() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));
        assert!(out.write_header().is_err());
    }
}
