//! Two piles of the top cards of a library: "Reveal the top five cards of your library. An opponent separates those cards
//! into two piles. Put one pile into your hand and the other into your graveyard." The opponent is chosen, splits the
//! cards with a `Choose` that binds both piles, and the controller may take the first pile; if they don't, the other.

use super::super::super::amounts::count_word;
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, ZoneKind};
use crate::value::{Amount, Condition};

/// Adds the wording of an ability whose effects are the two piles of the top cards of its controller's library.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let (Ability::Spell { effects, .. }
    | Ability::Activated { effects, .. }
    | Ability::Triggered { effects, .. }) = ability
    else {
        return;
    };
    if let Some(sentence) = c5_dice_piles_sentence(effects) {
        texts.extend(with_effects(ability, &sentence));
    }
}

/// The moves of one pile into the hand and the other into the graveyard.
fn c5_dice_moves(effects: &[Effect], kept: &str, left: &str) -> bool {
    matches!(
        effects,
        [
            Effect::Move { what: Ref::Bound(hand), to: Destination::Hand },
            Effect::Move { what: Ref::Bound(yard), to: Destination::Graveyard },
        ] if hand == kept && yard == left
    )
}

/// The sentence of the six effects of the piles, in lower case and without a period.
fn c5_dice_piles_sentence(effects: &[Effect]) -> Option<String> {
    let [
        Effect::LookAtTop {
            who: Ref::You,
            count: Amount::Fixed(count),
            bind: top,
        },
        Effect::Reveal {
            what: Ref::Bound(shown),
        },
        Effect::Choose {
            who: Ref::You,
            from: Ref::Opponents,
            count: one,
            bind: separator,
            rest_as: None,
        },
        Effect::Choose {
            who: Ref::Bound(splits),
            from: Ref::Each(pool),
            count: any,
            bind: pile,
            rest_as: Some(other),
        },
        Effect::May {
            who: Ref::You,
            effects: taken,
        },
        Effect::If {
            condition: Condition::Not(declined),
            then: instead,
            otherwise,
        },
    ] = effects
    else {
        return None;
    };
    let from_top = pool.0
        == [
            Pred::In(ZoneKind::Library),
            Pred::Is(Ref::Bound(top.clone())),
        ];
    (top == shown
        && from_top
        && *one == Bounds::default()
        && splits == separator
        && *any == Bounds::up_to(None)
        && **declined == Condition::Performed
        && otherwise.is_empty()
        && c5_dice_moves(taken, pile, other)
        && c5_dice_moves(instead, other, pile))
    .then_some(())?;
    let words = count_word(u32::try_from(*count).ok()?)?;
    Some(format!(
        "reveal the top {words} cards of your library. An opponent separates those cards into two piles. Put one pile into your hand and the other into your graveyard"
    ))
}
