//! The choice of modes as the card words it: "choose one —", "choose two —", "choose one or both —", "choose up to one —"
//! and a bullet line for each mode, with the mode's name before its text when the card prints one ("Sort Inventory —
//! Draw a card."). Written from the data only.

use super::amounts::count_word;
use super::modal::{BULLET, DASH};
use super::render::{alternatives, capitalize};
use crate::effect::Mode;
use crate::refs::Bounds;
use crate::value::Amount;

/// How many modes are chosen, as the line before the bullets says it; `None` for a choice no card words.
fn header(choose: &Bounds, modes: usize) -> Option<String> {
    let most = match &choose.max {
        Some(Amount::Fixed(most)) => Some(u32::try_from(*most).ok()?),
        Some(_) => return None,
        None => None,
    };
    // A card never asks for more modes than it has.
    if most.is_some_and(|most| usize::try_from(most).map_or(true, |most| most > modes)) {
        return None;
    }
    let word = |number: u32| count_word(number);
    let words = match (choose.min, most) {
        (1, Some(1)) => "one".to_owned(),
        (1, Some(2)) if modes == 2 => "one or both".to_owned(),
        (1, None) => "one or more".to_owned(),
        (0, None) => "any number".to_owned(),
        (0, Some(most)) if most >= 1 => format!("up to {}", word(most)?),
        (min, Some(most)) if min == most && min >= 2 => word(min)?.to_owned(),
        _ => return None,
    };
    Some(format!("choose {words} {DASH}"))
}

/// The lines of one mode: its sentences capitalized, in each wording, and with its name in front when it has one. A name
/// is a prompt for the player, so the card may print the mode with or without it.
fn bullets(mode: &Mode) -> Vec<String> {
    let mut found = Vec::new();
    for sentences in alternatives(&mode.targets, &mode.effects) {
        let capitalized: Vec<String> = sentences.iter().map(|s| capitalize(s)).collect();
        let text = capitalized.join(". ");
        // The creature the event is about is "it" in the data and may be "that creature" on the card.
        if let Some(head) = text.strip_suffix(" on it") {
            found.push(format!("{head} on that creature"));
        }
        found.push(text);
    }
    if let Some(label) = &mode.label {
        let named: Vec<String> = found
            .iter()
            .map(|text| format!("{label} {DASH} {text}"))
            .collect();
        found.extend(named);
    }
    found
}

/// "choose one —" and a bullet line for each mode, in every wording of each mode; the last bullet has no period, which the
/// sentence that holds the text adds.
pub(super) fn texts(choose: &Bounds, modes: &[Mode]) -> Vec<String> {
    let Some(header) = header(choose, modes.len()) else {
        return Vec::new();
    };
    let mut texts = vec![header];
    for (at, mode) in modes.iter().enumerate() {
        let lines = bullets(mode);
        let stop = if at + 1 == modes.len() { "" } else { "." };
        texts = texts
            .iter()
            .flat_map(|so_far| {
                lines
                    .iter()
                    .map(move |line| format!("{so_far}\n{BULLET} {line}{stop}"))
            })
            .take(64)
            .collect();
    }
    texts
}
