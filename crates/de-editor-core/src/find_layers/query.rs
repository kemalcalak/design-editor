//! The find-layers query: free text plus `key:value` filters.

/// A parsed, lowercased query. Words after a `key:value` filter extend
/// that filter (`text:sign in` is one filter), so free text goes first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindQuery {
    /// Free text: name, then text content, then id.
    pub terms: String,
    pub name: Option<String>,
    pub text: Option<String>,
    pub kind: Option<String>,
    pub id: Option<String>,
    pub component: Option<String>,
    /// Raw colour as typed (`#2563eb`); normalized when matching.
    pub fill: Option<String>,
    /// Variable name with or without its `$`.
    pub variable: Option<String>,
}

#[derive(Clone, Copy)]
enum Key {
    Name,
    Text,
    Kind,
    Id,
    Component,
    Fill,
    Variable,
}

fn key_for(word: &str) -> Option<Key> {
    Some(match word {
        "name" => Key::Name,
        "text" | "content" => Key::Text,
        "type" | "kind" => Key::Kind,
        "id" => Key::Id,
        "component" | "instance" | "instances" => Key::Component,
        "fill" | "color" | "colour" => Key::Fill,
        "var" | "variable" => Key::Variable,
        _ => return None,
    })
}

fn push_word(slot: &mut Option<String>, word: &str) {
    match slot {
        Some(value) if !value.is_empty() => {
            value.push(' ');
            value.push_str(word);
        }
        _ => *slot = Some(word.to_string()),
    }
}

impl FindQuery {
    /// Parse what the user typed.
    pub fn parse(raw: &str) -> Self {
        let mut query = FindQuery::default();
        let mut current: Option<Key> = None;
        for word in raw.split_whitespace() {
            let lower = word.to_lowercase();
            if let Some((key, value)) = lower
                .split_once(':')
                .and_then(|(k, v)| Some((key_for(k)?, v)))
            {
                current = Some(key);
                let slot = query.slot(key);
                *slot = Some(String::new());
                if !value.is_empty() {
                    push_word(slot, value);
                }
                continue;
            }
            match current {
                Some(key) => push_word(query.slot(key), &lower),
                None => {
                    if !query.terms.is_empty() {
                        query.terms.push(' ');
                    }
                    query.terms.push_str(&lower);
                }
            }
        }
        // `type:` with nothing after it filters nothing.
        for key in [
            Key::Name,
            Key::Text,
            Key::Kind,
            Key::Id,
            Key::Component,
            Key::Fill,
            Key::Variable,
        ] {
            let slot = query.slot(key);
            if slot.as_deref().is_some_and(str::is_empty) {
                *slot = None;
            }
        }
        if let Some(variable) = query.variable.as_mut() {
            if let Some(stripped) = variable.strip_prefix('$') {
                *variable = stripped.to_string();
            }
        }
        query
    }

    fn slot(&mut self, key: Key) -> &mut Option<String> {
        match key {
            Key::Name => &mut self.name,
            Key::Text => &mut self.text,
            Key::Kind => &mut self.kind,
            Key::Id => &mut self.id,
            Key::Component => &mut self.component,
            Key::Fill => &mut self.fill,
            Key::Variable => &mut self.variable,
        }
    }

    /// Nothing to search for.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
            && self.name.is_none()
            && self.text.is_none()
            && self.kind.is_none()
            && self.id.is_none()
            && self.component.is_none()
            && self.fill.is_none()
            && self.variable.is_none()
    }

    /// The text a replace rewrites: the `text:` filter, else the free text.
    pub fn replace_needle(&self) -> Option<&str> {
        self.text
            .as_deref()
            .or((!self.terms.is_empty()).then_some(self.terms.as_str()))
    }
}
