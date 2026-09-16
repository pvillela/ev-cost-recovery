//! What a number written for a person looks like, for the readers that have to take one back.
//!
//! Two documents state figures with thousands separated by commas — Evolute's Charges Report and
//! Toronto Hydro's bill — and neither reader can simply strip every comma, for the reason
//! [`commas_group_thousands`] gives. The rule is subtle enough that two copies of it would drift,
//! and it belongs to neither document, so it is here: a leaf that reads no file and knows no
//! format.

/// Whether every comma in `text` separates a group of three digits.
///
/// `1,234.5` and `12,345,678` pass; `1,2`, `,123` and `1,2345` do not, and neither does a comma
/// after the decimal point. A number with no comma in it passes untouched.
///
/// The readers ask before stripping, because stripping every comma reads `1,2` as 12 — a malformed
/// cell turned into a plausible figure, in readers whose posture is to refuse rather than to guess.
pub(crate) fn commas_group_thousands(text: &str) -> bool {
    if !text.contains(',') {
        return true;
    }
    let (integer, fraction) = text.split_once('.').unwrap_or((text, ""));
    if fraction.contains(',') {
        return false;
    }
    let digits = integer
        .strip_prefix('-')
        .or_else(|| integer.strip_prefix('+'))
        .unwrap_or(integer);
    let mut groups = digits.split(',');
    let leading = groups.next().unwrap_or("");
    (1..=3).contains(&leading.len()) && groups.all(|g| g.len() == 3)
}

// cargo test --lib -- number::test
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_comma_is_a_thousands_separator_or_the_number_is_malformed() {
        for good in [
            "1234",
            "1,234",
            "1,234.5",
            "12,345,678",
            "-1,234",
            "+1,234",
            "123",
            "",
        ] {
            assert!(commas_group_thousands(good), "{good:?}");
        }
        for bad in ["1,2", ",123", "1,2345", "1.2,3", "12,34,567", "1234,56"] {
            assert!(!commas_group_thousands(bad), "{bad:?}");
        }
    }
}
