//! Ordering of the product's dotted release versions (`0.8.0`), shared by
//! the desktop update probe and the "What's new" release notes (#251).

/// Whether `candidate` is a strictly newer version than `current`.
/// Both are dotted numeric strings (`0.8.0`); a pre-release suffix
/// (`-beta.1`) is dropped before the numeric compare so a stable
/// release of the same core version doesn't read as an update.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split('-')
            .next()
            .unwrap_or(v)
            .split('.')
            .map(|n| n.trim().parse::<u64>().unwrap_or(0))
            .collect()
    }
    let l = parts(candidate);
    let c = parts(current);
    let len = l.len().max(c.len());
    for i in 0..len {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv != cv {
            return lv > cv;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_numerically_part_by_part() {
        assert!(is_newer("1.0.10", "1.0.9"));
        assert!(is_newer("2.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0"));
        assert!(!is_newer("1.0.0-beta.1", "1.0.0"));
    }
}
