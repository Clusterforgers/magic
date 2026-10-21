//! Goal: a cost part with a counted amount ("Pay X life", "Remove X counters", "Sacrifice X creatures", "Discard X cards", "Exile X cards from your graveyard", "Pay X {E}") asks X when the ability is announced, from 0 to the most the player can pay (all their life, the counters on the source, the creatures, cards or energy they have), then asks the picks for that many, pays X of it with the events the engine logs for each part, refuses an X above the most without a change, backs out of at the question with no trace, and replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// What the cost of the Pitcher takes.
#[derive(Debug, Clone, Copy)]
enum Pays {
    Life,
    Counters,
    Sacrifice,
    Discard,
    Exile,
    Energy,
}

impl Pays {
    /// The cost as a card file holds it.
    const fn cost(self) -> &'static str {
        match self {
            Pays::Life => r#"[{"PayLife":"X"}]"#,
            Pays::Counters => r#"[{"RemoveCounters":{"counter":"charge","count":"X"}}]"#,
            Pays::Sacrifice => r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":"X"}}]"#,
            Pays::Discard => r#"[{"Discard":{"count":"X"}}]"#,
            Pays::Exile => r#"[{"ExileFromGraveyard":{"filter":[],"count":"X"}}]"#,
            Pays::Energy => r#"[{"PayEnergy":"X"}]"#,
        }
    }

    /// A table where the player can pay three of it: three life, three counters, three creatures (the Pitcher is one),
    /// three cards in the hand or the graveyard, three energy.
    fn scenario(self, amount: u32) -> Scenario {
        let pitcher = ready(pitcher(self.cost()));
        let cards = |count: u32| -> Vec<Arc<CardDefinition>> {
            (0..count)
                .map(|n| free_creature(&format!("Card {n}"), 1, 1))
                .collect()
        };
        let table = Scenario::new(2);
        match self {
            Pays::Life => table
                .life(0, i32::try_from(amount).unwrap())
                .battlefield(0, [pitcher]),
            Pays::Counters => {
                table.battlefield(0, [with_counters(pitcher, CounterKind::Charge, amount)])
            }
            Pays::Sacrifice => table.battlefield(
                0,
                std::iter::once(pitcher)
                    .chain((1..amount).map(|n| ready(free_creature(&format!("Wolf {n}"), 2, 2)))),
            ),
            Pays::Discard => table.hand(0, cards(amount)).battlefield(0, [pitcher]),
            Pays::Exile => table.graveyard(0, cards(amount)).battlefield(0, [pitcher]),
            Pays::Energy => table
                .counters(0, CounterKind::Other("energy".into()), amount)
                .battlefield(0, [pitcher]),
        }
    }

    /// What has been paid at the table that `scenario(3)` made.
    fn paid(self, driver: &Driver) -> usize {
        let state = driver.state();
        let held = |zone| state.zone(zone).len();
        let player = state.player(pid(0)).unwrap();
        match self {
            Pays::Life => 3 - usize::try_from(player.life()).unwrap(),
            Pays::Counters => {
                let pitcher = driver.id_of("Pitcher").unwrap();
                3 - usize::try_from(
                    state
                        .object(pitcher)
                        .unwrap()
                        .permanent()
                        .unwrap()
                        .counters_of(&CounterKind::Charge),
                )
                .unwrap()
            }
            Pays::Sacrifice | Pays::Discard => held(Zone::Graveyard(pid(0))),
            Pays::Exile => held(Zone::Exile),
            Pays::Energy => {
                3 - usize::try_from(player.counters_of(&CounterKind::Other("energy".into())))
                    .unwrap()
            }
        }
    }
}

/// A creature for nothing: "<cost>: Draw X cards."
fn pitcher(cost: &str) -> Arc<CardDefinition> {
    let ability =
        format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"Draw":{{"count":"X"}}}}]}}}}"#);
    Arc::new(
        CardDefinition::new("Pitcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(&ability).unwrap()),
    )
}

/// The range the pending number question offers.
fn range(driver: &Driver) -> (u32, Option<u32>, String, bool) {
    let DecisionKind::ChooseNumber(options) = &driver.decision().unwrap().kind else {
        panic!("a number was expected, not {:?}", driver.decision());
    };
    (
        options.min,
        options.max,
        options.text.clone(),
        options.can_cancel,
    )
}

fn say(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
        .map(|_| ())
}

/// Picks the first of the cards the pending question offers.
fn pick_first(driver: &mut Driver) -> Result<(), ScriptError> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("cards were expected, not {:?}", driver.decision());
    };
    let picks = options
        .from
        .iter()
        .take(usize::try_from(options.min).unwrap())
        .copied()
        .collect();
    driver
        .answer(Answer::ChooseCards(CardsChoice(picks)))
        .map(|_| ())
}

fn hash(driver: &Driver) -> (usize, u64) {
    (driver.engine().events().len(), driver.engine().state_hash())
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn x_is_announced_from_nothing_to_the_most_the_player_can_pay_and_that_much_is_paid(
    #[values(
        Pays::Life,
        Pays::Counters,
        Pays::Sacrifice,
        Pays::Discard,
        Pays::Exile,
        Pays::Energy
    )]
    pays: Pays,
    #[values(0, 1, 2, 3)] x: u32,
) -> Result<(), ScriptError> {
    let scenario = pays.scenario(3);
    let setup: GameSetup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    let drawn = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_activation("Pitcher")?;
    assert_eq!(
        range(&driver),
        (0, Some(3), "Choose a value for X.".to_owned(), true)
    );
    say(&mut driver, x)?;
    // The picks follow when there is a choice: not for 0, nor when the part takes everything there is.
    while matches!(
        driver.decision().map(|decision| &decision.kind),
        Ok(DecisionKind::ChooseCards(_))
    ) {
        pick_first(&mut driver)?;
    }
    assert_eq!(pays.paid(&driver), usize::try_from(x).unwrap());
    // A player may pay all their life (CR 119.4) and loses the game for it.
    if matches!(pays, Pays::Life) && x == 3 {
        assert!(!driver.state().player(pid(0)).unwrap().is_in_game());
        return Ok(());
    }
    settle(&mut driver)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let discarded = if matches!(pays, Pays::Discard) { x } else { 0 };
    assert_eq!(
        hand,
        drawn - usize::try_from(discarded).unwrap() + usize::try_from(x).unwrap()
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn an_x_above_the_most_is_refused_without_a_change_and_the_question_can_be_backed_out_of(
    #[values(
        Pays::Life,
        Pays::Counters,
        Pays::Sacrifice,
        Pays::Discard,
        Pays::Exile,
        Pays::Energy
    )]
    pays: Pays,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(pays.scenario(3))?;
    let before = hash(&driver);
    driver.begin_activation("Pitcher")?;
    assert!(say(&mut driver, 4).is_err(), "four is more than there is");
    assert_eq!(range(&driver).1, Some(3), "the question is still asked");
    driver.answer(Answer::ChooseNumber(NumberChoice::Cancel))?;
    assert_eq!(hash(&driver), before, "nothing was logged");
    Ok(())
}

#[rstest]
#[case::two_life(Pays::Life, 2)]
#[case::two_counters(Pays::Counters, 2)]
#[case::two_creatures(Pays::Sacrifice, 2)]
#[case::two_cards(Pays::Discard, 2)]
#[case::two_graveyard_cards(Pays::Exile, 2)]
#[case::two_energy(Pays::Energy, 2)]
fn the_most_is_what_the_player_has(
    #[case] pays: Pays,
    #[case] amount: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(pays.scenario(amount))?;
    driver.begin_activation("Pitcher")?;
    assert_eq!(range(&driver).1, Some(amount));
    Ok(())
}

#[test]
fn with_nothing_to_pay_x_is_zero_and_nothing_is_asked() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Pays::Counters.scenario(0))?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let events = driver.begin_activation("Pitcher")?;
    assert!(
        !matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseNumber(_)
        ),
        "{:?}",
        driver.decision()
    );
    assert!(!events.is_empty(), "the ability went on the stack");
    settle(&mut driver)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand,
        "X was 0"
    );
    Ok(())
}

#[test]
fn the_picks_of_a_counted_sacrifice_name_the_creatures_and_what_is_asked() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(Pays::Sacrifice.scenario(3))?;
    driver.begin_activation("Pitcher")?;
    say(&mut driver, 2)?;
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("the picks were expected");
    };
    let mut from: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    from.sort();
    assert_eq!(from, ["Pitcher", "Wolf 1", "Wolf 2"]);
    assert_eq!((options.min, options.max), (2, 2));
    assert_eq!(options.text, "Sacrifice 2 creatures.");
    Ok(())
}
