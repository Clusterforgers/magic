//! Goal: the prepare cards whose triggers need more than an attack work as printed at two to four seats: Dirgur Focusmage becomes prepared when an instant or sorcery spell with mana value 5 or greater is cast from the hand and not for a smaller spell or another kind, Naktamun Lorespinner when a player has one or fewer cards in hand at its controller's upkeep, Lorehold Archivist with three artifact and/or creature cards in the graveyard and its Restore Relic exiles one and returns a token copy, and their spell faces (Braingeyser, Wheel of Fortune) do what they print.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{names, pid};

const DIRGUR: &str = "Dirgur Focusmage // Braingeyser";
const NAKTAMUN: &str = "Naktamun Lorespinner // Wheel of Fortune";
const LOREHOLD: &str = "Lorehold Archivist // Restore Relic";

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

/// A spell of this kind that gains its caster a life, costing `generic` mana.
fn spell(name: &str, kind: CardType, generic: u32) -> Arc<CardDefinition> {
    let gain: Ability =
        serde_json::from_str(r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#).unwrap();
    let card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(cost(generic, &[]));
    Arc::new(if kind == CardType::Creature {
        card.with_power_toughness(1, 1)
    } else {
        card.with_rules(gain)
    })
}

/// Answers the questions of announcing a prepared spell: X, targets and the payment.
fn announce(driver: &mut Driver, x: u32, targets: &[mtg_engine::choose::Target]) {
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

#[rstest]
#[case::an_instant_of_five(CardType::Instant, 5, true)]
#[case::a_sorcery_of_six(CardType::Sorcery, 6, true)]
#[case::an_instant_of_four(CardType::Instant, 4, false)]
#[case::a_sorcery_of_three(CardType::Sorcery, 3, false)]
#[case::a_creature_of_six(CardType::Creature, 6, false)]
fn dirgur_becomes_prepared_for_a_big_instant_or_sorcery_cast_from_the_hand(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: CardType,
    #[case] mana_value: u32,
    #[case] expected: bool,
) {
    // Instants and sorceries cost {1} less beside the Focusmage, creatures do not.
    let paid = if kind == CardType::Creature {
        mana_value
    } else {
        mana_value - 1
    };
    let lands = (0..paid).map(|_| ready(island())).collect::<Vec<_>>();
    let mut battlefield = vec![ready(mtg_testkit::corpus::card(DIRGUR).unwrap())];
    battlefield.extend(lands);
    let scenario = table(seats)
        .hand(0, [spell("Big Spell", kind, mana_value)])
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .cast_tapping("Big Spell", &vec!["Island"; usize::try_from(paid).unwrap()])
        .unwrap();
    resolve_all(&mut driver);
    assert_eq!(prepared(&driver, "Dirgur Focusmage"), expected);
}

#[rstest]
fn the_prepared_braingeyser_draws_x_cards_for_the_target_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 3)] x: u32,
    #[values(0, 1)] target: usize,
) {
    let mut battlefield = vec![ready(mtg_testkit::corpus::card(DIRGUR).unwrap())];
    battlefield.extend((0..2 + x).map(|_| ready(island())));
    let scenario = table(seats)
        .hand(0, [spell("Big Spell", CardType::Sorcery, 5)])
        .battlefield(
            0,
            battlefield
                .into_iter()
                .chain((0..4).map(|_| ready(island()))),
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast_tapping("Big Spell", &["Island"; 4]).unwrap();
    resolve_all(&mut driver);
    assert!(prepared(&driver, "Dirgur Focusmage"));
    let before = driver.state().zone(Zone::Hand(pid(target))).len();
    driver.begin_prepared("Dirgur Focusmage").unwrap();
    let who = driver.target_player(target);
    announce(&mut driver, x, &[who]);
    resolve_all(&mut driver);
    assert!(!prepared(&driver, "Dirgur Focusmage"));
    let drawn = driver.state().zone(Zone::Hand(pid(target))).len() - before;
    assert_eq!(drawn, usize::try_from(x).unwrap());
}

/// Seat 0 holds the Lorespinner and the other seats hold these many cards; it is the upkeep of seat 0's next turn.
fn at_upkeep(seats: usize, hands: &[usize], lands: usize) -> Driver {
    let mut scenario = table(seats).at(1, 0, Step::Main1).battlefield(
        0,
        std::iter::once(ready(mtg_testkit::corpus::card(NAKTAMUN).unwrap()))
            .chain((0..lands).map(|_| ready(mountain()))),
    );
    for (seat, size) in hands.iter().enumerate() {
        scenario = scenario.hand(seat, vec![forest(); *size]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Upkeep).unwrap();
    resolve_all(&mut driver);
    driver
}

#[rstest]
#[case::mine_is_empty(&[0, 5, 5, 5])]
#[case::mine_has_one(&[1, 5, 5, 5])]
#[case::mine_has_two(&[2, 5, 5, 5])]
#[case::an_opponents_is_empty(&[5, 0, 5, 5])]
#[case::an_opponents_has_one(&[5, 1, 5, 5])]
#[case::the_last_has_one(&[5, 5, 5, 1])]
#[case::everyones_has_two(&[2, 2, 2, 2])]
fn naktamun_becomes_prepared_when_any_player_has_one_or_fewer_cards_in_hand(
    #[values(2, 3, 4)] seats: usize,
    #[case] hands: &[usize],
) {
    // A table of two has no third or fourth hand; each opponent has drawn a card on their turn before this upkeep.
    let hands = &hands[..seats];
    let driver = at_upkeep(seats, hands, 3);
    let any_small = hands
        .iter()
        .enumerate()
        .any(|(seat, size)| *size + usize::from(seat > 0) <= 1);
    assert_eq!(
        prepared(&driver, "Naktamun Lorespinner"),
        any_small,
        "{hands:?}"
    );
}

#[rstest]
fn the_prepared_wheel_of_fortune_gives_every_player_seven_new_cards(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut hands = vec![4; seats];
    hands[1] = 0;
    let mut driver = at_upkeep(seats, &hands, 3);
    assert!(prepared(&driver, "Naktamun Lorespinner"));
    // The upkeep of seat 0 is over after the draw step: the spell is a sorcery.
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1).unwrap();
    let draws = u32::try_from(seats).unwrap();
    let _ = draws;
    driver.cast_prepared("Naktamun Lorespinner").unwrap();
    resolve_all(&mut driver);
    for seat in 0..seats {
        assert_eq!(
            driver.state().zone(Zone::Hand(pid(seat))).len(),
            7,
            "seat {seat}"
        );
    }
}

fn graveyard_cards(kinds: &[&str]) -> Vec<Arc<CardDefinition>> {
    kinds
        .iter()
        .enumerate()
        .map(|(at, kind)| match *kind {
            "creature" => free_creature(&format!("Dead Creature {at}"), 2, 2),
            "artifact" => Arc::new(
                CardDefinition::new(format!("Dead Artifact {at}")).with_type(CardType::Artifact),
            ),
            "land" => swamp(),
            _ => spell(&format!("Dead Instant {at}"), CardType::Instant, 1),
        })
        .collect()
}

fn lorehold_upkeep(seats: usize, kinds: &[&str]) -> Driver {
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            std::iter::once(ready(mtg_testkit::corpus::card(LOREHOLD).unwrap())).chain([
                ready(mountain()),
                ready(plains()),
                ready(mountain()),
                ready(plains()),
            ]),
        )
        .graveyard(0, graveyard_cards(kinds));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Upkeep).unwrap();
    resolve_all(&mut driver);
    driver
}

#[rstest]
#[case::three_creatures(&["creature", "creature", "creature"], true)]
#[case::artifacts_and_creatures(&["artifact", "creature", "artifact"], true)]
#[case::three_artifacts(&["artifact", "artifact", "artifact"], true)]
#[case::two_creatures(&["creature", "creature"], false)]
#[case::other_cards_do_not_count(&["creature", "creature", "land", "instant"], false)]
#[case::an_empty_graveyard(&[], false)]
fn lorehold_becomes_prepared_with_three_artifact_and_or_creature_cards_in_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[case] kinds: &[&str],
    #[case] expected: bool,
) {
    let driver = lorehold_upkeep(seats, kinds);
    assert_eq!(prepared(&driver, "Lorehold Archivist"), expected);
}

#[rstest]
fn restore_relic_exiles_a_card_and_returns_a_token_copy_of_it(
    #[values(2, 3, 4)] seats: usize,
    #[values("creature", "artifact")] kind: &str,
) {
    let mut driver = lorehold_upkeep(seats, &[kind, "creature", "creature"]);
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1).unwrap();
    assert!(prepared(&driver, "Lorehold Archivist"));
    let wanted = if kind == "creature" {
        "Dead Creature 0"
    } else {
        "Dead Artifact 0"
    };
    driver.begin_prepared("Lorehold Archivist").unwrap();
    // The graveyard card of the kind is the first of the targets offered.
    let target = {
        let DecisionKind::ChooseTargets(_) = driver.decision().unwrap().kind else {
            panic!("a target is chosen");
        };
        let id = in_zone(&driver, Zone::Graveyard(pid(0)), wanted);
        mtg_engine::choose::Target::Object(driver.state().object(id).unwrap().reference())
    };
    announce(&mut driver, 0, &[target]);
    resolve_all(&mut driver);
    assert!(names(driver.state(), Zone::Exile).contains(&wanted.to_owned()));
    assert!(!names(driver.state(), Zone::Graveyard(pid(0))).contains(&wanted.to_owned()));
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| *name == wanted)
            .count(),
        1,
        "a token copy"
    );
    assert!(!prepared(&driver, "Lorehold Archivist"));
}

/// The object called `name` in a zone.
fn in_zone(driver: &Driver, zone: Zone, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| crate::common::name_of(driver.state(), *id) == name)
        .unwrap()
}
