//! Phrases the renderers write one way and the cards another, each with the shape of the rules it applies to; each swap
//! also edits the wordings the swaps before it made.

use super::c5_af_json;
use crate::ability::Ability;

/// What the rules must hold for a swap to apply.
type C5AfGuard = fn(&str) -> bool;

/// A rework of a whole wording into the card's; `None` when the wording is not the one it reworks.
type C5AfRework = fn(&str) -> Option<String>;

/// The phrase swaps: where they apply, the renderers' words and the card's.
const C5_AF_PHRASES: [(C5AfGuard, &str, &str); 10] = [
    (
        c5_af_counter_then_damage,
        ". Then ~ deals ",
        " and it deals ",
    ),
    (
        c5_af_counter_on_dealer,
        " counter on it",
        " counter on that creature",
    ),
    (
        c5_af_if_tapped,
        " if it's a tapped permanent",
        " if it's tapped",
    ),
    (
        c5_af_clue_per_opponent,
        "you investigate for each opponent you have",
        "investigate X times, where X is the number of opponents you have",
    ),
    (
        c5_af_divided_x,
        " among up to X target ",
        " among any number of target ",
    ),
    (c5_af_counters_on_groups, ". Then put ", " and "),
    (c5_af_other, " target other ", " other target "),
    (
        c5_af_opponents_discard,
        "Each other player discards ",
        "Each of your opponents discards ",
    ),
    (c5_af_opponents_discard, ". Then put ", ". Put "),
    (
        c5_af_regenerates_after_counters,
        ": Regenerate ~.",
        ": Regenerate it.",
    ),
];

/// The swaps that rework a whole wording: where they apply and the rework.
const C5_AF_REWORDS: [(C5AfGuard, C5AfRework); 5] = [
    (c5_af_grants_to_two_kinds, c5_af_each_and),
    (c5_af_sacrificed_power, c5_af_where_sacrificed),
    (c5_af_prevents_noncombat, c5_af_prevent_all),
    (c5_af_enters_with_sum, c5_af_equal_to),
    (c5_af_returns_itself, c5_af_when),
];

fn c5_af_counter_then_damage(rules: &str) -> bool {
    rules.contains(r#""PutCounters":{"on":"This""#)
        && rules.contains(r#""DealDamage":{"to":"Opponents""#)
}

fn c5_af_counter_on_dealer(rules: &str) -> bool {
    rules.contains(r#""PutCounters":{"on":"Dealer""#)
}

fn c5_af_if_tapped(rules: &str) -> bool {
    rules.contains(r#""selector":["Tapped"]"#)
}

fn c5_af_clue_per_opponent(rules: &str) -> bool {
    rules.contains(r#"{"Named":"clue"},"count":{"Players":"Opponents"}"#)
}

fn c5_af_divided_x(rules: &str) -> bool {
    rules.contains(r#""max":"X"},"divide":"X""#)
}

fn c5_af_counters_on_groups(rules: &str) -> bool {
    rules.matches(r#""PutCounters":{"on":{"Each""#).count() >= 2
}

fn c5_af_other(rules: &str) -> bool {
    rules.contains(r#""Other""#)
}

fn c5_af_opponents_discard(rules: &str) -> bool {
    rules.contains(r#""Discard":{"who":"Opponents"}"#)
}

fn c5_af_regenerates_after_counters(rules: &str) -> bool {
    rules.contains(r#"{"RemoveCounters":"#) && rules.contains(r#"{"Regenerate":{"what":"This"}}"#)
}

fn c5_af_grants_to_two_kinds(rules: &str) -> bool {
    rules.contains(r#""Static":{"affected":{"Each":[{"AnyOf":[{"Subtype""#)
        && rules.contains(r#""GrantAbility""#)
}

fn c5_af_sacrificed_power(rules: &str) -> bool {
    rules.contains(r#"{"Sacrifice":{"filter""#)
        && rules.contains(r#"{"Stat":{"of":"Paid","stat":"Power"}}"#)
}

fn c5_af_prevents_noncombat(rules: &str) -> bool {
    rules.contains(r#""combat":false}},"by":"Prevent""#)
}

fn c5_af_enters_with_sum(rules: &str) -> bool {
    rules.contains(r#""WithCounters""#) && rules.contains(r#""count":{"Sum""#)
}

fn c5_af_returns_itself(rules: &str) -> bool {
    rules.contains(r#"{"Move":{"what":"This","to":"Hand"}}"#)
        && !rules.contains(r#""what":["This"]"#)
}

/// "Each Fungus or Saproling you control has "{T}: Add {G}."." as "Each Fungus and Saproling you control has ...".
fn c5_af_each_and(text: &str) -> Option<String> {
    let rest = text.strip_prefix("Each ")?;
    let (kinds, tail) = rest.split_once(" you control has ")?;
    let (first, second) = kinds.split_once(" or ")?;
    let tail = tail
        .strip_suffix(".\".")
        .map_or_else(|| tail.to_owned(), |quote| format!("{quote}.\""));
    Some(format!("Each {first} and {second} you control has {tail}"))
}

/// "You gain X life. Then put a number of +1/+1 counters on target creature you control equal to the revealed card's
/// power." as the gain and the counters by X, "where X is the sacrificed creature's power".
fn c5_af_where_sacrificed(text: &str) -> Option<String> {
    let (cost, rest) = text.split_once(": You gain X life. Then put a number of ")?;
    let what = rest.strip_suffix(" equal to the revealed card's power.")?;
    Some(format!(
        "{cost}: You gain X life and put X {what}, where X is the sacrificed creature's power."
    ))
}

/// "If noncombat damage would be dealt to a Spirit you control, prevent that damage." as "Prevent all noncombat damage
/// that would be dealt to Spirits you control."
fn c5_af_prevent_all(text: &str) -> Option<String> {
    let rest = text.strip_prefix("If noncombat damage would be dealt to a ")?;
    let (kind, group) = rest
        .strip_suffix(", prevent that damage.")?
        .split_once(' ')?;
    let kinds = match kind.strip_suffix('y') {
        Some(stem) => format!("{stem}ies"),
        None => format!("{kind}s"),
    };
    Some(format!(
        "Prevent all noncombat damage that would be dealt to {kinds} {group}."
    ))
}

/// "~ enters with X +1/+1 counters on it, where X is one plus ..." as "... with a number of +1/+1 counters on it equal to
/// one plus ...".
fn c5_af_equal_to(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(" enters with X ")?;
    let (counters, count) = rest.split_once(" on it, where X is ")?;
    Some(format!(
        "{head} enters with a number of {counters} on it equal to {count}"
    ))
}

/// "Whenever a Dragon you control enters, return ~ to its owner's hand." as "When ...": the card triggers once.
fn c5_af_when(text: &str) -> Option<String> {
    text.strip_prefix("Whenever ")
        .map(|rest| format!("When {rest}"))
}

/// Adds the wordings with each swap that applies, one swap at a time.
pub(super) fn c5_af_add(ability: &Ability, texts: &mut Vec<String>) {
    let rules = c5_af_json(ability);
    for (applies, from, to) in C5_AF_PHRASES {
        if applies(&rules) {
            let found: Vec<String> = texts
                .iter()
                .filter(|text| text.contains(from))
                .map(|text| text.replacen(from, to, 1))
                .collect();
            texts.extend(found);
        }
    }
    for (applies, rework) in C5_AF_REWORDS {
        if applies(&rules) {
            let found: Vec<String> = texts.iter().filter_map(|text| rework(text)).collect();
            texts.extend(found);
        }
    }
}
