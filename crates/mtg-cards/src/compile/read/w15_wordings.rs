//! Wordings of one meaning that the oracle text and the writers say differently (w15 win-back of `Differs` clauses):
//! "also deals", a toll counted as "{X}, where X is the number of", "any number" of sacrificed creatures, the other card
//! of two looked at, the top card revealed and sorted by "If it's a ..., Otherwise", a pair of types in a count, and a
//! copy of "any other" card in a graveyard. Both sides of a comparison are normalized, so a rule of another meaning
//! still differs.

/// The text with each of the wordings below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = text.replace(" also deals ", " deals ");
    for step in [
        pays_each,
        sacrifice_any_number,
        the_other_card,
        reveal_one,
        any_other_in_a_graveyard,
        divided_among,
        attached_pair,
    ] {
        if let Some(changed) = step(&text) {
            text = changed;
        }
    }
    text
}

/// The words that end the noun a count is of ("the number of cards in your hand").
const NOUN_ENDS: [&str; 8] = [
    " among ", " in ", " you ", " your ", " that ", " on ", " with ", " of ",
];

/// "pays {X}, where X is the number of cards in your hand" as "pays {1} for each card in your hand".
fn pays_each(text: &str) -> Option<String> {
    const HEAD: &str = "pays {X}, where X is the number of ";
    let at = text.find(HEAD)?;
    let after = at + HEAD.len();
    let end = text[after..]
        .find(['.', ',', '\n'])
        .map_or(text.len(), |end| end + after);
    let counted = &text[after..end];
    let noun_end = NOUN_ENDS
        .iter()
        .filter_map(|word| counted.find(word))
        .min()
        .unwrap_or(counted.len());
    let singular = singular(&counted[..noun_end]);
    Some(format!(
        "{}pays {{1}} for each {singular}{}{}",
        &text[..at],
        &counted[noun_end..],
        &text[end..]
    ))
}

/// A plural noun phrase as singular: its last word loses the plural ("basic land types" as "basic land type").
fn singular(phrase: &str) -> String {
    if let Some(stem) = phrase.strip_suffix("ies") {
        format!("{stem}y")
    } else if phrase.ends_with("ss") {
        phrase.to_owned()
    } else {
        phrase.strip_suffix('s').unwrap_or(phrase).to_owned()
    }
}

/// "As an additional cost to cast this spell, you may sacrifice any number of Spirits." as "..., sacrifice X Spirits.":
/// the caster announces how many, and none is a number.
fn sacrifice_any_number(text: &str) -> Option<String> {
    const COST: &str = "As an additional cost to cast ";
    let at = text.find(COST)?;
    let comma = text[at..].find(", ")? + at + 2;
    let rest = &text[comma..];
    let rest = rest.strip_prefix("you may ").unwrap_or(rest);
    let rest = rest.strip_prefix("sacrifice any number of ")?;
    Some(format!("{}sacrifice X {rest}", &text[..comma]))
}

/// "Look at the top two cards of your library. Put one of them into your hand and the other on the bottom of your
/// library." as "... and the rest on the bottom of your library in any order": one card is left, whatever its order.
fn the_other_card(text: &str) -> Option<String> {
    const LOOK: &str = "ook at the top two cards of your library. Put one of them into your hand and the ";
    const OTHER: &str = "other on the bottom of your library";
    let at = text.find(LOOK)? + LOOK.len();
    let tail = text[at..].strip_prefix(OTHER)?;
    let tail = tail.strip_prefix(" in any order").unwrap_or(tail);
    Some(format!(
        "{}rest on the bottom of your library in any order{tail}",
        &text[..at]
    ))
}

/// The top card revealed and sorted: "Reveal the top card of your library. If it's a creature card, put it onto the
/// battlefield. Otherwise, put it into your graveyard." as "Reveal the top card of your library. Then put all creature
/// cards revealed this way onto the battlefield and the rest into your graveyard." The plain "revealed" step is
/// followed by "and put all ..." or ". Then put all ...", and one card has no order.
fn reveal_one(text: &str) -> Option<String> {
    const REVEAL: &str = "eveal the top card of your library";
    let at = text.find(REVEAL)?.checked_sub(1)?;
    let reveal = &text[at..at + 1 + REVEAL.len()];
    if !reveal.starts_with(['R', 'r']) {
        return None;
    }
    let after = at + reveal.len();
    let tail = &text[after..];
    let sorted = if let Some(rest) = tail.strip_prefix(" and put all ") {
        format!(". Then put all {rest}")
    } else if let Some(rest) = tail.strip_prefix(". If it's ") {
        reveal_sorted(rest)?
    } else {
        tail.to_owned()
    };
    // One card has no order.
    let sorted = sorted.replace(
        " and the rest on the bottom of your library in any order",
        " and the rest on the bottom of your library",
    );
    let changed = format!("{}{reveal}{sorted}", &text[..at]);
    (changed != text).then_some(changed)
}

/// "a creature card, put it onto the battlefield. Otherwise, put it into your graveyard." as ". Then put all creature
/// cards revealed this way onto the battlefield and the rest into your graveyard." (the text after "If it's ").
fn reveal_sorted(rest: &str) -> Option<String> {
    let rest = rest
        .strip_prefix("a ")
        .or_else(|| rest.strip_prefix("an "))?;
    let (kind, rest) = rest.split_once(" card, put it ")?;
    let (kept, rest) = rest.split_once(". Otherwise, put it ")?;
    Some(format!(
        ". Then put all {kind} cards revealed this way {kept} and the rest {rest}"
    ))
}

/// "enter as a copy of any other creature card in a graveyard" as "any creature card": the card that copies is not in
/// the graveyard it looks at.
fn any_other_in_a_graveyard(text: &str) -> Option<String> {
    const HEAD: &str = "enter as a copy of any other ";
    let at = text.find(HEAD)? + HEAD.len();
    let end = text[at..].find(['.', ','])? + at;
    text[at..end]
        .ends_with(" card in a graveyard")
        .then(|| format!("{}enter as a copy of any {}", &text[..at - HEAD.len()], &text[at..]))
}

/// "deals X damage divided as you choose among up to X targets" as "among any number of targets": each target takes at
/// least one damage, so no more than X can be chosen.
fn divided_among(text: &str) -> Option<String> {
    const HEAD: &str = " damage divided as you choose among up to X target";
    let at = text.find(HEAD)?;
    text[..at].ends_with("deals X").then(|| {
        format!(
            "{} damage divided as you choose among any number of target{}",
            &text[..at],
            &text[at + HEAD.len()..]
        )
    })
}

/// "for each Aura and Equipment attached to it" as "for each Aura and/or Equipment": a count of permanents that are
/// either, in the order of the names.
fn attached_pair(text: &str) -> Option<String> {
    const HEAD: &str = "for each ";
    const TAIL: &str = " attached to ";
    let at = text.find(HEAD)? + HEAD.len();
    let end = text[at..].find(TAIL)? + at;
    let pair = &text[at..end];
    let (left, right) = pair
        .split_once(" and/or ")
        .or_else(|| pair.split_once(" and "))?;
    if left.contains([' ', ',']) || right.contains([' ', ',']) {
        return None;
    }
    let (first, second) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    Some(format!(
        "{}{first} and/or {second}{}",
        &text[..at],
        &text[end..]
    ))
}
