//! You and another player draw the same number of cards: "You and target opponent each draw three cards." The
//! renderers write two clauses joined by "and".

use super::super::super::amounts::cards_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::refs::Targetable;

/// Adds the wording of a spell that makes you and its one target player draw the same cards.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, .. } = ability else {
        return;
    };
    let [
        Effect::Draw {
            who: Ref::You,
            count,
        },
        Effect::Draw {
            who: Ref::Target(0),
            count: other,
        },
    ] = ability.effects()
    else {
        return;
    };
    let [target] = targets.as_slice() else {
        return;
    };
    let player = match target.what {
        Targetable::Opponent => "opponent",
        Targetable::Player => "player",
        _ => return,
    };
    let (Some(cards), true) = (cards_text(count), count == other) else {
        return;
    };
    texts.push(format!("You and target {player} each draw {cards}."));
}
