//! More Oracle wordings of the same trigger: the card that triggered calls itself "it" when nothing else was named
//! before, the two events of "~ dies or deals combat damage" in either order, "a blue or red card" for "a card that's
//! blue or red", "you don't control an Ogre" for "you control no Ogres", and "and/or" in a list of kinds.

/// The colors a "that's" phrase names.
const COLORS: [&str; 5] = ["white", "blue", "black", "red", "green"];

/// The words that name another object, after which "it" no longer means the card.
const OBJECTS: [&str; 7] = [
    "creature",
    "permanent",
    "card",
    "spell",
    "token",
    "target",
    "artifact",
];

/// Every one-step rewrite of a text.
pub(super) fn rewrites(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(itself_as_it(text));
    out.extend(power_first(text));
    out.extend(events_swapped(text));
    out.extend(colors_before(text));
    out.extend(none_controlled(text));
    out.extend(and_or(text));
    out
}

/// The head of a trigger about the card itself ("Whenever ~ attacks or blocks,") and the rest of the text.
fn own_head(text: &str) -> Option<(&str, &str)> {
    let (head, rest) = text.split_at(text.find(", ")?);
    let event = head
        .strip_prefix("Whenever ")
        .or_else(|| head.strip_prefix("When "))?;
    (!OBJECTS.iter().any(|word| event.contains(word)) && event.matches('~').count() == 1)
        .then_some((head, rest))
}

/// "Whenever ~ attacks or blocks, destroy ~ at end of combat" as "..., destroy it at end of combat", and "put a +1/+1
/// counter on ~ and untap ~" as "... and untap it": "it" is the card while no other object was named since.
fn itself_as_it(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (opens, rest) = match own_head(text) {
        Some((head, rest)) => (head.len(), rest),
        None => match text
            .find('~')
            .and_then(|first| text.find(", ").filter(|comma| *comma < first))
        {
            Some(comma) => match text[comma..].find('~') {
                Some(first) => (comma + first + 1, &text[comma + first + 1..]),
                None => return out,
            },
            None => return out,
        },
    };
    let Some(at) = rest.find('~') else {
        return out;
    };
    let tail = &rest[at + 1..];
    if !OBJECTS.iter().any(|word| rest[..at].contains(word))
        && !rest[..at].contains(". ")
        && !tail.starts_with('\'')
    {
        out.push(format!("{}{}it{tail}", &text[..opens], &rest[..at]));
    }
    out
}

/// "deals damage to target creature equal to its power" as "deals damage equal to its power to target creature".
fn power_first(text: &str) -> Option<String> {
    let text = text.replace(" equal to her power", " equal to its power");
    let at = text.find(" deals damage to ")?;
    let after = &text[at + " deals damage to ".len()..];
    let end = after.find(" equal to its power")?;
    let whom = &after[..end];
    (!whom.contains(',')).then(|| {
        format!(
            "{} deals damage equal to its power to {whom}{}",
            &text[..at],
            &after[end + " equal to its power".len()..]
        )
    })
}

/// "Whenever ~ dies or deals combat damage to a player," as "Whenever ~ deals combat damage to a player or dies,".
fn events_swapped(text: &str) -> Option<String> {
    let (head, rest) = own_head(text)?;
    let opener = if head.starts_with("When ~ ") {
        "When ~ "
    } else {
        "Whenever ~ "
    };
    let event = &head[opener.len()..];
    let (first, second) = event.split_once(" or ")?;
    (!second.contains(" or ")).then(|| format!("{opener}{second} or {first}{rest}"))
}

/// "target creature card that's red or blue" as "target blue or red creature card": the colors of a kind before it.
fn colors_before(text: &str) -> Option<String> {
    let at = text.find(" that's ")?;
    let after = &text[at + " that's ".len()..];
    let words: Vec<&str> = after.splitn(4, ' ').collect();
    let [one, "or", two, ..] = words.as_slice() else {
        return None;
    };
    if !COLORS.contains(one) || !COLORS.contains(two) {
        return None;
    }
    let before = &text[..at];
    let opens = ["target ", "a ", "an "]
        .iter()
        .filter_map(|article| before.rfind(article).map(|found| found + article.len()))
        .max()?;
    let kind = &before[opens..];
    if kind.split(' ').count() > 2 || kind.contains(',') {
        return None;
    }
    let rest = &after[one.len() + " or ".len() + two.len()..];
    Some(format!("{}{two} or {one} {kind}{rest}", &text[..opens]))
}

/// "if you control no Ogres" as "if you don't control an Ogre".
fn none_controlled(text: &str) -> Option<String> {
    let at = text.find("if you control no ")?;
    let after = &text[at + "if you control no ".len()..];
    let end = after
        .find(|c: char| !c.is_alphanumeric())
        .unwrap_or(after.len());
    let plural = &after[..end];
    let single = plural.strip_suffix('s')?;
    if !single.starts_with(char::is_uppercase) || single.ends_with('s') {
        return None;
    }
    let article = if single.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    Some(format!(
        "{}if you don't control {article} {single}{}",
        &text[..at],
        &after[end..]
    ))
}

/// The class of a word that names a kind: `1` for a subtype, `2` for a card type.
fn kind(word: &str) -> Option<u8> {
    let word = word.trim_end_matches(',');
    let types = [
        "artifact",
        "creature",
        "enchantment",
        "land",
        "planeswalker",
        "battle",
        "instant",
        "sorcery",
    ];
    if word.starts_with(char::is_uppercase) {
        Some(1)
    } else if types
        .iter()
        .any(|kind| word == *kind || word.strip_suffix('s') == Some(kind))
    {
        Some(2)
    } else {
        None
    }
}

/// "Villains or Heroes" and "Cats and Zombies" as "and/or", and a list of three or more kinds joined by "and/or" as
/// "and" or "or": a list of kinds of one class is any of them.
fn and_or(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out = Vec::new();
    for at in 1..words.len().saturating_sub(1) {
        let class = kind(words[at - 1]);
        if class.is_none() || class != kind(words[at + 1]) {
            continue;
        }
        let long = words[at - 1].ends_with(',');
        let others: &[&str] = match words[at] {
            "or" | "and" => &["and/or"],
            "and/or" if long => &["and", "or"],
            "and/or" => &["or"],
            _ => &[],
        };
        for other in others {
            let mut changed: Vec<&str> = words.clone();
            changed[at] = other;
            out.push(changed.join(" "));
        }
    }
    out
}
