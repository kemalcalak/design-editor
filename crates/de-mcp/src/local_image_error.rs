//! `LocalImageError`: why a local image path in a write was refused (#268).

/// Why a local image url could not become a `data:` url.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalImageError {
    /// No reader on this host (a browser document must not pull in the
    /// daemon host's files).
    Unavailable {
        url: String,
    },
    Unreadable {
        url: String,
        detail: String,
    },
    UnsupportedType {
        url: String,
    },
    TooLarge {
        url: String,
        bytes: u64,
        max: u64,
    },
}

impl std::fmt::Display for LocalImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (url, reason) = match self {
            Self::Unavailable { url } => (url, "local files are not read on this host".to_string()),
            Self::Unreadable { url, detail } => (url, detail.clone()),
            Self::UnsupportedType { url } => {
                (url, "not a png, jpeg, gif, webp or svg file".to_string())
            }
            Self::TooLarge { url, bytes, max } => {
                (url, format!("{bytes} bytes is over the {max}-byte limit"))
            }
        };
        write!(
            f,
            "image url {url} cannot be loaded: {reason}; use a data: or http(s) url"
        )
    }
}

impl std::error::Error for LocalImageError {}
