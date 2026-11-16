//! Goal: the r66-02 pack cards play as printed at two to four seats: Not Dead After All returns the creature that dies
//! tapped with a Wicked Role attached (the Role's own death costs each opponent 1 life), and Druid of Purification lets
//! each player in turn order from its controller choose an artifact or enchantment its controller doesn't control,
//! destroying each one chosen once.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// A corpus card for no mana.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

/// An artifact with nothing else.
fn relic(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Artifact))
}

/// The permanents of this name on the battlefield.
fn on_battlefield(driver: &Driver, name: &str) -> Vec<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(state, *id) == name)
        .collect()
}

/// Passes and answers until the stack is empty: the n-th choice of cards takes the card `picks[n]` names when it is
/// offered (nothing otherwise), any other question its passive answer. Returns the seats asked, in order.
fn settle(driver: &mut Driver, picks: &[&str]) -> Result<Vec<usize>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => driver.pass()?,
            DecisionKind::PayMana(_) => driver.answer(Answer::PayMana(PayChoice::Auto))?,
            DecisionKind::ChooseCards(options) => {
                asked.push(decision.player.index());
                let state = driver.state();
                let wanted = picks.get(asked.len() - 1).copied().unwrap_or("");
                let pick: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| name_of(state, *id) == wanted)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(pick)))?
            }
            _ => driver.answer_passively()?,
        };
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn r6602_not_dead_after_all_returns_the_creature_with_a_wicked_role(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(free_creature("Ogre", 3, 3))])
            .hand(0, [free("Not Dead After All"), free("Doom Blade")]),
    )?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Not Dead After All", &[ogre])?;
    settle(&mut driver, &[])?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Doom Blade", &[ogre])?;
    settle(&mut driver, &[])?;
    let back = on_battlefield(&driver, "Ogre");
    assert_eq!(back.len(), 1, "the Ogre returned");
    let state = driver.state();
    let object = state.object(back[0]).unwrap();
    assert!(object.permanent().unwrap().tapped, "it returns tapped");
    let roles = on_battlefield(&driver, "Wicked Role");
    assert_eq!(roles.len(), 1, "one Wicked Role");
    assert_eq!(state.host_of(roles[0]), Some(back[0]));
    let chars = mtg_engine::characteristics::characteristics(state, back[0]).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(4), Some(4)));
    Ok(())
}

#[rstest]
fn r6602_the_wicked_role_drains_each_opponent_when_it_dies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(free_creature("Ogre", 3, 3))])
            .hand(
                0,
                [
                    free("Not Dead After All"),
                    free("Doom Blade"),
                    free("Doom Blade"),
                ],
            ),
    )?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Not Dead After All", &[ogre])?;
    settle(&mut driver, &[])?;
    for _ in 0..2 {
        let ogre = driver.target_named("Ogre")?;
        driver.cast_at("Doom Blade", &[ogre])?;
        settle(&mut driver, &[])?;
    }
    assert!(on_battlefield(&driver, "Ogre").is_empty(), "the second death keeps it");
    assert!(on_battlefield(&driver, "Wicked Role").is_empty());
    let state = driver.state();
    let start = state.player(pid(0)).life;
    for seat in 1..seats {
        assert_eq!(state.player(pid(seat)).life, start - 1, "seat {seat} lost 1 life");
    }
    Ok(())
}

#[rstest]
#[case::each_takes_another(&["Relic1", "Relic2", "Relic3", "Relic1"], 3)]
#[case::all_take_the_same(&["Relic1", "Relic1", "Relic1", "Relic1"], 1)]
#[case::the_controller_alone(&["Relic1"], 1)]
#[case::nobody_takes_any(&[], 0)]
fn r6602_druid_destroys_what_each_player_chose(
    #[values(2, 3, 4)] seats: usize,
    #[case] picks: &[&str],
    #[case] destroyed: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(relic("Mine"))])
        .hand(0, [free("Druid of Purification")]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(relic(&format!("Relic{seat}")))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Druid of Purification")?;
    // The n-th player asked takes the n-th relic of the list when it is there.
    let asked = settle(&mut driver, picks)?;
    let expected: Vec<usize> = (0..seats).collect();
    assert_eq!(asked, expected, "every player is asked, starting with the controller");
    assert_eq!(on_battlefield(&driver, "Mine").len(), 1, "never its controller's own");
    let left = (1..seats)
        .filter(|seat| on_battlefield(&driver, &format!("Relic{seat}")).is_empty())
        .count();
    let named: std::collections::BTreeSet<&str> = picks
        .iter()
        .take(seats)
        .copied()
        .filter(|name| name[5..].parse::<usize>().is_ok_and(|seat| seat < seats))
        .collect();
    assert_eq!(left, named.len());
    assert!(left <= destroyed);
    Ok(())
}

#[rstest]
fn r6602_kozilek_discards_a_card_of_the_value_x_to_counter_a_spell_of_that_value(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    use mtg_engine::choose::{NumberChoice, Target, TargetChoice};
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(card("Kozilek, the Great Distortion").unwrap()),
                    ready(free_creature("Ogre", 3, 3)),
                ],
            )
            .hand(0, [free("Doom Blade"), relic("Trinket")]),
    )?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Doom Blade", &[ogre])?;
    let blade = driver
        .state()
        .stack()
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Doom Blade")
        .unwrap();
    driver.begin_activation("Kozilek, the Great Distortion")?;
    let mut numbers = Vec::new();
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseNumber(options) => {
                numbers.push((options.min, options.max));
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(0)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let reference = driver.state().object(blade).unwrap().reference();
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
                    Target::Object(reference),
                ])))?;
            }
            DecisionKind::Priority(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    // The only spell and the only card in hand both have mana value 0: X can be nothing else.
    assert!(numbers.iter().all(|range| *range == (0, 0)), "{numbers:?}");
    settle(&mut driver, &[])?;
    assert_eq!(on_battlefield(&driver, "Ogre").len(), 1, "Doom Blade was countered");
    let state = driver.state();
    assert!(state.zone(Zone::Hand(pid(0))).is_empty(), "the Trinket was discarded");
    Ok(())
}
