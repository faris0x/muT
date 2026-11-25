// Copyright (c) 2026 Faris Alfarhan
// SPDX-License-Identifier: MIT

//! Text search and replacement helpers.

/// Count non-overlapping occurrences of `query` in `text`.
pub fn count_matches(text: &str, query: &str) -> usize {
    if query.is_empty() {
        return 0;
    }
    text.match_indices(query).count()
}

/// Replace every non-overlapping occurrence of `query` in `text`.
///
/// Returns the new text and the number of replacements. The replacement is
/// built in a single pass over the original text, so a replacement that
/// contains the query cannot loop.
pub fn replace_all(text: &str, query: &str, replacement: &str) -> (String, usize) {
    if query.is_empty() {
        return (text.to_string(), 0);
    }

    let mut output = String::with_capacity(text.len());
    let mut last = 0;
    let mut count = 0;

    for (index, _) in text.match_indices(query) {
        output.push_str(&text[last..index]);
        output.push_str(replacement);
        last = index + query.len();
        count += 1;
    }
    output.push_str(&text[last..]);

    (output, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_occurrence() {
        assert_eq!(count_matches("foo bar foo baz foo", "foo"), 3);
    }

    #[test]
    fn counts_nothing_for_empty_query() {
        assert_eq!(count_matches("anything", ""), 0);
    }

    #[test]
    fn replaces_each_occurrence() {
        let (text, count) = replace_all("foo bar foo", "foo", "qux");
        assert_eq!(text, "qux bar qux");
        assert_eq!(count, 2);
    }

    #[test]
    fn leaves_text_alone_when_query_is_missing() {
        let (text, count) = replace_all("alpha beta", "gamma", "delta");
        assert_eq!(text, "alpha beta");
        assert_eq!(count, 0);
    }

    #[test]
    fn leaves_text_alone_for_empty_query() {
        let (text, count) = replace_all("alpha", "", "beta");
        assert_eq!(text, "alpha");
        assert_eq!(count, 0);
    }

    #[test]
    fn replacement_may_contain_query() {
        let (text, count) = replace_all("a a", "a", "aa");
        assert_eq!(text, "aa aa");
        assert_eq!(count, 2);
    }

    #[test]
    fn empty_replacement_deletes_matches() {
        let (text, count) = replace_all("one two one", "one ", "");
        assert_eq!(text, "two one");
        assert_eq!(count, 1);
    }

    #[test]
    fn replacement_may_add_lines() {
        let (text, count) = replace_all("a;b", ";", "\n");
        assert_eq!(text, "a\nb");
        assert_eq!(count, 1);
    }

    #[test]
    fn handles_unicode_offsets() {
        let (text, count) = replace_all("café café", "café", "tea");
        assert_eq!(text, "tea tea");
        assert_eq!(count, 2);
    }
}
