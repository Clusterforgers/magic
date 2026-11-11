//! Goal: splice plays out through the engine at tables of two to four seats: a card with splice onto Arcane is offered while an Arcane spell is cast and its cost can be paid, splicing it reveals it, adds its cost and its targets, the spell resolves with both effects and the card stays in the hand; a card is never offered onto a spell of another kind or past the mana there is, and every game replays from its log.

#![allow(clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::names;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::{Keyword, SpliceOnto};
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CostsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn arcane(card: &Arc<CardDefinition>) -> CardDefinition {
    (**card).clone().with_subtype("Arcane")
}

/// An Arcane instant for {R} that deals `amount` damage to any target.
fn arcane_bolt(name: &str, amount: i32) -> Arc<CardDefinition> {
    Arc::new(arcane(&burn(name, amount)))
}

/// An Arcane instant that deals 2 damage to any target, with splice onto `onto` for {1}{R}.
fn glacial_ray(onto: SpliceOnto) -> Arc<CardDefinition> {
    Arc::new(
        arcane(&burn("Glacial Ray", 2)).with_rules(Ability::Keyword(Keyword::Splice {
            onto,
            cost: Cost::mana("{1}{R}".parse().expect("a cost")),
        })),
    )
}

/// `seats` players; player 0 holds `cast` and the Glacial Ray in hand with `mountains` Mountains, in their main phase.
fn table(seats: usize, cast: Arc<CardDefinition>, onto: SpliceOnto, mountains: usize) -> GameSetup {
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [cast, glacial_ray(onto)])
        .battlefield(0, (0..mountains).map(|_| ready(mountain())))
        .setup()
        .expect("the table is valid")
}

fn start(setup: &GameSetup) -> Driver {
    Driver::new(Engine::new(setup.clone()).expect("the game starts")).expect("a driver")
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .expect("a seat")
        .life()
}

/// Passes until the stack is empty.
fn resolve_all(driver: &mut Driver) {
    for _ in 0..20 {
        if driver.state().zone(Zone::Stack).is_empty() {
            return;
        }
        driver.pass().expect("a pass");
    }
    panic!("the stack does not empty");
}

/// The labels of the costs the pending decision offers, if it asks for costs.
fn offered(driver: &Driver) -> Option<Vec<String>> {
    match &driver.decision().expect("a decision").kind {
        DecisionKind::ChooseCosts(options) => Some(
            options
                .optional
                .iter()
                .map(|cost| cost.label.clone())
                .collect(),
        ),
        _ => None,
    }
}

#[rstest]
fn a_spliced_card_adds_its_effect_and_stays_in_the_hand(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] spliced: bool,
) {
    let setup = table(seats, arcane_bolt("Arcane Bolt", 3), SpliceOnto::Arcane, 3);
    let mut driver = start(&setup);
    let (foe, other) = (seats - 1, 1);
    driver.begin_cast("Arcane Bolt").expect("the cast begins");
    assert_eq!(
        offered(&driver),
        Some(vec!["Splice Glacial Ray {1}{R}".to_owned()])
    );
    let picked = if spliced { vec![0] } else { Vec::new() };
    driver
        .answer(Answer::ChooseCosts(CostsChoice::Costs(picked)))
        .expect("the splice choice");
    driver
        .choose_targets(&[driver.target_player(foe)])
        .expect("the bolt's target");
    if spliced {
        driver
            .choose_targets(&[driver.target_player(other)])
            .expect("the ray's target");
    }
    let cast = driver
        .answer(Answer::PayMana(PayChoice::Auto))
        .expect("the payment");
    let revealed = cast.iter().any(|event| matches!(event, Event::Revealed(_)));
    assert_eq!(revealed, spliced);
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .and_then(mtg_engine::object::GameObject::permanent)
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count();
    assert_eq!(tapped, if spliced { 3 } else { 1 });
    resolve_all(&mut driver);
    let ray = if spliced { 2 } else { 0 };
    if foe == other {
        assert_eq!(life(&driver, foe), 17 - ray);
    } else {
        assert_eq!(life(&driver, foe), 17);
        assert_eq!(life(&driver, other), 20 - ray);
    }
    assert_eq!(
        names(driver.state(), Zone::Hand(PlayerId(0))),
        ["Glacial Ray"]
    );
    let replayed = replay(&setup, driver.engine().events()).expect("the log replays");
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::onto_a_spell_that_is_not_arcane(burn("Plain Bolt", 3), SpliceOnto::Arcane, 3)]
#[case::without_the_mana_for_the_splice(arcane_bolt("Arcane Bolt", 3), SpliceOnto::Arcane, 2)]
#[case::instant_or_sorcery_onto_an_instant(burn("Plain Bolt", 3), SpliceOnto::InstantOrSorcery, 3)]
fn a_card_is_offered_only_onto_its_kind_and_for_mana_there_is(
    #[case] cast: Arc<CardDefinition>,
    #[case] onto: SpliceOnto,
    #[case] mountains: usize,
) {
    let name = cast.name.clone();
    let offers = onto == SpliceOnto::InstantOrSorcery;
    let mut driver = start(&table(2, cast, onto, mountains));
    driver.begin_cast(&name).expect("the cast begins");
    assert_eq!(offered(&driver).is_some(), offers);
    if !offers {
        assert!(matches!(
            driver.decision().expect("a decision").kind,
            DecisionKind::ChooseTargets(_)
        ));
    }
}

#[test]
fn the_spliced_card_resolves_its_own_target() {
    let setup = table(2, arcane_bolt("Arcane Bolt", 3), SpliceOnto::Arcane, 3);
    let mut driver = start(&setup);
    driver.begin_cast("Arcane Bolt").expect("the cast begins");
    driver
        .answer(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))
        .expect("splice");
    let me = driver.target_player(0);
    driver
        .choose_targets(&[driver.target_player(1)])
        .expect("bolt");
    driver.choose_targets(&[me]).expect("ray at its caster");
    driver
        .answer(Answer::PayMana(PayChoice::Auto))
        .expect("the payment");
    resolve_all(&mut driver);
    assert_eq!((life(&driver, 0), life(&driver, 1)), (18, 17));
}
