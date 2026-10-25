//! Property-panel state enums shared by editor core and widget hosts.

/// What the right panel shows — toggled by the canvas `</>` button and
/// `Cmd+Shift+C`. There is no tab strip (#30): the Code view has its own
/// header with a close button back to Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyTab {
    Design,
    Code,
}

/// Variants the Fill section's type-selector pill exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillType {
    Solid,
    LinearGradient,
    RadialGradient,
    /// Angular (conic / sweep) gradient: stops run clockwise around a
    /// centre, starting at `rotation` degrees from 12 o'clock.
    AngularGradient,
    MeshGradient,
    /// Native SkSL shader fill (v1). The type picker offers it and the
    /// panel swaps its body between the built-in presets
    /// (`crate::shader_presets`); raw SkSL authoring stays MCP / file only.
    Shader,
    Image,
}

impl FillType {
    /// Linear (direction) and Angular (start rotation) carry the angle
    /// the panel's Angle row edits.
    pub fn has_angle(self) -> bool {
        matches!(self, Self::LinearGradient | Self::AngularGradient)
    }
}

/// Image-fill fit modes exposed by the image-fill popover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFillMode {
    Fill,
    Fit,
    Crop,
    Tile,
    /// Scale the image to the node box, ignoring its aspect ratio. Image
    /// FILLS only: a standalone Image node's `objectFit` has no stretch.
    Stretch,
}

impl ImageFillMode {
    /// Every mode an image fill can take, in popover order.
    pub const ALL: [Self; 5] = [Self::Fill, Self::Fit, Self::Crop, Self::Tile, Self::Stretch];
    /// The modes a standalone Image node supports (no Stretch).
    pub const IMAGE_NODE: [Self; 4] = [Self::Fill, Self::Fit, Self::Crop, Self::Tile];

    /// The popover's mode row: [`Self::IMAGE_NODE`] for an Image node,
    /// [`Self::ALL`] for an image fill.
    pub fn modes_for(image_node: bool) -> &'static [Self] {
        if image_node {
            &Self::IMAGE_NODE
        } else {
            &Self::ALL
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Fill => "image.fill",
            Self::Fit => "image.fitMode",
            Self::Crop => "image.crop",
            Self::Tile => "image.tile",
            Self::Stretch => "image.stretch",
        }
    }

    pub fn to_schema(self) -> jian_ops_schema::style::ImageFillMode {
        match self {
            Self::Fill => jian_ops_schema::style::ImageFillMode::Fill,
            Self::Fit => jian_ops_schema::style::ImageFillMode::Fit,
            Self::Crop => jian_ops_schema::style::ImageFillMode::Crop,
            Self::Tile => jian_ops_schema::style::ImageFillMode::Tile,
            Self::Stretch => jian_ops_schema::style::ImageFillMode::Stretch,
        }
    }

    pub fn from_schema(value: Option<&jian_ops_schema::style::ImageFillMode>) -> Self {
        match value {
            Some(jian_ops_schema::style::ImageFillMode::Fit) => Self::Fit,
            Some(jian_ops_schema::style::ImageFillMode::Crop) => Self::Crop,
            Some(jian_ops_schema::style::ImageFillMode::Tile) => Self::Tile,
            Some(jian_ops_schema::style::ImageFillMode::Stretch) => Self::Stretch,
            Some(jian_ops_schema::style::ImageFillMode::Fill) | None => Self::Fill,
        }
    }

    /// The Image-node `objectFit` for this mode; `None` for Stretch,
    /// which an Image node cannot express.
    pub fn to_image_node_schema(self) -> Option<jian_ops_schema::node::image::ImageFitMode> {
        match self {
            Self::Fill => Some(jian_ops_schema::node::image::ImageFitMode::Fill),
            Self::Fit => Some(jian_ops_schema::node::image::ImageFitMode::Fit),
            Self::Crop => Some(jian_ops_schema::node::image::ImageFitMode::Crop),
            Self::Tile => Some(jian_ops_schema::node::image::ImageFitMode::Tile),
            Self::Stretch => None,
        }
    }

    pub fn from_image_node_schema(
        value: Option<&jian_ops_schema::node::image::ImageFitMode>,
    ) -> Self {
        match value {
            Some(jian_ops_schema::node::image::ImageFitMode::Fit) => Self::Fit,
            Some(jian_ops_schema::node::image::ImageFitMode::Crop) => Self::Crop,
            Some(jian_ops_schema::node::image::ImageFitMode::Tile) => Self::Tile,
            Some(jian_ops_schema::node::image::ImageFitMode::Fill) | None => Self::Fill,
        }
    }
}

/// Image-fill adjustment sliders. Values are clamped to `[-100, 100]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageAdjustmentField {
    Exposure,
    Contrast,
    Saturation,
    Temperature,
    Tint,
    Highlights,
    Shadows,
}

impl ImageAdjustmentField {
    pub const ALL: [Self; 7] = [
        Self::Exposure,
        Self::Contrast,
        Self::Saturation,
        Self::Temperature,
        Self::Tint,
        Self::Highlights,
        Self::Shadows,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Exposure => "image.exposure",
            Self::Contrast => "image.contrast",
            Self::Saturation => "image.saturation",
            Self::Temperature => "image.temperature",
            Self::Tint => "image.tint",
            Self::Highlights => "image.highlights",
            Self::Shadows => "image.shadows",
        }
    }
}

/// Three flex-layout modes the property panel exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexLayout {
    Free,
    Vertical,
    Horizontal,
}

/// How the padding section shows its inputs — TS `PaddingMode`.
/// `Single` = one value for all four sides; `Axis` = vertical +
/// horizontal; `Individual` = top / right / bottom / left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaddingEditMode {
    Single,
    Axis,
    Individual,
}

impl PaddingEditMode {
    pub const ALL: [Self; 3] = [Self::Single, Self::Axis, Self::Individual];

    /// i18n key for the gear-popover row label.
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Single => "padding.oneValue",
            Self::Axis => "padding.horizontalVertical",
            Self::Individual => "padding.topRightBottomLeft",
        }
    }

    /// Derive the mode from the four effective padding values — mirrors
    /// the TS `parsePaddingValues` (uniform first, then axis, else
    /// individual).
    pub fn from_values(t: f32, r: f32, b: f32, l: f32) -> Self {
        if t == r && r == b && b == l {
            Self::Single
        } else if t == b && r == l {
            Self::Axis
        } else {
            Self::Individual
        }
    }
}

/// Path boolean ops — TS parity with Paper.js (Ctrl+Alt+U/S/I/X).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Subtract,
    Intersect,
    Exclude,
}

/// Export dialog format. State enum ported from shell-core's
/// `widgets/export_dialog::ExportFormat` (the widget render code stays
/// in shell-core). `Html` / `HtmlTailwind` are the flex-layout code
/// exports (#170); the rest render the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Jpeg,
    Webp,
    Svg,
    Pdf,
    Html,
    HtmlTailwind,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 7] = [
        ExportFormat::Png,
        ExportFormat::Jpeg,
        ExportFormat::Webp,
        ExportFormat::Svg,
        ExportFormat::Pdf,
        ExportFormat::Html,
        ExportFormat::HtmlTailwind,
    ];

    /// True for the HTML code exports (no raster scale applies).
    pub fn is_html(self) -> bool {
        matches!(self, ExportFormat::Html | ExportFormat::HtmlTailwind)
    }

    /// True for the raster exports, the only ones the 1x / 2x / 3x
    /// scale affects (SVG / PDF are vector, HTML / Tailwind are code).
    pub fn uses_scale(self) -> bool {
        // Exhaustive on purpose: a new format must decide.
        match self {
            ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::Webp => true,
            ExportFormat::Svg
            | ExportFormat::Pdf
            | ExportFormat::Html
            | ExportFormat::HtmlTailwind => false,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Png => "PNG",
            ExportFormat::Jpeg => "JPEG",
            ExportFormat::Webp => "WEBP",
            ExportFormat::Svg => "SVG",
            ExportFormat::Pdf => "PDF",
            ExportFormat::Html => "HTML",
            ExportFormat::HtmlTailwind => "Tailwind",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Png => "png",
            ExportFormat::Jpeg => "jpg",
            ExportFormat::Webp => "webp",
            ExportFormat::Svg => "svg",
            ExportFormat::Pdf => "pdf",
            ExportFormat::Html | ExportFormat::HtmlTailwind => "html",
        }
    }
}
