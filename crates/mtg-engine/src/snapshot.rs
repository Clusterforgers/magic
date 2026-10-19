//! Last known information: an object as it was, kept after it has changed or gone (CR 113.7a, 608.2h).

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::{Characteristics, characteristics};
use crate::object::ObjectRef;
use crate::state::GameState;
use crate::zones::Zone;

/// An object as it was at one moment; the engine keeps one in the places listed in `docs/cards-dsl.md` only.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Snapshot {
    /// The object.
    pub id: ObjectId,
    /// Its zone-change counter then: the snapshot describes this object and no later one.
    pub counter: u32,
    /// Who controlled it.
    pub controller: PlayerId,
    /// Who owned it.
    pub owner: PlayerId,
    /// Where it was.
    pub zone: Zone,
    /// What it was, with every continuous effect applied.
    pub characteristics: Characteristics,
    /// The counters on it, by kind (none until counters exist).
    pub counters: Vec<(CounterKind, u32)>,
    /// The objects attached to it (none until attachments exist).
    pub attachments: Vec<ObjectId>,
}

impl Snapshot {
    /// The reference that names the object this snapshot describes.
    pub const fn reference(&self) -> ObjectRef {
        ObjectRef {
            id: self.id,
            counter: self.counter,
        }
    }
}

/// The object `id` as it is now, or `None` when there is no such object.
pub fn snapshot(state: &GameState, id: ObjectId) -> Option<Snapshot> {
    let object = state.object(id)?;
    Some(Snapshot {
        id,
        counter: object.counter(),
        controller: object.controller(),
        owner: object.owner(),
        zone: object.zone(),
        characteristics: characteristics(state, id)?,
        counters: Vec::new(),
        attachments: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a snapshot says what an object was with every continuous effect applied, names the object it describes by its zone-change counter, and the counter moves with every change of zone and with nothing else.
    use mtg_core::card::CardType;
    use mtg_core::ids::EffectId;
    use rstest::rstest;

    use super::*;
    use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::turn::Step;

    fn game() -> GameState {
        state(
            vec![vec![creature("Spell", 0, 0)], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![creature("Ogre", 3, 4)]],
        )
    }

    fn bear(game: &GameState) -> ObjectId {
        game.zone(Zone::Battlefield)[0]
    }

    fn moved(object: ObjectId, from: Zone, to: Zone) -> Event {
        Event::ZoneChanged {
            object,
            from,
            to,
            cause: ZoneCause::Moved,
        }
    }

    #[test]
    fn a_snapshot_records_who_owned_and_controlled_what_and_where() {
        let game = game();
        let shot = snapshot(&game, bear(&game)).unwrap();
        assert_eq!(shot.id, bear(&game));
        assert_eq!(shot.counter, 0);
        assert_eq!((shot.owner, shot.controller), (PlayerId(0), PlayerId(0)));
        assert_eq!(shot.zone, Zone::Battlefield);
        assert!(shot.characteristics.types.contains(CardType::Creature));
        assert_eq!(
            (shot.characteristics.power, shot.characteristics.toughness),
            (Some(2), Some(2))
        );
        assert!(shot.counters.is_empty() && shot.attachments.is_empty());
        assert_eq!(
            shot.reference(),
            game.object(bear(&game)).unwrap().reference()
        );
    }

    #[test]
    fn a_snapshot_has_the_continuous_effects_applied() {
        let mut game = game();
        let id = bear(&game);
        let effect = ContinuousEffect::new(
            EffectId(0),
            Selector::Object(id),
            Modification::ModifyPowerToughness {
                power: 3,
                toughness: 3,
            },
            Duration::UntilEndOfTurn,
            0,
        );
        apply_event(&mut game, &Event::EffectCreated { effect }).unwrap();
        let shot = snapshot(&game, id).unwrap();
        assert_eq!(shot.characteristics.power, Some(5));
        // The printed card did not change; the snapshot is of the derived object.
        assert_eq!(game.object(id).unwrap().card().power, Some(2));
    }

    #[test]
    fn there_is_no_snapshot_of_what_does_not_exist() {
        assert!(snapshot(&game(), ObjectId(999)).is_none());
    }

    #[rstest]
    #[case::to_the_graveyard(Zone::Graveyard(PlayerId(0)), 1)]
    #[case::to_exile(Zone::Exile, 1)]
    #[case::to_a_hand(Zone::Hand(PlayerId(0)), 1)]
    fn every_change_of_zone_makes_a_new_object(#[case] to: Zone, #[case] counter: u32) {
        let mut game = game();
        let id = bear(&game);
        let before = game.object(id).unwrap().reference();
        apply_event(&mut game, &moved(id, Zone::Battlefield, to)).unwrap();
        let after = game.object(id).unwrap().reference();
        assert_eq!(after.counter, counter);
        assert_ne!(before, after);
        // And back again is yet another object.
        apply_event(&mut game, &moved(id, to, Zone::Battlefield)).unwrap();
        assert_eq!(game.object(id).unwrap().counter(), counter + 1);
    }

    #[test]
    fn nothing_but_a_change_of_zone_moves_the_counter() {
        let mut game = game();
        let id = bear(&game);
        let events = [
            Event::PermanentTapped { object: id },
            Event::PermanentUntapped { object: id },
            Event::DamageCleared,
            Event::PriorityGranted {
                player: PlayerId(0),
            },
            Event::StepBegan { step: Step::Main2 },
            Event::LifeGained {
                player: PlayerId(0),
                amount: 3,
            },
        ];
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(game.object(id).unwrap().counter(), 0);
    }

    #[test]
    fn a_spell_and_its_card_are_one_object_whose_counter_moves_when_it_is_cast() {
        let mut game = game();
        let card = game.zone(Zone::Hand(PlayerId(0)))[0];
        assert_eq!(game.object(card).unwrap().counter(), 0);
        apply_event(
            &mut game,
            &Event::SpellCast {
                player: PlayerId(0),
                object: card,
                choices: crate::choose::Choices::default(),
            },
        )
        .unwrap();
        assert_eq!(game.object(card).unwrap().counter(), 1);
        assert_eq!(game.object(card).unwrap().zone(), Zone::Stack);
    }
}
