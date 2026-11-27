//! Surface present-mode and frame-latency settings.
//!
//! Generic render-backend configuration: how the swapchain presents frames. The selection
//! is a pure function over the modes the surface reports, so it is testable without a GPU.

use std::fmt;

/// Requested surface present mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresentModeSetting {
    /// Vertical sync, always supported (the default).
    #[default]
    Fifo,
    /// Low-latency vsync with replacement; falls back to Fifo when unsupported.
    Mailbox,
    /// No sync (may tear); falls back to Fifo when unsupported.
    Immediate,
    /// Mailbox when supported, otherwise Fifo. Never Immediate.
    Auto,
}

/// Error for a rejected present setting string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentParseError(pub String);

impl fmt::Display for PresentParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PresentParseError {}

impl PresentModeSetting {
    /// Parse `fifo|mailbox|immediate|auto` (lower case, exact).
    ///
    /// # Errors
    /// Any other string.
    pub fn parse(s: &str) -> Result<Self, PresentParseError> {
        match s {
            "fifo" => Ok(Self::Fifo),
            "mailbox" => Ok(Self::Mailbox),
            "immediate" => Ok(Self::Immediate),
            "auto" => Ok(Self::Auto),
            other => Err(PresentParseError(format!(
                "unknown present mode `{other}` (expected fifo|mailbox|immediate|auto)"
            ))),
        }
    }

    /// The lower-case name accepted by [`PresentModeSetting::parse`].
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Fifo => "fifo",
            Self::Mailbox => "mailbox",
            Self::Immediate => "immediate",
            Self::Auto => "auto",
        }
    }
}

/// Maximum number of frames the presentation engine may queue (1..=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLatency(u32);

impl FrameLatency {
    /// Validate a latency in `1..=3`.
    ///
    /// # Errors
    /// Values outside the range.
    pub fn new(n: u32) -> Result<Self, PresentParseError> {
        if (1..=3).contains(&n) {
            Ok(Self(n))
        } else {
            Err(PresentParseError(format!(
                "frame latency {n} out of range (expected 1, 2 or 3)"
            )))
        }
    }

    /// Parse `1|2|3`.
    ///
    /// # Errors
    /// Non-numeric or out-of-range input.
    pub fn parse(s: &str) -> Result<Self, PresentParseError> {
        let n: u32 = s
            .parse()
            .map_err(|_| PresentParseError(format!("invalid frame latency `{s}`")))?;
        Self::new(n)
    }

    /// The latency value.
    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }
}

impl Default for FrameLatency {
    fn default() -> Self {
        Self(2)
    }
}

/// Present configuration passed to the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PresentConfig {
    /// Requested present mode.
    pub mode: PresentModeSetting,
    /// Maximum frame latency.
    pub frame_latency: FrameLatency,
}

/// Result of [`resolve_present_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    /// The mode to configure.
    pub active: wgpu::PresentMode,
    /// True when an explicit request could not be honoured.
    pub fell_back: bool,
}

/// Pick the surface present mode for `request` among the `supported` modes.
///
/// Fifo is guaranteed by the spec, so it is the fallback. An empty `supported` list
/// resolves to Fifo with `fell_back` set (except for `Auto`, which never errors).
#[must_use]
pub fn resolve_present_mode(
    request: PresentModeSetting,
    supported: &[wgpu::PresentMode],
) -> Resolved {
    let has = |m| supported.contains(&m);
    let fifo = |fell_back| Resolved {
        active: wgpu::PresentMode::Fifo,
        fell_back,
    };
    let want = |m| {
        if has(m) {
            Resolved {
                active: m,
                fell_back: false,
            }
        } else {
            fifo(true)
        }
    };
    match request {
        PresentModeSetting::Fifo => fifo(supported.is_empty()),
        PresentModeSetting::Mailbox => want(wgpu::PresentMode::Mailbox),
        PresentModeSetting::Immediate => want(wgpu::PresentMode::Immediate),
        PresentModeSetting::Auto => fifo(false).pick_mailbox(supported),
    }
}

impl Resolved {
    fn pick_mailbox(self, supported: &[wgpu::PresentMode]) -> Self {
        if supported.contains(&wgpu::PresentMode::Mailbox) {
            Self {
                active: wgpu::PresentMode::Mailbox,
                fell_back: false,
            }
        } else {
            self
        }
    }
}

/// What the backend actually configured (strings, so callers need no wgpu types).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentInfo {
    /// The requested setting.
    pub requested: PresentModeSetting,
    /// The active mode name, or `none (offscreen)` without a surface.
    pub active: String,
    /// Modes the surface supports.
    pub supported: Vec<String>,
    /// Configured maximum frame latency.
    pub frame_latency: u32,
    /// True when the request fell back to Fifo.
    pub fell_back: bool,
}

impl PresentInfo {
    /// Info for a backend without a surface.
    #[must_use]
    pub fn offscreen(config: &PresentConfig) -> Self {
        Self {
            requested: config.mode,
            active: "none (offscreen)".into(),
            supported: Vec::new(),
            frame_latency: config.frame_latency.get(),
            fell_back: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::PresentMode as P;

    #[test]
    fn mode_parse_round_trips_and_is_strict() {
        for n in ["fifo", "mailbox", "immediate", "auto"] {
            assert_eq!(PresentModeSetting::parse(n).unwrap().name(), n);
        }
        for bad in ["Fifo", "", " fifo", "vsync"] {
            assert!(PresentModeSetting::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn latency_parse_range() {
        for (s, v) in [("1", 1), ("2", 2), ("3", 3)] {
            assert_eq!(FrameLatency::parse(s).unwrap().get(), v);
        }
        for bad in ["0", "4", "x", ""] {
            assert!(FrameLatency::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn resolution_rules() {
        let all = [P::Fifo, P::Mailbox, P::Immediate];
        let fifo_only = [P::Fifo];
        let r = resolve_present_mode(PresentModeSetting::Fifo, &all);
        assert_eq!((r.active, r.fell_back), (P::Fifo, false));
        let r = resolve_present_mode(PresentModeSetting::Mailbox, &all);
        assert_eq!((r.active, r.fell_back), (P::Mailbox, false));
        let r = resolve_present_mode(PresentModeSetting::Mailbox, &fifo_only);
        assert_eq!((r.active, r.fell_back), (P::Fifo, true));
        let r = resolve_present_mode(PresentModeSetting::Immediate, &all);
        assert_eq!((r.active, r.fell_back), (P::Immediate, false));
        let r = resolve_present_mode(PresentModeSetting::Immediate, &fifo_only);
        assert_eq!((r.active, r.fell_back), (P::Fifo, true));
        let r = resolve_present_mode(PresentModeSetting::Auto, &all);
        assert_eq!((r.active, r.fell_back), (P::Mailbox, false));
        let r = resolve_present_mode(PresentModeSetting::Auto, &[P::Fifo, P::Immediate]);
        assert_eq!((r.active, r.fell_back), (P::Fifo, false));
        for m in [
            PresentModeSetting::Fifo,
            PresentModeSetting::Mailbox,
            PresentModeSetting::Immediate,
        ] {
            let r = resolve_present_mode(m, &[]);
            assert_eq!(r.active, P::Fifo);
            assert!(r.fell_back, "{m:?}");
        }
        assert_eq!(
            resolve_present_mode(PresentModeSetting::Auto, &[]).active,
            P::Fifo
        );
    }

    #[test]
    fn default_config_is_fifo_latency_two() {
        let c = PresentConfig::default();
        assert_eq!(c.mode, PresentModeSetting::Fifo);
        assert_eq!(c.frame_latency.get(), 2);
        let info = PresentInfo::offscreen(&c);
        assert_eq!(info.active, "none (offscreen)");
        assert_eq!(info.frame_latency, 2);
    }
}
