//! The device layer: what an operating-system device is, and which ones a
//! token opens.
//!
//! # A ruleset cannot exist without a token
//!
//! [`DeviceRuleset::for_token`] is the only constructor. There is no `Default`,
//! no `new`, and no way to name a device class and get a ruleset holding it, so
//! a value of this type is proof that `mint_capture_capability` returned a
//! token — which is proof that `bind_permission` ran. That is section 3.7's
//! "the recorder holds no microphone capability by default" as a type rather
//! than as a check somebody has to remember to write.
//!
//! # Allowed media is enforced here, not above here
//!
//! [`DeviceClass::of`] is a total map from section 3.7's four media to the
//! three device kinds an operating system actually hands out. A grant naming
//! only `AUDIO` produces a ruleset holding only [`DeviceClass::Microphone`], so
//! a camera request is refused at the layer that would have opened the camera.
//! With the `native-capture` feature the same ruleset is what the platform
//! backend installs, so the refusal is the operating system's rather than this
//! module's — see [`crate::native`].

use academic_consent::{CaptureCapabilityToken, CaptureMedium};

/// The operating-system device kinds this system opens.
///
/// Closed. Section 3.7 names four media; an operating system exposes three
/// device kinds for them, and [`DeviceClass::of`] is the map. Screen capture is
/// a device kind here because it is a separate platform permission, not because
/// it is a camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DeviceClass {
    /// An audio input endpoint.
    Microphone,
    /// An image or video input endpoint.
    Camera,
    /// The presented screen.
    Screen,
}

/// Every device class, in declaration order.
///
/// The suites walk this rather than a list they restate, so a class added to
/// the enum without a row here fails to compile.
pub const DEVICE_CLASSES: [DeviceClass; 3] = [
    DeviceClass::Microphone,
    DeviceClass::Camera,
    DeviceClass::Screen,
];

impl DeviceClass {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Microphone => "MICROPHONE",
            Self::Camera => "CAMERA",
            Self::Screen => "SCREEN",
        }
    }

    /// Which device a medium is captured with, when this crate classifies it.
    ///
    /// `CaptureMedium` is `#[non_exhaustive]` and belongs to another crate, so
    /// this map cannot be exhaustive at the compiler. The wildcard is `None`
    /// rather than a device: a medium this crate has not classified opens
    /// nothing, which is the fail-closed direction, and
    /// `every_capture_medium_is_classified` reads `CaptureMedium`'s variants
    /// out of `crates/consent/src/permission.rs` and fails the day a fifth one
    /// is declared without a row here. A `_ => Self::Microphone` would have
    /// opened a microphone for a medium nobody had considered.
    ///
    /// It is deliberately not invertible: a photograph of a board and a video
    /// of the room are the same camera, so `Camera` names two media and a
    /// reverse map would have to pick one of them.
    #[must_use]
    pub const fn of(medium: CaptureMedium) -> Option<Self> {
        match medium {
            CaptureMedium::Audio => Some(Self::Microphone),
            CaptureMedium::PhotoOfBoard | CaptureMedium::Video => Some(Self::Camera),
            CaptureMedium::ScreenCapture => Some(Self::Screen),
            _ => None,
        }
    }
}

/// The device classes one token opens, and nothing else.
///
/// Ordered and deduplicated so two tokens with the same media set in a
/// different order produce the same ruleset, which is what makes the native
/// backends' installed rules a function of the grant rather than of the
/// caller's argument order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRuleset {
    classes: Vec<DeviceClass>,
    unclassified: Vec<CaptureMedium>,
}

impl DeviceRuleset {
    /// Derives the ruleset from a minted token.
    ///
    /// The only constructor. The media it reads are the token's `bound` media —
    /// the ones `bind_permission` compared against the grant — and not the
    /// request's, so a request field the binding refused cannot reach a rule.
    #[must_use]
    pub fn for_token(token: &CaptureCapabilityToken) -> Self {
        let mut classes = Vec::new();
        let mut unclassified = Vec::new();
        for medium in token.media() {
            match DeviceClass::of(*medium) {
                Some(class) => classes.push(class),
                None => unclassified.push(*medium),
            }
        }
        classes.sort_unstable();
        classes.dedup();
        unclassified.sort_unstable();
        unclassified.dedup();
        Self {
            classes,
            unclassified,
        }
    }

    /// Whether this ruleset opens `class`.
    #[must_use]
    pub fn permits(&self, class: DeviceClass) -> bool {
        self.classes.contains(&class)
    }

    /// Every class this ruleset opens, sorted and deduplicated.
    #[must_use]
    pub fn classes(&self) -> &[DeviceClass] {
        &self.classes
    }

    /// The media on the token that this crate does not classify.
    ///
    /// Empty today, and asserted empty for all four of section 3.7's media. It
    /// exists so that a medium added to `CaptureMedium` later is visible as a
    /// grant that opened nothing rather than dropped between the token and the
    /// device.
    #[must_use]
    pub fn unclassified(&self) -> &[CaptureMedium] {
        &self.unclassified
    }

    /// Whether this ruleset opens nothing.
    ///
    /// `GATE-38-019` empty is one way to reach this: a grant with an empty
    /// media set mints no token at all, so an empty ruleset is unreachable
    /// through [`for_token`](Self::for_token) today. It is still asked, because
    /// the native backends install what they are given and "install no rule"
    /// and "install every rule" must not be the same code path.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

/// Which platform backend installed a ruleset.
///
/// `None` is the default lane and it is not a failure: with `native-capture`
/// off this crate installs nothing and every type in it is bookkeeping, exactly
/// as `academic-worker` reports `BackendId::None` with `native-sandbox` off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BackendId {
    /// No backend is compiled in.
    None,
    /// A Landlock ruleset whose rules are the granted device trees.
    LinuxLandlock,
    /// An AppContainer whose token holds no device capability.
    WindowsAppContainer,
}

impl BackendId {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::LinuxLandlock => "LINUX_LANDLOCK",
            Self::WindowsAppContainer => "WINDOWS_APPCONTAINER",
        }
    }
}

/// What is actually enforcing the ruleset while a capture runs.
///
/// The daemon decides this once, from [`crate::native::availability`], and
/// hands it to [`crate::session::open_device`]. It is an argument rather than a
/// query inside the session so that a caller cannot reach a device by asking at
/// a moment when the answer is convenient, and so the default lane's honest
/// state -- nothing but this crate's own comparisons -- is a value a test can
/// pass rather than a condition a test cannot reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceLayer {
    /// A platform backend installed the ruleset. The refusals are the
    /// operating system's.
    Enforced(BackendId),
    /// No backend was asked for. The only comparisons in force are this
    /// crate's, and an artefact records that.
    Bookkeeping,
    /// A backend was asked for and could not install one.
    Unavailable,
}

impl DeviceLayer {
    /// The backend behind it, when one installed.
    #[must_use]
    pub const fn backend(self) -> BackendId {
        match self {
            Self::Enforced(backend) => backend,
            Self::Bookkeeping | Self::Unavailable => BackendId::None,
        }
    }

    /// Whether an operating system is enforcing the ruleset.
    #[must_use]
    pub const fn is_enforced(self) -> bool {
        matches!(self, Self::Enforced(_))
    }
}
