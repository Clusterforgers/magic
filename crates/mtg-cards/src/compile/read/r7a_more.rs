//! More Oracle wordings of the same rules: "untap each other creature you control" for "untap all other creatures you
//! control", "that land's controller" and "that spell's controller" for the controller of a destroyed land or a countered
//! spell, "That creature" and "them" for the object the sentence before named, "can block only creatures with",
//! "can't be blocked except by", "A, B, then C" for "A, B, and C", and the damage of a count "to that player".

use super::subtypes::{is_subtype, plural_subtype, singular_subtype};

/// The verbs whose group of all objects Oracle text names one by one with "each".
const EACH_VERBS: [&str; 4] = ["untap all ", "Untap all ", "Return all ", "return all "];

/// The basic land types a destroyed land may be named by.
const LAND_TYPES: [&str; 6] = ["Plains", "Island", "Swamp", "Mountain", "Forest", "land"];

/// Every wording the rows give for one wording.
pub(super) fn rewrites(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(each_one(text));
    out.extend(counter_on_all(text));
    out.extend(controller_named(text));
    out.extend(named_again(text));
    out.extend(block_only(text));
    out.extend(blocked_except(text));
    out.extend(then_last(text));
    out.extend(damage_to_player(text));
    out
}

/// A noun in the singular when it is the plural of a card type, a subtype, "card" or "permanent".
fn one_of(word: &str) -> Option<String> {
    let known = |one: &str| {
        is_subtype(one)
            || [
                "artifact",
                "creature",
                "enchantment",
                "land",
                "planeswalker",
                "card",
                "permanent",
            ]
            .contains(&one)
    };
    let one = singular_subtype(word, known)?;
    (one != word || is_subtype(word)).then_some(one)
}

/// "untap all other creatures you control" is "untap each other creature you control": the group's first plural noun in
/// the singular, its other words kept.
fn each_one(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for verb in EACH_VERBS {
        let Some(at) = text.find(verb) else {
            continue;
        };
        let rest = &text[at + verb.len()..];
        let words: Vec<&str> = rest.split(' ').collect();
        let Some(noun_at) = words.iter().take(3).position(|word| one_of(word).is_some()) else {
            continue;
        };
        let Some(one) = one_of(words[noun_at]) else {
            continue;
        };
        let mut said: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        said[noun_at] = one;
        // One object at a time has one owner: "each creature ... to its owner's hand".
        let said = said
            .join(" ")
            .replacen(" to their owners' ", " to its owner's ", 1);
        out.push(format!(
            "{}{}each {said}",
            &text[..at],
            &verb[..verb.len() - "all ".len()],
        ));
    }
    out
}

/// "Put a manabond counter on each creature." is "... on all creatures.".
fn counter_on_all(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(" counter on each ")?;
    let (noun, tail) = rest.split_once('.')?;
    ["creature", "artifact", "land", "permanent"]
        .contains(&noun)
        .then(|| format!("{head} counter on all {}.{tail}", plural_subtype(noun)))
}

/// "Destroy target Mountain. ~ deals 3 damage to that permanent's controller." names the land "that land", and "Counter
/// target spell. ~ deals 2 damage to that permanent's controller." names the spell "that spell".
fn controller_named(text: &str) -> Option<String> {
    const OWNER: &str = " that permanent's controller";
    if !text.contains(OWNER) {
        return None;
    }
    let first = text.split(". ").next()?;
    let named = if let Some(kinds) = first.strip_prefix("Destroy target ") {
        kinds
            .split(" or ")
            .all(|kind| LAND_TYPES.contains(&kind))
            .then_some(" that land's controller")
    } else {
        (first == "Counter target spell" || first.starts_with("Counter target spell unless "))
            .then_some(" that spell's controller")
    }?;
    Some(text.replacen(OWNER, named, 1))
}

/// The object a sentence before named, named again: "Then it can't be blocked this turn." after one target creature is
/// "That creature can't be blocked this turn.", "Untap it." is "Untap that creature.", "Untap those creatures." is "Untap
/// them.", and "one of them" of the cards looked at is "one of those cards".
fn named_again(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(at) = text.find(". ") {
        let first = &text[..at];
        let one_target = first.matches("target ").count() == 1
            && first.contains("target creature")
            && !first.contains(" may ");
        let rest = &text[at + ". ".len()..];
        if one_target {
            for (written, printed) in [
                ("Then it can't ", "That creature can't "),
                ("Untap it.", "Untap that creature."),
            ] {
                if let Some(tail) = rest.strip_prefix(written) {
                    out.push(format!("{first}. {printed}{tail}"));
                }
            }
        }
    }
    if text.contains(". Untap those creatures.") {
        out.push(text.replacen(". Untap those creatures.", ". Untap them.", 1));
    }
    if text.contains("Look at the top ") && text.contains(". Put one of them ") {
        out.push(text.replacen(". Put one of them ", ". Put one of those cards ", 1));
    }
    out
}

/// "can't block creatures without flying" is "can block only creatures with flying".
fn block_only(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(" can't block creatures without ")?;
    let (keyword, tail) = rest.split_once('.')?;
    keyword
        .chars()
        .all(|c| c.is_ascii_lowercase())
        .then(|| format!("{head} can block only creatures with {keyword}.{tail}"))
}

/// "can't be blocked by creatures without flying" is "can't be blocked except by creatures with flying", "can't be
/// blocked by non-Wall creatures" is "... except by Walls", and "can't be blocked by nonartifact, nonwhite creatures" is
/// "... except by artifact creatures and/or white creatures".
fn blocked_except(text: &str) -> Option<String> {
    const BY: &str = " can't be blocked by ";
    let (head, rest) = text.split_once(BY)?;
    let (group, tail) = rest.split_once('.')?;
    let except = if let Some(keyword) = group.strip_prefix("creatures without ") {
        keyword
            .chars()
            .all(|c| c.is_ascii_lowercase())
            .then(|| format!("creatures with {keyword}"))?
    } else if let Some(kind) = group
        .strip_prefix("non-")
        .and_then(|rest| rest.strip_suffix(" creatures"))
    {
        (is_subtype(kind) && !kind.contains(' ')).then(|| plural_subtype(kind))?
    } else {
        let (first, second) = group.strip_suffix(" creatures")?.split_once(", ")?;
        let first = first.strip_prefix("non")?;
        let second = second.strip_prefix("non")?;
        let plain = |word: &str| !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase());
        (plain(first) && plain(second))
            .then(|| format!("{first} creatures and/or {second} creatures"))?
    };
    Some(format!("{head} can't be blocked except by {except}.{tail}"))
}

/// "You draw two cards, lose 2 life, and mill two cards." is "..., then mill two cards.": the steps of one sentence
/// happen in order either way.
fn then_last(text: &str) -> Option<String> {
    let body = text.strip_suffix('.')?;
    if body.contains(". ")
        || body.contains(" may ")
        || body.contains("If ")
        || body.contains(" for each ")
    {
        return None;
    }
    let (head, last) = body.rsplit_once(", and ")?;
    head.contains(", ").then(|| format!("{head}, then {last}."))
}

/// "~ deals damage to target player equal to the number of cards in that player's hand" is "~ deals damage equal to
/// the number of cards in target player's hand to that player".
fn damage_to_player(text: &str) -> Option<String> {
    const SAID: &str =
        " deals damage to target player equal to the number of cards in that player's hand";
    text.contains(SAID).then(|| {
        text.replacen(
            SAID,
            " deals damage equal to the number of cards in target player's hand to that player",
            1,
        )
    })
}
