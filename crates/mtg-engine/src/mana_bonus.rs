//! Triggered mana abilities (CR 605.1b): the mana of "Whenever you tap a land for mana, add one mana of any type that
//! land produced" and "Whenever enchanted land is tapped for mana, its controller adds an additional {G}".
//!
//! Such an ability never uses the stack (CR 605.4a) and `triggers/detect.rs` skips it. Its mana joins the mana of the tap
//! that triggers it: each option of a land that taps for mana lists it in `made`, so the payment planner and the
//! activation agree on what a tap makes. A trigger that lets the player pick (any color) gives the option one variant
//! for each pick, numbered `STRIDE` apart so every variant is a source of its own.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Produce;
use mtg_cards::mana_trigger::mana_trigger;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;

use crate::ability_reader::abilities_of;
use crate::characteristics::controller_of_object;
use crate::mana::ManaOption;
use crate::mana_ability::split;
use crate::mana_kinds::{COLORS, mana_of};
use crate::object::{Chosen, GameObject, ObjectRef, PermanentState};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The distance between the source numbers of the variants a trigger's pick makes of one option: variant `n` of the
/// option numbered `a` is `a + n * STRIDE`.
pub(crate) const STRIDE: usize = 1 << 12;

/// A triggered mana ability on the battlefield.
struct Trigger {
    /// Its source.
    source: ObjectRef,
    /// Its controller.
    controller: PlayerId,
    /// The ability.
    body: Arc<Ability>,
}

/// The triggered mana abilities of the permanents in play, in the order of the battlefield.
fn triggers(state: &GameState) -> Vec<Trigger> {
    let mut found = Vec::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        for (_, body) in abilities_of(state, id) {
            if mana_trigger(&body).is_some() {
                found.push(Trigger {
                    source: object.reference(),
                    controller: controller_of_object(state, object),
                    body,
                });
            }
        }
    }
    found
}

/// Each way `trigger` adds its mana for a tap that makes `option`: one way for a fixed mana, one for each color of
/// "any color". A mana of a color the source never chose is none (a way of no mana).
fn ways(
    state: &GameState,
    trigger: &Trigger,
    mana: &Produce,
    option: &ManaOption,
) -> Vec<Vec<ManaType>> {
    match mana {
        Produce::Fixed(list) => vec![list.clone()],
        Produce::OfTapped => vec![vec![option.produces]],
        Produce::AnyColor(count) => COLORS
            .iter()
            .map(|color| vec![*color; *count as usize])
            .collect(),
        Produce::ChosenColor(count) => {
            let chosen = state
                .object(trigger.source.id)
                .and_then(GameObject::permanent)
                .and_then(PermanentState::chosen);
            match chosen {
                Some(Chosen::Color(color)) => vec![vec![mana_of(*color); *count as usize]],
                _ => vec![Vec::new()],
            }
        }
        _ => vec![Vec::new()],
    }
}

/// Whether `trigger` fires as `player` taps the permanent `land` for mana.
fn fires(state: &GameState, trigger: &Trigger, player: PlayerId, land: ObjectRef) -> bool {
    let Some(shape) = mana_trigger(&trigger.body) else {
        return false;
    };
    let scope = Scope::announcing(state, trigger.controller, trigger.source);
    scope
        .players(shape.by)
        .is_ok_and(|players| players.contains(&player))
        && scope.selects(shape.what, land).unwrap_or(false)
        && shape
            .condition
            .is_none_or(|condition| scope.holds(condition).unwrap_or(false))
}

/// `option` with `added` mana joined to what it makes.
fn with(mut option: ManaOption, added: &[ManaType]) -> ManaOption {
    let mut made = option.made();
    made.extend_from_slice(added);
    if let Some((produces, count, also)) = split(&made) {
        option.produces = produces;
        option.count = count;
        option.also = also;
    }
    option
}

/// The options one option becomes once the triggered mana abilities of the permanents in play add their mana to its tap.
fn expand(
    state: &GameState,
    player: PlayerId,
    triggers: &[Trigger],
    option: ManaOption,
) -> Vec<ManaOption> {
    let land = state
        .object(option.source.object)
        .map(GameObject::reference);
    // Only a tap for mana triggers them: an ability that does not tap its source is no tap for mana.
    let Some(land) = land.filter(|_| !option.tapless) else {
        return vec![option];
    };
    // Each option with its number among the variants the picks of the triggers make.
    let mut found = vec![(option, 0)];
    for trigger in triggers.iter().filter(|t| fires(state, t, player, land)) {
        let Some(shape) = mana_trigger(&trigger.body) else {
            continue;
        };
        found = found
            .into_iter()
            .flat_map(|(option, number)| {
                let all = ways(state, trigger, shape.mana, &option);
                let width = all.len();
                all.into_iter()
                    .enumerate()
                    .map(|(pick, mana)| (with(option.clone(), &mana), number * width + pick))
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    found
        .into_iter()
        .map(|(mut option, number)| {
            option.source.ability += number * STRIDE;
            option
        })
        .collect()
}

/// `options`, the mana abilities `player` could use, with the mana of the triggered mana abilities their taps trigger.
pub(crate) fn fold(
    state: &GameState,
    player: PlayerId,
    options: Vec<ManaOption>,
) -> Vec<ManaOption> {
    let triggers = triggers(state);
    if triggers.is_empty() {
        return options;
    }
    options
        .into_iter()
        .flat_map(|option| expand(state, player, &triggers, option))
        .collect()
}

#[cfg(test)]
mod tests;
