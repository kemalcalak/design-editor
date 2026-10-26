//! Import warnings, counted per kind so a 40-slide deck with a chart on
//! every slide reports `chart (fallback picture) ×40`, not forty lines.

use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Warnings {
    counts: BTreeMap<String, usize>,
}

impl Warnings {
    pub fn add(&mut self, what: impl Into<String>) {
        *self.counts.entry(what.into()).or_insert(0) += 1;
    }

    pub fn into_vec(self) -> Vec<String> {
        self.counts
            .into_iter()
            .map(|(what, n)| if n > 1 { format!("{what} ×{n}") } else { what })
            .collect()
    }
}
