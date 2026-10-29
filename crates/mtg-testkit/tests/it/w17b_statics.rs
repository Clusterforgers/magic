//! Goal: the static rules of Herald of Eternal Dawn, Dragonlord Dromoka, Ground Seal, Skullbriar and combat damage by
//! toughness play through the engine at two to four seats: a player at 0 life keeps playing while they can't lose and
//! loses once the rule is gone, opponents can't cast spells during the controller's turn but can on their own, cards in
//! graveyards are no legal targets, counters remain on a card that dies (a new object, CR 400.7) and not on one without
//! the rule, and a creature assigns combat damage by its toughness with trample and first strike; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector, Targetable, ZoneKind};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::Duration;
use mtg_engine::replay;
use mtg_engine::rules::Rule;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature, keyworded_creature, mountain, one_target};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

/// The log of the driver replays to its state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Whether the seat is still in the game.
fn alive(driver: &Driver, seat: usize) -> bool {
    driver.state().is_alive(pid(seat))
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn w17b_a_player_at_zero_life_plays_on_and_loses_when_the_herald_dies(#[case] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .life(0, 0)
        .battlefield(
            0,
            [
                PresetPermanent::new(card("Herald of Eternal Dawn").unwrap()),
                PresetPermanent::new(mountain()),
            ],
        )
        .hand(0, [burn("Bolt", 6)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    assert!(
        alive(&driver, 0),
        "the Herald keeps its controller in the game"
    );
    let herald = driver.target_named("Herald of Eternal Dawn").unwrap();
    driver.cast_at("Bolt", &[herald]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert!(!alive(&driver, 0), "the loss happens once the rule is gone");
    replays(&scenario, &driver);
}

#[rstest]
#[case::two_on_my_turn(2, 0, false)]
#[case::two_on_theirs(2, 1, true)]
#[case::three_on_my_turn(3, 0, false)]
#[case::four_on_a_third_turn(4, 2, true)]
fn w17b_opponents_cast_only_off_the_dragonlords_turn(
    #[case] seats: usize,
    #[case] active: usize,
    #[case] casts: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, active, Step::Main1)
        .battlefield_named(0, ["Dragonlord Dromoka"])
        .hand(1, [burn("Bolt", 1)])
        .battlefield(1, [PresetPermanent::new(mountain())]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    while driver.engine().decision().map(|d| d.player) != Some(pid(1)) {
        driver.pass().unwrap();
    }
    assert_eq!(driver.begin_cast("Bolt").is_ok(), casts);
    replays(&scenario, &driver);
}

#[rstest]
#[case::sealed(true, 0)]
#[case::open(false, 1)]
fn w17b_cards_in_graveyards_are_no_targets_under_ground_seal(
    #[case] sealed: bool,
    #[case] legal: usize,
) {
    let mut scenario = Scenario::new(3)
        .at(3, 0, Step::Main1)
        .graveyard(2, [creature("Corpse", 1, &[], 2, 2)]);
    if sealed {
        scenario = scenario.battlefield_named(1, ["Ground Seal"]);
    }
    let driver = Driver::from_scenario(scenario).unwrap();
    let spec = one_target(Targetable::Object(Selector(vec![Pred::In(
        ZoneKind::Graveyard,
    )])));
    let source = driver.state().zone(Zone::Graveyard(pid(2)))[0];
    let targets = mtg_engine::choose::legal_targets(driver.state(), &spec, source, pid(0));
    assert_eq!(
        targets
            .iter()
            .filter(|t| matches!(t, Target::Object(_)))
            .count(),
        legal
    );
}

#[rstest]
#[case::skullbriar("Skullbriar, the Walking Grave", 2)]
#[case::a_creature_without_the_rule("Grizzly", 0)]
fn w17b_counters_remain_on_a_card_that_dies(#[case] name: &str, #[case] kept: u32) {
    let card = if name == "Grizzly" {
        creature("Grizzly", 1, &[], 1, 1)
    } else {
        card(name).unwrap()
    };
    let mut permanent = PresetPermanent::new(card);
    permanent.counters = vec![(CounterKind::PlusOnePlusOne, 2)];
    let scenario = Scenario::new(2)
        .at(3, 1, Step::Main1)
        .battlefield(0, [permanent])
        .hand(1, [burn("Bolt", 5)])
        .battlefield(1, [PresetPermanent::new(mountain())]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let id = driver.id_of(name).unwrap();
    let before = driver.state().object(id).unwrap().counter();
    let target = Target::Object(driver.state().object(id).unwrap().reference());
    driver.cast_at("Bolt", &[target]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let dead = driver.state().object(id).unwrap();
    assert_eq!(dead.zone(), Zone::Graveyard(pid(0)));
    assert_ne!(dead.counter(), before, "a new object (CR 400.7)");
    assert_eq!(dead.card_counters_of(&CounterKind::PlusOnePlusOne), kept);
    replays(&scenario, &driver);
}

#[rstest]
#[case::unblocked_by_toughness(true, &[], 5)]
#[case::unblocked_by_power(false, &[], 1)]
#[case::trample_over_a_blocker(true, &[Keyword::Trample], 3)]
#[case::first_strike_by_toughness(true, &[Keyword::FirstStrike], 5)]
fn w17b_combat_damage_by_toughness(
    #[case] rule: bool,
    #[case] keywords: &[Keyword],
    #[case] lost: i32,
) {
    let blocked = keywords.contains(&Keyword::Trample);
    let mut scenario = Scenario::new(2).at(3, 0, Step::Main1).battlefield(
        0,
        [PresetPermanent::new(keyworded_creature(
            "Wall",
            1,
            &[],
            1,
            5,
            keywords,
        ))],
    );
    if blocked {
        scenario = scenario.battlefield(1, [PresetPermanent::new(creature("Chump", 1, &[], 1, 2))]);
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    if rule {
        driver
            .add_rule(
                Rule::ToughnessAssignsDamage(vec![pid(0)]),
                Duration::Forever,
            )
            .unwrap();
    }
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Wall", 1)]).unwrap();
    if blocked {
        driver.pass_until(Step::DeclareBlockers).unwrap();
        driver.block(&[("Chump", "Wall")]).unwrap();
    }
    let life = driver.state().player(pid(1)).unwrap().life();
    if blocked {
        // Lethal damage to the 1/2 blocker, the rest of the toughness tramples over.
        while !matches!(
            driver.engine().decision().map(|d| &d.kind),
            Some(DecisionKind::AssignDamage(_))
        ) {
            driver.pass().unwrap();
        }
        driver.assign_damage(&[("Chump", 2)]).unwrap();
    }
    driver.idle_until(3, Step::End).unwrap();
    assert_eq!(life - driver.state().player(pid(1)).unwrap().life(), lost);
    replays(&scenario, &driver);
}
