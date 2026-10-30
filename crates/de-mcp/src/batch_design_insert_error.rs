//! Why an insert payload is refused before it is parsed: a path with no
//! outline (#267) or a local image path that cannot be read (#268).

use crate::local_image::LocalImageError;

/// A path inserted with neither `d` nor `anchors` paints nothing; the insert
/// is refused with this instead of landing an invisible node (#267).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissingPathOutline {
    pub(crate) node: String,
}

impl std::fmt::Display for MissingPathOutline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "path node {} has no outline: set `d` (SVG path data) or `anchors`",
            self.node
        )
    }
}

/// Why an insert payload is refused before it is parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InsertPayloadError {
    MissingOutline(MissingPathOutline),
    LocalImage(LocalImageError),
}

impl std::fmt::Display for InsertPayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingOutline(e) => e.fmt(f),
            Self::LocalImage(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for InsertPayloadError {}

impl From<LocalImageError> for InsertPayloadError {
    fn from(e: LocalImageError) -> Self {
        Self::LocalImage(e)
    }
}
