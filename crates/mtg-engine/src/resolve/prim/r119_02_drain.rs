//! `Effect::R119ActivateLandMana` and `Effect::R119DrainMana`: a player made to activate a mana ability of each land
//! they control, and their unspent mana taken by the controller (Drain Power).

use mtg_cards::effect::Effect;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;

use super::wrong_effect;
use crate::characteristics::{characteristics, controller_of};
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::events::Event;
use crate::mana::ManaOption;
use crate::mana_ability;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The ways a land can be made to add mana now: its mana abilities whose cost is no more than tapping it, paying life
/// it can pay or sacrificing it (an ability that costs mana or other permanents is paid only inside a payment).
fn ways_of(options: &[ManaOption], land: mtg_core::ids::ObjectId, life: i32) -> Vec<ManaOption> {
    options
        .iter()
        .filter(|option| {
            option.source.object == land
                && !option.is_costly()
                && option.takes.is_empty()
                && !option.priority_only
                && option.holds_price(life)
        })
        .cloned()
        .collect()
}

/// The words of one way: the mana it adds, as symbols.
fn label(option: &ManaOption) -> String {
    let mut words = "Add ".to_owned();
    for mana in option.made() {
        words.push('{');
        words.push(mana.symbol());
        words.push('}');
    }
    words
}

/// Each named player still in the game activates one mana ability of each land they control that has one they can
/// activate now: a land with one way adds its mana, of a land with more its controller is asked which, one land at a
/// time in the order of the battlefield. Every activation is worked out from the state before them, as the sources of
/// one payment are.
pub(super) fn run_activate(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R119ActivateLandMana { who } = effect else {
        return Err(wrong_effect("R119ActivateLandMana"));
    };
    let state = cx.state();
    let mut choosers = Vec::new();
    let mut picks: Vec<(PlayerId, Vec<ManaOption>)> = Vec::new();
    for player in cx.scope.players(who)? {
        let Some(seat) = state.player(player).filter(|_| state.is_alive(player)) else {
            continue;
        };
        let options = mana_ability::options(state, player);
        for &land in state.zone(Zone::Battlefield) {
            let Some(chars) = characteristics(state, land) else {
                continue;
            };
            if !chars.types.contains(CardType::Land) || controller_of(state, land) != Some(player) {
                continue;
            }
            let ways = ways_of(&options, land, seat.life());
            if ways.is_empty() {
                continue;
            }
            choosers.push(OptionChooser {
                player,
                options: ways
                    .iter()
                    .map(|way| ChoiceOption::Label(label(way)))
                    .collect(),
                text: format!(
                    "Choose the mana ability of {} to activate.",
                    state
                        .object(land)
                        .map_or("a land", |object| object.card().name.as_str())
                ),
            });
            picks.push((player, ways));
        }
    }
    match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => Ok(Run::Ask(decision)),
        Optioned::Done(taken) => {
            let mut events = Vec::new();
            for ((player, ways), (_, at)) in picks.iter().zip(taken) {
                let Some(way) = ways.get(at) else {
                    continue;
                };
                events.extend(
                    mana_ability::logged_activation(state, *player, way, &[])
                        .map_err(|error| ResolveError::Invalid(error.to_string()))?,
                );
            }
            mana_ability::r5g_distinct(state, &mut events);
            Ok(Run::events(events))
        }
    }
}

/// Binds the cards spliced onto each spell the reference names that is still on the stack, as they are now; a spell
/// nothing was spliced onto binds nothing.
pub(super) fn run_bind_spliced(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R119BindSpliced { of, bind } = effect else {
        return Err(wrong_effect("R119BindSpliced"));
    };
    let state = cx.state();
    let mut named = Vec::new();
    for spell in cx.scope.objects(of)? {
        let Some(spell) = state.object(spell.id).filter(|o| o.zone() == Zone::Stack) else {
            continue;
        };
        for card in spell.choices().spliced() {
            if let Some(card) = state.object(*card) {
                let found = crate::choose::Target::Object(card.reference());
                if !named.contains(&found) {
                    named.push(found);
                }
            }
        }
    }
    Ok(Run::Done {
        events: Vec::new(),
        bound: vec![(bind.clone(), named)],
        snapshots: Vec::new(),
        next: crate::resolve::Next::Sibling,
    })
}

/// Each named player still in the game loses all unspent mana, whatever it lasts for, and the controller of the effect
/// adds the same mana, each keeping what it may be spent on; an empty pool is no event.
pub(super) fn run_drain(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R119DrainMana { from } = effect else {
        return Err(wrong_effect("R119DrainMana"));
    };
    let state = cx.state();
    let to = cx.controller();
    let mut events = Vec::new();
    for player in cx.scope.players(from)? {
        let Some(pool) = state
            .player(player)
            .filter(|_| state.is_alive(player))
            .map(crate::player::Player::mana)
            .filter(|pool| !pool.is_empty())
        else {
            continue;
        };
        let mut added = Vec::new();
        for mana in ManaType::ALL {
            for _ in 0..pool.get(mana) {
                added.push((mana, Vec::new()));
            }
        }
        added.extend(
            pool.restricted()
                .iter()
                .map(|unit| (unit.mana, unit.only.clone())),
        );
        events.push(Event::ManaPoolEmptied {
            player,
            through: mtg_cards::spend::ManaLasts::Turn,
            kept: Vec::new(),
            becomes: None,
        });
        if state.is_alive(to) {
            events.extend(added.into_iter().map(|(mana, only)| Event::ManaAdded {
                player: to,
                mana,
                source: None,
                only,
                lasts: mtg_cards::spend::ManaLasts::default(),
            }));
        }
    }
    Ok(Run::events(events))
}
