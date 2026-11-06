//! Printed trigger words a Forge script leaves out: a printed "you may" over the one step of a trigger whose script has
//! no decider (the step becomes optional), and a printed "target creature you control" whose script targets any
//! creature, or a printed "it deals" of another object whose script lets the card deal the damage (gaps: the script and
//! the card disagree).

use serde_json::{Value, json};

use super::ctx::{Tr, unmapped};

/// The effects of a trigger with its printed words applied: a printed "you may" over the one step asks unless the
/// script has a decider or a cost to pay (`(decider, pays)`); a printed "you may search" without a decider, a printed
/// own target left open and a printed "it deals" from the card are gaps.
pub fn printed(
    description: Option<&str>,
    (decider, pays): (bool, bool),
    itself: bool,
    effects: Vec<Value>,
    targets: &[Value],
) -> Tr<Vec<Value>> {
    // "you may search" printed with no decider in the script: the search would be forced.
    if !decider
        && description.is_some_and(|text| text.contains("you may search"))
        && !json!(effects).to_string().contains("\"May\":")
    {
        return unmapped("trigger:may-search-without-decider");
    }
    if unfiltered_own_target(description, targets) {
        return unmapped("trigger:printed-own-target-unfiltered");
    }
    if !itself && other_deals_from_host(description, &effects) {
        return unmapped("trigger:printed-it-deals-from-host");
    }
    Ok(
        if !decider && !pays && printed_may_step(description, &effects) {
            super::frame::ask(effects, &json!("You"))
        } else {
            effects
        },
    )
}

/// The verbs whose printed "you may" the trigger keeps when the script forgets its decider.
const OPTIONAL_VERBS: [&str; 4] = ["put", "remove", "tap", "untap"];

/// The description without reminder text.
fn without_reminder(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Whether the printed effect is one optional step ("..., you may put a +1/+1 counter on ~.") and the translation is
/// that one step without a choice.
fn printed_may_step(description: Option<&str>, effects: &[Value]) -> bool {
    let Some(text) = description.map(without_reminder) else {
        return false;
    };
    let Some((head, step)) = text.split_once(", you may ") else {
        return false;
    };
    let one_sentence = step
        .trim_end()
        .trim_end_matches('.')
        .contains(". ")
        .eq(&false);
    let verb = step.split_whitespace().next().unwrap_or_default();
    effects.len() == 1
        && one_sentence
        && !head.contains("may")
        && !head.contains(". ")
        && OPTIONAL_VERBS.contains(&verb)
        && !step.starts_with("tap or untap")
        && !serde_json::to_string(effects)
            .unwrap_or_default()
            .contains("\"May\":")
}

/// Whether the card prints "target creature you control" while no target of the translation is narrowed by controller.
fn unfiltered_own_target(description: Option<&str>, targets: &[Value]) -> bool {
    let Some(text) = description.map(without_reminder) else {
        return false;
    };
    text.contains("target creature you control")
        && !text.contains('"')
        && !targets.is_empty()
        && !serde_json::to_string(targets)
            .unwrap_or_default()
            .contains("ControlledBy")
}

/// Whether a trigger about another object prints "it deals" (the object of the event deals the damage) while the
/// script's damage has no source, so the card itself would deal it.
fn other_deals_from_host(description: Option<&str>, effects: &[Value]) -> bool {
    let Some(text) = description.map(without_reminder) else {
        return false;
    };
    let json = serde_json::to_string(effects).unwrap_or_default();
    // "Whenever CARDNAME is dealt damage, it deals ...": "it" is the card itself.
    let Some((head, step)) = text.split_once(", ") else {
        return false;
    };
    let about_the_card = ["CARDNAME", "this ", "This ", "~"]
        .iter()
        .any(|word| head.contains(word));
    !about_the_card
        && step.starts_with("it deals ")
        && json.contains("\"DealDamage\":")
        && !json.contains("\"source\":")
}
