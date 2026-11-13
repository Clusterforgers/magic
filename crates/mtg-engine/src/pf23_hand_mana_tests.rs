//! Goal: a card of the hand that exiles itself for mana (a Spirit Guide) is a mana source of its owner beside the permanents: offered at priority and for payments, counted for what is castable (never for its own cast), and exiled from the hand when it is used.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaType};
use rstest::rstest;

use crate::events::{Event, ZoneCause};
use crate::fixtures::state;
use crate::mana_ability::{activation, options};
use crate::priority::priority_options;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

const GUIDE: &str = r#"{"Activated":{"from":"Hand","cost":["ExileSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#;

/// A Spirit Guide that costs `cost` to cast.
fn guide(cost: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(GUIDE).unwrap();
    Arc::new(
        CardDefinition::new("Guide")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse(cost).unwrap())
            .with_rules(rules),
    )
}

fn mountain() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(Color::Red)))
}

fn bolt() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Bolt")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{1}{R}").unwrap()),
    )
}

/// Seat 0 holds `hand` and controls `lands` Mountains; seat 1 holds a Spirit Guide of its own.
fn game(hand: Vec<Arc<CardDefinition>>, lands: usize) -> GameState {
    state(
        vec![hand, vec![guide("{R}")]],
        vec![(0..lands).map(|_| mountain()).collect(), vec![]],
    )
}

fn named(game: &GameState, name: &str) -> Vec<ObjectId> {
    game.zone(Zone::Hand(ME))
        .iter()
        .copied()
        .filter(|id| game.object(*id).is_some_and(|o| o.card().name == name))
        .collect()
}

#[rstest]
#[case::hand_source(1, 0, 1, 1)]
#[case::battlefield_source(0, 1, 0, 1)]
#[case::both(1, 1, 1, 2)]
#[case::two_guides(2, 0, 2, 2)]
#[case::neither(0, 0, 0, 0)]
fn a_guide_in_the_hand_is_a_source_beside_the_lands(
    #[case] guides: usize,
    #[case] lands: usize,
    #[case] exiling: usize,
    #[case] sources: usize,
) {
    let game = game((0..guides).map(|_| guide("{R}")).collect(), lands);
    let found = options(&game, ME);
    assert_eq!(found.len(), sources, "{found:?}");
    assert!(found.iter().all(|o| o.produces == ManaType::Red));
    let hand: Vec<_> = found.iter().filter(|o| o.exile).collect();
    assert_eq!(hand.len(), exiling, "the opponent's guide is never mine");
    assert!(
        hand.iter()
            .all(|o| o.tapless && !o.sacrifice && o.gives_up())
    );
    let offered = priority_options(&game, ME).mana_abilities;
    assert_eq!(offered.len(), sources, "used at priority too");
}

#[rstest]
#[case::the_guide_pays_for_another_spell(vec![bolt(), guide("{R}")], 1, &["Bolt", "Guide"])]
#[case::the_lands_alone_fall_short(vec![bolt()], 1, &[])]
#[case::two_guides_pay_for_the_bolt(vec![bolt(), guide("{5}"), guide("{5}")], 0, &["Bolt"])]
#[case::a_guide_never_pays_for_itself(vec![guide("{R}")], 0, &[])]
#[case::a_land_pays_for_the_guide(vec![guide("{R}")], 1, &["Guide"])]
fn the_guide_counts_for_what_is_castable_but_not_for_itself(
    #[case] hand: Vec<Arc<CardDefinition>>,
    #[case] lands: usize,
    #[case] castable: &[&str],
) {
    let game = game(hand, lands);
    let offered = priority_options(&game, ME).castable;
    let mut names: Vec<_> = offered
        .iter()
        .filter_map(|id| game.object(*id).map(|o| o.card().name.clone()))
        .collect();
    names.sort();
    names.dedup();
    assert_eq!(names, castable);
}

#[test]
fn using_the_guide_exiles_it_from_the_hand_and_adds_its_mana() {
    let game = game(vec![guide("{R}")], 0);
    let [id] = named(&game, "Guide")[..] else {
        panic!("one guide in the hand");
    };
    let option = options(&game, ME).into_iter().find(|o| o.exile).unwrap();
    let events = activation(&game, ME, &option, &[]).unwrap();
    // An activation without {T} is logged first, for the abilities that wait for one.
    assert!(matches!(events[0], Event::AbilityActivated { source, .. } if source.id == id));
    assert_eq!(
        events[1..],
        [
            Event::ZoneChanged {
                object: id,
                from: Zone::Hand(ME),
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            },
            Event::ManaAdded {
                player: ME,
                mana: ManaType::Red,
                source: Some(id),
                only: Vec::new(),
                lasts: mtg_cards::spend::ManaLasts::Step,
            },
        ]
    );
}
