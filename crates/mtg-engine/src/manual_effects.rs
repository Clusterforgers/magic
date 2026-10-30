//! The manual actions that play an ability by hand: damage from a card of the seat, mana into the seat's pool and a
//! power/toughness change until end of turn, each as the events an effect that does the same produces.

use mtg_cards::spend::ManaLasts;
use mtg_core::card::CardType;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use mtg_core::mana::ManaType;

use crate::characteristics::characteristics;
use crate::combat::DamageTarget;
use crate::damage::{Dealer, dealt_by};
use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
use crate::events::Event;
use crate::manual::{MAX_DAMAGE, MAX_MANA, MAX_STAT, ManualRefusal};
use crate::object::ObjectKind;
use crate::state::GameState;
use crate::zones::Zone;

/// The events of `amount` damage that the seat's card `source` deals to `target`, a player in the game or a creature,
/// planeswalker or battle on the battlefield; lifelink, deathtouch, wither and infect of the source apply.
pub(crate) fn damage(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    target: DamageTarget,
    amount: u32,
) -> Result<Vec<Event>, ManualRefusal> {
    if !(1..=MAX_DAMAGE).contains(&amount) {
        return Err(ManualRefusal::OutOfRange("the damage"));
    }
    let dealer = state
        .object(source)
        .filter(|object| object.kind() != ObjectKind::Ability)
        .ok_or(ManualRefusal::Unknown(source))?;
    if dealer.owner() != player && dealer.controller() != player {
        return Err(ManualRefusal::NotYours);
    }
    // A card in a hidden zone would be named to every seat as the source of the damage.
    if matches!(dealer.zone(), Zone::Hand(_) | Zone::Library(_)) {
        return Err(ManualRefusal::WrongZone {
            object: source,
            zone: dealer.zone(),
        });
    }
    match target {
        DamageTarget::Player(who) => {
            if !state.is_alive(who) {
                return Err(ManualRefusal::NotDamageable);
            }
        }
        DamageTarget::Object(id) => {
            let on_battlefield = state
                .object(id)
                .is_some_and(|object| object.zone() == Zone::Battlefield);
            let takes_damage = characteristics(state, id).is_some_and(|c| {
                c.is_creature()
                    || c.types.contains(CardType::Planeswalker)
                    || c.types.contains(CardType::Battle)
            });
            if !on_battlefield || !takes_damage {
                return Err(ManualRefusal::NotDamageable);
            }
        }
    }
    let events = dealt_by(state, &Dealer::live(state, source), target, amount);
    if events.is_empty() {
        return Err(ManualRefusal::NoChange);
    }
    Ok(events)
}

/// The events of `count` mana of `mana` added to the seat's pool, with no source and no restriction, for the step.
pub(crate) fn mana(
    player: PlayerId,
    mana: ManaType,
    count: u32,
) -> Result<Vec<Event>, ManualRefusal> {
    if !(1..=MAX_MANA).contains(&count) {
        return Err(ManualRefusal::OutOfRange("the amount of mana"));
    }
    Ok((0..count)
        .map(|_| Event::ManaAdded {
            player,
            mana,
            source: None,
            only: Vec::new(),
            lasts: ManaLasts::Step,
        })
        .collect())
}

/// The event of a creature on the battlefield getting +`power`/+`toughness` until end of turn, as an effect the seat
/// controls.
pub(crate) fn pump(
    state: &GameState,
    player: PlayerId,
    object: ObjectId,
    power: i32,
    toughness: i32,
) -> Result<Vec<Event>, ManualRefusal> {
    let range = -MAX_STAT..=MAX_STAT;
    if !range.contains(&power) || !range.contains(&toughness) {
        return Err(ManualRefusal::OutOfRange(
            "the change of power or toughness",
        ));
    }
    if power == 0 && toughness == 0 {
        return Err(ManualRefusal::NoChange);
    }
    let creature = state
        .object(object)
        .filter(|held| held.zone() == Zone::Battlefield)
        .filter(|_| characteristics(state, object).is_some_and(|c| c.is_creature()))
        .ok_or(ManualRefusal::NotACreature(object))?;
    let effect = ContinuousEffect::new(
        EffectId(state.next_effect),
        Selector::Objects(vec![creature.reference()]),
        Modification::ModifyPowerToughness { power, toughness },
        Duration::UntilEndOfTurn,
        state.next_timestamp,
    )
    .begun(player, state.turn().number());
    Ok(vec![Event::EffectCreated {
        effect: effect.into(),
    }])
}
