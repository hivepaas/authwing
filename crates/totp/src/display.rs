//! How a code is shown.

/// `code` in two halves, easier to read and type: `482 913`, `4829 1307`,
/// the first half the shorter when the count is odd. Codes of five
/// characters or fewer, and anything not all digits (Steam's), stay as they
/// are.
pub fn grouped(code: &str) -> String {
    if code.len() < 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return code.to_string();
    }
    let (first, second) = code.split_at(code.len() / 2);
    format!("{first} {second}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_digits_in_two() {
        assert_eq!(grouped("482913"), "482 913");
        assert_eq!(grouped("48291307"), "4829 1307");
        assert_eq!(grouped("4829130"), "482 9130");
        assert_eq!(grouped("4829130755"), "48291 30755");
        assert_eq!(grouped("48291"), "48291");
        assert_eq!(grouped("VHHQY"), "VHHQY");
        assert_eq!(grouped(""), "");
    }
}
