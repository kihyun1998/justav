use core::fmt;

/// Individual audio channel identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Channel {
    FrontLeft,
    FrontRight,
    FrontCenter,
    Lfe,
    BackLeft,
    BackRight,
    FrontLeftOfCenter,
    FrontRightOfCenter,
    BackCenter,
    SideLeft,
    SideRight,
    TopCenter,
    TopFrontLeft,
    TopFrontCenter,
    TopFrontRight,
    TopBackLeft,
    TopBackCenter,
    TopBackRight,
}

impl Channel {
    /// Short name for this channel.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::FrontLeft => "FL",
            Self::FrontRight => "FR",
            Self::FrontCenter => "FC",
            Self::Lfe => "LFE",
            Self::BackLeft => "BL",
            Self::BackRight => "BR",
            Self::FrontLeftOfCenter => "FLC",
            Self::FrontRightOfCenter => "FRC",
            Self::BackCenter => "BC",
            Self::SideLeft => "SL",
            Self::SideRight => "SR",
            Self::TopCenter => "TC",
            Self::TopFrontLeft => "TFL",
            Self::TopFrontCenter => "TFC",
            Self::TopFrontRight => "TFR",
            Self::TopBackLeft => "TBL",
            Self::TopBackCenter => "TBC",
            Self::TopBackRight => "TBR",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Describes the speaker layout for an audio stream.
///
/// Can be a well-known layout (e.g. stereo, 5.1) or a custom list of channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelLayout {
    channels: Vec<Channel>,
}

impl ChannelLayout {
    // ── Standard layouts ──

    /// Mono (front center).
    pub fn mono() -> Self {
        Self { channels: vec![Channel::FrontCenter] }
    }

    /// Stereo (front left + front right).
    pub fn stereo() -> Self {
        Self { channels: vec![Channel::FrontLeft, Channel::FrontRight] }
    }

    /// 2.1 (stereo + LFE).
    pub fn layout_2point1() -> Self {
        Self { channels: vec![Channel::FrontLeft, Channel::FrontRight, Channel::Lfe] }
    }

    /// Surround 3.0 (FL + FR + FC).
    pub fn surround() -> Self {
        Self {
            channels: vec![Channel::FrontLeft, Channel::FrontRight, Channel::FrontCenter],
        }
    }

    /// Quad 4.0 (FL + FR + BL + BR).
    pub fn quad() -> Self {
        Self {
            channels: vec![
                Channel::FrontLeft, Channel::FrontRight,
                Channel::BackLeft, Channel::BackRight,
            ],
        }
    }

    /// 5.0 (FL + FR + FC + BL + BR).
    pub fn layout_5point0() -> Self {
        Self {
            channels: vec![
                Channel::FrontLeft, Channel::FrontRight, Channel::FrontCenter,
                Channel::BackLeft, Channel::BackRight,
            ],
        }
    }

    /// 5.1 (FL + FR + FC + LFE + BL + BR).
    pub fn layout_5point1() -> Self {
        Self {
            channels: vec![
                Channel::FrontLeft, Channel::FrontRight, Channel::FrontCenter,
                Channel::Lfe, Channel::BackLeft, Channel::BackRight,
            ],
        }
    }

    /// 7.1 (FL + FR + FC + LFE + BL + BR + SL + SR).
    pub fn layout_7point1() -> Self {
        Self {
            channels: vec![
                Channel::FrontLeft, Channel::FrontRight, Channel::FrontCenter,
                Channel::Lfe, Channel::BackLeft, Channel::BackRight,
                Channel::SideLeft, Channel::SideRight,
            ],
        }
    }

    // ── Custom ──

    /// Create a layout from an explicit list of channels.
    pub fn from_channels(channels: Vec<Channel>) -> Self {
        Self { channels }
    }

    // ── Queries ──

    /// Number of channels in this layout.
    pub fn nb_channels(&self) -> u16 {
        self.channels.len() as u16
    }

    /// Get the channel at a given index.
    pub fn channel(&self, index: usize) -> Option<Channel> {
        self.channels.get(index).copied()
    }

    /// Iterate over all channels.
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// Returns true if this layout contains the given channel.
    pub fn contains(&self, ch: Channel) -> bool {
        self.channels.contains(&ch)
    }

    /// Returns true if this is a mono layout.
    pub fn is_mono(&self) -> bool {
        self.channels.len() == 1 && self.channels[0] == Channel::FrontCenter
    }

    /// Returns true if this is a stereo layout.
    pub fn is_stereo(&self) -> bool {
        self.channels.len() == 2
            && self.channels[0] == Channel::FrontLeft
            && self.channels[1] == Channel::FrontRight
    }

    /// Get a standard layout name, if this matches one.
    pub fn name(&self) -> &'static str {
        if *self == Self::mono() { return "mono"; }
        if *self == Self::stereo() { return "stereo"; }
        if *self == Self::layout_2point1() { return "2.1"; }
        if *self == Self::surround() { return "3.0"; }
        if *self == Self::quad() { return "quad"; }
        if *self == Self::layout_5point0() { return "5.0"; }
        if *self == Self::layout_5point1() { return "5.1"; }
        if *self == Self::layout_7point1() { return "7.1"; }
        "custom"
    }

    /// Parse a channel layout from a well-known name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "mono" => Some(Self::mono()),
            "stereo" => Some(Self::stereo()),
            "2.1" => Some(Self::layout_2point1()),
            "3.0" | "surround" => Some(Self::surround()),
            "quad" | "4.0" => Some(Self::quad()),
            "5.0" => Some(Self::layout_5point0()),
            "5.1" => Some(Self::layout_5point1()),
            "7.1" => Some(Self::layout_7point1()),
            _ => None,
        }
    }
}

impl fmt::Display for ChannelLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.name();
        if name != "custom" {
            return f.write_str(name);
        }
        // Custom: print channel names joined by '+'.
        for (i, ch) in self.channels.iter().enumerate() {
            if i > 0 { f.write_str("+")?; }
            fmt::Display::fmt(ch, f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn mono_layout() {
        let l = ChannelLayout::mono();
        assert_eq!(l.nb_channels(), 1);
        assert!(l.is_mono());
        assert!(!l.is_stereo());
        assert_eq!(l.name(), "mono");
    }

    #[test]
    fn stereo_layout() {
        let l = ChannelLayout::stereo();
        assert_eq!(l.nb_channels(), 2);
        assert!(l.is_stereo());
        assert!(!l.is_mono());
        assert_eq!(l.name(), "stereo");
    }

    #[test]
    fn layout_5point1() {
        let l = ChannelLayout::layout_5point1();
        assert_eq!(l.nb_channels(), 6);
        assert!(l.contains(Channel::Lfe));
        assert!(l.contains(Channel::FrontCenter));
        assert_eq!(l.name(), "5.1");
    }

    #[test]
    fn layout_7point1() {
        let l = ChannelLayout::layout_7point1();
        assert_eq!(l.nb_channels(), 8);
        assert!(l.contains(Channel::SideLeft));
        assert!(l.contains(Channel::SideRight));
        assert_eq!(l.name(), "7.1");
    }

    #[test]
    fn channel_by_index() {
        let l = ChannelLayout::stereo();
        assert_eq!(l.channel(0), Some(Channel::FrontLeft));
        assert_eq!(l.channel(1), Some(Channel::FrontRight));
    }

    #[test]
    fn display_standard() {
        assert_eq!(format!("{}", ChannelLayout::mono()), "mono");
        assert_eq!(format!("{}", ChannelLayout::layout_5point1()), "5.1");
    }

    #[test]
    fn display_custom() {
        let l = ChannelLayout::from_channels(vec![Channel::FrontLeft, Channel::Lfe]);
        assert_eq!(format!("{l}"), "FL+LFE");
    }

    #[test]
    fn from_name_valid() {
        assert_eq!(ChannelLayout::from_name("stereo"), Some(ChannelLayout::stereo()));
        assert_eq!(ChannelLayout::from_name("5.1"), Some(ChannelLayout::layout_5point1()));
        assert_eq!(ChannelLayout::from_name("surround"), Some(ChannelLayout::surround()));
        assert_eq!(ChannelLayout::from_name("quad"), Some(ChannelLayout::quad()));
    }

    #[test]
    fn standard_layouts_roundtrip_name() {
        let layouts = [
            ("mono", ChannelLayout::mono()),
            ("stereo", ChannelLayout::stereo()),
            ("2.1", ChannelLayout::layout_2point1()),
            ("5.0", ChannelLayout::layout_5point0()),
            ("5.1", ChannelLayout::layout_5point1()),
            ("7.1", ChannelLayout::layout_7point1()),
        ];
        for (name, layout) in &layouts {
            assert_eq!(layout.name(), *name);
            assert_eq!(ChannelLayout::from_name(name).as_ref(), Some(layout));
        }
    }

    #[test]
    fn equality() {
        assert_eq!(ChannelLayout::stereo(), ChannelLayout::stereo());
        assert_ne!(ChannelLayout::mono(), ChannelLayout::stereo());
    }

    #[test]
    fn contains_channel() {
        let l = ChannelLayout::stereo();
        assert!(l.contains(Channel::FrontLeft));
        assert!(!l.contains(Channel::Lfe));
    }

    #[test]
    fn channel_names() {
        assert_eq!(Channel::FrontLeft.name(), "FL");
        assert_eq!(Channel::Lfe.name(), "LFE");
        assert_eq!(Channel::SideRight.name(), "SR");
    }

    // ── Negative ──

    #[test]
    fn from_name_invalid() {
        assert_eq!(ChannelLayout::from_name("11.1"), None);
        assert_eq!(ChannelLayout::from_name(""), None);
    }

    #[test]
    fn channel_index_out_of_bounds() {
        let l = ChannelLayout::mono();
        assert_eq!(l.channel(1), None);
        assert_eq!(l.channel(100), None);
    }

    // ── Edge ──

    #[test]
    fn empty_custom_layout() {
        let l = ChannelLayout::from_channels(vec![]);
        assert_eq!(l.nb_channels(), 0);
        assert!(!l.is_mono());
        assert!(!l.is_stereo());
        assert_eq!(l.name(), "custom");
    }
}
