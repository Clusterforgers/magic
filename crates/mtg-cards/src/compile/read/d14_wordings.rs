//! Wordings of one meaning that the oracle text and the writers say differently (d14 triage of the `Differs` clauses): the
//! name of a mode, the possessive of "owner", "until end of turn, whenever", "any number of target" and "add an
//! additional". Both sides of a comparison are normalized, so a rule of another meaning still differs.

/// The text with each of the wordings below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = mode_names(text);
    for (from, to) in OWNERS {
        if text.contains(from) {
            text = text.replace(from, to);
        }
    }
    if text.contains("add an additional {") {
        text = text.replace("add an additional {", "add {");
    }
    if let Some(moved) = this_turn(&text) {
        text = moved;
    }
    if let Some(any) = any_number(&text) {
        text = any;
    }
    text
}

/// The possessive of "owner" in the places a card goes: "to their owners' hands" and "to its owner's hand" name the owner.
const OWNERS: &[(&str, &str)] = &[
    ("owners' hands", "owner's hand"),
    ("owner's hands", "owner's hand"),
    ("owners'", "owner's"),
    ("their owner's", "its owner's"),
];

/// A mode line without its name: "• Charge Them — Creatures you control get +2/+0" is flavor before the dash (CR 207.2c
/// names ability words; the names of modes are the same kind of label).
fn mode_names(text: &str) -> String {
    if !text.contains("\n• ") && !text.starts_with("• ") {
        return text.to_owned();
    }
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| match line.strip_prefix("• ") {
            Some(rest) => match rest.split_once(" \u{2014} ") {
                Some((name, body)) if is_name(name) => format!("• {body}"),
                _ => line.to_owned(),
            },
            None => line.to_owned(),
        })
        .collect();
    lines.join("\n")
}

/// Whether the words before a dash are a title: capitalized words, at most five, no digits, symbols or punctuation but
/// an apostrophe or a comma.
fn is_name(name: &str) -> bool {
    let words: Vec<&str> = name.split(' ').collect();
    words.len() <= 5
        && words.iter().enumerate().all(|(at, word)| {
            let small = at > 0 && matches!(*word, "of" | "the" | "a" | "to" | "and" | "in" | "for");
            let capital = word.chars().next().is_some_and(char::is_uppercase);
            (small || capital)
                && word
                    .chars()
                    .all(|c| c.is_alphabetic() || matches!(c, '\'' | ',' | '-'))
        })
}

/// "Until end of turn, whenever you cast a spell, copy it." as "Whenever you cast a spell this turn, copy it.": the
/// trigger's event ends at the first comma that follows its words.
fn this_turn(text: &str) -> Option<String> {
    const HEAD: &str = "Until end of turn, whenever ";
    let at = text.find(HEAD)?;
    let after = at + HEAD.len();
    let comma = text[after..].find(", ")? + after;
    let event = &text[after..comma];
    if event.contains(" this turn") {
        return None;
    }
    Some(format!(
        "{}Whenever {event} this turn{}",
        &text[..at],
        &text[comma..]
    ))
}

/// "up to X target creatures you control, where X is the number of creatures you control" as "any number of target
/// creatures you control": the most that can be chosen is all of them.
fn any_number(text: &str) -> Option<String> {
    const HEAD: &str = "p to X target ";
    const WHERE: &str = ", where X is the number of ";
    // The letter before the head is the case of the sentence.
    let head = text.find(HEAD)?;
    let at = head.checked_sub(1)?;
    let any = match text.get(at..head)? {
        "U" => "Any",
        "u" => "any",
        _ => return None,
    };
    let what = head + HEAD.len();
    let split = text[what..].find(WHERE)? + what;
    let kind = &text[what..split];
    let after = split + WHERE.len();
    let end = text[after..]
        .find(['.', ',', '\n'])
        .map_or(text.len(), |end| end + after);
    let counted = &text[after..end];
    let kind_in = kind.replace(" from ", " in ");
    // The words after the pool are the verb of the spell ("phase out"); a pool ends at its owner or its zone.
    let ends_pool = [" you control", " in your graveyard", " on the battlefield"]
        .iter()
        .any(|end| counted.ends_with(end));
    let same = counted == kind_in
        || counted.strip_suffix(" on the battlefield") == Some(kind)
        || (ends_pool && kind_in.starts_with(&format!("{counted} ")));
    same.then(|| {
        format!(
            "{}{any} number of target {kind}{}",
            &text[..at],
            &text[end..]
        )
    })
}
