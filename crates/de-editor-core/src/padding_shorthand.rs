//! CSS-style padding shorthand for the property panel's single `Pad`
//! field (#83): `16` sets every side, `96 120` sets vertical then
//! horizontal, `8 16 4` sets top / horizontal / bottom and `8 16 8 4`
//! sets top / right / bottom / left.

/// Parse a shorthand draft into `[top, right, bottom, left]`. Returns
/// `None` for an empty draft, more than four values, or any value that
/// is not a finite number. Negative values clamp to zero.
pub fn parse_padding_shorthand(draft: &str) -> Option<[f64; 4]> {
    let mut values = Vec::with_capacity(4);
    for part in draft.split_whitespace() {
        let value: f64 = part.parse().ok()?;
        if !value.is_finite() {
            return None;
        }
        values.push(value.max(0.0));
    }
    match values.as_slice() {
        [all] => Some([*all; 4]),
        [v, h] => Some([*v, *h, *v, *h]),
        [t, h, b] => Some([*t, *h, *b, *h]),
        [t, r, b, l] => Some([*t, *r, *b, *l]),
        _ => None,
    }
}

/// Format `[top, right, bottom, left]` as the shortest shorthand that
/// parses back to the same four values.
pub fn format_padding_shorthand(values: [f32; 4], format: impl Fn(f32) -> String) -> String {
    let [t, r, b, l] = values;
    let parts: Vec<f32> = if t == r && r == b && b == l {
        vec![t]
    } else if t == b && r == l {
        vec![t, r]
    } else if r == l {
        vec![t, r, b]
    } else {
        vec![t, r, b, l]
    };
    parts.into_iter().map(format).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(values: [f32; 4]) -> String {
        format_padding_shorthand(values, |v| v.to_string())
    }

    #[test]
    fn parses_one_to_four_values() {
        assert_eq!(parse_padding_shorthand("16"), Some([16.0; 4]));
        assert_eq!(
            parse_padding_shorthand(" 96  120 "),
            Some([96.0, 120.0, 96.0, 120.0])
        );
        assert_eq!(
            parse_padding_shorthand("8 16 4"),
            Some([8.0, 16.0, 4.0, 16.0])
        );
        assert_eq!(
            parse_padding_shorthand("8 16 8 4"),
            Some([8.0, 16.0, 8.0, 4.0])
        );
        assert_eq!(
            parse_padding_shorthand("-4 2.5"),
            Some([0.0, 2.5, 0.0, 2.5])
        );
    }

    #[test]
    fn rejects_empty_extra_and_non_numeric_drafts() {
        assert_eq!(parse_padding_shorthand(""), None);
        assert_eq!(parse_padding_shorthand("   "), None);
        assert_eq!(parse_padding_shorthand("1 2 3 4 5"), None);
        assert_eq!(parse_padding_shorthand("1 x"), None);
        assert_eq!(parse_padding_shorthand("inf"), None);
    }

    #[test]
    fn formats_the_shortest_round_trip() {
        assert_eq!(fmt([16.0; 4]), "16");
        assert_eq!(fmt([96.0, 120.0, 96.0, 120.0]), "96 120");
        assert_eq!(fmt([8.0, 16.0, 4.0, 16.0]), "8 16 4");
        assert_eq!(fmt([8.0, 16.0, 8.0, 4.0]), "8 16 8 4");
        for values in [[1.0, 2.0, 3.0, 4.0], [5.0, 5.0, 6.0, 5.0], [0.0; 4]] {
            let back = parse_padding_shorthand(&fmt(values)).unwrap();
            assert_eq!(back.map(|v| v as f32), values);
        }
    }
}
