//! The choice of modes as the card words it: "choose one —", "choose two —", "choose one or both —", "choose up to one —"
//! and a bullet line for each mode, with the mode's name before its text when the card prints one ("Sort Inventory —
//! Draw a card."). Written from the data only.

use super::amounts::count_word;
use super::modal::{BULLET, DASH};
use super::render::{alternatives, capitalize};
use super::{statics, where_x};
use crate::ability::Ability;
use crate::effect::Mode;
use crate::refs::Bounds;
use crate::value::{Amount, Condition};

/// How many modes are chosen, as the line before the bullets says it; `None` for a choice no card words.
fn header(choose: &Bounds, modes: usize, repeat: bool) -> Option<String> {
    let most = match &choose.max {
        Some(Amount::Fixed(most)) => Some(u32::try_from(*most).ok()?),
        Some(_) => return None,
        None => None,
    };
    // A card never asks for more modes than it has, unless a mode may repeat.
    if !repeat && most.is_some_and(|most| usize::try_from(most).map_or(true, |most| most > modes)) {
        return None;
    }
    let word = |number: u32| count_word(number);
    let words = match (choose.fixed_min()?, most) {
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

/// "choose one. If you control a commander as you cast this spell, you may choose both instead." for a spell and the same
/// without "as you cast this spell" for a trigger, in each wording of the condition.
fn both_instead(condition: &Condition) -> Vec<String> {
    statics::control_texts(condition)
        .iter()
        .flat_map(|held| {
            [
                format!(
                    "choose one. If {held} as you cast this spell, you may choose both instead."
                ),
                format!("choose one. If {held}, you may choose both instead."),
            ]
        })
        .collect()
}

/// The lines of a mode with one count or characteristic as its X, named at the end of its sentence ("Put X +1/+1
/// counters on target creature, where X is the number of lands you control."), without the last period.
fn c11d_where_x(mode: &Mode) -> Vec<String> {
    let spell = Ability::Spell {
        targets: mode.targets.clone(),
        effects: mode.effects.clone(),
    };
    where_x::wordings(&spell)
        .into_iter()
        .map(|text| capitalize(text.strip_suffix('.').unwrap_or(&text)))
        .collect()
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
    found.extend(c11d_where_x(mode));
    if let Some(label) = &mode.label {
        let mut named: Vec<String> = found
            .iter()
            .map(|text| format!("{label} {DASH} {text}"))
            .collect();
        // The lines with the name come first: the product of every mode's lines is cut short, and a card that names its
        // modes prints the names.
        named.extend(found);
        return named;
    }
    found
}

/// The flags of a `Modal` that change its header.
#[derive(Clone, Copy)]
pub(super) struct C11dFlags {
    /// A mode may be chosen more than once.
    pub(super) repeat: bool,
    /// The modes are picked at random.
    pub(super) at_random: bool,
    /// A mode is chosen once each turn, and once ever.
    pub(super) once: (bool, bool),
}

/// The header lines a `Modal` may start with, or `None` for flags no card words; an empty line when the condition of
/// `all_if` words the header instead (its wordings come back second).
fn c11d_headers(
    header: String,
    one: bool,
    flags: C11dFlags,
    all_if: Option<&Condition>,
) -> Option<(Vec<String>, Vec<String>)> {
    let C11dFlags {
        repeat,
        at_random,
        once: (once_each_turn, once_ever),
    } = flags;
    if repeat {
        // "choose three. You may choose the same mode more than once.": a fixed number of two or more.
        let head = header.strip_suffix(&format!(" {DASH}"))?;
        let plain = all_if.is_none() && !at_random && !once_each_turn && !once_ever;
        return (plain && !one && !head.contains(" or ")).then(|| {
            (
                vec![format!(
                    "{head}. You may choose the same mode more than once."
                )],
                Vec::new(),
            )
        });
    }
    match (all_if, at_random, once_each_turn, once_ever) {
        (Some(condition), false, false, false) if one => {
            Some((vec![String::new()], both_instead(condition)))
        }
        (None, false, false, false) => Some((vec![header], Vec::new())),
        (None, true, false, false) if one => {
            Some((vec![format!("choose one at random {DASH}")], Vec::new()))
        }
        (None, false, true, false) if one => Some((
            vec![
                format!("choose one that hasn't been chosen this turn {DASH}"),
                "choose one that hasn't been chosen this turn.".to_owned(),
            ],
            Vec::new(),
        )),
        (None, false, false, true) if one => Some((
            vec![format!("choose one that hasn't been chosen {DASH}")],
            Vec::new(),
        )),
        _ => None,
    }
}

/// "choose one —" and a bullet line for each mode, in every wording of each mode; the last bullet has no period, which the
/// sentence that holds the text adds.
pub(super) fn texts(
    choose: &Bounds,
    modes: &[Mode],
    flags: C11dFlags,
    all_if: Option<&Condition>,
) -> Vec<String> {
    let Some(header) = header(choose, modes.len(), flags.repeat) else {
        return Vec::new();
    };
    // "choose one at random —" and "choose one that hasn't been chosen this turn —", which a card may end with a period.
    let one = header.starts_with("choose one ") && choose.is_one();
    if all_if.is_some() && modes.len() != 2 {
        return Vec::new();
    }
    // The wordings of a header that comes in several are put in front once the bullets are written, so they do not
    // use up the room for the wordings of the modes.
    let Some((mut texts, headers)) = c11d_headers(header, one, flags, all_if) else {
        return Vec::new();
    };
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
    if headers.is_empty() {
        return texts;
    }
    headers
        .iter()
        .flat_map(|head| texts.iter().map(move |text| format!("{head}{text}")))
        .collect()
}
