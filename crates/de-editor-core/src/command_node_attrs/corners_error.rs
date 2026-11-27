//! Why `SetNodeCorners` (#389) refuses a write: a typed error the MCP
//! tool returns to the agent before the command is queued.

use std::fmt;

/// A corner write the node cannot take. Nothing is written.
#[derive(Debug, Clone, PartialEq)]
pub enum CornersError {
    /// Neither a radius nor a smoothing was given.
    NothingToWrite,
    /// A radius that is negative or not finite.
    InvalidRadius,
    /// A smoothing outside `0..=1` or not finite.
    InvalidSmoothing,
    /// No node with this id on the active page.
    NodeNotFound { node_id: String },
    /// A node kind without corners (text, line, note, …).
    NoCorners { kind: &'static str },
    /// Four different radii on a kind that stores one.
    PerCornerUnsupported { kind: &'static str },
    /// Smoothing on a kind without a smoothing field.
    SmoothingUnsupported { kind: &'static str },
}

impl fmt::Display for CornersError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingToWrite => f.write_str("a radius or a smoothing is required"),
            Self::InvalidRadius => {
                f.write_str("radius values must be non-negative finite doc-px numbers")
            }
            Self::InvalidSmoothing => f.write_str("smoothing must be from 0 to 1"),
            Self::NodeNotFound { node_id } => {
                write!(f, "node {node_id:?} was not found on the active page")
            }
            Self::NoCorners { kind } => write!(
                f,
                "a {kind} node has no corners; corner radius fits frames, groups, rectangles, \
                 images, browsers, polygons / stars, paths and ellipses"
            ),
            Self::PerCornerUnsupported { kind } => write!(
                f,
                "a {kind} node takes one radius for every corner, not four different ones; \
                 per-corner radii fit frames, groups, rectangles, images and browsers"
            ),
            Self::SmoothingUnsupported { kind } => write!(
                f,
                "a {kind} node has no corner smoothing; smoothing fits frames, groups, \
                 rectangles, polygons / stars and paths"
            ),
        }
    }
}

impl std::error::Error for CornersError {}
