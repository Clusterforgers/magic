//! Extra turns and added phases (CR 500.7, 500.8): "Take an extra turn after this one.", "Target player takes two extra
//! turns after this one.", "Skip the untap step of that turn.", "After this main phase, there is an additional combat
//! phase followed by an additional main phase." (`Effect::ExtraTurn`, `Effect::AddPhase`). The writer's side is
//! `effect_text`, given the words for the player.

use super::amounts::{count_word, number_word};
use super::tokens::Reader;
use crate::effect::{Effect, PhaseKind};
use crate::refs::Ref;
use crate::value::Amount;

/// "take an extra turn after this one" and its siblings with a count, after the subject: the effect of one clause.
pub(super) fn extra_turns(r: &mut Reader<'_>, who: &Ref) -> Option<Effect> {
    let count = number_word(r.take_word()?).filter(|count| *count >= 1)?;
    r.need(if count == 1 {
        "extra turn"
    } else {
        "extra turns"
    })?;
    r.need("after this one")?;
    Some(Effect::ExtraTurn {
        who: who.clone(),
        count: Amount::Fixed(i32::try_from(count).ok()?),
        skip_untap: false,
    })
}

/// "Take an extra turn after this one." with "Skip the untap step of that turn." after it.
pub fn extra_turn(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("take")?;
    let mut effect = extra_turns(r, &Ref::You)?;
    let skipped = r.attempt(|r| {
        r.punct('.').then_some(())?;
        r.need("skip the untap step of that turn")
    });
    if skipped.is_some()
        && let Effect::ExtraTurn { skip_untap, .. } = &mut effect
    {
        *skip_untap = true;
    }
    Some(vec![effect])
}

/// One or more added phases after "an additional" or a count: "an additional combat phase followed by an additional
/// main phase", "two additional combat phases".
fn phases(r: &mut Reader<'_>) -> Option<Vec<PhaseKind>> {
    let count = number_word(r.take_word()?)?;
    r.word("additional").then_some(())?;
    let kind = if r.word("combat") {
        PhaseKind::Combat
    } else if r.word("main") {
        PhaseKind::Main
    } else {
        return None;
    };
    r.word(if count == 1 { "phase" } else { "phases" })
        .then_some(())?;
    let mut list = vec![kind; usize::try_from(count).ok()?];
    if r.words("followed by an additional main phase") {
        list.push(PhaseKind::Main);
    }
    Some(list)
}

/// "After this main phase, there is an additional combat phase followed by an additional main phase.", "After this
/// phase, there is an additional combat phase.", "There is an additional combat phase after this phase.".
pub fn added_phases(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let list = if r.word("after") {
        r.need("this")?;
        r.word("main");
        r.need("phase")?;
        r.punct(',').then_some(())?;
        (r.words("there is") || r.words("there are")).then_some(())?;
        phases(r)?
    } else {
        (r.words("there is") || r.words("there are")).then_some(())?;
        let list = phases(r)?;
        r.need("after this phase")?;
        list
    };
    Some(vec![Effect::AddPhase { phases: list }])
}

/// The words of a player for a count of turns: "an extra turn", "two extra turns".
fn turns_text(count: i32) -> Option<String> {
    let number = u32::try_from(count).ok()?;
    if number == 1 {
        return Some("an extra turn".to_owned());
    }
    Some(format!("{} extra turns", count_word(number)?))
}

/// An added phase list as the card words it, after the anchor.
fn phases_text(list: &[PhaseKind]) -> Option<String> {
    let (anchor, text) = match list {
        [PhaseKind::Combat] => ("phase", "there is an additional combat phase".to_owned()),
        [PhaseKind::Combat, PhaseKind::Main] => (
            "main phase",
            "there is an additional combat phase followed by an additional main phase".to_owned(),
        ),
        [PhaseKind::Combat, PhaseKind::Combat] => (
            "main phase",
            "there are two additional combat phases".to_owned(),
        ),
        _ => return None,
    };
    Some(format!("after this {anchor}, {text}"))
}

/// The effects `extra_turn`, the player's `extra_turns` and `added_phases` read, written back; `subject` is the words
/// for the player of an extra turn.
pub fn effect_text(effect: &Effect, subject: Option<&str>) -> Option<String> {
    match effect {
        Effect::ExtraTurn {
            who,
            count: Amount::Fixed(count),
            skip_untap,
        } => {
            let turns = turns_text(*count)?;
            let skip = if *skip_untap {
                (*who == Ref::You).then_some(". Skip the untap step of that turn")?
            } else {
                ""
            };
            match subject {
                Some("you") | None if *who == Ref::You => {
                    Some(format!("take {turns} after this one{skip}"))
                }
                Some(subject) => Some(format!("{subject} takes {turns} after this one{skip}")),
                None => None,
            }
        }
        Effect::AddPhase { phases } => phases_text(phases),
        _ => None,
    }
}
