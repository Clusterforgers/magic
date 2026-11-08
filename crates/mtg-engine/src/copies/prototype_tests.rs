//! Goal: a copy of a prototyped permanent, or of its last-known form after it left, has the prototype's mana cost, power, toughness and colors (CR 718.3c); a card cast for its full cost copies the printed values.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::*;
use crate::choose::Choices;
use crate::events::ZoneCause;
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::snapshot::snapshot;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// A colorless 6/6 for {6} with prototype {1}{R} 2/1.
fn beast() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Proto Beast")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse("{6}").unwrap())
            .with_power_toughness(6, 6)
            .with_rules(Ability::Keyword(Keyword::Prototype {
                cost: Cost::mana("{1}{R}".parse().unwrap()),
                power: 2,
                toughness: 1,
            })),
    )
}

fn cast_and_enter(way: Option<CastWay>) -> (GameState, ObjectRef) {
    let mut game = state(vec![vec![beast()], vec![]], vec![vec![], vec![]]);
    let id = game.zone(Zone::Hand(ME))[0];
    let choices = Choices {
        way,
        ..Choices::default()
    };
    for event in [
        Event::SpellCast {
            player: ME,
            object: id,
            choices,
        },
        Event::ZoneChanged {
            object: id,
            from: Zone::Stack,
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        },
    ] {
        apply_event(&mut game, &event).unwrap();
    }
    let reference = game.object(id).map(GameObject::reference).unwrap();
    (game, reference)
}

#[rstest]
#[case::prototyped(Some(CastWay::Prototype), 2, 2, 1, true)]
#[case::full_cost(None, 6, 6, 6, false)]
fn the_copiable_values_follow_the_way_it_was_cast(
    #[case] way: Option<CastWay>,
    #[case] value: u32,
    #[case] power: i32,
    #[case] toughness: i32,
    #[case] red: bool,
) {
    let (game, reference) = cast_and_enter(way);
    let values = copiable_values(&game, reference, None).unwrap();
    assert_eq!(values.mana_value(), value);
    assert_eq!(
        (values.power, values.toughness),
        (Some(power), Some(toughness))
    );
    assert_eq!(values.colors().iter().count() == 1, red);
}

#[test]
fn a_prototyped_permanent_that_left_is_copied_by_its_last_known_form() {
    let (mut game, reference) = cast_and_enter(Some(CastWay::Prototype));
    let shot = snapshot(&game, reference.id).unwrap();
    let event = Event::ZoneChanged {
        object: reference.id,
        from: Zone::Battlefield,
        to: Zone::Graveyard(ME),
        cause: ZoneCause::Moved,
    };
    apply_event(&mut game, &event).unwrap();
    let values = copiable_values(&game, reference, Some(&shot)).unwrap();
    assert_eq!((values.power, values.toughness), (Some(2), Some(1)));
    assert_eq!(values.mana_value(), 2);
}
