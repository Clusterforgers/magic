//! Forms of w17 that mean the same to the rules: one verb that applies to two things ("Exile A, then exile B" and "Exile
//! A and B"), the costs of an ability in any order, exclusions in any order, and small forms ("it's", "-0/-2", "1 of
//! your opponents", "owners' hands", "sacrifice ~ unless ...").

use super::w16b_labels::ENDS;

/// The text with each of the forms below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = text.to_owned();
    for step in [
        repeat_verb,
        cost_order,
        non_list,
        non_pairs,
        adjectives,
        self_it,
        count_last,
        condition_last,
        qualifiers,
        there_is,
        all_counters,
        other_than,
        comma_and,
        each_of,
        small_forms,
        joined_damage,
        may_each,
    ] {
        if let Some(changed) = step(&text) {
            text = changed;
        }
    }
    text
}

/// Words that make a join of actions carry a meaning: a choice, a condition or a count between them.
const BLOCKERS: [&str; 9] = [
    "unless",
    " may ",
    " if ",
    "instead",
    "for each",
    "choose",
    "as long as",
    "each time",
    "where",
];

/// The verbs that repeat when one action applies to two things ("Exile A. Exile B.").
const REPEATED: [&str; 7] = [
    "put", "exile", "return", "destroy", "tap", "untap", "create",
];

/// The text before and after the first ": " when what comes before it is a cost.
fn split_cost(text: &str) -> (&str, &str) {
    match text.split_once(": ") {
        Some((cost, _)) if !cost.contains(['.', '"', '\u{2014}']) => text.split_at(cost.len() + 2),
        _ => ("", text),
    }
}

/// "Exile A, then exile B" and "Exile A. Exile B" as "Exile A and B": one verb that applies to two things.
fn repeat_verb(text: &str) -> Option<String> {
    let (cost, effect) = split_cost(text);
    let lower = effect.to_ascii_lowercase();
    if BLOCKERS.iter().any(|word| lower.contains(word)) || effect.contains('"') {
        return None;
    }
    let verb = lower.split(' ').next()?;
    if !REPEATED.contains(&verb) {
        return None;
    }
    let capital = format!("{}{} ", verb[..1].to_ascii_uppercase(), &verb[1..]);
    let mut joined = effect.to_owned();
    for from in [
        format!(", then {verb} "),
        format!(". {capital}"),
        format!(". {verb} "),
        format!(". Then {verb} "),
        format!(" and {verb} "),
    ] {
        joined = joined.replace(&from, " and ");
    }
    let joined = shared_zone(&joined);
    (joined != effect).then(|| format!("{cost}{joined}"))
}

/// "A from your graveyard to your hand and B from your graveyard to your hand" as "A and B from your graveyard to your
/// hand".
fn shared_zone(text: &str) -> String {
    let sentences: Vec<String> = text.split(". ").map(shared_zone_sentence).collect();
    sentences.join(". ")
}

/// [`shared_zone`] for one sentence.
fn shared_zone_sentence(text: &str) -> String {
    let body = text.trim_end_matches('.');
    let Some((left, right)) = body.rsplit_once(" and ") else {
        return text.to_owned();
    };
    let Some((_, zone)) = right.split_once(" from ") else {
        return text.to_owned();
    };
    let zone = format!(" from {zone}");
    match left.strip_suffix(zone.as_str()) {
        Some(left) => format!("{left} and {right}{}", &text[body.len()..]),
        None => text.to_owned(),
    }
}

/// The costs of an ability in one order: "{T}, Sacrifice ~" and "Sacrifice ~, {T}" pay the same, and a list of things to
/// sacrifice reads the same with commas or with "and".
fn cost_order(text: &str) -> Option<String> {
    let (cost, effect) = split_cost(text);
    if cost.is_empty() || !cost.contains(',') {
        return None;
    }
    let cost = cost
        .trim_end_matches(": ")
        .replace(", and a ", " and a ")
        .replace(", a ", " and a ");
    // The sort key is the part as the comparison reads it: the card as "~", the mana symbols in order.
    let key = |part: &str| super::tokens::symbols_sorted(&super::tokens::canonical(part));
    let mut parts: Vec<String> = cost
        .split(", ")
        .map(|part| match part.strip_prefix("Sacrifice ") {
            Some(list) => {
                let mut items: Vec<&str> = list.split(" and ").collect();
                items.sort_unstable_by_key(|item| key(item));
                format!("Sacrifice {}", items.join(" and "))
            }
            None => part.to_owned(),
        })
        .collect();
    parts.sort_unstable_by_key(|part| key(part));
    let changed = format!("{}: {effect}", parts.join(", "));
    (changed != text).then_some(changed)
}

/// The words that describe an object and have no order: "attacking non-Human creature" is "non-Human attacking creature".
const ADJECTIVES: [&str; 11] = [
    "attacking",
    "blocking",
    "tapped",
    "untapped",
    "goaded",
    "nontoken",
    "nonland",
    "noncreature",
    "nonartifact",
    "nonbasic",
    "legendary",
];

/// Whether the word is one of the adjectives, or a "non-Human" exclusion.
fn adjective(word: &str) -> bool {
    ADJECTIVES.contains(&word) || (word.starts_with("non-") && !word.ends_with(ENDS))
}

/// "a untapped nontoken artifact" and "a nontoken untapped artifact" are one text: the adjectives of a noun have no
/// order.
fn adjectives(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        let end = (at..words.len())
            .take_while(|place| adjective(&words[*place].to_ascii_lowercase()))
            .last()
            .map_or(at, |last| last + 1);
        if end - at >= 2 {
            let mut run = words[at..end].to_vec();
            run.sort_unstable_by_key(|word| word.to_ascii_lowercase());
            out.extend(run);
            at = end;
        } else {
            out.push(words[at]);
            at += 1;
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// "target noncreature artifact or noncreature enchantment" in the other order: the choices of an "or" have no order.
fn non_pairs(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        let pair = words.get(at..at + 5).filter(|w| {
            w[0].starts_with("non")
                && w[2] == "or"
                && w[3].starts_with("non")
                && !w[1].ends_with(ENDS)
        });
        if let Some(w) = pair {
            let (tail_word, tail) = split_end(w[4]);
            let mut first = [w[0].to_owned(), w[1].to_owned()].join(" ");
            let mut second = [w[3].to_owned(), tail_word.to_owned()].join(" ");
            if first > second {
                std::mem::swap(&mut first, &mut second);
            }
            out.push(format!("{first} or {second}{tail}"));
            at += 5;
        } else {
            out.push(words[at].to_owned());
            at += 1;
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// The text before the lists of w16 sort: a two-word keyword is one word ("first strike" is "firststrike") and "Red
/// spells and white spells" lists its colors in order.
pub(super) fn before_lists(text: &str) -> Option<String> {
    let mut changed = text.replace("first strike", "firststrike");
    changed = changed.replace("double strike", "doublestrike");
    let words: Vec<&str> = changed.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        let pair = words.get(at..at + 5).filter(|w| {
            let color = |word: &str| COLORS.contains(&word.to_ascii_lowercase().as_str());
            color(w[0]) && w[1] == "spells" && w[2] == "and" && color(w[3]) && w[4] == "spells"
        });
        if let Some(w) = pair {
            let mut colors = [w[0].to_ascii_lowercase(), w[3].to_ascii_lowercase()];
            colors.sort();
            let [first, second] = colors;
            let first = if w[0].starts_with(char::is_uppercase) {
                format!("{}{}", first[..1].to_ascii_uppercase(), &first[1..])
            } else {
                first
            };
            out.push(format!("{first} spells and {second} spells"));
            at += 5;
        } else {
            out.push(words[at].to_owned());
            at += 1;
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// The colors a spell may have.
const COLORS: [&str; 5] = ["white", "blue", "black", "red", "green"];

/// The phrases after a noun that restrict it and have no order.
const QUALIFIERS: [&str; 8] = [
    "on the battlefield",
    "of the chosen color",
    "without flying",
    "with flying",
    "with shadow",
    "you control",
    "your opponents control",
    "you don't control",
];

/// "creatures on the battlefield with shadow" as "creatures with shadow on the battlefield": the phrases that restrict a
/// noun have no order.
fn qualifiers(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let (mut run, closing, after) = qualifier_run(rest);
        if run.len() >= 2 {
            run.sort_unstable();
            out.push_str(&run.join(" "));
            out.push_str(closing);
            if !after.is_empty() {
                out.push(' ');
            }
            rest = after;
            continue;
        }
        // Copy one word and go on.
        let end = rest.find(' ').map_or(rest.len(), |at| at + 1);
        out.push_str(&rest[..end]);
        rest = &rest[end..];
    }
    (out != text).then_some(out)
}

/// The qualifier phrases that open `text`, the punctuation behind the last one, and the text after them.
fn qualifier_run(text: &str) -> (Vec<&'static str>, &str, &str) {
    let mut run: Vec<&'static str> = Vec::new();
    let mut rest = text;
    loop {
        let found = QUALIFIERS.iter().find_map(|phrase| {
            let after = rest.strip_prefix(phrase)?;
            let word_end = after.trim_start_matches(ENDS);
            let ends = after.len() - word_end.len();
            (word_end.is_empty() || word_end.starts_with(' ')).then_some((
                *phrase,
                &after[..ends],
                &after[ends..],
            ))
        });
        let Some((phrase, ends, after)) = found else {
            return (run, "", rest);
        };
        run.push(phrase);
        if !ends.is_empty() || after.is_empty() {
            return (run, ends, after.strip_prefix(' ').unwrap_or(after));
        }
        rest = after.strip_prefix(' ').unwrap_or(after);
    }
}

/// "a creature other than ~" as "another creature", "target creature you control other than ~" as "target other
/// creature you control", and "Another target creature" as "Target other creature": the object is any but the card.
fn other_than(text: &str) -> Option<String> {
    let mut words: Vec<String> = text.split(' ').map(str::to_owned).collect();
    let mut from = 0;
    while let Some(k) = (from..words.len().saturating_sub(2)).find(|k| {
        words[*k] == "other"
            && words[k + 1] == "than"
            // "~" may be the last word once the card's name is canonical; "this" needs its noun after it.
            && (words[k + 2].starts_with('~') || (words[k + 2] == "this" && k + 3 < words.len()))
    }) {
        from = k + 1;
        // The card is "~" or "this creature"; what ends the phrase stays.
        let span = if words[k + 2] == "this" { 4 } else { 3 };
        let tail = words[k + span - 1]
            .trim_start_matches(|c: char| c == '~' || c.is_alphabetic())
            .to_owned();
        // The phrase is the object itself: an article or "target", then adjectives, a noun and "you control". Words that
        // open another phrase ("on each creature") mean the exclusion belongs to something this cannot tell.
        let object = (k.saturating_sub(7)..k)
            .rev()
            .find(|place| matches!(words[*place].to_ascii_lowercase().as_str(), "a" | "target"));
        let Some(start) = object.filter(|start| {
            words[start + 1..k].iter().all(|word| {
                !matches!(
                    word.as_str(),
                    "on" | "from"
                        | "to"
                        | "of"
                        | "in"
                        | "with"
                        | "and"
                        | "or"
                        | "each"
                        | "all"
                        | "that"
                ) && !word.ends_with(',')
            })
        }) else {
            continue;
        };
        let mut head: Vec<String> = words[..start].to_vec();
        let article = words[start].eq_ignore_ascii_case("a");
        if article {
            head.push(
                if words[start] == "A" {
                    "Another"
                } else {
                    "another"
                }
                .to_owned(),
            );
        } else {
            head.push(words[start].clone());
            head.push("other".to_owned());
        }
        head.extend(words[start + 1..k].iter().cloned());
        if let Some(last) = head.last_mut() {
            last.push_str(&tail);
        }
        head.extend(words[k + span..].iter().cloned());
        words = head;
    }
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        if words[at].eq_ignore_ascii_case("another")
            && words.get(at + 1).is_some_and(|w| w == "target")
        {
            let first = if words[at] == "Another" {
                "Target"
            } else {
                "target"
            };
            out.extend([first.to_owned(), "other".to_owned()]);
            at += 2;
        } else {
            out.push(words[at].clone());
            at += 1;
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// "remove X +1/+1 counters from ~, where X is the number of +1/+1 counters on it" as "remove all +1/+1 counters from
/// ~": the count is every counter of the kind.
fn all_counters(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("emove X ")?;
    let (kind, rest) = rest.split_once(" counters from ")?;
    let (object, rest) = rest.split_once(", where X is the number of ")?;
    let counted = rest.strip_prefix(kind)?.strip_prefix(" counters on ")?;
    let after = counted
        .strip_prefix("it")
        .or_else(|| counted.strip_prefix('~'))?;
    Some(format!(
        "{head}emove all {kind} counters from {object}{after}"
    ))
}

/// "if 8 or more cards is in your graveyard" as "if there is 8 or more cards in your graveyard": a count of cards is
/// said with or without "there is".
fn there_is(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len() + 2);
    let mut at = 0;
    while at < words.len() {
        let counted = words.get(at) == Some(&"if")
            && words.get(at + 2) == Some(&"or")
            && words.get(at + 3) == Some(&"more");
        let verb = counted
            .then(|| (at + 4..(at + 10).min(words.len())).find(|place| words[*place] == "is"))
            .flatten()
            .filter(|place| words.get(place + 1) == Some(&"in"));
        if let Some(verb) = verb {
            out.extend(["if", "there", "is"]);
            out.extend(&words[at + 1..verb]);
            at = verb + 1;
        } else {
            out.push(words[at]);
            at += 1;
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// The words after a comma that go on a list of actions or choices.
const LIST_NEXT: [&str; 8] = [
    "has", "can", "can't", "gains", "loses", "gets", "have", "up",
];

/// "gets +2/+2, has haste, and can't block" as "gets +2/+2 and has haste and can't block", and the same for "up to 1
/// target A, up to 1 target B, and up to 1 target C": a list of what one thing does or what an action chooses.
fn comma_and(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    // "up to" continues a list only after an earlier "up to": "When ~ enters, up to one target ..." is no list.
    for (at, word) in words.iter().enumerate() {
        let next = words.get(at + 1).copied().unwrap_or_default();
        let next_after_and = words.get(at + 2).copied().unwrap_or_default();
        let seen_up = words[..at].windows(2).any(|pair| pair == ["up", "to"]);
        let listed = |word: &str| LIST_NEXT.contains(&word) && (word != "up" || seen_up);
        if *word == "and" && at > 0 && words[at - 1].ends_with(',') && listed(next) {
            out.push((*word).to_owned());
        } else if word.ends_with(',')
            && (listed(next) || (next == "and" && listed(next_after_and)))
            && !word.ends_with("\",")
        {
            // The comma joins like "and"; the "and" behind it stays.
            let core = word.trim_end_matches(',');
            out.push(core.to_owned());
            if next != "and" {
                out.push("and".to_owned());
            }
        } else {
            out.push((*word).to_owned());
        }
    }
    let changed = out.join(" ");
    let changed = changed.replace(" and and ", " and ");
    (changed != text).then_some(changed)
}

/// "each of those creatures" as "those creatures", and "each of any number of target permanents" as "any number of
/// target permanents": the action applies to every one anyway.
fn each_of(text: &str) -> Option<String> {
    let mut changed = text.to_owned();
    for next in ["those ", "any number of ", "up to "] {
        changed = changed.replace(&format!("each of {next}"), next);
        changed = changed.replace(
            &format!("Each of {next}"),
            &super::w17_wordings::capital_of(next),
        );
    }
    (changed != text).then_some(changed)
}

/// The verbs that start the second half of "gets +1/+1 for each X and gains trample".
const SECOND_HALF: [&str; 4] = [" and gains ", " and has ", " and loses ", " and can't "];

/// "gets +1/+1 for each X and gains trample" as "gets +1/+1 and gains trample for each X": the count applies to the stat
/// either way, and is said last.
fn count_last(text: &str) -> Option<String> {
    let start = [" for each ", " for all "]
        .iter()
        .find_map(|count| text.find(count))?;
    let rest = &text[start..];
    let (phrase, half) = SECOND_HALF.iter().find_map(|verb| {
        let at = rest.find(verb)?;
        Some((&rest[..at], &rest[at..]))
    })?;
    // The second half runs to the end of the sentence: a count there is not this phrase's.
    if half.contains(" for each ") || half.contains(" for all ") {
        return None;
    }
    let tail_at = half
        .find(" [")
        .unwrap_or_else(|| half.trim_end_matches('.').len());
    let (clause, tail) = half.split_at(tail_at);
    Some(format!("{}{clause}{phrase}{tail}", &text[..start]))
}

/// "As long as X, Y" as "Y as long as X": a condition says the same first or last.
fn condition_last(text: &str) -> Option<String> {
    let rest = text.strip_prefix("As long as ")?;
    let (condition, effect) = rest.split_once(", ")?;
    if effect.contains(". ") || condition.contains(',') {
        return None;
    }
    let (effect, tail) = match effect.strip_suffix('.') {
        Some(effect) => (effect, "."),
        None => (effect, ""),
    };
    Some(format!("{effect} as long as {condition}{tail}"))
}

/// The word without the characters that end it, and those characters.
fn split_end(word: &str) -> (&str, &str) {
    let core = word.trim_end_matches(ENDS);
    (core, &word[core.len()..])
}

/// "target nonartifact, nonblack creature" and "target nonblack, nonartifact creature": the exclusions have no order.
fn non_list(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        let mut end = at;
        while words
            .get(end)
            .is_some_and(|word| word.starts_with("non") && word.ends_with(','))
        {
            end += 1;
        }
        if end > at && words.get(end).is_some_and(|word| word.starts_with("non")) {
            let mut list: Vec<&str> = words[at..=end]
                .iter()
                .map(|word| split_end(word).0)
                .collect();
            list.sort_unstable();
            let tail = split_end(words[end]).1;
            let final_place = list.len() - 1;
            for (place, word) in list.into_iter().enumerate() {
                out.push(if place == final_place {
                    format!("{word}{tail}")
                } else {
                    format!("{word},")
                });
            }
            at = end + 1;
        } else {
            out.push(words[at].to_owned());
            at += 1;
        }
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// "~ deals 5 damage to A and 2 damage to B" as "~ deals 5 damage to A. ~ deals 2 damage to B", and "A, then ~ deals ..."
/// as "A and ~ deals ...": a source that deals damage twice, or one action after another, in order.
fn joined_damage(text: &str) -> Option<String> {
    let mut changed = text.to_owned();
    if changed.starts_with("~ deals ") {
        let digit = changed
            .match_indices(" and ")
            .find(|(at, _)| {
                changed[at + 5..]
                    .split(' ')
                    .next()
                    .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit() || c == 'X'))
                    && changed[at + 5..].contains(" damage to ")
            })
            .map(|(at, _)| at);
        if let Some(at) = digit {
            changed.replace_range(at..at + 5, ". ~ deals ");
        }
    }
    let lower = changed.to_ascii_lowercase();
    // An order ("Counter target spell and ~ deals 3 damage") is two sentences, as the other side writes them.
    let order = lower
        .split(' ')
        .next()
        .is_some_and(|first| first == "counter");
    if !BLOCKERS.iter().any(|word| lower.contains(word)) {
        changed = changed.replace(", then ~ ", ". ~ ");
        if order {
            changed = changed.replace(" and ~ deals ", ". ~ deals ");
        }
    }
    for from in [
        ". it can't ",
        ". Then it can't ",
        " then it can't ",
        " and it can't ",
    ] {
        changed = changed.replace(from, " and can't ");
    }
    if changed.contains("Search your library for") {
        changed = changed.replace(
            ", put it onto the battlefield",
            ". put it onto the battlefield",
        );
    }
    (changed != text).then_some(changed)
}

/// "you may each Ally you control gains flying" as "you may have Allies you control gain flying": the choice stays, and
/// "have" names the creatures of one type.
fn may_each(text: &str) -> Option<String> {
    let (head, tail) = text.split_once("you may each ")?;
    let (kind, rest) = tail.split_once(' ')?;
    if !kind.starts_with(char::is_uppercase) || !kind.chars().all(char::is_alphabetic) {
        return None;
    }
    let plural = match kind.strip_suffix('y') {
        Some(stem) => format!("{stem}ies"),
        None => format!("{kind}s"),
    };
    Some(format!("{head}you may have {plural} {rest}"))
}

/// Words that name an object a pronoun could mean in place of the card.
const OBJECTS_IT_MAY_MEAN: [&str; 13] = [
    "creature",
    "permanent",
    "card",
    "target",
    "player",
    "opponent",
    "land",
    "artifact",
    "token",
    "spell",
    "enchantment",
    "that",
    "another",
];

/// "As long as ... on ~, it has flying" as "..., ~ has flying": the only object the condition names is the card.
fn self_it(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(", it ")?;
    // The card is "~" whether the text says "this creature" or "~".
    let head = super::tokens::canonical(head);
    let lower = head.to_ascii_lowercase();
    let names_other = lower
        .split(' ')
        .any(|word| OBJECTS_IT_MAY_MEAN.contains(&word.trim_end_matches(ENDS)));
    let own = head.contains('~');
    (own && !names_other).then(|| format!("{head}, ~ {tail}"))
}

/// The spellings of one meaning, as (written, same).
const SPELLINGS: [(&str, &str); 9] = [
    ("it's ", "it is "),
    ("there's ", "there is "),
    ("1 of your opponents", "a opponent"),
    ("their owners' hands", "its owner's hand"),
    ("their owner's hand", "its owner's hand"),
    ("owners' hands", "owner's hand"),
    ("owners' hand", "owner's hand"),
    ("-0/", "+0/"),
    ("/-0", "/+0"),
];

/// Small forms of one meaning: "it's" is "it is", "-0/-2" is "+0/-2", "1 of your opponents" is "an opponent", "owners'
/// hands" is "owner's hand", and "if X wasn't spent, sacrifice ~" is "sacrifice ~ unless X was spent".
fn small_forms(text: &str) -> Option<String> {
    let mut changed = text.to_owned();
    for (from, to) in SPELLINGS {
        changed = changed.replace(from, to);
    }
    let changed = unless_spent(&changed).unwrap_or(changed);
    let changed = attach_dread(&changed).unwrap_or(changed);
    (changed != text).then_some(changed)
}

/// "manifest dread, then attach this Equipment to that creature" as "manifest dread. Attach ~ to it": the card goes onto
/// the manifested creature.
fn attach_dread(text: &str) -> Option<String> {
    let (head, tail) = text.split_once("anifest dread")?;
    let tail = [", then attach ", ". Then attach ", ". Attach ", ". attach "]
        .iter()
        .find_map(|start| tail.strip_prefix(start))?;
    let (card, after) = tail
        .split_once(" to that creature")
        .or_else(|| tail.split_once(" to it"))?;
    let own = matches!(card, "~" | "it")
        || card
            .strip_prefix("this ")
            .is_some_and(|noun| !noun.contains(' '));
    own.then(|| format!("{head}anifest dread. Attach ~ to it{after}"))
}

/// "..., if {W} wasn't spent to cast it, sacrifice ~." as "..., sacrifice ~ unless {W} was spent to cast it.".
fn unless_spent(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(", if ")?;
    let (condition, action) = rest.split_once(", sacrifice ")?;
    let (spent, cast) = condition.split_once(" wasn't spent ")?;
    let action = action.trim_end_matches('.');
    Some(format!(
        "{head}, sacrifice {action} unless {spent} was spent {cast}."
    ))
}
