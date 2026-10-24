//! The element tree both flex targets print.
//!
//! The walker (`markup.rs`) turns the design into this target-neutral tree:
//! tags, attributes and CSS declarations. The HTML + CSS printer gives every
//! styled element a class and writes the declarations into a `<style>`
//! block; the Tailwind printer turns the same declarations into utility
//! classes. Keeping one tree means the two targets cannot drift on layout.

/// One CSS declaration, `property: value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decl {
    pub prop: &'static str,
    pub value: String,
}

/// Declarations under construction, in emission order. A later write of the
/// same property replaces the earlier one in place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Decls(pub Vec<Decl>);

impl Decls {
    pub fn set(&mut self, prop: &'static str, value: impl Into<String>) {
        let value = value.into();
        match self.0.iter_mut().find(|d| d.prop == prop) {
            Some(existing) => existing.value = value,
            None => self.0.push(Decl { prop, value }),
        }
    }

    pub fn get(&self, prop: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|d| d.prop == prop)
            .map(|d| d.value.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Decl> {
        self.0.iter()
    }
}

/// A run of inline text inside a text element: a styled segment, optionally
/// a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Inline {
    pub text: String,
    pub decls: Decls,
    pub href: Option<String>,
}

/// What sits between an element's tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Content {
    Empty,
    /// Plain or styled text runs, escaped at print time.
    Text(Vec<Inline>),
    Children(Vec<Element>),
    /// Markup built elsewhere (form widgets, inline SVG) — already escaped.
    Raw(String),
}

/// One output element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Element {
    pub tag: &'static str,
    /// Attribute name / unescaped value pairs, printed in order.
    pub attrs: Vec<(String, String)>,
    pub decls: Decls,
    /// Readable stem for the class name (layer name, else the node kind).
    pub name_hint: String,
    pub content: Content,
}

impl Element {
    pub fn new(tag: &'static str, name_hint: String) -> Self {
        Self {
            tag,
            attrs: Vec::new(),
            decls: Decls::default(),
            name_hint,
            content: Content::Empty,
        }
    }

    /// Markup printed as-is with no wrapper of its own (a tab bar).
    pub fn raw(markup: String) -> Self {
        let mut el = Self::new("", String::new());
        el.content = Content::Raw(markup);
        el
    }

    /// True for [`Element::raw`] markup.
    pub fn is_raw(&self) -> bool {
        self.tag.is_empty()
    }

    /// Void elements print without a closing tag.
    pub fn is_void(&self) -> bool {
        matches!(self.tag, "img" | "input" | "br")
    }
}
