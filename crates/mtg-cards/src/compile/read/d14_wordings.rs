//! Wordings of one meaning that the oracle text and the writers say differently (d14 triage of the `Differs` clauses):
//! "until end of turn, whenever", "any number of target" and "add an additional". Both sides of a comparison are normalized, so a rule of another meaning still differs.

/// The text with each of the wordings below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = text.to_owned();
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

/// "Until end of turn, whenever you cast a spell, copy it." as "Whenever you cast a spell this turn, copy it.": the
/// trigger's event ends at the first comma that follows its words.
fn this_turn(text: &str) -> Option<String> {
    const HEAD: &str = "Until end of turn, whenever ";
    // After the head of another ability ("When ~ enters, until end of turn, whenever ...") the words are in lower case.
    const INNER: &str = ", until end of turn, whenever ";
    let (at, after, word) = if let Some(at) = text.find(HEAD) {
        (at, at + HEAD.len(), "Whenever")
    } else {
        let at = text.find(INNER)? + 2;
        (at, at + INNER.len() - 2, "whenever")
    };
    let comma = text[after..].find(", ")? + after;
    let event = &text[after..comma];
    if event.contains(" this turn") {
        return None;
    }
    Some(format!(
        "{}{word} {event} this turn{}",
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
