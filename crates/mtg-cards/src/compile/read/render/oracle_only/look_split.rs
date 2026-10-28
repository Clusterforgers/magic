//! The top cards of the library split three ways, one card each: "Look at the top three cards of your library. Put one
//! of them into your hand, put one of them on the bottom of your library, and exile one of them. You may play the
//! exiled card this turn."

use super::super::super::amounts::count_word;
use crate::ability::Ability;
use crate::effect::{Destination, Effect, Place};
use crate::modify::{CastPermission, Duration, GameRule};
use crate::refs::{Bounds, Ref, Selector};
use crate::value::Amount;

/// Adds the wording of a spell that looks at the top cards, puts one into the hand, one on the bottom and exiles one,
/// and lets its controller play the exiled card.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let [
        Effect::LookAtTop {
            who: Ref::You,
            count: Amount::Fixed(count),
            bind: top,
        },
        Effect::Choose {
            who: Ref::You,
            from: Ref::Bound(from_top),
            count: first,
            bind: kept,
            rest_as: Some(others),
        },
        Effect::Move {
            what: Ref::Bound(moved_kept),
            to: Destination::Hand,
        },
        Effect::Choose {
            who: Ref::You,
            from: Ref::Bound(from_others),
            count: second,
            bind: bottomed,
            rest_as: Some(exiled),
        },
        Effect::Move {
            what: Ref::Bound(moved_bottomed),
            to:
                Destination::Library {
                    place: Place::Bottom,
                    order: None,
                },
        },
        Effect::Exile {
            what: Ref::Bound(taken),
            bind: Some(card),
            duration: None,
        },
        Effect::Rule {
            rule: GameRule::MayCast(permission),
            duration: Duration::UntilEndOfTurn,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let chained = from_top == top
        && moved_kept == kept
        && from_others == others
        && moved_bottomed == bottomed
        && taken == exiled;
    let only_one = *first == Bounds::default() && *second == Bounds::default();
    let play = *permission
        == CastPermission {
            who: Ref::You,
            what: Selector::default(),
            from: Some(crate::refs::ZoneKind::Exile),
            free: false,
            flash: false,
            only: Some(Ref::Bound(card.clone())),
            play: true,
            once: false,
        };
    let Some(words) = u32::try_from(*count).ok().and_then(count_word) else {
        return;
    };
    if targets.is_empty() && chained && only_one && play {
        texts.push(format!(
            "Look at the top {words} cards of your library. Put one of them into your hand, put one of them on the \
             bottom of your library, and exile one of them. You may play the exiled card this turn."
        ));
    }
}
