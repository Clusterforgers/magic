//! Wordings of the same rule that the writers print one way and cards print another: "that player" for "them" after
//! damage, and "another X permanent" for "an X permanent other than ~".

/// The words of a permanent that may stand between "another" and "permanent": one or two lowercase words.
fn kind_words(text: &str) -> bool {
    let words: Vec<&str> = text.split(' ').collect();
    matches!(words.len(), 1 | 2)
        && words
            .iter()
            .all(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase()))
}

/// "another legendary permanent" as "a legendary permanent other than ~" (the first occurrence); none without one.
fn other_than(text: &str) -> Option<String> {
    let start = text.find("another ")?;
    let after = &text[start + "another ".len()..];
    let end = after.find(" permanent")?;
    kind_words(&after[..end]).then_some(())?;
    let kind = &after[..end];
    let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    let tail = &after[end + " permanent".len()..];
    Some(format!(
        "{}{article} {kind} permanent other than ~{tail}",
        &text[..start]
    ))
}

/// The other wordings of the texts written for an ability.
pub(super) fn t18_wordings(texts: &[String]) -> Vec<String> {
    let mut more = Vec::new();
    for text in texts {
        if let Some(other) = other_than(text) {
            more.push(other);
        }
        if text.contains(" damage to that player.") {
            more.push(text.replace(" damage to that player.", " damage to them."));
        }
    }
    more
}
