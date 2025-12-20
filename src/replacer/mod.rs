use crate::error::{Error, Result};
use crate::model::{LineRange, ReplacementRange, ValidationMode};
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
    range: Option<LineRange>,
    allowed_ranges: Option<Vec<ReplacementRange>>,
    expand: bool,
}

impl Replacer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pattern: &str,
        replacement: &str,
        fixed_strings: bool,
        ignore_case: bool,
        smart_case: bool,
        word_regexp: bool,
        multiline: bool,
        single_line: bool,
        dot_matches_newline: bool,
        no_unicode: bool,
        max_replacements: usize,
        range: Option<LineRange>,
        allowed_ranges: Option<Vec<ReplacementRange>>,
        expand: bool,
        validation_mode: ValidationMode,
    ) -> Result<Self> {
        // 1. Validate replacement pattern for capture group references
        if !expand {
             // If we don't expand, we don't strictly need to validate $1, but it might be nice to warn?
             // Actually, the original code called validate::validate_replacement which checks for $N validity.
             // If expand is false, $1 is literal "$1", so valid.
             // If expand is true, $1 must be valid.
             // We should probably only validate if expand is true.
        } else {
            // If validation fails in Warn mode, it might return an error?
            // Actually, for Warn mode, validate_replacement should probably return the *fixed* replacement string.
            // But Replacer takes `replacement: &str`.
            // We should change the `replacement` variable if sanitize/validation returns a new string.
            // But validate::validate_replacement currently returns Result<()>.
            // I should change it to return Result<Cow<str>> or String.
        }
        
        let validated_replacement = if expand {
            validate::validate_replacement(replacement, validation_mode)?
        } else {
            Cow::Borrowed(replacement)
        };
        
        // Use validated_replacement for check below
        
        // Determine if we can use efficient literal matcher
        // We can use Literal matcher only if:
        // - fixed_strings is requested (or pattern is literal) -> handled by caller passing fixed_strings
        // - NO regex flags that affect matching (ignore_case, smart_case, word_regexp, multiline etc)
        // - NO expansion (if expand is true, we need regex engine to resolve captures, UNLESS replacement has no $ signs)
        // Note: multiline/dot_matches_newline don't apply to literal strings unless we search line by line?
        // memmem works on bytes, ignores lines.
        // word_regexp requires checking boundaries -> complex for memmem, use regex.
        // ignore_case -> complex for memmem, use regex.
        
        let use_literal_matcher = fixed_strings 
            && !ignore_case 
            && !smart_case 
            && !word_regexp
            && (!expand || !validated_replacement.contains("$")); // If expansion requested but no $ involved, literal is fine

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

        let replacement_bytes = validated_replacement.as_bytes().to_vec();

        let mut allowed_ranges = allowed_ranges;
        if let Some(ref mut ranges) = allowed_ranges {
            ranges.sort();
        }

        Ok(Self {
            matcher,
            replacement: replacement_bytes,
            max_replacements,
            range,
            allowed_ranges,
            expand,
        })
    }

    /// Count the number of matches in the given text.
    pub fn count_matches(&self, text: &[u8]) -> usize {
        if self.range.is_some() || self.allowed_ranges.is_some() {
             // If range filters are set, we must iterate to check bounds
             let mut count = 0;
             let line_offsets = if self.range.is_some() {
                 Some(build_line_offsets(text))
             } else {
                 None
             };
             
             let mut allowed_cursor = 0;

             match &self.matcher {
                Matcher::Regex(re) => {
                    for m in re.find_iter(text) {
                        if let Some(range) = &self.range {
                            if !is_in_range(m.start(), range, line_offsets.as_ref().unwrap()) {
                                continue;
                            }
                        }
                        if let Some(allowed) = &self.allowed_ranges {
                            if !check_allowed_range_optimized(m.start(), m.end(), allowed, &mut allowed_cursor) {
                                continue;
                            }
                        }
                        count += 1;
                    }
                },
                Matcher::Literal(needle) => {
                     for m in memmem::find_iter(text, needle) {
                        if let Some(range) = &self.range {
                            if !is_in_range(m, range, line_offsets.as_ref().unwrap()) {
                                continue;
                            }
                        }
                        let end = m + needle.len();
                        if let Some(allowed) = &self.allowed_ranges {
                            if !check_allowed_range_optimized(m, end, allowed, &mut allowed_cursor) {
                                continue;
                            }
                        }
                        count += 1;
                     }
                }
             }
             return count;
        }

        match &self.matcher {
            Matcher::Regex(re) => re.find_iter(text).count(),
            Matcher::Literal(needle) => memmem::find_iter(text, needle).count(),
        }
    }

    /// Replace matches in text and return the replaced text along with the number of replacements performed.
    pub fn replace_with_count<'a>(&self, text: &'a [u8]) -> (Cow<'a, [u8]>, usize) {
        // If no range filter and regex replacement, use regex methods for speed
        if self.range.is_none() && self.allowed_ranges.is_none() {
            if let Matcher::Regex(re) = &self.matcher {
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
                 
                 let replaced = if self.max_replacements == 0 {
                    if self.expand {
                        re.replace_all(text, &self.replacement[..])
                    } else {
                        re.replace_all(text, NoExpand(&self.replacement))
                    }
                 } else if self.expand {
                      re.replacen(text, self.max_replacements, &self.replacement[..])
                 } else {
                      re.replacen(text, self.max_replacements, NoExpand(&self.replacement))
                 };
                 return (replaced, actual_replacements);
            }
        }

        // Manual replacement loop required for:
        // 1. Literal matcher (no replace_all)
        // 2. Range filtering (must check each match)
        
        let mut new_data = Vec::with_capacity(text.len());
        let mut last_match_end = 0;
        let mut count = 0;
        
        let line_offsets = if self.range.is_some() {
            Some(build_line_offsets(text))
        } else {
            None
        };
        
        let mut allowed_cursor = 0;

        match &self.matcher {
            Matcher::Regex(re) => {
                 for m in re.captures_iter(text) {
                    if self.max_replacements > 0 && count >= self.max_replacements {
                        break;
                    }
                    
                    let match_start = m.get(0).unwrap().start();
                    let match_end = m.get(0).unwrap().end();
                    
                    if let Some(range) = &self.range {
                        if !is_in_range(match_start, range, line_offsets.as_ref().unwrap()) {
                            continue;
                        }
                    }

                    if let Some(allowed) = &self.allowed_ranges {
                        if !check_allowed_range_optimized(match_start, match_end, allowed, &mut allowed_cursor) {
                            continue;
                        }
                    }

                    new_data.extend_from_slice(&text[last_match_end..match_start]);
                    
                    if self.expand {
                        m.expand(&self.replacement, &mut new_data);
                    } else {
                        new_data.extend_from_slice(&self.replacement);
                    }
                    
                    last_match_end = match_end;
                    count += 1;
                 }
            },
            Matcher::Literal(needle) => {
                for m in memmem::find_iter(text, needle) {
                    if self.max_replacements > 0 && count >= self.max_replacements {
                        break;
                    }
                    
                    if let Some(range) = &self.range {
                         if !is_in_range(m, range, line_offsets.as_ref().unwrap()) {
                            continue;
                        }
                    }

                    let end = m + needle.len();
                    if let Some(allowed) = &self.allowed_ranges {
                        if !check_allowed_range_optimized(m, end, allowed, &mut allowed_cursor) {
                            continue;
                        }
                    }

                    new_data.extend_from_slice(&text[last_match_end..m]);
                    new_data.extend_from_slice(&self.replacement);
                    last_match_end = end;
                    count += 1;
                }
            }
        }

        if count == 0 {
            return (Cow::Borrowed(text), 0);
        }

        new_data.extend_from_slice(&text[last_match_end..]);
        (Cow::Owned(new_data), count)
    }
}

/// Precompute line start offsets.
/// Returns a vector where index i is the byte offset of the start of line i+1.
fn build_line_offsets(text: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::new();
    offsets.push(0);
    for (i, &b) in text.iter().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Check if a byte offset is within the allowed line range.
fn is_in_range(byte_offset: usize, range: &LineRange, line_offsets: &[usize]) -> bool {
    // Find line number for byte_offset using binary search
    // line_offsets[i] <= byte_offset < line_offsets[i+1]
    
    let line_idx = match line_offsets.binary_search(&byte_offset) {
        Ok(i) => i, // Exact match means start of line i+1 (0-based idx i)
        Err(i) => i - 1, // Insertion point is i, so it belongs to line i-1 (0-based)
    };
    
    let line_number = line_idx + 1; // 1-based line number

    if line_number < range.start {
        return false;
    }
    if let Some(end) = range.end {
        if line_number > end {
            return false;
        }
    }
    true
}

/// Optimized check for allowed ranges using a cursor.
/// Assumes matches are processed in order and allowed ranges are sorted by start.
fn check_allowed_range_optimized(start: usize, end: usize, allowed: &[ReplacementRange], cursor: &mut usize) -> bool {
    // Fast forward cursor: skip ranges that end before the match starts.
    while *cursor < allowed.len() && allowed[*cursor].end <= start {
        *cursor += 1;
    }

    if *cursor >= allowed.len() {
        return false;
    }

    let r = &allowed[*cursor];
    // Check intersection: start < r.end && r.start < end
    // We know r.end > start (from loop).
    // So we just need r.start < end.
    if r.start < end {
        return true;
    }

    // No overlap.
    // Since allowed ranges are sorted by start, any subsequent range r' will have r'.start >= r.start >= end.
    // So no future overlap is possible for this match.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_replacement() {
        let replacer = Replacer::new(
            "foo",
            "bar",
            false, // fixed_strings
            false, // ignore_case
            false, // smart_case
            false, // word_regexp
            false, // multiline
            false, // single_line
            false, // dot_matches_newline
            false, // no_unicode
            0,     // max_replacements
            None,
            None,
            false,
            ValidationMode::default(),
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
            false, // ignore_case
            false, // smart_case
            false, // word_regexp
            false, // multiline
            false, // single_line
            false, // dot_matches_newline
            false, // no_unicode
            0,     // max_replacements
            None,
            None,
            false,
            ValidationMode::default(),
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
            false, false, false, false, false, false, false, false, 0, None, None,
            false, // expand=false
            ValidationMode::default(),
        ).unwrap();
        let input = b"abc 123 def";
        let output = replacer.replace_with_count(input).0;
        // Should NOT expand $1
        assert_eq!(&output[..], b"abc number-$1 def");
    }

    #[test]
    fn test_capture_group_with_expand() {
        let replacer = Replacer::new(
            r"(\d+)",
            "number-$1",
            false, false, false, false, false, false, false, false, 0, None, None,
            true, // expand=true
            ValidationMode::default(),
        ).unwrap();
        let input = b"abc 123 def";
        let output = replacer.replace_with_count(input).0;
        // Should expand $1
        assert_eq!(&output[..], b"abc number-123 def");
    }

    #[test]
    fn test_max_replacements() {
        let replacer = Replacer::new(
            "x",
            "y",
            false, false, false, false, false, false, false, false, 2, None, None,
            false,
            ValidationMode::default(),
        ).unwrap();
        let input = b"x x x x";
        let output = replacer.replace_with_count(input).0;
        assert_eq!(&output[..], b"y y x x");
    }

    #[test]
    fn test_allowed_ranges_optimization() {
        use crate::model::ReplacementRange;
        // Allowed ranges: [0..1], [4..5] (matches 0th and 2nd 'x')
        // Input: "x x x"
        // Indices: 0, 2, 4
        let allowed = vec![
            ReplacementRange { start: 4, end: 5 },
            ReplacementRange { start: 0, end: 1 },
        ]; // Unsorted to test sorting
        
        let replacer = Replacer::new(
            "x",
            "y",
            false, false, false, false, false, false, false, false, 0, None, 
            Some(allowed),
            false,
            ValidationMode::default(),
        ).unwrap();
        
        let input = b"x x x";
        let (output, count) = replacer.replace_with_count(input);
        assert_eq!(count, 2);
        assert_eq!(&output[..], b"y x y");
    }
}
