//! A choice of modes, each mode written as a spell of its own with every wording the checker accepts and its label
//! before a dash: "Choose one —", "choose one. If you control a commander, you may choose both instead.", "Choose two —"
//! and "Choose five. You may choose the same mode more than once.", then "• Jump Kick — Destroy target artifact.".

use super::super::super::amounts::count_word;
use super::super::super::as_long_as;
use super::super::capitalize;
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::{Effect, Mode};
use crate::refs::Bounds;
use crate::value::Amount;

/// The most wordings one ability gets: the modes multiply their wordings.
const W4E_MOST: usize = 512;

/// The wordings of an ability whose only step is a choice of modes.
pub(super) fn wordings(ability: &Ability) -> Vec<String> {
    let (Ability::Spell { targets, effects }
    | Ability::Activated {
        targets, effects, ..
    }
    | Ability::Triggered {
        targets, effects, ..
    }) = ability
    else {
        return Vec::new();
    };
    let [
        Effect::Modal {
            choose,
            repeat,
            at_random: false,
            once_each_turn: false,
            once_ever: false,
            all_if,
            modes,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if !targets.is_empty() {
        return Vec::new();
    }
    let mut written = Vec::new();
    for head in w4e_heads(choose, *repeat, all_if.as_ref(), modes.len()) {
        if matches!(ability, Ability::Spell { .. }) {
            written.push(capitalize(&head));
        } else {
            written.extend(
                with_effects(ability, &head)
                    .iter()
                    .filter_map(|text| text.strip_suffix('.').map(str::to_owned)),
            );
        }
    }
    for mode in modes {
        let lines = mode_lines(mode);
        if lines.is_empty() {
            return Vec::new();
        }
        let mut next = Vec::new();
        'all: for before in &written {
            for line in &lines {
                if next.len() >= W4E_MOST {
                    break 'all;
                }
                next.push(format!("{before}\n\u{2022} {line}"));
            }
        }
        written = next;
    }
    written
}

/// The ways one mode is written, after its label.
fn mode_lines(mode: &Mode) -> Vec<String> {
    let alone = Ability::Spell {
        targets: mode.targets.clone(),
        effects: mode.effects.clone(),
    };
    let lines = super::super::written_variants(&alone);
    match &mode.label {
        Some(label) => lines
            .iter()
            .map(|line| format!("{label} \u{2014} {line}"))
            .collect(),
        None => lines,
    }
}

/// "choose one —", "choose one. If you control a commander, you may choose both instead.", "choose two —", "choose
/// five. You may choose the same mode more than once.", each in lower case.
fn w4e_heads(
    choose: &Bounds,
    repeat: bool,
    all_if: Option<&crate::value::Condition>,
    modes: usize,
) -> Vec<String> {
    let Some(Amount::Fixed(most)) = &choose.max else {
        return Vec::new();
    };
    if choose.min != Amount::Fixed(*most) {
        return Vec::new();
    }
    let Some(word) = u32::try_from(*most).ok().and_then(count_word) else {
        return Vec::new();
    };
    match (all_if, repeat) {
        (None, false) => vec![format!("choose {word} \u{2014}")],
        (None, true) => vec![format!(
            "choose {word}. You may choose the same mode more than once."
        )],
        (Some(condition), false) if *most == 1 && modes == 2 => as_long_as::texts(condition)
            .iter()
            .map(|said| format!("choose {word}. If {said}, you may choose both instead."))
            .collect(),
        _ => Vec::new(),
    }
}
