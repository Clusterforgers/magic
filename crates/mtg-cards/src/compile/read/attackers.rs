//! "Each opponent attacking that player does the same.": the sentence that follows the effects of a trigger on the
//! attack at the enchanted player (the Curses of Bounty, Disturbance, Opulence, Verbosity and Vitality) repeats them
//! for the opponents of the controller who have a creature attacking that player. Read from the wording and written
//! back.

use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use mtg_core::card::CardType;

/// The sentence, without its period.
const SAME: &str = "each opponent attacking that player does the same";

/// The opponents of the controller with a creature attacking the enchanted player.
fn attacking_opponents() -> Ref {
    Ref::ControllerOf(Box::new(Ref::Each(Selector(vec![
        Pred::AttackingPlayer(Ref::AttachedPlayer),
        Pred::ControlledBy(Ref::Opponents),
    ]))))
}

/// The effect again with the attacking opponents in the controller's place, when it is one of the effects that name
/// their player or whose objects the controller controls.
fn again(effect: &Effect) -> Option<Effect> {
    let theirs = attacking_opponents();
    Some(match effect {
        Effect::Draw { who: Ref::You, count } => Effect::Draw {
            who: theirs,
            count: count.clone(),
        },
        Effect::GainLife { who: Ref::You, amount } => Effect::GainLife {
            who: theirs,
            amount: amount.clone(),
        },
        Effect::LoseLife { who: Ref::You, amount } => Effect::LoseLife {
            who: theirs,
            amount: amount.clone(),
        },
        Effect::CreateToken {
            who: Ref::You,
            token,
            count,
            tapped,
            attacking,
            bind: None,
        } => Effect::CreateToken {
            who: theirs,
            token: token.clone(),
            count: count.clone(),
            tapped: *tapped,
            attacking: *attacking,
            bind: None,
        },
        Effect::Untap { what } if *what == nonland_permanents(Ref::You) => Effect::Untap {
            what: nonland_permanents(theirs),
        },
        _ => return None,
    })
}

/// Every nonland permanent the player controls.
fn nonland_permanents(who: Ref) -> Ref {
    Ref::Each(Selector(vec![
        Pred::Not(Box::new(Pred::Type(CardType::Land))),
        Pred::ControlledBy(who),
    ]))
}

/// The sentence for the opponents after these effects: Curse of Bounty spells out the untapping.
pub(super) fn sentence(head: &[Effect]) -> &'static str {
    if matches!(head, [Effect::Untap { .. }]) {
        "each opponent attacking that player untaps all nonland permanents they control"
    } else {
        SAME
    }
}

/// "Each opponent attacking that player does the same." after the effects of the trigger: the same effects again for
/// the opponents who attack the enchanted player.
pub(super) fn does_the_same(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    r.attacked.then_some(())?;
    (!effects.is_empty()).then_some(())?;
    let repeated: Vec<Effect> = effects.iter().map(again).collect::<Option<_>>()?;
    r.need(sentence(effects))?;
    r.sentence_end().then(|| effects.extend(repeated))
}

/// The effects that come first when the rest repeats them for the attacking opponents.
pub(super) fn repeated(effects: &[Effect]) -> Option<&[Effect]> {
    let (head, tail) = effects.split_at(effects.len() / 2);
    (!head.is_empty()
        && effects.len() % 2 == 0
        && head
            .iter()
            .zip(tail)
            .all(|(first, second)| again(first).as_ref() == Some(second)))
    .then_some(head)
}
