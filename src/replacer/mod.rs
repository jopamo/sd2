use crate::error::{Error, Result};
use regex::bytes::{Regex, RegexBuilder, NoExpand};
use std::borrow::Cow;
use memchr::memmem;

mod validate;

enum Matcher {
    Regex(Regex),
    Literal(Vec<u8>),
}

pub struct Replacer {
    matcher: Matcher,
    replacement: Vec<u8>,
    max_replacements: usize,
    // TODO: track validation mode (strict, warn, none)
}

impl Replacer {
    pub fn new(
        pattern: &str,
        replacement: &str,
        fixed_strings: bool,
        ignore_case: bool,
        smart_case: bool,
        _case_sensitive: bool,
        word_regexp: bool,
        multiline: bool,
        single_line: bool,
        dot_matches_newline: bool,
        no_unicode: bool,
        _crlf: bool,
        max_replacements: usize,
    ) -> Result<Self> {
        // 1. Validate replacement pattern for capture group references
        // Even though we don't expand by default, we might validation?
        // Actually, if we don't expand, validating $1 is annoying.
        // But let's keep it for now as it was there.
        validate::validate_replacement(replacement)?;

        // Determine if we can use efficient literal matcher
        // We can use Literal matcher only if:
        // - fixed_strings is requested (or pattern is literal) -> handled by caller passing fixed_strings
        // - NO regex flags that affect matching (ignore_case, smart_case, word_regexp, multiline etc)
        // Note: multiline/dot_matches_newline don't apply to literal strings unless we search line by line?
        // memmem works on bytes, ignores lines.
        // word_regexp requires checking boundaries -> complex for memmem, use regex.
        // ignore_case -> complex for memmem, use regex.
        
        let use_literal_matcher = fixed_strings 
            && !ignore_case 
            && !smart_case 
            && !word_regexp;

        let matcher = if use_literal_matcher {
            Matcher::Literal(pattern.as_bytes().to_vec())
        } else {
            // Build regex
            let pattern = if fixed_strings {
                regex::escape(pattern)
            } else {
                pattern.to_string()
            };

            let pattern = if word_regexp {
                format!(r"\b{}\b", pattern)
            } else {
                pattern
            };

            let mut builder = RegexBuilder::new(&pattern);
            builder.unicode(!no_unicode);

            // Case handling
            if ignore_case {
                builder.case_insensitive(true);
            } else if smart_case {
                let is_lowercase = pattern.chars().all(|c| !c.is_uppercase());
                builder.case_insensitive(is_lowercase);
            } else {
                builder.case_insensitive(false);
            }

            builder.multi_line(multiline && !single_line);
            builder.dot_matches_new_line(dot_matches_newline);
            
            let regex = builder.build().map_err(Error::Regex)?;
            Matcher::Regex(regex)
        };

        let replacement_bytes = replacement.as_bytes().to_vec();

        Ok(Self {
            matcher,
            replacement: replacement_bytes,
            max_replacements,
        })
    }

    /// Count the number of matches in the given text.
    pub fn count_matches(&self, text: &[u8]) -> usize {
        match &self.matcher {
            Matcher::Regex(re) => re.find_iter(text).count(),
            Matcher::Literal(needle) => memmem::find_iter(text, needle).count(),
        }
    }

    /// Replace matches in text and return the replaced text along with the number of replacements performed.
    pub fn replace_with_count<'a>(&self, text: &'a [u8]) -> (Cow<'a, [u8]>, usize) {
        let matches_count = self.count_matches(text);
        if matches_count == 0 {
            return (Cow::Borrowed(text), 0);
        }

        let actual_replacements = if self.max_replacements == 0 {
            matches_count
        } else {
            std::cmp::min(matches_count, self.max_replacements)
        };

        if actual_replacements == 0 {
            return (Cow::Borrowed(text), 0);
        }

        match &self.matcher {
            Matcher::Regex(re) => {
                // Use NoExpand to ensure replacement is treated literally
                let replaced = if self.max_replacements == 0 {
                    re.replace_all(text, NoExpand(&self.replacement))
                } else {
                    re.replacen(text, self.max_replacements, NoExpand(&self.replacement))
                };
                (replaced, actual_replacements)
            },
            Matcher::Literal(needle) => {
                // Manual replacement for literal
                // We can use memmem::find_iter and build result
                let mut new_data = Vec::with_capacity(text.len()); // heuristic
                let mut last_match_end = 0;
                let mut count = 0;

                for m in memmem::find_iter(text, needle) {
                    if count >= actual_replacements {
                        break;
                    }
                    new_data.extend_from_slice(&text[last_match_end..m]);
                    new_data.extend_from_slice(&self.replacement);
                    last_match_end = m + needle.len();
                    count += 1;
                }
                new_data.extend_from_slice(&text[last_match_end..]);
                (Cow::Owned(new_data), count)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_replacement() {
        let replacer = Replacer::new(
            "foo",
            "bar",
            false, // fixed_strings (treated as regex since false? No, depends on caller logic. Here false means regex? Wait. engine.rs sets it. 
                   // new() takes fixed_strings directly. If false, it tries regex parse. "foo" is valid regex.)
            false, // ignore_case
            false, // smart_case
            true,  // case_sensitive
            false, // word_regexp
            false, // multiline
            false, // single_line
            false, // dot_matches_newline
            false, // no_unicode
            false, // crlf
            0,     // max_replacements
        ).unwrap();
        let input = b"foo baz foo";
        let output = replacer.replace_with_count(input).0;
        assert_eq!(&output[..], b"bar baz bar");
    }

    #[test]
    fn test_literal_replacement_optimized() {
        // fixed_strings = true
        let replacer = Replacer::new(
            "foo",
            "bar",
            true, // fixed_strings -> Should use Matcher::Literal
            false, false, true, false, false, false, false, false, false, 0
        ).unwrap();
        let input = b"foo baz foo";
        let output = replacer.replace_with_count(input).0;
        assert_eq!(&output[..], b"bar baz bar");
    }

    #[test]
    fn test_capture_group_no_expand() {
        // v1 behavior: replacement is literal, no expansion
        let replacer = Replacer::new(
            r"(\d+)",
            "number-$1",
            false, false, false, true, false, false, false, false, false, false, 0
        ).unwrap();
        let input = b"abc 123 def";
        let output = replacer.replace_with_count(input).0;
        // Should NOT expand $1
        assert_eq!(&output[..], b"abc number-$1 def");
    }

    #[test]
    fn test_max_replacements() {
        let replacer = Replacer::new(
            "x",
            "y",
            false, false, false, true, false, false, false, false, false, false, 2
        ).unwrap();
        let input = b"x x x x";
        let output = replacer.replace_with_count(input).0;
        assert_eq!(&output[..], b"y y x x");
    }
}
