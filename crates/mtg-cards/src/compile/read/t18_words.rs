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

/// The wordings the cards print for what a trigger of the player an event names does to them: "that player exiles the top
/// card of their library", "that player discards a card and you untap all lands you control", "destroy up to one target
/// planeswalker and up to one target artifact", "historic card" for the permanent cards that are historic, and the kind
/// of a card before "historic" ("non-Assassin historic card").
pub(super) fn w20_wordings(text: &str) -> Vec<String> {
    let mut more = Vec::new();
    if text.contains("exile the top") && text.contains(" of that player's library") {
        more.push(
            text.replace("exile the top", "that player exiles the top")
                .replace(" of that player's library", " of their library"),
        );
    }
    if text.contains(" and untap each land you control") {
        more.push(text.replace(
            " and untap each land you control",
            " and you untap all lands you control",
        ));
    }
    if let Some(at) = text.find("destroy up to one target ")
        && let Some(rest) = text[at..].strip_prefix("destroy up to one target ")
        && let Some(then) = rest.find(", then destroy up to one target ")
        && !rest[..then].contains(['.', ','])
    {
        let first = &rest[..then];
        let second = &rest[then + ", then destroy up to one target ".len()..];
        more.push(format!(
            "{}destroy up to one target {first} and up to one target {second}",
            &text[..at]
        ));
    }
    if text.contains("historic attacking creature") {
        more.push(text.replace("historic attacking creature", "attacking historic creature"));
    }
    if text.contains("historic permanent card") {
        more.push(text.replace("historic permanent card", "historic card"));
    }
    if let Some(at) = text.find("historic non-")
        && let Some(word) = text[at + "historic non-".len()..].split(' ').next()
        && text[at + "historic non-".len() + word.len()..].starts_with(" card")
    {
        more.push(text.replace(
            &format!("historic non-{word} card"),
            &format!("non-{word} historic card"),
        ));
    }
    more
}

/// The other wordings of the texts written for an ability.
pub(super) fn t18_wordings(texts: &[String]) -> Vec<String> {
    let mut more = Vec::new();
    for text in texts {
        if let Some(other) = other_than(text) {
            more.push(other);
        }
        more.extend(w20_wordings(text));
        if text.contains(" damage to that player.") {
            more.push(text.replace(" damage to that player.", " damage to them."));
        }
    }
    more
}
