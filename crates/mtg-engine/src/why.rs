//! The detail of why a card or ability cannot be played now ([`Why`], `docs/engine.md`, Refusals): each part is read
//! off the check that decided the reason, so the detail never disagrees with it.

use std::collections::BTreeMap;

use mtg_cards::ability::Ability;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType};

use crate::decision::Why;
use crate::mana::{ManaOption, ManaPool};
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;
use crate::zones::Zone;

/// The target of `rules`, a spell or ability of `source`, that has no legal choice for `player`, in words; `None` when
/// every target can be chosen.
pub(crate) fn target(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    rules: &Ability,
) -> Option<Why> {
    let spec = crate::choose::unmet_target(state, player, source, rules)?;
    let (needs, zone) = crate::wording::target_need(spec);
    Some(Why::Target { needs, zone })
}

/// The rule found by [`crate::prohibit::forbidder`], named by the object that put it in force while every player can
/// see that object: a face-up permanent, a spell or ability on the stack, or an object in the command zone.
pub(crate) fn forbidden_by(state: &GameState, source: Option<ObjectRef>) -> Why {
    let seen = source
        .and_then(|source| state.object(source.id))
        .filter(|object| {
            object.face_down().is_none()
                && matches!(
                    object.zone(),
                    Zone::Battlefield | Zone::Stack | Zone::Command(_)
                )
        });
    Why::ForbiddenBy {
        source: seen.map(GameObject::id),
        name: seen.map(|object| object.card().name.clone()),
    }
}

/// The spell with split second on the stack, if one is there (CR 702.61a).
pub(crate) fn split_second(state: &GameState) -> Option<Why> {
    let spell = crate::keywords::split_second_spell(state)?;
    Some(Why::SplitSecond {
        spell,
        name: state
            .object(spell)
            .map(|object| object.card().name.clone())
            .unwrap_or_default(),
    })
}

/// `cost` against the mana at hand: the pool and one use of each source, and the kinds of mana the cost asks for by a
/// symbol of one kind that neither the pool nor any source makes.
pub(crate) fn mana(cost: ManaCost, pool: &ManaPool, sources: &[ManaOption]) -> Why {
    let mut best: BTreeMap<ObjectId, u32> = BTreeMap::new();
    for option in sources {
        let most = best.entry(option.source.object).or_default();
        *most = (*most).max(option.count.max(1));
    }
    let at_hand = pool.total().saturating_add(best.values().sum::<u32>());
    let mut missing: Vec<ManaType> = Vec::new();
    for symbol in cost.symbols() {
        let kind = match symbol {
            ManaSymbol::Colored(color) => ManaType::from(*color),
            ManaSymbol::Colorless => ManaType::Colorless,
            _ => continue,
        };
        let made = pool.get(kind) > 0 || sources.iter().any(|option| option.produces == kind);
        if !made && !missing.contains(&kind) {
            missing.push(kind);
        }
    }
    Why::Mana {
        cost,
        at_hand,
        missing,
    }
}
