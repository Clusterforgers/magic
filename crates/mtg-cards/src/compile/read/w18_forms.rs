//! Forms of w18 that mean the same to the rules: a permanent that becomes a creature with base stats, abilities lost and
//! keywords gained in any wording, "any number of" targets bounded by their own count, mana that may not pay for a kind of
//! spell, damage prevented from a kind of source, and a second action on the same object.

/// The text with each of the forms below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = text.to_owned();
    for step in [
        becomes_sets,
        any_number,
        mana_restriction,
        prevent_damage,
        small_forms,
        owner_shuffles,
        self_it,
        lower_starts,
    ] {
        if let Some(changed) = step(&text) {
            text = changed;
        }
    }
    text
}

/// The first letter of a sentence, of a clause after a cost and of the text in upper case or lower: "Snow creatures you
/// control get" and "snow creatures you control get" are one subject.
fn lower_starts(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut start = true;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if start && c.is_ascii_uppercase() && chars.peek().is_some_and(char::is_ascii_lowercase) {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
        start = match c {
            ' ' => start,
            '.' | ':' | '\n' => true,
            _ => false,
        };
    }
    (out != text).then_some(out)
}

/// The text with `change` applied to each sentence that it reads; `None` when no sentence changes.
fn per_sentence(text: &str, change: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut any = false;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, newline) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        let mut parts = body.split(". ").peekable();
        while let Some(part) = parts.next() {
            let last = parts.peek().is_none();
            let (core, dot) = match part.strip_suffix('.') {
                Some(core) if last => (core, "."),
                _ => (part, ""),
            };
            match change(core) {
                Some(changed) => {
                    any = true;
                    out.push_str(&changed);
                }
                None => out.push_str(core),
            }
            out.push_str(dot);
            if !last {
                out.push_str(". ");
            }
        }
        out.push_str(newline);
    }
    any.then_some(out)
}

/// The keywords a permanent may gain in a list.
const KEYWORDS: [&str; 17] = [
    "flying",
    "haste",
    "hexproof",
    "trample",
    "vigilance",
    "indestructible",
    "deathtouch",
    "lifelink",
    "reach",
    "menace",
    "defender",
    "shroud",
    "first strike",
    "double strike",
    "fear",
    "intimidate",
    "flash",
];

/// Whether the text is a power and toughness: "4/4", "0/1", "*/*".
fn is_size(text: &str) -> bool {
    text.split_once('/').is_some_and(|(power, toughness)| {
        [power, toughness].iter().all(|side| {
            !side.is_empty()
                && side
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '*' | 'X' | '+' | '-'))
        })
    })
}

/// "~ becomes a Dragon with base power and toughness 4/4, flying, and haste" and "~ has base power and toughness 4/4 and
/// becomes a Dragon and gains haste and flying" are one text: what a permanent becomes, its size, the abilities it loses
/// and the keywords it gains have no order and no join.
fn becomes_sets(text: &str) -> Option<String> {
    if text.contains('"') {
        return None;
    }
    per_sentence(text, becomes_sentence)
}

/// One sentence of [`becomes_sets`] written one way, when every part of it is a part that sets what a permanent is.
fn becomes_sentence(sentence: &str) -> Option<String> {
    let (cost, rest) = match sentence.split_once(": ") {
        Some((cost, rest)) if !cost.contains(' ') || cost.contains('{') => {
            (&sentence[..cost.len() + 2], rest)
        }
        _ => ("", sentence),
    };
    let mut rest = rest.to_owned();
    let mut until = false;
    if let Some(tail) = rest.strip_prefix("Until end of turn, ") {
        until = true;
        rest = tail.to_owned();
    }
    if let Some(head) = rest
        .strip_suffix(" [until end of turn]")
        .or_else(|| rest.strip_suffix(" until end of turn"))
    {
        until = true;
        rest = head.to_owned();
    }
    let rest = rest.replace("base power and toughness", "bpt");
    let marker = [
        " becomes a ",
        " becomes an ",
        " become a ",
        " become an ",
        " has bpt ",
        " have bpt ",
        " loses all abilities",
        " lose all abilities",
    ]
    .iter()
    .filter_map(|marker| rest.find(marker))
    .min()?;
    let subject = &rest[..marker];
    if subject.contains(',') || subject.is_empty() {
        return None;
    }
    let predicate = rest[marker + 1..]
        .replace(", and ", "|")
        .replace(", ", "|")
        .replace(" and ", "|");
    let mut size = None;
    let mut kinds: Vec<&str> = Vec::new();
    let mut gains: Vec<&str> = Vec::new();
    let mut loses = false;
    let mut listing = false;
    for item in predicate.split('|') {
        if let Some(rest) = item
            .strip_prefix("has bpt ")
            .or_else(|| item.strip_prefix("have bpt "))
        {
            if !is_size(rest) || size.replace(rest).is_some() {
                return None;
            }
        } else if let Some(rest) = ["becomes a ", "becomes an ", "become a ", "become an "]
            .iter()
            .find_map(|prefix| item.strip_prefix(prefix))
        {
            let (kind, with) = match rest.split_once(" with bpt ") {
                Some((kind, with)) => (kind, Some(with)),
                None => (rest, None),
            };
            if kind.is_empty() {
                return None;
            }
            kinds.push(kind);
            if let Some(with) = with {
                if !is_size(with) || size.replace(with).is_some() {
                    return None;
                }
                listing = true;
            }
        } else if let Some(key) = item
            .strip_prefix("gains ")
            .or_else(|| item.strip_prefix("gain "))
        {
            if !KEYWORDS.contains(&key) {
                return None;
            }
            gains.push(key);
            listing = true;
        } else if matches!(item, "loses all abilities" | "lose all abilities") {
            loses = true;
        } else if listing && KEYWORDS.contains(&item) {
            gains.push(item);
        } else {
            return None;
        }
    }
    let size = size?;
    gains.sort_unstable();
    Some(format!(
        "{cost}{subject} becomes [{}] with bpt {size}, gains [{}], loses {loses}{}",
        kinds.join(" "),
        gains.join(" "),
        if until { " [until end of turn]" } else { "" }
    ))
}

/// "up to X target artifacts, where X is the number of artifacts on the battlefield" is "any number of target artifacts":
/// the bound is the number of things the targets can be.
fn any_number(text: &str) -> Option<String> {
    const OPEN: &str = "up to X target ";
    const WHERE: &str = ", where X is the number of ";
    let start = text.find(OPEN)?;
    let after = &text[start + OPEN.len()..];
    let (kind, bound) = after.split_once(WHERE)?;
    let end = bound.find(['.', '\n']).unwrap_or(bound.len());
    let pool = bound[..end].trim_end_matches(" on the battlefield");
    let same = |text: &str| text.replace("and/or", "and");
    let (kind_same, pool_same) = (same(kind), same(pool));
    let named = kind_same.strip_prefix(pool_same.as_str())?;
    // Another "X" in the targets ("with total mana value X or less") is not the number the bound counts.
    let other_x = kind.split(' ').any(|word| word == "X");
    if pool.is_empty() || other_x || !(named.is_empty() || named.starts_with(' ')) {
        return None;
    }
    Some(format!(
        "{}any number of target {kind}{}",
        &text[..start],
        &bound[end..]
    ))
}

/// The text before, between and after the words `open` and `close` (found without regard to case).
fn between_ci<'a>(text: &'a str, open: &str, close: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(open)?;
    let from = start + open.len();
    let end = from + lower[from..].find(close)?;
    Some((&text[..start], &text[from..end], &text[end + close.len()..]))
}

/// The word of a kind without its article: "an artifact" is "artifact".
fn without_article(kind: &str) -> Option<&str> {
    kind.strip_prefix("an ").or_else(|| kind.strip_prefix("a "))
}

/// "This mana can't be spent to cast a nonartifact spell." and "Spend this mana only to cast an artifact spell or to
/// activate an ability." are one restriction: the mana pays for the one kind of spell and for abilities.
fn mana_restriction(text: &str) -> Option<String> {
    let canon = |before: &str, kind: &str, after: &str| {
        format!("{before}spend this mana only to cast [{kind}] spell or activate ability{after}")
    };
    if let Some((before, kind, after)) =
        between_ci(text, "this mana can't be spent to cast ", " spell")
    {
        let kind = without_article(kind)?.strip_prefix("non")?;
        return Some(canon(before, kind, after));
    }
    let (before, rest, after) = between_ci(text, "spend this mana only to cast ", " ability")?;
    let (kind, tail) = rest.split_once(" spell or to activate ")?;
    if tail != "a" && tail != "an" {
        return None;
    }
    let before = before
        .strip_suffix("Then ")
        .or_else(|| before.strip_suffix("then "))
        .unwrap_or(before);
    Some(canon(before, without_article(kind)?, after))
}

/// "Prevent all damage that would be dealt to ~ by Deserts." and "Prevent that damage if a Desert would deal damage to
/// ~.": one prevention of the damage a kind of source deals to an object.
fn prevent_damage(text: &str) -> Option<String> {
    per_sentence(text, |sentence| {
        let canon = |source: &str, object: &str| {
            format!("Prevent all damage from [{source}] to [{object}]")
        };
        // The same with combat damage alone (Marble Priest).
        if let Some(rest) =
            sentence.strip_prefix("Prevent all combat damage that would be dealt to ")
        {
            let (object, source) = rest.split_once(" by ")?;
            let source = source.strip_suffix('s')?;
            return Some(format!(
                "Prevent all combat damage from [{source}] to [{object}]"
            ));
        }
        if let Some(rest) = sentence.strip_prefix("Prevent that damage if ")
            && let Some((source, object)) = rest.split_once(" would deal combat damage to ")
        {
            let source = source
                .strip_prefix("an ")
                .or_else(|| source.strip_prefix("a "))
                .unwrap_or(source);
            let object = object.strip_prefix("a ").unwrap_or(object);
            return Some(format!(
                "Prevent all combat damage from [{source}] to [{object}]"
            ));
        }
        if let Some(rest) = sentence.strip_prefix("Prevent all damage that would be dealt to ") {
            let (object, source) = rest.split_once(" by ")?;
            let source = match source.strip_suffix(" sources") {
                Some(kind) => kind,
                None => source.strip_suffix('s')?,
            };
            return Some(canon(source, object));
        }
        if let Some(rest) = sentence.strip_prefix("Prevent all damage that ")
            && let Some((source, object)) = rest
                .split_once(" would deal to ")
                .or_else(|| rest.split_once(" would deals to "))
        {
            let object = object.strip_suffix('s')?;
            return Some(canon(source, object));
        }
        let rest = sentence.strip_prefix("Prevent that damage if ")?;
        if let Some((source, object)) = rest
            .split_once(" would deal damage to ")
            .or_else(|| rest.split_once(" would deals damage to "))
        {
            let article = |text: &str| -> String {
                text.strip_prefix("an ")
                    .or_else(|| text.strip_prefix("a "))
                    .unwrap_or(text)
                    .to_owned()
            };
            return Some(canon(&article(source), &article(object)));
        }
        None
    })
}

/// "gets +1/+0 until end of turn, then that creature can't be blocked this turn" and "... and can't be blocked this
/// turn": a second action on the same object.
pub(super) fn early(text: &str) -> Option<String> {
    let changed = text
        .replace(", then that creature can't ", " and can't ")
        .replace(". That creature can't ", " and can't ")
        .replace(", then it can't ", " and can't ")
        .replace(". It can't ", " and can't ")
        .replace(" and it can't ", " and can't ")
        .replace(", then it gains ", " and it gains ");
    (changed != text).then_some(changed)
}

/// Whether the byte is a character of a word.
fn word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'\''
}

/// The text with `from` replaced by `to` where it stands as whole words.
fn replace_words(text: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(from) {
        let before = rest[..at].bytes().next_back();
        let after = rest[at + from.len()..].bytes().next();
        let whole =
            before.is_none_or(|byte| !word_byte(byte)) && after.is_none_or(|byte| !word_byte(byte));
        out.push_str(&rest[..at]);
        out.push_str(if whole { to } else { from });
        rest = &rest[at + from.len()..];
    }
    out.push_str(rest);
    out
}

/// Small forms of one meaning: a list of "and/or" is a list of "and", energy is said as symbols or as counters, "that
/// card" is "it", and "that land" is "that permanent" (the noun repeats what the text already says).
fn small_forms(text: &str) -> Option<String> {
    let mut changed = text
        .replace(" and/or ", " and ")
        .replace(" owners' ", " owner's ")
        .replace("an energy counter", "{E}")
        .replace("a energy counter", "{E}")
        .replace("1 energy counter", "{E}");
    for count in 2..=9 {
        changed = changed.replace(&format!("{count} energy counters"), &"{E}".repeat(count));
    }
    for (from, to) in [
        ("that card's", "its"),
        ("that card", "it"),
        ("those cards", "them"),
        ("that land's", "that permanent's"),
        ("that land", "that permanent"),
        ("that artifact's", "that permanent's"),
        ("that artifact", "that permanent"),
    ] {
        changed = replace_words(&changed, from, to);
    }
    (changed != text).then_some(changed)
}

/// The words that name an object other than the card itself, which the "it" of an action may be.
const OTHER_OBJECTS: [&str; 6] = [
    "target",
    "creature",
    "permanent",
    "token",
    "spell",
    "another",
];

/// "you may exile it" after "When ~ dies," is "you may exile ~": with no other object before it, "it" is the card.
fn self_it(text: &str) -> Option<String> {
    let mut changed = text.to_owned();
    for phrase in [" exile it", " sacrifice it", " from it"] {
        let Some(at) = changed.find(phrase) else {
            continue;
        };
        let end = at + phrase.len();
        let word_end = changed[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        let before = &changed[..at];
        let alone = !before.split(' ').any(|word| {
            let word = word.trim_matches(|c: char| !c.is_alphanumeric());
            OTHER_OBJECTS.contains(&word.to_ascii_lowercase().as_str())
        });
        if word_end && alone {
            let to = phrase.replace(" it", " ~");
            changed.replace_range(at..end, &to);
        }
    }
    (changed != text).then_some(changed)
}

/// "~'s owner shuffles it into their library" and "shuffles ~ into its owner's library" are one action, as are "Choose
/// target artifact. Its owner shuffles it into their library" and "Target artifact's owner shuffles it into their
/// library".
fn owner_shuffles(text: &str) -> Option<String> {
    let mut changed = text
        .replace(
            "~'s owner shuffles it into their library",
            "shuffle ~ into its owner's library",
        )
        .replace(
            "shuffles ~ into its owner's library",
            "shuffle ~ into its owner's library",
        );
    if let Some((head, tail)) = changed.split_once("Choose target ")
        && let Some((kind, rest)) = tail.split_once(". Its owner shuffles it into their library")
    {
        changed = format!("{head}Target {kind}'s owner shuffles it into their library{rest}");
    }
    (changed != text).then_some(changed)
}
