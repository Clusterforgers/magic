//! The modes of a triggered "choose one —" written one by one: "Whenever you attack, choose one — • Create a tapped
//! and attacking X/X green Dinosaur creature token ..., where X is ... • Create X 1/1 white Vampire creature tokens
//! ...". Each mode is written as the trigger with the mode's steps alone, so the words a whole ability gets ("where X
//! is ...") are in it, and the head of the trigger is taken off.

use crate::ability::Ability;
use crate::effect::{Effect, Mode};

/// The most wordings one ability gets: the modes multiply their wordings.
const C5_GL_MOST: usize = 512;

/// Adds the wordings of a triggered ability whose only step is a choice of one mode.
pub(super) fn c5_gl_add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return;
    };
    let [
        Effect::Modal {
            choose,
            repeat: false,
            at_random: false,
            once_each_turn: false,
            all_if: None,
            modes,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    if !choose.is_one() || !targets.is_empty() || modes.len() < 2 {
        return;
    }
    let mut heads: Option<Vec<(String, Vec<String>)>> = None;
    for Mode {
        targets, effects, ..
    } in modes
    {
        let alone = Ability::Triggered {
            when: when.clone(),
            intervening_if: intervening_if.clone(),
            limit: *limit,
            from: *from,
            targets: targets.clone(),
            effects: effects.clone(),
        };
        let split = c5_gl_split(&super::super::variants(&alone));
        heads = Some(match heads {
            None => split
                .into_iter()
                .map(|(head, rest)| (head, vec![rest]))
                .collect(),
            Some(before) => c5_gl_joined(before, &split),
        });
    }
    for (head, sentences) in heads.unwrap_or_default() {
        texts.extend(
            sentences
                .into_iter()
                .map(|modes| format!("{head}, choose one —{modes}")),
        );
    }
}

/// Each wording as its head and its effects as a line of a mode ("\n• Create ...").
fn c5_gl_split(wordings: &[String]) -> Vec<(String, String)> {
    wordings
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(", ")?;
            let mut chars = rest.chars();
            let first = chars.next()?.to_uppercase();
            Some((head.to_owned(), format!("\n• {first}{}", chars.as_str())))
        })
        .collect()
}

/// The modes so far joined with the wordings of the next mode under the same head, up to [`C5_GL_MOST`].
fn c5_gl_joined(
    before: Vec<(String, Vec<String>)>,
    next: &[(String, String)],
) -> Vec<(String, Vec<String>)> {
    before
        .into_iter()
        .filter_map(|(head, lines)| {
            let joined: Vec<String> = lines
                .iter()
                .flat_map(|line| {
                    next.iter()
                        .filter(|(own, _)| *own == head)
                        .map(move |(_, rest)| format!("{line}{rest}"))
                })
                .take(C5_GL_MOST)
                .collect();
            (!joined.is_empty()).then_some((head, joined))
        })
        .collect()
}
