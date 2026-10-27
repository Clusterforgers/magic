//! Goal: the prepare cards that wait for combat damage and the cards that choose a player work as printed at two to four seats: Inspired Skypainter becomes prepared when it enters and again when a token deals combat damage to a player (and not for a creature that is no token), Striding Shotcaller when a creature of its controller does, and its Run the Play puts counters on up to X creatures; Stensian Sanguinist when the creature it gave deathtouch deals combat damage to a player this combat and not otherwise, and its Exsanguinate drains each opponent; Sawhorn Nemesis doubles the damage to the chosen player and the permanents they control, and Saskia the Unyielding repeats the combat damage of her controller's creatures to the chosen player.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::NumberChoice;
use mtg_engine::choose::{OptionChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ready, sick};
use rstest::rstest;

use crate::common::{name_of, names, pid};

const SKYPAINTER: &str = "Inspired Skypainter // Maestro's Gift";
const SHOTCALLER: &str = "Striding Shotcaller // Run the Play";
const SANGUINIST: &str = "Stensian Sanguinist // Exsanguinate";
const SAWHORN: &str = "Sawhorn Nemesis";
const SASKIA: &str = "Saskia the Unyielding";

fn prepared(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(PermanentState::prepared)
}

fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 16]);
    }
    scenario
}

fn resolve_all(driver: &mut Driver) {
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn power(driver: &Driver, id: ObjectId) -> i32 {
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

/// Answers the questions of announcing a prepared spell: X, targets and the payment.
fn announce(driver: &mut Driver, x: u32, targets: &[Target]) {
    loop {
        let kind = driver.decision().unwrap().kind.clone();
        match kind {
            DecisionKind::ChooseNumber(_) => {
                driver
                    .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
                    .unwrap();
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(targets).unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
                return;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

/// Plays on from the declared attack to the second main phase of turn 1, choosing `target` when a trigger asks for one.
fn through_combat(driver: &mut Driver, target: Option<&str>) {
    if let (Some(name), DecisionKind::ChooseTargets(_)) =
        (target, driver.decision().unwrap().kind.clone())
    {
        let target = driver.target_named(name).unwrap();
        driver.choose_targets(&[target]).unwrap();
    }
    driver.idle_until(1, Step::Main2).unwrap();
}

#[rstest]
fn skypainter_is_prepared_by_entering_and_again_by_a_token_that_hurts_a_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] defender: usize,
) {
    if defender >= seats {
        return;
    }
    let mut battlefield = vec![sick(free_creature("Bears", 2, 2))];
    battlefield.extend((0..4).map(|_| ready(island())));
    battlefield.extend((0..3).map(|_| ready(mountain())));
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [SKYPAINTER])
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .cast_tapping("Inspired Skypainter", &["Island", "Mountain"])
        .unwrap();
    resolve_all(&mut driver);
    assert!(prepared(&driver, "Inspired Skypainter"), "it entered");
    // Maestro's Gift copies the Bears; the token has haste, the Bears are summoning sick.
    driver.begin_prepared("Inspired Skypainter").unwrap();
    let bears = driver.target_named("Bears").unwrap();
    announce(&mut driver, 0, &[bears]);
    resolve_all(&mut driver);
    assert!(
        !prepared(&driver, "Inspired Skypainter"),
        "the spell unprepared it"
    );
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| *name == "Bears")
            .count(),
        2
    );
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver.attack(&[("Bears", defender)]).unwrap();
    through_combat(&mut driver, None);
    assert!(
        prepared(&driver, "Inspired Skypainter"),
        "a token hurt a player"
    );
}

#[rstest]
fn a_creature_that_is_no_token_does_not_prepare_the_skypainter(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(mtg_testkit::corpus::card(SKYPAINTER).unwrap()),
            ready(free_creature("Bears", 2, 2)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(!prepared(&driver, "Inspired Skypainter"));
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver
        .attack(&[("Bears", 1), ("Inspired Skypainter", 1)])
        .unwrap();
    through_combat(&mut driver, None);
    assert!(
        life(&driver, 1) < 20,
        "the attackers hurt the defending player"
    );
    assert!(!prepared(&driver, "Inspired Skypainter"));
}

#[rstest]
fn shotcaller_is_prepared_by_combat_damage_and_run_the_play_puts_counters_on_up_to_x_creatures(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] x: u32,
) {
    let mut battlefield = vec![
        ready(mtg_testkit::corpus::card(SHOTCALLER).unwrap()),
        ready(free_creature("Bears", 2, 2)),
        ready(free_creature("Wall", 0, 5)),
    ];
    battlefield.extend([ready(forest()), ready(island())]);
    battlefield.extend((0..3).map(|_| ready(plains())));
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver.attack(&[("Bears", 1)]).unwrap();
    through_combat(&mut driver, None);
    assert!(prepared(&driver, "Striding Shotcaller"));
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_prepared("Striding Shotcaller").unwrap();
    let wanted = ["Bears", "Wall"];
    let targets: Vec<Target> = wanted
        .iter()
        .take(usize::try_from(x).unwrap())
        .map(|name| driver.target_named(name).unwrap())
        .collect();
    announce(&mut driver, x, &targets);
    resolve_all(&mut driver);
    let bears = driver.id_of("Bears").unwrap();
    let wall = driver.id_of("Wall").unwrap();
    assert_eq!(power(&driver, bears), 2 + i32::from(x >= 1));
    assert_eq!(power(&driver, wall), i32::from(x >= 2));
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + 1,
        "it draws a card"
    );
    assert!(!prepared(&driver, "Striding Shotcaller"));
}

#[rstest]
fn shotcaller_stays_unprepared_without_combat_damage(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(mtg_testkit::corpus::card(SHOTCALLER).unwrap()),
            ready(free_creature("Bears", 2, 2)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::Main2).unwrap();
    assert!(!prepared(&driver, "Striding Shotcaller"));
}

/// Seat 0 attacks seat 1 with the Bears; the Sanguinist gives `target` deathtouch.
fn sanguinist_combat(seats: usize, target: &str, blocked: bool) -> Driver {
    let mut battlefield = vec![
        ready(mtg_testkit::corpus::card(SANGUINIST).unwrap()),
        ready(free_creature("Bears", 2, 2)),
        ready(free_creature("Wall", 0, 5)),
    ];
    battlefield.extend((0..2).map(|_| ready(swamp())));
    battlefield.extend((0..3).map(|_| ready(plains())));
    let mut scenario = table(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, battlefield);
    if blocked {
        scenario = scenario.battlefield(1, [ready(free_creature("Guard", 0, 6))]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver.attack(&[("Bears", 1)]).unwrap();
    if blocked {
        // The trigger waits for its target; the defender blocks once the declaration reaches them.
        through_blocks(&mut driver, target);
    } else {
        through_combat(&mut driver, Some(target));
    }
    driver
}

fn through_blocks(driver: &mut Driver, target: &str) {
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseTargets(_)
    ) {
        let target = driver.target_named(target).unwrap();
        driver.choose_targets(&[target]).unwrap();
    }
    for _ in 0..40 {
        if driver.state().turn().step() == Step::Main2 && driver.decision().is_ok() {
            return;
        }
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(&[("Guard", "Bears")]).unwrap();
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision)).unwrap();
            }
        }
    }
}

#[rstest]
#[case::the_attacker_unblocked("Bears", false, true)]
#[case::a_creature_that_stays_home("Wall", false, false)]
#[case::the_attacker_blocked("Bears", true, false)]
fn sanguinist_is_prepared_only_when_the_creature_it_gave_deathtouch_hurts_a_player(
    #[values(2, 3, 4)] seats: usize,
    #[case] target: &str,
    #[case] blocked: bool,
    #[case] expected: bool,
) {
    let driver = sanguinist_combat(seats, target, blocked);
    assert_eq!(prepared(&driver, "Stensian Sanguinist"), expected);
}

#[rstest]
fn exsanguinate_drains_each_opponent_and_gains_the_life_lost(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] x: u32,
) {
    let mut driver = sanguinist_combat(seats, "Bears", false);
    assert!(prepared(&driver, "Stensian Sanguinist"));
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    driver.begin_prepared("Stensian Sanguinist").unwrap();
    announce(&mut driver, x, &[]);
    resolve_all(&mut driver);
    let x = i32::try_from(x).unwrap();
    for (seat, was) in before.iter().enumerate().skip(1) {
        assert_eq!(life(&driver, seat), was - x, "seat {seat}");
    }
    let opponents = i32::try_from(seats - 1).unwrap();
    assert_eq!(life(&driver, 0), before[0] + x * opponents);
}

/// Casts the named card for seat 0 and chooses the player at `pick` as it enters; the turn is turn 1 of seat 0.
fn entered_choosing(
    seats: usize,
    name: &str,
    lands: Vec<mtg_engine::setup::PresetPermanent>,
    pick: usize,
) -> Driver {
    let mut scenario = table(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [name])
        .battlefield(0, lands)
        .hand(1, [burn("Bolt", 3)])
        .battlefield(1, [ready(mountain()), ready(free_creature("Wall", 0, 5))]);
    if seats > 2 {
        scenario = scenario.battlefield(
            seats - 1,
            [ready(free_creature("Far Wall", 0, 5)), ready(mountain())],
        );
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast(name).unwrap();
    for _ in 0..seats {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseOption(_)
        ) {
            break;
        }
        driver.pass().unwrap();
    }
    driver
        .answer(Answer::ChooseOption(OptionChoice(pick)))
        .unwrap();
    driver
}

fn sawhorn_lands() -> Vec<mtg_engine::setup::PresetPermanent> {
    (0..4).map(|_| ready(mountain())).collect()
}

#[rstest]
fn sawhorn_nemesis_doubles_the_damage_to_the_chosen_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] chosen: usize,
    #[values(0, 1, 2, 3)] hit: usize,
) {
    if chosen >= seats || hit >= seats {
        return;
    }
    let mut driver = entered_choosing(seats, SAWHORN, sawhorn_lands(), chosen);
    resolve_all(&mut driver);
    // Seat 1 holds a Bolt and a Mountain; it is seat 0's turn, so seat 0 passes and seat 1 acts.
    driver.pass().unwrap();
    let target = driver.target_player(hit);
    driver.cast_at("Bolt", &[target]).unwrap();
    resolve_all(&mut driver);
    let lost = 20 - life(&driver, hit);
    assert_eq!(
        lost,
        if hit == chosen { 6 } else { 3 },
        "chosen {chosen}, hit {hit}"
    );
}

#[rstest]
fn sawhorn_nemesis_doubles_the_damage_to_the_permanents_of_the_chosen_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(1)] chosen: usize,
) {
    // A Wall of toughness 5 survives 3 damage and dies to the doubled 6; the Wall of another player does not.
    let mut driver = entered_choosing(seats, SAWHORN, sawhorn_lands(), chosen);
    resolve_all(&mut driver);
    driver.pass().unwrap();
    let wall = driver.target_named("Wall").unwrap();
    driver.cast_at("Bolt", &[wall]).unwrap();
    resolve_all(&mut driver);
    assert!(!names(driver.state(), Zone::Battlefield).contains(&"Wall".to_owned()));
    if seats > 2 {
        assert!(names(driver.state(), Zone::Battlefield).contains(&"Far Wall".to_owned()));
    }
}

#[rstest]
fn saskia_repeats_the_combat_damage_of_her_controllers_creatures_to_the_chosen_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] chosen: usize,
    #[values(1, 2, 3)] defender: usize,
) {
    if chosen >= seats || defender >= seats {
        return;
    }
    let lands = vec![
        ready(swamp()),
        ready(mountain()),
        ready(forest()),
        ready(plains()),
    ];
    let mut driver = entered_choosing(seats, SASKIA, lands, chosen);
    resolve_all(&mut driver);
    // Saskia has haste: her controller attacks with her at once.
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver.attack(&[(SASKIA, defender)]).unwrap();
    resolve_all(&mut driver);
    driver.idle_until(1, Step::Main2).unwrap();
    let saskia = driver.id_of(SASKIA).unwrap();
    let hit = power(&driver, saskia);
    assert_eq!(hit, 3);
    let mut expected = vec![20; seats];
    expected[defender] -= hit;
    expected[chosen] -= hit;
    for (seat, expected) in expected.iter().enumerate() {
        assert_eq!(
            life(&driver, seat),
            *expected,
            "chosen {chosen}, defender {defender}, seat {seat}"
        );
    }
}

#[test]
fn the_name_helper_reads_an_object() {
    let scenario = table(2).battlefield(0, [ready(free_creature("Bears", 2, 2))]);
    let driver = Driver::from_scenario(scenario).unwrap();
    let id = driver.id_of("Bears").unwrap();
    assert_eq!(name_of(driver.state(), id), "Bears");
}
