//! Wordings of one meaning that the oracle text and the writers say differently (w15 win-back of `Differs` clauses):
//! "also deals", a toll counted as "{X}, where X is the number of", "any number" of sacrificed creatures, the other card
//! of two looked at, the top card revealed and sorted by "If it's a ..., Otherwise", a pair of types in a count, the names
//! of quoted abilities, support, blight and adamant. Both sides of a comparison are
//! normalized, so a rule of another meaning still differs.

/// The text with each of the wordings below written one way.
pub(super) fn normal(text: &str) -> String {
    let mut text = text.replace(" also deals ", " deals ");
    for step in [
        pays_each,
        sacrifice_any_number,
        the_other_card,
        reveal_one,
        divided_among,
        attached_pair,
        flavor_names,
        blocks_a_permanent,
        same_words,
        costs_in_order,
        lesser_value,
        counter_actions,
        adamant,
        small_words,
        for_each_count,
        either_kind,
        there_is_one,
        there_is_an,
        look_and_may_reveal,
    ] {
        if let Some(changed) = step(&text) {
            text = changed;
        }
    }
    text
}

/// The words that end the noun a count is of ("the number of cards in your hand").
const NOUN_ENDS: [&str; 9] = [
    " among ", " in ", " you ", " you've ", " your ", " that ", " on ", " with ", " of ",
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
    const LOOK: &str =
        "ook at the top two cards of your library. Put one of them into your hand and the ";
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
    let (kept, rest) = rest
        .split_once(". Otherwise, put it ")
        .or_else(|| rest.split_once(". Otherwise, put that card "))?;
    Some(format!(
        ". Then put all {kind} cards revealed this way {kept} and the rest {rest}"
    ))
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

/// Words of one meaning: a ticket counter is {TK}, a player who is not the controller is an opponent, and the combat step
/// of your turn is the beginning of combat on your turn.
fn same_words(text: &str) -> Option<String> {
    const PAIRS: [(&str, &str); 5] = [
        (
            "cards with the same name as that creature in all graveyards",
            "cards in all graveyards with the same name as that spell",
        ),
        ("{TK}", "a ticket counter"),
        ("each other player", "each opponent"),
        (
            "beginning of your combat step",
            "beginning of combat on your turn",
        ),
        ("~ enter,", "~ enters,"),
    ];
    let mut changed = text.to_owned();
    for (from, to) in PAIRS {
        changed = changed.replace(from, to);
    }
    (changed != text).then_some(changed)
}

/// "As an additional cost to cast this spell, pay {4} or sacrifice an artifact or creature." as "..., sacrifice an artifact
/// or creature or pay {4}.": the two costs are alternatives in either order.
fn costs_in_order(text: &str) -> Option<String> {
    const COST: &str = "As an additional cost to cast ";
    let at = text.find(COST)?;
    let comma = text[at..].find(", ")? + at + 2;
    let rest = text[comma..].strip_prefix("pay {")?;
    let close = rest.find("} or sacrifice ")?;
    let mana = &rest[..close];
    let after = &rest[close + "} or sacrifice ".len()..];
    let end = after.find('.').unwrap_or(after.len());
    Some(format!(
        "{}sacrifice {} or pay {{{mana}}}{}",
        &text[..comma],
        &after[..end],
        &after[end..]
    ))
}

/// The number as the word the renderers write, for one to nine.
fn number_word(digit: &str) -> Option<&'static str> {
    [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ]
    .get(digit.parse::<usize>().ok()?.checked_sub(1)?)
    .copied()
}

/// The keyword actions "support 2" and "blight 1" as the counters they put: "put a +1/+1 counter on each of up to two
/// target other creatures" and "put a -1/-1 counter on a creature you control".
fn counter_actions(text: &str) -> Option<String> {
    let mut changed = text.to_owned();
    for (keyword, make) in [
        ("support ", true),
        ("Support ", true),
        ("blight ", false),
        ("Blight ", false),
    ] {
        let Some(at) = changed.find(keyword) else {
            continue;
        };
        let digit = &changed[at + keyword.len()..];
        let digit = &digit[..digit
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(digit.len())];
        let Some(word) = number_word(digit) else {
            continue;
        };
        let words = if make {
            format!("put a +1/+1 counter on each of up to {word} target other creatures")
        } else if word == "one" {
            "put a -1/-1 counter on a creature you control".to_owned()
        } else {
            format!("put {word} -1/-1 counters on a creature you control")
        };
        let words = if keyword.starts_with('S') || keyword.starts_with('B') {
            let mut chars = words.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        } else {
            words
        };
        changed = format!(
            "{}{words}{}",
            &changed[..at],
            &changed[at + keyword.len() + digit.len()..]
        );
    }
    (changed != text).then_some(changed)
}

/// "If at least three white mana was spent to cast this spell, this creature enters with a +1/+1 counter on it." as "~
/// enters that has a +1/+1 counter on it if {W}{W}{W} was spent to cast it."
fn adamant(text: &str) -> Option<String> {
    const IF: &str = "If at least three ";
    let at = text.find(IF)?;
    let after = at + IF.len();
    let (color, rest) = text[after..].split_once(" mana was spent to cast ")?;
    let symbol = match color {
        "white" => 'W',
        "blue" => 'U',
        "black" => 'B',
        "red" => 'R',
        "green" => 'G',
        _ => return None,
    };
    let (_, rest) = rest.split_once(", ")?;
    let (_, rest) = rest.split_once(" enters with ")?;
    let end = rest.find('.').unwrap_or(rest.len());
    Some(format!(
        "{}~ enters that has {} if {{{symbol}}}{{{symbol}}}{{{symbol}}} was spent to cast it{}",
        &text[..at],
        &rest[..end],
        &rest[end..]
    ))
}

/// The wordings of a cost and a choice of two: "Remove two counters from ~ and sacrifice it:" as ", Sacrifice it:", "you
/// create" as "create", and "create a Food token or create a Treasure token" as "create a Food token or a Treasure token".
fn small_words(text: &str) -> Option<String> {
    let changed = text
        .replace(" and sacrifice it:", ", Sacrifice it:")
        .replace(", you create ", ", create ")
        .replace(" token or create a ", " token or a ");
    (changed != text).then_some(changed)
}

/// "lesser mana value than the creature that died" as "mana value less than that creature's mana value".
fn lesser_value(text: &str) -> Option<String> {
    const THAN: &str = " with lesser mana value than the creature that died";
    const LESS: &str = " with mana value less than that creature's mana value";
    let changed = text
        .replace(THAN, LESS)
        .replace(" with lesser mana value", LESS);
    (changed != text).then_some(changed)
}

/// "gets +1/+0 for each card you've drawn this turn" as "gets +X/+0, where X is the number of cards you've drawn this
/// turn": the number counted once for each.
fn for_each_count(text: &str) -> Option<String> {
    const WHERE: &str = ", where X is the number of ";
    const SHAPES: [(&str, &str); 3] = [("+X/+0", "+1/+0"), ("+0/+X", "+0/+1"), ("+X/+X", "+1/+1")];
    for (counted, each) in SHAPES {
        let head = format!("{counted}{WHERE}");
        let Some(at) = text.find(&head) else {
            continue;
        };
        let after = at + head.len();
        let end = text[after..]
            .find(['.', ',', '"', '\n'])
            .map_or(text.len(), |end| end + after);
        let things = &text[after..end];
        let noun_end = NOUN_ENDS
            .iter()
            .filter_map(|word| things.find(word))
            .min()
            .unwrap_or(things.len());
        return Some(format!(
            "{}{each} for each {}{}{}",
            &text[..at],
            singular(&things[..noun_end]),
            &things[noun_end..],
            &text[end..]
        ));
    }
    None
}

/// "or" and "and/or" between two kinds said one way, in the order of their names: "an artifact or enchantment card"
/// and "an enchantment and/or artifact card" choose one card of either kind.
fn either_kind(text: &str) -> Option<String> {
    const NOUNS: [&str; 6] = [
        "card",
        "cards",
        "creature",
        "creatures",
        "permanent",
        "permanents",
    ];
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    let mut changed = false;
    while at < words.len() {
        // A kind follows "a", "an" or "target": "this creature or another creature" is no pair of kinds.
        if let [first, joint, second, noun] = words[at..].get(..4).unwrap_or_default()
            && matches!(*joint, "or" | "and/or")
            && NOUNS.contains(noun)
            && at
                .checked_sub(1)
                .and_then(|before| words.get(before))
                .is_some_and(|before| matches!(*before, "a" | "an" | "target"))
            && ![first, second].iter().any(|word| {
                matches!(
                    **word,
                    "another" | "other" | "this" | "that" | "equipped" | "enchanted"
                )
            })
            && ![first, second]
                .iter()
                .any(|word| word.contains(['.', ',', ':', '"']))
        {
            let (low, high) = if first.to_lowercase() <= second.to_lowercase() {
                (first, second)
            } else {
                (second, first)
            };
            out.extend([(*low).to_owned(), "or".to_owned(), (*high).to_owned()]);
            changed = true;
            at += 3;
        } else {
            out.push(words[at].to_owned());
            at += 1;
        }
    }
    changed.then(|| out.join(" "))
}

/// "blocks a permanent" as "blocks a creature": only a creature can be blocked.
fn blocks_a_permanent(text: &str) -> Option<String> {
    text.contains(" blocks a permanent")
        .then(|| text.replace(" blocks a permanent", " blocks a creature"))
}

/// A count of one or more as "there's a": "if there are 1 or more Lesson cards in your graveyard" as "if there's a
/// Lesson card in your graveyard".
fn there_is_one(text: &str) -> Option<String> {
    const MANY: &str = "there are 1 or more ";
    let at = text.find(MANY)?;
    let after = at + MANY.len();
    let end = [" in ", " among ", " you control", " on "]
        .iter()
        .filter_map(|word| text[after..].find(word))
        .min()?
        + after;
    Some(format!(
        "{}there's a {}{}",
        &text[..at],
        singular(&text[after..end]),
        &text[end..]
    ))
}

/// "there's an Aura card" as "there's a Aura card": the article follows the vowel, not the rules.
fn there_is_an(text: &str) -> Option<String> {
    text.contains("there's an ")
        .then(|| text.replace("there's an ", "there's a "))
}

/// "look at the top four cards of your library and may reveal ..." as "... library. You may reveal ...", and the rest of
/// those cards put back "on the bottom" as "on the bottom of your library".
fn look_and_may_reveal(text: &str) -> Option<String> {
    let mut changed = text.to_owned();
    if let Some(at) = changed.find(" cards of your library and may reveal ") {
        let at = at + " cards of your library".len();
        changed = format!(
            "{}. You may reveal {}",
            &changed[..at],
            &changed[at + " and may reveal ".len()..]
        );
    }
    changed = changed
        .replace(
            ". Then put the rest of those cards on the bottom in a random order",
            ". Put the rest on the bottom of your library in a random order",
        )
        .replace("noncreature, nonland card", "noncreature nonland card");
    (changed != text).then_some(changed)
}

/// The text without the names of quoted abilities ("gains "Landfall — Whenever ..."") that carry no rules: an ability
/// word is flavor. The names that open a mode stay: a mode name is part of the printed text.
fn flavor_names(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut changed = false;
    while let Some(at) = rest.find('"') {
        let after = at + 1;
        out.push_str(&rest[..after]);
        rest = &rest[after..];
        if let Some(len) = super::super::classify::ability_word_len(rest) {
            rest = &rest[len..];
            changed = true;
        }
    }
    out.push_str(rest);
    changed.then_some(out)
}
