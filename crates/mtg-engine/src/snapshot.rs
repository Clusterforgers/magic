//! Last known information: an object as it was, kept after it has changed or gone (CR 113.7a, 608.2h).

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::{Characteristics, seen_as};
use crate::keywords::keywords_of;
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
    /// The keywords it had once layer 6 was through with it: those it prints and those effects gave, less those
    /// effects took away (CR 613.1f), which last-known information reads (CR 608.2h).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(type = "Array<unknown>", optional))]
    pub keywords: Vec<Keyword>,
    /// The counters on it, by kind.
    pub counters: Vec<(CounterKind, u32)>,
    /// The objects attached to it, in id order.
    pub attachments: Vec<ObjectId>,
    /// The X it was cast with, as the object remembers it (CR 107.3m): a spell on the stack and the permanent it became.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub x: Option<u32>,
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
    let (characteristics, controller) = seen_as(state, id)?;
    Some(Snapshot {
        id,
        counter: object.counter(),
        controller,
        owner: object.owner(),
        zone: object.zone(),
        characteristics,
        keywords: keywords_of(state, id).into_iter().cloned().collect(),
        counters: object.permanent().map_or_else(
            || {
                object
                    .card_counters
                    .iter()
                    .map(|(kind, count)| (kind.clone(), *count))
                    .collect()
            },
            |permanent| {
                permanent
                    .counters()
                    .map(|(kind, count)| (kind.clone(), count))
                    .collect()
            },
        ),
        attachments: state.attachments_of(id),
        x: object.announced_x(),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a snapshot says what an object was with every continuous effect applied, names the object it describes by its zone-change counter, and the counter moves with every change of zone and with nothing else.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::refs::{Pred, Selector as Select};
    use mtg_core::card::CardType;
    use mtg_core::ids::EffectId;
    use rstest::rstest;

    use super::*;
    use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::keywords::keywords_of;
    use crate::resolve::Scope;
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
            Selector::objects(&game, [id]),
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

    fn printing(keywords: &[Keyword]) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new("Flier")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2);
        for keyword in keywords {
            card = card.with_rules(Ability::Keyword(keyword.clone()));
        }
        Arc::new(card)
    }

    /// Seat 0 controls a creature that prints flying and lifelink.
    fn flier() -> (GameState, ObjectId) {
        let game = state(
            vec![vec![], vec![]],
            vec![
                vec![printing(&[Keyword::Flying, Keyword::Lifelink])],
                vec![],
            ],
        );
        let id = game.zone(Zone::Battlefield)[0];
        (game, id)
    }

    fn change(game: &mut GameState, id: ObjectId, modification: Modification, n: u64) {
        let effect = ContinuousEffect::new(
            EffectId(n),
            Selector::objects(game, [id]),
            modification,
            Duration::UntilEndOfTurn,
            n,
        );
        apply_event(game, &Event::EffectCreated { effect }).unwrap();
    }

    #[rstest]
    #[case::as_printed(&[], &[Keyword::Flying, Keyword::Lifelink])]
    #[case::one_given(
        &[Modification::AddKeyword(Keyword::Haste)],
        &[Keyword::Flying, Keyword::Lifelink, Keyword::Haste]
    )]
    #[case::one_given_twice(
        &[Modification::AddKeyword(Keyword::Haste), Modification::AddKeyword(Keyword::Haste)],
        &[Keyword::Flying, Keyword::Lifelink, Keyword::Haste]
    )]
    #[case::one_printed_given_again(
        &[Modification::AddKeyword(Keyword::Flying)],
        &[Keyword::Flying, Keyword::Lifelink]
    )]
    #[case::one_taken_away(&[Modification::RemoveKeyword(Keyword::Flying)], &[Keyword::Lifelink])]
    #[case::one_taken_away_that_was_not_there(
        &[Modification::RemoveKeyword(Keyword::Haste)],
        &[Keyword::Flying, Keyword::Lifelink]
    )]
    #[case::every_ability_taken_away(&[Modification::LoseAbilities], &[])]
    #[case::given_after_the_loss(
        &[Modification::LoseAbilities, Modification::AddKeyword(Keyword::Haste)],
        &[Keyword::Haste]
    )]
    #[case::given_before_the_loss(
        &[Modification::AddKeyword(Keyword::Haste), Modification::LoseAbilities],
        &[]
    )]
    #[case::given_and_taken_away(
        &[Modification::AddKeyword(Keyword::Haste), Modification::RemoveKeyword(Keyword::Haste)],
        &[Keyword::Flying, Keyword::Lifelink]
    )]
    fn a_snapshot_holds_the_keywords_the_object_had_after_layer_6(
        #[case] effects: &[Modification],
        #[case] expected: &[Keyword],
    ) {
        let (mut game, id) = flier();
        for (n, modification) in effects.iter().enumerate() {
            change(&mut game, id, modification.clone(), n as u64 + 1);
        }
        assert_eq!(snapshot(&game, id).unwrap().keywords, expected);
    }

    #[test]
    fn a_card_that_is_not_a_permanent_has_the_keywords_it_prints() {
        let game = state(
            vec![vec![printing(&[Keyword::Haste])], vec![]],
            vec![vec![], vec![]],
        );
        let card = game.zone(Zone::Hand(PlayerId(0)))[0];
        assert_eq!(snapshot(&game, card).unwrap().keywords, [Keyword::Haste]);
    }

    #[test]
    fn the_keywords_are_left_out_of_the_data_while_there_are_none() {
        let (game, id) = flier();
        let with = serde_json::to_value(snapshot(&game, id).unwrap()).unwrap();
        assert_eq!(with["keywords"], serde_json::json!(["Flying", "Lifelink"]));
        let bare = state(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        );
        let none = serde_json::to_value(snapshot(&bare, bare.zone(Zone::Battlefield)[0]).unwrap())
            .unwrap();
        assert!(none.get("keywords").is_none(), "{none}");
        let again: Snapshot = serde_json::from_value(with).unwrap();
        assert_eq!(again, snapshot(&game, id).unwrap());
    }

    #[rstest]
    #[case::given_before_it_left(
        Some(Modification::AddKeyword(Keyword::Haste)),
        Keyword::Haste,
        true
    )]
    #[case::printed_and_kept(None, Keyword::Flying, true)]
    #[case::printed_and_taken_away(
        Some(Modification::RemoveKeyword(Keyword::Flying)),
        Keyword::Flying,
        false
    )]
    #[case::printed_and_lost(Some(Modification::LoseAbilities), Keyword::Lifelink, false)]
    #[case::never_had(None, Keyword::Haste, false)]
    fn last_known_information_reads_the_keywords_the_object_had_not_the_ones_its_card_prints(
        #[case] effect: Option<Modification>,
        #[case] keyword: Keyword,
        #[case] had: bool,
    ) {
        let (mut game, id) = flier();
        if let Some(modification) = effect {
            change(&mut game, id, modification, 1);
        }
        let source = game.object(id).unwrap().reference();
        let shot = snapshot(&game, id).unwrap();
        apply_event(
            &mut game,
            &moved(id, Zone::Battlefield, Zone::Graveyard(PlayerId(0))),
        )
        .unwrap();
        // The card in the graveyard prints flying and lifelink whatever happened to the permanent it was.
        assert_eq!(
            keywords_of(&game, id),
            [&Keyword::Flying, &Keyword::Lifelink]
        );
        let scope = Scope {
            snapshots: vec![&shot],
            ..Scope::announcing(&game, PlayerId(0), source)
        };
        let selector = Select(vec![Pred::HasKeyword(keyword)]);
        assert_eq!(scope.selects(&selector, shot.reference()).unwrap(), had);
    }
}
