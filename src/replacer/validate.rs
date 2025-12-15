use crate::error::{Error, Result};
use std::str::CharIndices;

/// Validate replacement string for valid capture group references.
/// Checks for $0, $1, $2, ..., ${1}, ${name}.
/// Detects ambiguous forms like $1bad (should be ${1}bad).
pub fn validate_replacement(replacement: &str) -> Result<()> {
    for capture in CaptureIter::new(replacement) {
        let name = capture.name;
        // Handle braced references: ${...}
        let inner = if name.starts_with('{') && name.ends_with('}') {
            &name[1..name.len() - 1]
        } else {
            name
        };
        // Check if inner starts with digit and has trailing non-digit characters
        let mut chars = inner.char_indices();
        if let Some((_, first)) = chars.next() {
            if first.is_ascii_digit() {
                // Count leading digits
                let mut digit_count = 1;
                let mut has_non_digit = false;
                for (_, c) in chars {
                    if c.is_ascii_digit() {
                        digit_count += 1;
                    } else {
                        has_non_digit = true;
                        break;
                    }
                }
                if has_non_digit {
                    return Err(Error::AmbiguousReplacement(format!(
                        "Ambiguous capture group reference `${}` followed by non-digit characters. Use `${{{}}}` to disambiguate.",
                        &inner[..digit_count],
                        &inner[..digit_count]
                    )));
                }
            }
        }
    }
    Ok(())
}

/// A capture group reference found in the replacement string.
#[derive(Debug)]
struct Capture<'a> {
    name: &'a str,
}

/// Iterator over capture group references in a replacement string.
/// Adapted from regex-automata and sd's implementation.
struct CaptureIter<'a>(CharIndices<'a>);

impl<'a> CaptureIter<'a> {
    fn new(s: &'a str) -> Self {
        Self(s.char_indices())
    }
}

impl<'a> Iterator for CaptureIter<'a> {
    type Item = Capture<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (_, c) = self.0.next()?;
            if c != '$' {
                continue;
            }

            let remaining = self.0.as_str();
            let bytes = remaining.as_bytes();

            if bytes.is_empty() {
                continue;
            }

            match bytes[0] {
                b'$' => {
                    // Escaped dollar sign, skip it
                    self.0.next().unwrap();
                    continue;
                }
                b'{' => {
                    // Braced reference: ${...}
                    if let Some(cap) = parse_braced_reference(bytes) {
                        // Advance iterator past the capture
                        let name_len = cap.name.len();
                        let mut consumed = 0;
                        while consumed < name_len {
                            let (_, c) = self.0.next().unwrap();
                            consumed += c.len_utf8();
                        }
                        return Some(cap);
                    } else {
                        // Invalid braced reference, treat as literal?
                        continue;
                    }
                }
                _ => {
                    // Unbraced reference: $name or $number
                    if let Some(cap) = parse_unbraced_reference(bytes) {
                        let name_len = cap.name.len();
                        let mut consumed = 0;
                        while consumed < name_len {
                            let (_, c) = self.0.next().unwrap();
                            consumed += c.len_utf8();
                        }
                        return Some(cap);
                    } else {
                        // Not a valid capture reference, treat as literal?
                        continue;
                    }
                }
            }
        }
    }
}

/// Parse a braced reference: ${...}
fn parse_braced_reference(bytes: &[u8]) -> Option<Capture<'_>> {
    assert_eq!(bytes[0], b'{');
    let mut end = 1;
    while end < bytes.len() && bytes[end] != b'}' {
        end += 1;
    }
    if end >= bytes.len() || bytes[end] != b'}' {
        return None;
    }
    // Include the closing brace in the name
    let name_bytes = &bytes[..=end];
    let name = std::str::from_utf8(name_bytes).ok()?;
    Some(Capture {
        name,
    })
}

/// Parse an unbraced reference: $name where name consists of valid characters.
fn parse_unbraced_reference(bytes: &[u8]) -> Option<Capture<'_>> {
    let mut end = 0;
    while end < bytes.len() && is_valid_capture_char(bytes[end]) {
        end += 1;
    }
    if end == 0 {
        return None;
    }
    let name_bytes = &bytes[..end];
    let name = std::str::from_utf8(name_bytes).ok()?;
    Some(Capture {
        name,
    })
}

fn is_valid_capture_char(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ambiguous_capture() {
        let cases = [
            ("$1", true),           // valid
            ("$123", true),         // valid
            ("$1bad", false),       // ambiguous
            ("$1bad$2", false),     // first ambiguous
            ("${1}bad", true),      // braced okay
            ("$foo", true),         // named
            ("$$", true),           // escaped dollar
            ("$1_", false),         // underscore after digits is ambiguous
        ];
        for (input, should_validate) in cases {
            let result = validate_replacement(input);
            if should_validate {
                assert!(result.is_ok(), "Expected OK for {:?}, got {:?}", input, result);
            } else {
                assert!(result.is_err(), "Expected error for {:?}, got {:?}", input, result);
            }
        }
    }
}
