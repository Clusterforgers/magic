//! Goal: casting offers exactly the affordable spells, accepts only exact valid payments, and never taps or spends anything on a rejected or cancelled cast.

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType, ParseManaCostError};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PayOptions, PriorityAction};
use mtg_engine::mana::{ManaSource, Payment};
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp, wastes};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn spell(cost: &str) -> Result<Arc<CardDefinition>, ParseManaCostError> {
    Ok(Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse(cost)?),
    ))
}

fn lands(letters: &str) -> Vec<mtg_engine::PresetPermanent> {
    letters
        .chars()
        .filter_map(|c| match c {
            'W' => Some(ready(plains())),
            'U' => Some(ready(island())),
            'B' => Some(ready(swamp())),
            'R' => Some(ready(mountain())),
            'G' => Some(ready(forest())),
            'C' => Some(ready(wastes())),
            _ => None,
        })
        .collect()
}

fn driver_with(cost: &str, lands_in_play: &str) -> Result<Driver, ScriptError> {
    let card = spell(cost).map_err(|e| ScriptError::Expectation(e.to_string()))?;
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [card])
            .battlefield(0, lands(lands_in_play)),
    )
}

fn pay_options(driver: &Driver) -> Result<PayOptions, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::PayMana(o) => Ok(o.clone()),
        other => Err(ScriptError::UnexpectedDecision {
            wanted: "pay-mana",
            got: other.name().to_owned(),
        }),
    }
}

fn is_castable(driver: &Driver) -> Result<bool, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Priority(o) => Ok(!o.castable.is_empty()),
        other => Err(ScriptError::UnexpectedDecision {
            wanted: "priority",
            got: other.name().to_owned(),
        }),
    }
}

#[rstest]
#[case::generic_and_colored("{1}{G}", "GG", true)]
#[case::generic_with_any_colors("{1}{G}", "GW", true)]
#[case::wrong_colors("{1}{G}", "WW", false)]
#[case::double_colored("{G}{G}", "GG", true)]
#[case::double_colored_one_short("{G}{G}", "GW", false)]
#[case::generic_only("{2}", "WU", true)]
#[case::too_few_lands("{2}", "W", false)]
#[case::no_lands("{G}", "", false)]
#[case::colorless_needs_colorless("{C}", "G", false)]
#[case::colorless_from_wastes("{C}", "C", true)]
#[case::colorless_plus_generic("{1}{C}", "GC", true)]
#[case::colorless_plus_generic_without_wastes("{1}{C}", "GG", false)]
#[case::five_colors("{W}{U}{B}{R}{G}", "WUBRG", true)]
#[case::five_colors_missing_one("{W}{U}{B}{R}{G}", "WUBRR", false)]
#[case::big_generic("{3}{G}{G}", "GGWUB", true)]
#[case::big_generic_missing_green("{3}{G}{G}", "GWWUB", false)]
#[case::free_spell("{0}", "", true)]
fn castable_follows_the_affordability_table(
    #[case] cost: &str,
    #[case] lands_in_play: &str,
    #[case] affordable: bool,
) -> Result<(), ScriptError> {
    let mut driver = driver_with(cost, lands_in_play)?;
    assert_eq!(is_castable(&driver)?, affordable);
    if affordable {
        driver.cast("Spell")?;
        let tapped = lands_in_play.len();
        let total = ManaCost::parse(cost)
            .map_err(|e| ScriptError::Expectation(e.to_string()))?
            .mana_value() as usize;
        let state = driver.state();
        let tapped_count = state
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| state.object(*id)?.permanent())
            .filter(|p| p.tapped())
            .count();
        assert_eq!(
            tapped_count, total,
            "tapped exactly the mana value out of {tapped} lands"
        );
        assert_eq!(state.stack().len(), 1);
        assert!(
            state.player(pid(0)).is_some_and(|p| p.mana().is_empty()),
            "no mana left floating"
        );
    }
    Ok(())
}

#[test]
fn auto_pay_leaves_the_scarce_color_untapped() -> Result<(), ScriptError> {
    let mut driver = driver_with("{1}", "GGGW")?;
    driver.cast("Spell")?;
    let state = driver.state();
    let untapped: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.permanent().is_some_and(|p| !p.tapped()))
        .map(|o| o.card().name.clone())
        .collect();
    assert!(untapped.contains(&"Plains".to_owned()), "{untapped:?}");
    Ok(())
}

#[rstest]
#[case::exact(&["Forest", "Plains"], None)]
#[case::wrong_color(&["Plains", "Plains"], Some("MissingMana"))]
#[case::overpay(&["Forest", "Plains", "Forest"], Some("WrongTotal"))]
#[case::underpay(&["Forest"], Some("WrongTotal"))]
fn manual_payments_must_be_exact(
    #[case] taps: &[&str],
    #[case] rejection: Option<&str>,
) -> Result<(), ScriptError> {
    let mut driver = driver_with("{1}{G}", "GWWG")?;
    driver.begin_cast("Spell")?;
    let before = driver.engine().state_hash();
    let options = pay_options(&driver)?;
    let mut remaining = options.sources.clone();
    let mut chosen = Vec::new();
    for name in taps {
        let pos = remaining
            .iter()
            .position(|o| {
                driver
                    .state()
                    .object(o.source.object)
                    .is_some_and(|ob| ob.card().name == *name)
            })
            .ok_or_else(|| ScriptError::Expectation(format!("no {name} left to tap")))?;
        chosen.push(remaining.remove(pos).source);
    }
    let result = driver.answer(Answer::PayMana(PayChoice::Manual(Payment {
        restricted: Vec::new(),
        pool: vec![],
        taps: chosen,
        life: 0,
    })));
    match rejection {
        None => {
            result?;
            assert_eq!(driver.state().stack().len(), 1);
        }
        Some(kind) => {
            let error = format!("{:?}", result.expect_err("payment must be rejected"));
            assert!(error.contains(kind), "{error}");
            assert_eq!(
                driver.engine().state_hash(),
                before,
                "a rejected payment changes nothing"
            );
            driver.expect_decision(0, "pay-mana")?;
        }
    }
    Ok(())
}

#[test]
fn tapping_the_same_land_twice_or_an_unknown_source_is_rejected() -> Result<(), ScriptError> {
    let mut driver = driver_with("{G}{G}", "GG")?;
    driver.begin_cast("Spell")?;
    let sources = pay_options(&driver)?.sources;
    let first = sources[0].source;
    let twice = Payment {
        restricted: Vec::new(),
        pool: vec![],
        taps: vec![first, first],
        life: 0,
    };
    assert!(matches!(
        driver.answer(Answer::PayMana(PayChoice::Manual(twice))),
        Err(ScriptError::Engine(EngineError::Illegal(_)))
    ));
    let stranger = ManaSource {
        object: mtg_core::ids::ObjectId(9_999),
        ability: 0,
    };
    let unknown = Payment {
        restricted: Vec::new(),
        pool: vec![],
        taps: vec![first, stranger],
        life: 0,
    };
    assert!(
        driver
            .answer(Answer::PayMana(PayChoice::Manual(unknown)))
            .is_err()
    );
    driver.expect_decision(0, "pay-mana")
}

#[test]
fn cancelling_a_cast_changes_nothing() -> Result<(), ScriptError> {
    let mut driver = driver_with("{1}{G}", "GG")?;
    let before = driver.engine().state_hash();
    let log_len = driver.engine().events().len();
    driver.begin_cast("Spell")?;
    assert_eq!(
        driver.engine().state_hash(),
        before,
        "choosing a spell alone does not mutate the game"
    );
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.engine().state_hash(), before);
    assert_eq!(driver.engine().events().len(), log_len);
    driver.expect_decision(0, "priority")?;
    assert!(is_castable(&driver)?);
    Ok(())
}

#[test]
fn the_payment_decision_lists_the_cost_and_every_untapped_source() -> Result<(), ScriptError> {
    let mut driver = driver_with("{1}{G}", "GWR")?;
    driver.tap_for_mana("Mountain")?;
    driver.begin_cast("Spell")?;
    let options = pay_options(&driver)?;
    assert_eq!(
        options.cost,
        ManaCost::parse("{1}{G}").map_err(|e| ScriptError::Expectation(e.to_string()))?
    );
    assert_eq!(
        options.sources.len(),
        2,
        "the tapped Mountain is not offered"
    );
    assert_eq!(
        options.pool.get(ManaType::Red),
        1,
        "its mana floats and may be spent"
    );
    assert!(options.auto_available);
    Ok(())
}

#[test]
fn floating_mana_pays_first_and_may_be_chosen_explicitly() -> Result<(), ScriptError> {
    let mut driver = driver_with("{1}{G}", "GGR")?;
    driver.tap_for_mana("Mountain")?;
    driver.tap_for_mana("Forest")?;
    driver.cast("Spell")?;
    let state = driver.state();
    let untapped = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id)?.permanent())
        .filter(|p| !p.tapped())
        .count();
    assert_eq!(
        untapped, 1,
        "auto-payment used the floating mana instead of tapping more lands"
    );
    assert!(state.player(pid(0)).is_some_and(|p| p.mana().is_empty()));

    let mut driver = driver_with("{1}{G}", "GGR")?;
    driver.tap_for_mana("Mountain")?;
    driver.begin_cast("Spell")?;
    let forest = pay_options(&driver)?
        .sources
        .into_iter()
        .find(|o| o.produces == ManaType::Green)
        .map(|o| o.source)
        .ok_or_else(|| ScriptError::Expectation("no green source".into()))?;
    driver.answer(Answer::PayMana(PayChoice::Manual(Payment {
        restricted: Vec::new(),
        pool: vec![ManaType::Red],
        taps: vec![forest],
        life: 0,
    })))?;
    driver.expect_event("ManaSpent of R and G", |e| {
        matches!(e, Event::ManaSpent { mana, .. } if mana.len() == 2 && mana.contains(&ManaType::Red) && mana.contains(&ManaType::Green))
    })
}

#[test]
fn spending_floating_mana_that_does_not_exist_is_rejected() -> Result<(), ScriptError> {
    let mut driver = driver_with("{G}", "G")?;
    driver.begin_cast("Spell")?;
    let before = driver.engine().state_hash();
    let result = driver.answer(Answer::PayMana(PayChoice::Manual(Payment {
        restricted: Vec::new(),
        pool: vec![ManaType::Green],
        taps: vec![],
        life: 0,
    })));
    assert!(result.is_err());
    assert_eq!(driver.engine().state_hash(), before);
    Ok(())
}

#[test]
fn free_spells_spend_no_mana() -> Result<(), ScriptError> {
    let mut driver = driver_with("{0}", "")?;
    let events = driver.cast("Spell")?;
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::ManaSpent { .. } | Event::ManaAdded { .. }))
    );
    assert!(events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
    Ok(())
}

#[test]
fn only_castable_spells_can_be_started() -> Result<(), ScriptError> {
    let mut driver = driver_with("{G}{G}", "G")?;
    let card = driver.state().zone(Zone::Hand(pid(0)))[0];
    let before = driver.engine().state_hash();
    let result = driver.answer(Answer::Priority(PriorityAction::CastSpell(card)));
    assert!(matches!(
        result,
        Err(ScriptError::Engine(EngineError::Illegal(_)))
    ));
    assert_eq!(driver.engine().state_hash(), before);
    Ok(())
}

fn source_of(driver: &Driver, mana: ManaType) -> Result<ManaSource, ScriptError> {
    pay_options(driver)?
        .sources
        .into_iter()
        .find(|o| o.produces == mana)
        .map(|o| o.source)
        .ok_or_else(|| ScriptError::Expectation(format!("no {mana:?} source")))
}

#[test]
fn completing_a_partial_payment_keeps_the_chosen_land_and_pays_the_rest() -> Result<(), ScriptError>
{
    let mut driver = driver_with("{1}{G}", "WGR")?;
    driver.begin_cast("Spell")?;
    let plains = source_of(&driver, ManaType::White)?;
    let events = driver.answer(Answer::PayMana(PayChoice::Complete(Payment {
        restricted: Vec::new(),
        pool: vec![],
        taps: vec![plains],
        life: 0,
    })))?;
    let tapped: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::PermanentTapped { object } => Some(*object),
            _ => None,
        })
        .collect();
    assert_eq!(
        tapped.len(),
        2,
        "the chosen Plains and the Forest the engine added"
    );
    assert!(tapped.contains(&plains.object));
    driver.expect_event("ManaSpent of W and G", |e| {
        matches!(e, Event::ManaSpent { mana, .. } if mana.len() == 2 && mana.contains(&ManaType::White) && mana.contains(&ManaType::Green))
    })
}

#[rstest]
#[case::wastes_a_colour("{G}{G}", "GGW", &[ManaType::White])]
fn a_partial_payment_that_cannot_be_finished_is_rejected_whole(
    #[case] cost: &str,
    #[case] lands_in_play: &str,
    #[case] chosen: &[ManaType],
) -> Result<(), ScriptError> {
    let mut driver = driver_with(cost, lands_in_play)?;
    driver.begin_cast("Spell")?;
    let before = driver.engine().state_hash();
    let log_len = driver.engine().events().len();
    let taps = chosen
        .iter()
        .map(|m| source_of(&driver, *m))
        .collect::<Result<Vec<_>, _>>()?;
    let result = driver.answer(Answer::PayMana(PayChoice::Complete(Payment {
        restricted: Vec::new(),
        pool: vec![],
        taps,
        life: 0,
    })));
    assert!(matches!(
        result,
        Err(ScriptError::Engine(EngineError::Illegal(_)))
    ));
    assert_eq!(driver.engine().state_hash(), before);
    assert_eq!(driver.engine().events().len(), log_len);
    driver.expect_decision(0, "pay-mana")
}

#[test]
fn a_cancelled_cast_can_be_started_and_paid_again() -> Result<(), ScriptError> {
    let mut driver = driver_with("{G}", "G")?;
    driver.begin_cast("Spell")?;
    assert!(pay_options(&driver)?.can_cancel);
    let events = driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert!(
        events.is_empty(),
        "the log shows nothing of a cancelled cast"
    );
    let in_hand = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(in_hand, 1, "the spell never left the hand");
    driver.cast("Spell")?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    Ok(())
}
