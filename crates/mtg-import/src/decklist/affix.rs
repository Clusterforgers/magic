//! Splits the printing, foil marker and tag groups off the end of a card line.

pub(super) fn is_set_code(text: &str) -> bool {
    (2..=6).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub(super) fn is_collector_number(text: &str) -> bool {
    text.bytes().any(|b| b.is_ascii_digit()) && !text.starts_with(['(', '[', '*'])
}

/// Splits a trailing `open ... close` group off `text` when it is separated from the rest by whitespace.
pub(super) fn split_trailing_group(text: &str, open: char, close: char) -> Option<(&str, &str)> {
    let without_close = text.strip_suffix(close)?;
    let start = without_close.rfind(open)?;
    let head = &without_close[..start];
    if !head.is_empty() && !head.ends_with(char::is_whitespace) {
        return None;
    }
    Some((head.trim_end(), &without_close[start + open.len_utf8()..]))
}

/// Index of the `#` that opens a Moxfield tag list: the first one that follows whitespace.
pub(super) fn hash_tags_start(body: &str) -> Option<usize> {
    body.char_indices()
        .find(|&(at, c)| c == '#' && (at == 0 || body[..at].ends_with(char::is_whitespace)))
        .map(|(at, _)| at)
}

/// Splits a trailing `*F*` / `*E*` finish marker off `text`.
pub(super) fn strip_foil_marker(text: &str) -> Option<&str> {
    let body = text.strip_suffix('*')?;
    let (head, marker) = body.rsplit_once('*')?;
    let single_letter = marker.len() == 1 && marker.bytes().all(|b| b.is_ascii_alphabetic());
    (single_letter && (head.is_empty() || head.ends_with(char::is_whitespace)))
        .then_some(head.trim_end())
}

/// Splits `NAME (SET) NUMBER`, `NAME (SET)` and (optionally) `NAME [SET]` suffixes.
pub(super) fn split_printing(
    body: &str,
    bracket_sets: bool,
) -> (&str, Option<String>, Option<String>) {
    if let Some((head, number)) = body.rsplit_once(char::is_whitespace)
        && is_collector_number(number)
        && let Some((name, set)) = split_trailing_group(head.trim_end(), '(', ')')
        && is_set_code(set)
    {
        return (
            name,
            Some(set.to_ascii_uppercase()),
            Some(number.to_owned()),
        );
    }
    let groups: &[(char, char)] = if bracket_sets {
        &[('(', ')'), ('[', ']')]
    } else {
        &[('(', ')')]
    };
    for &(open, close) in groups {
        if let Some((name, set)) = split_trailing_group(body, open, close)
            && is_set_code(set)
        {
            return (name, Some(set.to_ascii_uppercase()), None);
        }
    }
    (body, None, None)
}
