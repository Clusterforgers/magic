//! "Damage isn't removed from ~ during cleanup steps." (`GameRule::KeepsDamage`): the damage the cleanup step removes
//! (CR 514.2) is that of every permanent without the rule.

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::ZoneKind;
use mtg_core::ids::ObjectId;

use crate::ability_reader::abilities_of;
use crate::characteristics::controller_of_object;
use crate::events::Event;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// Whether the permanent has the rule now, with its condition holding.
fn r5516_keeps(state: &GameState, id: ObjectId) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    abilities_of(state, id).into_iter().any(|(_, ability)| {
        let Ability::Rule {
            rule: GameRule::KeepsDamage,
            condition,
            from: ZoneKind::Battlefield,
        } = &*ability
        else {
            return false;
        };
        condition.as_ref().is_none_or(|condition| {
            let scope = Scope::announcing(
                state,
                controller_of_object(state, object),
                object.reference(),
            );
            scope.holds(condition).unwrap_or(false)
        })
    })
}

/// The events of the cleanup step that remove damage: one `DamageCleared` when no damaged permanent keeps its damage,
/// else a `DamageRemoved` for each damaged permanent that does not; nothing when no permanent is damaged.
pub(crate) fn r5516_cleared(state: &GameState) -> Vec<Event> {
    let damaged: Vec<ObjectId> = state
        .objects()
        .filter(|o| o.permanent().is_some_and(|p| p.damage() > 0))
        .map(crate::object::GameObject::id)
        .collect();
    let (kept, removed): (Vec<ObjectId>, Vec<ObjectId>) = damaged.into_iter().partition(|id| {
        state
            .object(*id)
            .is_some_and(|o| o.zone() == Zone::Battlefield)
            && r5516_keeps(state, *id)
    });
    if kept.is_empty() {
        if removed.is_empty() {
            return Vec::new();
        }
        return vec![Event::DamageCleared];
    }
    removed
        .into_iter()
        .map(|object| Event::DamageRemoved { object })
        .collect()
}
