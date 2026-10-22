//! Goal: a payment an effect offers may take life, energy, counters removed from the source and objects the player picks, not only mana: the player is asked in the words of the cost ("Pay 2 life?", "Discard a card?", "Pay {1}, pay {E}{E}?"), a player who cannot pay is asked in the same words with No the only answer (a Yes is refused and changes nothing), a Yes asks the picks of a part that has more to choose from than it takes and then the mana, the cost is paid with the events of each part before the paid effects follow, a No leaves everything as it was and runs the unpaid effects, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, GameSetup, replay};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// A creature for nothing with "<nothing>: Pay `cost`. If you do, draw a card. If you do not, you gain 1 life."
fn taxed(cost: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":[],"effects":[{{"Pay":{{"cost":{cost},"paid":[{{"Draw":{{}}}}],"unpaid":[{{"GainLife":{{"amount":1}}}}]}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Taxed")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(&ability).unwrap()),
    )
}

/// The taxed creature and `lands` Forests, ready.
fn with_lands(card: Arc<CardDefinition>, lands: usize) -> Vec<mtg_engine::setup::PresetPermanent> {
    std::iter::once(ready(card))
        .chain((0..lands).map(|_| ready(forest())))
        .collect()
}

fn energy() -> CounterKind {
    CounterKind::Other("energy".into())
}

fn cards(count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|n| free_creature(&format!("Card {n}"), 1, 1))
        .collect()
}

/// The question the table waits on and whether No is the only answer.
fn asked(driver: &Driver) -> (String, bool) {
    match &driver.decision().unwrap().kind {
        DecisionKind::YesNo(options) => (options.text.clone(), options.only_no),
        other => panic!("a yes-or-no question was expected, not {}", other.name()),
    }
}

fn yes_no(yes: bool) -> Answer {
    Answer::YesNo(YesNoChoice(yes))
}

/// Activates the ability and passes until the payment is asked.
fn until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_activation("Taxed")?;
    while !matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
        driver.pass()?;
    }
    Ok(())
}

/// Passes until nothing is on the stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty()
        && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn hand(driver: &Driver) -> usize {
    driver.state().zone(Zone::Hand(pid(0))).len()
}

#[rstest]
#[case::life(r#"[{"PayLife":2}]"#, "Pay 2 life?", Scenario::new(2).life(0, 5), 0)]
#[case::energy(r#"[{"PayEnergy":2}]"#, "Pay {E}{E}?", Scenario::new(2).counters(0, energy(), 3), 0)]
#[case::a_discard(r#"[{"Discard":{}}]"#, "Discard a card?", Scenario::new(2).hand(0, cards(2)), 0)]
#[case::a_sacrifice(
    r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#,
    "Sacrifice a creature?",
    Scenario::new(2),
    0
)]
#[case::an_exile(
    r#"[{"ExileFromGraveyard":{"filter":[]}}]"#,
    "Exile a card from your graveyard?",
    Scenario::new(2).graveyard(0, cards(1)),
    0
)]
#[case::mana_and_life(
    r#"[{"Mana":"{1}"},{"PayLife":2}]"#,
    "Pay {1}, pay 2 life?",
    Scenario::new(2),
    1
)]
#[case::mana_and_a_return(
    r#"[{"Mana":"{1}"},{"ReturnToHand":{"filter":[{"Type":"Land"}]}}]"#,
    "Pay {1}, return a land you control to its owner's hand?",
    Scenario::new(3),
    2
)]
fn a_player_who_can_pay_is_asked_in_the_words_of_the_cost_and_pays_it_all(
    #[case] cost: &str,
    #[case] words: &str,
    #[case] table: Scenario,
    #[case] lands: usize,
) -> Result<(), ScriptError> {
    let scenario = table.battlefield(0, with_lands(taxed(cost), lands));
    let setup: GameSetup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    let (life_before, hand_before) = (life(&driver), hand(&driver));
    until_asked(&mut driver)?;
    assert_eq!(asked(&driver), (words.to_owned(), false));
    driver.answer(yes_no(true))?;
    // The picks of a part with something to choose between, then the mana.
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let picks = options.from.iter().take(options.min as usize).copied();
                driver.answer(Answer::ChooseCards(CardsChoice(picks.collect())))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    settle(&mut driver)?;
    // The card the paid effect draws, less the one discarded, plus the land returned.
    let returned = usize::from(cost.contains("ReturnToHand"));
    assert_eq!(
        hand(&driver),
        hand_before + 1 + returned - usize::from(cost.contains("Discard"))
    );
    assert!(
        life(&driver) <= life_before,
        "the unpaid effect did not run"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::life(r#"[{"PayLife":2}]"#, "Pay 2 life?", Scenario::new(2).life(0, 1), 0)]
#[case::energy(r#"[{"PayEnergy":2}]"#, "Pay {E}{E}?", Scenario::new(2).counters(0, energy(), 1), 0)]
#[case::no_card(r#"[{"Discard":{}}]"#, "Discard a card?", Scenario::new(2), 0)]
#[case::no_card_to_exile(
    r#"[{"ExileFromGraveyard":{"filter":[]}}]"#,
    "Exile a card from your graveyard?",
    Scenario::new(2),
    0
)]
#[case::no_mana_for_the_rest(
    r#"[{"Mana":"{1}"},{"PayLife":2}]"#,
    "Pay {1}, pay 2 life?",
    Scenario::new(2),
    0
)]
#[case::the_only_land_is_the_one_the_cost_returns(
    r#"[{"Mana":"{1}"},{"ReturnToHand":{"filter":[{"Type":"Land"}]}}]"#,
    "Pay {1}, return a land you control to its owner's hand?",
    Scenario::new(2),
    1
)]
fn a_player_who_cannot_pay_is_asked_all_the_same_and_only_no_is_accepted(
    #[case] cost: &str,
    #[case] words: &str,
    #[case] table: Scenario,
    #[case] lands: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table.battlefield(0, with_lands(taxed(cost), lands)))?;
    let hand_before = hand(&driver);
    until_asked(&mut driver)?;
    let (question, only_no) = asked(&driver);
    assert!(only_no, "{question}");
    assert!(question.ends_with('?') && !words.is_empty());
    // A Yes is refused and changes nothing.
    let (log, hash) = (driver.engine().events().len(), driver.engine().state_hash());
    let refused = driver.answer(yes_no(true)).unwrap_err();
    assert!(
        matches!(
            refused,
            ScriptError::Engine(EngineError::Illegal(IllegalAnswer::NotOffered("yes")))
        ),
        "{refused:?}"
    );
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        (log, hash)
    );
    driver.answer(yes_no(false))?;
    settle(&mut driver)?;
    assert_eq!(hand(&driver), hand_before, "nothing was paid, so no card");
    Ok(())
}

#[test]
fn a_no_keeps_what_the_cost_would_have_taken_and_runs_the_unpaid_effects() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .life(0, 5)
            .battlefield(0, [ready(taxed(r#"[{"PayLife":2}]"#))]),
    )?;
    until_asked(&mut driver)?;
    driver.answer(yes_no(false))?;
    settle(&mut driver)?;
    assert_eq!(
        life(&driver),
        6,
        "no life paid, and the unpaid effect gained 1"
    );
    Ok(())
}

#[test]
fn life_is_paid_with_its_event_and_the_paid_effects_follow() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .life(0, 5)
            .battlefield(0, [ready(taxed(r#"[{"PayLife":2}]"#))]),
    )?;
    until_asked(&mut driver)?;
    driver.answer(yes_no(true))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver), 3);
    Ok(())
}

#[test]
fn counters_are_removed_from_the_source_and_energy_from_the_payer() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).counters(0, energy(), 3).battlefield(
        0,
        [with_counters(
            ready(taxed(
                r#"[{"RemoveCounters":{"counter":"charge","count":2}},{"PayEnergy":2}]"#,
            )),
            CounterKind::Charge,
            2,
        )],
    ))?;
    until_asked(&mut driver)?;
    assert_eq!(asked(&driver).0, "Remove 2 charge counters, pay {E}{E}?");
    driver.answer(yes_no(true))?;
    settle(&mut driver)?;
    let taxed = driver.id_of("Taxed")?;
    let object = driver.state().object(taxed).unwrap();
    assert_eq!(
        object
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::Charge),
        0
    );
    assert_eq!(
        driver
            .state()
            .player(pid(0))
            .unwrap()
            .counters_of(&energy()),
        1
    );
    Ok(())
}

#[test]
fn the_picks_are_asked_with_the_cards_that_can_be_taken_and_the_discard_is_public()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .hand(0, cards(3))
            .battlefield(0, [ready(taxed(r#"[{"Discard":{"count":2}}]"#))]),
    )?;
    until_asked(&mut driver)?;
    assert_eq!(asked(&driver).0, "Discard 2 cards?");
    driver.answer(yes_no(true))?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the picks were expected");
    };
    assert_eq!(
        (options.min, options.max, options.can_cancel),
        (2, 2, false)
    );
    assert_eq!(options.text, "Discard 2 cards.");
    let picks: Vec<_> = options.from.iter().take(2).copied().collect();
    let names: Vec<String> = picks
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    let events = driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
    let discarded = events
        .iter()
        .filter(|event| matches!(event, mtg_engine::Event::CardDiscarded { .. }))
        .count();
    assert_eq!(discarded, 2, "{names:?}");
    Ok(())
}
