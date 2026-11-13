//! Goal: a permission to spend mana as though it were mana of any color for spells the player doesn't own also reaches
//! the abilities of permanents they control but don't own when it says so (`GameRule::SpendManaAs.abilities`), and
//! never the abilities of their own permanents: under Nathan Drake green mana pays {R} for the firebreathing of a
//! stolen creature, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A 2/2 with "{R}: This creature gets +1/+0 until end of turn."
fn r5512_firebreather(name: &str) -> Arc<CardDefinition> {
    let pump: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{R}","effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(pump),
    )
}

/// Priority passes until the stack is empty, payments made automatically and other questions answered passively.
fn r5512_settle(driver: &mut Driver) {
    for _ in 0..80 {
        match driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

#[rstest]
#[case::stolen_under_nathan("Nathan Drake, Treasure Hunter", "Stolen Breather", true)]
#[case::stolen_without_nathan("Grizzly Bears", "Stolen Breather", false)]
#[case::own_under_nathan("Nathan Drake, Treasure Hunter", "Own Breather", false)]
fn r5512_green_pays_red_for_a_stolen_permanent(
    #[values(2, 3, 4)] seats: usize,
    #[case] lord: &str,
    #[case] breather: &str,
    #[case] pays: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card(lord).unwrap()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(forest()),
                ready(r5512_firebreather("Own Breather")),
            ],
        )
        .battlefield(1, [ready(r5512_firebreather("Stolen Breather"))])
        .hand(0, [card("Act of Treason").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let stolen = driver.target_named("Stolen Breather").unwrap();
    driver.begin_cast("Act of Treason").unwrap();
    driver.choose_targets(&[stolen]).unwrap();
    driver
        .pay_tapping(&["Mountain", "Mountain", "Mountain"], 0)
        .unwrap();
    r5512_settle(&mut driver);
    let paid =
        driver.begin_activation(breather).is_ok() && driver.pay_tapping(&["Forest"], 0).is_ok();
    r5512_settle(&mut driver);
    assert_eq!(paid, pays);
    let id = driver.id_of(breather).unwrap();
    let power = characteristics(driver.state(), id).unwrap().power;
    assert_eq!(power, Some(if pays { 3 } else { 2 }));
}

#[rstest]
fn r5512_nathan_exiles_the_top_card_of_each_library(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Nathan Drake, Treasure Hunter").unwrap())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, [card("Grizzly Bears").unwrap(), forest(), forest()]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver
        .attack(&[("Nathan Drake, Treasure Hunter", 1)])
        .unwrap();
    r5512_settle(&mut driver);
    let state = driver.state();
    for player in state.players() {
        let left = state
            .zone(mtg_engine::zones::Zone::Library(player.id()))
            .len();
        assert_eq!(left, 2, "one card of each library is exiled");
    }
    let exiled = state.zone(mtg_engine::zones::Zone::Exile).len();
    let cast = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|object| object.card().name == "Grizzly Bears")
        })
        .count();
    assert_eq!(exiled + cast, seats, "at most one of them was cast");
    assert!(cast <= 1);
}
