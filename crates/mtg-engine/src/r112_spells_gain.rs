//! "As you cast spells from your hand this turn, they gain cascade." (`Rule::R112SpellsGain`): a spell its controller
//! casts that the rule's selector picks, judged where it is cast from, gains the keyword as it is cast. The effect names
//! the spell itself (CR 611.2c), so the keyword stays while the spell is on the stack and a keyword that triggers on
//! the cast (cascade, CR 702.85a) triggers.

use crate::effects::{Duration, Modification};
use mtg_cards::keyword::Keyword;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::effects::{ContinuousEffect, Selector};
use crate::error::EngineError;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::Scope;
use crate::rules::Rule;
use crate::state::GameState;

/// The keywords the rules give the card `spell` that `player` casts, each with the source of its rule, judged before
/// the card leaves the zone it is cast from.
pub(crate) fn r112_gained_as_cast(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
) -> Vec<(Keyword, Option<ObjectRef>)> {
    let Some(target) = state.object(spell).map(GameObject::reference) else {
        return Vec::new();
    };
    state
        .rule_effects
        .iter()
        .filter_map(|effect| {
            let Rule::R112SpellsGain { what, keyword } = &effect.rule else {
                return None;
            };
            // The card is judged where it is cast from, before it moves: a selector naming a hand reads the hand.
            let scope =
                Scope::announcing(state, effect.controller, effect.source.unwrap_or(target));
            (effect.controller == player
                && (what.is_empty() || scope.selects(what, target).unwrap_or(false)))
            .then(|| (keyword.clone(), effect.source))
        })
        .collect()
}

/// Gives the spell `spell`, now on the stack, the keywords it gained as it was cast.
pub(crate) fn r112_give(
    state: &mut GameState,
    player: PlayerId,
    spell: ObjectId,
    gained: Vec<(Keyword, Option<ObjectRef>)>,
) -> Result<(), EngineError> {
    let Some(target) = state.object(spell).map(GameObject::reference) else {
        return Ok(());
    };
    let turn = state.turn().number;
    for (keyword, source) in gained {
        let mut effect = ContinuousEffect::new(
            EffectId(state.next_effect),
            Selector::Objects(vec![target]),
            Modification::AddKeyword(keyword),
            Duration::Forever,
            state.next_timestamp,
        );
        effect.source = source;
        effect.controller = player;
        effect.turn = turn;
        crate::fold::create_effect(state, &effect)?;
    }
    // The cast read the layers before the spell gained anything; the trigger detection after it reads them anew.
    state.layers.reset();
    state.rules_cache.reset();
    Ok(())
}
