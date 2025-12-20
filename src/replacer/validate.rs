use crate::error::{Error, Result};
use crate::model::ValidationMode;
use std::borrow::Cow;
use std::str::CharIndices;

/// Validate replacement string for valid capture group references.
/// Checks for $0, $1, $2, ..., ${1}, ${name}.
/// Detects ambiguous forms like $1bad (should be ${1}bad).
pub fn validate_replacement(replacement: &str, mode: ValidationMode) -> Result<Cow<'_, str>> {
    if mode == ValidationMode::None {
        return Ok(Cow::Borrowed(replacement));
    }

    let mut new_replacement = String::with_capacity(replacement.len());
    let mut last_end = 0;
    let mut modified = false;

    for capture in CaptureIter::new(replacement) {
        let name = capture.name;
        // Handle braced references: ${...}
        if name.starts_with('{') && name.ends_with('}') {
            // Braced is unambiguous
            continue;
        }

        // Unbraced reference: $name
        // Check if name starts with digit and has trailing non-digit characters
        let mut chars = name.char_indices();
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
                    match mode {
                        ValidationMode::Strict => {
                             return Err(Error::AmbiguousReplacement(format!(
                                "Ambiguous capture group reference `${}` followed by non-digit characters. Use `${{{}}}` to disambiguate.",
                                &name[..digit_count],
                                &name[..digit_count]
                            )));
                        }
                        ValidationMode::Warn => {
                            // Rewrite: $1bad -> ${1}bad
                            if !modified {
                                new_replacement.push_str(&replacement[..capture.start]);
                                modified = true;
                            } else {
                                new_replacement.push_str(&replacement[last_end..capture.start]);
                            }

                            eprintln!("WARN: Ambiguous capture group reference `${}` rewritten to `${{{}}}`.", 
                                      &name[..digit_count], &name[..digit_count]);

                            new_replacement.push_str("${");
                            new_replacement.push_str(&name[..digit_count]);
                            new_replacement.push_str("}");
                            new_replacement.push_str(&name[digit_count..]);
                            
                            last_end = capture.end;
                        }
                        ValidationMode::None => unreachable!(),
                    }
                }
            }
        }
    }

    if modified {
        new_replacement.push_str(&replacement[last_end..]);
        Ok(Cow::Owned(new_replacement))
    } else {
        Ok(Cow::Borrowed(replacement))
    }
}

/// A capture group reference found in the replacement string.
#[derive(Debug)]
struct Capture<'a> {
    name: &'a str,
    start: usize, // Index of '$'
    end: usize,   // Index after name
}

/// Iterator over capture group references in a replacement string.
/// Adapted from regex-automata and sd's implementation.
struct CaptureIter<'a> {
    chars: CharIndices<'a>,
}

impl<'a> CaptureIter<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            chars: s.char_indices(),
        }
    }
}

impl<'a> Iterator for CaptureIter<'a> {
    type Item = Capture<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (start, c) = self.chars.next()?;
            if c != '$' {
                continue;
            }

            let remaining = self.chars.as_str();
            let bytes = remaining.as_bytes();

            if bytes.is_empty() {
                continue;
            }

            match bytes[0] {
                b'$' => {
                    // Escaped dollar sign, skip it
                    self.chars.next().unwrap();
                    continue;
                }
                b'{' => {
                    // Braced reference: ${...}
                    if let Some(cap_name) = parse_braced_reference(bytes) {
                        // Advance iterator past the capture
                        let name_len = cap_name.len();
                        let mut consumed = 0;
                        while consumed < name_len {
                            let (_, c) = self.chars.next().unwrap();
                            consumed += c.len_utf8();
                        }
                        return Some(Capture {
                            name: cap_name,
                            start,
                            end: start + 1 + name_len, // $ + name
                        });
                    } else {
                        // Invalid braced reference, treat as literal?
                        continue;
                    }
                }
                _ => {
                    // Unbraced reference: $name or $number
                    if let Some(cap_name) = parse_unbraced_reference(bytes) {
                        let name_len = cap_name.len();
                        let mut consumed = 0;
                        while consumed < name_len {
                            let (_, c) = self.chars.next().unwrap();
                            consumed += c.len_utf8();
                        }
                         return Some(Capture {
                            name: cap_name,
                            start,
                            end: start + 1 + name_len, // $ + name
                        });
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
/// Returns the full content including braces, e.g. "{foo}".
/// Actually logic below returns "{foo}".
fn parse_braced_reference(bytes: &[u8]) -> Option<&str> {
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
    std::str::from_utf8(name_bytes).ok()
}

/// Parse an unbraced reference: $name where name consists of valid characters.
/// Returns name, e.g. "foo".
fn parse_unbraced_reference(bytes: &[u8]) -> Option<&str> {
    let mut end = 0;
    while end < bytes.len() && is_valid_capture_char(bytes[end]) {
        end += 1;
    }
    if end == 0 {
        return None;
    }
    let name_bytes = &bytes[..end];
    std::str::from_utf8(name_bytes).ok()
}

fn is_valid_capture_char(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ambiguous_capture_strict() {
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
            let result = validate_replacement(input, ValidationMode::Strict);
            if should_validate {
                assert!(result.is_ok(), "Expected OK for {:?}, got {:?}", input, result);
                assert_eq!(result.unwrap(), input);
            } else {
                assert!(result.is_err(), "Expected error for {:?}, got {:?}", input, result);
            }
        }
    }

    #[test]
    fn test_ambiguous_capture_warn() {
        // $1bad -> ${1}bad
        let result = validate_replacement("$1bad", ValidationMode::Warn).unwrap();
        assert_eq!(result, "${1}bad");

        // $1bad$2ok -> ${1}bad${2}ok
        let result = validate_replacement("$1bad$2ok", ValidationMode::Warn).unwrap();
        assert_eq!(result, "${1}bad${2}ok");
        
        // $10bad -> ${10}bad
        let result = validate_replacement("$10bad", ValidationMode::Warn).unwrap();
        assert_eq!(result, "${10}bad");
    }

    #[test]
    fn test_ambiguous_capture_none() {
        let input = "$1bad";
        let result = validate_replacement(input, ValidationMode::None).unwrap();
        assert_eq!(result, input);
    }
}
