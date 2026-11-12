//! Goal: Ketramose, the New Dawn draws its controller a card and makes them lose 1 life once for each batch of cards put
//! into exile from graveyards and/or the battlefield during their turn, at tables of two, three and four seats: cards from a
//! graveyard, a nontoken permanent, and both in one batch trigger once each; a token exiled alone, a card exiled from a
//! hand and an exile on another player's turn trigger nothing; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A free instant with these effects, given as JSON.
fn r307c_instant(effects: &str) -> Arc<CardDefinition> {
    let ability: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":[{effects}]}}}}"#)).unwrap();
    Arc::new(
        CardDefinition::new("Free Exile")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

const GRAVEYARDS: &str = r#"{"Exile":{"what":{"Each":[{"In":"Graveyard"}]}}}"#;
const BEARS: &str =
    r#"{"Exile":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}}"#;
const TOKENS: &str = r#"{"CreateToken":{"token":{"Custom":{"name":"Spark","types":["Creature"],"power":1,"toughness":1}}}},{"Exile":{"what":{"Each":["Token"]}}}"#;
const HAND: &str = r#"{"Exile":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]}}}"#;

/// Passes or answers passively until the stack is empty and seat `seat` holds priority.
fn r307c_settle(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?;
        if decision.player == pid(seat)
            && matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().zone(Zone::Stack).is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack does not settle".to_owned()))
}

#[rstest]
#[case::graveyard_cards(GRAVEYARDS.to_owned(), 0, 1)]
#[case::a_permanent(BEARS.to_owned(), 0, 1)]
#[case::both_in_one_batch(format!(r#"{{"Together":{{"effects":[{GRAVEYARDS},{BEARS}]}}}}"#), 0, 1)]
#[case::two_batches(format!("{GRAVEYARDS},{BEARS}"), 0, 2)]
#[case::a_token_alone(TOKENS.to_owned(), 0, 0)]
#[case::from_a_hand(HAND.to_owned(), 0, 0)]
#[case::on_another_turn(GRAVEYARDS.to_owned(), 1, 0)]
fn r307c_ketramose_draws_once_per_batch_exiled(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: String,
    #[case] active: usize,
    #[case] triggers: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, active, Step::Main1)
        .battlefield(0, [ready(card("Ketramose, the New Dawn").unwrap())])
        .hand(0, [r307c_instant(&effects), bear()])
        .library_named(0, ["Forest"; 6]);
    for seat in 1..seats {
        scenario = scenario
            .battlefield(seat, [ready(bear())])
            .graveyard(seat, [bear(), bear()])
            .library_named(seat, ["Forest"; 6]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let life = driver.state().player(pid(0)).unwrap().life();
    if active != 0 {
        r307c_settle(&mut driver, 0)?;
    }
    driver.cast("Free Exile")?;
    r307c_settle(&mut driver, active)?;
    let cast_and_exiled = usize::from(effects.contains(r#""In":"Hand""#));
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand - 1 - cast_and_exiled + triggers
    );
    let lost = i32::try_from(triggers).unwrap();
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life - lost);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
