//! Goal: a spell remembers which permanents made the mana spent on it (`SpendTraits::made_by`), so "whenever you cast
//! ... using mana produced by ~" (`Pred::ManaFrom`) triggers for a matching spell paid in part with the source's mana
//! and not for one paid without it or for a spell of another kind: Barracks of the Thousand makes a Gnome Soldier and
//! The Myriad Pools makes another permanent a copy of the spell until end of turn, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The back face of a transforming card as a card of its own, so it starts on the battlefield.
fn r5512_back(name: &str) -> Arc<CardDefinition> {
    Arc::new(card(name).unwrap().faces[1].clone())
}

/// Priority passes until the stack is empty, payments made automatically and other questions answered passively;
/// a target asked for is the permanent named `target` when given.
fn r5512_settle(driver: &mut Driver, target: Option<&str>) {
    for _ in 0..80 {
        match driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::ChooseTargets(_) if target.is_some() => {
                let at = driver.target_named(target.unwrap()).unwrap();
                driver.choose_targets(&[at]).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// How many permanents have the name.
fn r5512_named(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|object| object.card().name == name)
        })
        .count()
}

#[rstest]
#[case::creature_with_its_mana("Grizzly Bears", &["Barracks of the Thousand", "Forest"], 1)]
#[case::creature_without_its_mana("Grizzly Bears", &["Forest", "Forest"], 0)]
#[case::artifact_with_its_mana("Sol Ring", &["Barracks of the Thousand"], 1)]
#[case::enchantment_with_its_mana("Pacifism", &["Barracks of the Thousand", "Plains"], 0)]
fn r5512_barracks_makes_a_gnome_for_spells_it_paid_for(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] paid_with: &[&str],
    #[case] gnomes: usize,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(r5512_back(
                    "Thousand Moons Smithy // Barracks of the Thousand",
                )),
                ready(forest()),
                ready(forest()),
                ready(plains()),
            ],
        )
        .battlefield(1, [ready(card("Grizzly Bears").unwrap())])
        .hand(0, [card(spell).unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast(spell).unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseTargets(_)
    ) {
        let bears = driver.target_named("Grizzly Bears").unwrap();
        driver.choose_targets(&[bears]).unwrap();
    }
    driver.pay_tapping(paid_with, 0).unwrap();
    r5512_settle(&mut driver, None);
    assert_eq!(r5512_named(&driver, "Gnome Soldier"), gnomes);
}

#[rstest]
#[case::with_its_mana(&["The Myriad Pools", "Forest"], true)]
#[case::without_its_mana(&["Forest", "Forest"], false)]
fn r5512_myriad_pools_copies_the_spell_until_end_of_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] paid_with: &[&str],
    #[case] copies: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(r5512_back("The Everflowing Well // The Myriad Pools")),
                ready(forest()),
                ready(forest()),
                ready(card("Llanowar Elves").unwrap()),
            ],
        )
        .hand(0, [card("Grizzly Bears").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast_tapping("Grizzly Bears", paid_with).unwrap();
    r5512_settle(&mut driver, Some("Llanowar Elves"));
    let bears = if copies { 2 } else { 1 };
    assert_eq!(r5512_named(&driver, "Grizzly Bears"), bears);
    assert_eq!(r5512_named(&driver, "Llanowar Elves"), 2 - bears);
    driver.idle_until(4, Step::Upkeep).unwrap();
    assert_eq!(
        r5512_named(&driver, "Grizzly Bears"),
        1,
        "the copy ends with the turn"
    );
    assert_eq!(r5512_named(&driver, "Llanowar Elves"), 1);
}
