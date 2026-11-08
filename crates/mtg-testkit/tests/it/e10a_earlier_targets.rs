//! Goal: a target that depends on the targets chosen before it plays at tables of two to four seats: "another target" leaves out an earlier player, "up to three target cards from their graveyard" offers only the graveyard of the player chosen first (also inside the second mode of a modal spell), and "with total mana value 3 or less" offers only what fits, refuses a set over the total and moves what was chosen; every game replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{ModeChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::c4_control::replays;
use crate::common::{names, pid, spell};

/// "Deals 2 damage to any target and 1 damage to another target."
fn e10a_trail() -> Arc<CardDefinition> {
    spell(
        "Trail",
        CardType::Sorcery,
        r#"[{"what":"AnyTarget"},{"what":{"Another":{"what":"AnyTarget","earlier":[0]}}}]"#,
        r#"[{"DealDamage":{"to":{"Target":0},"amount":2}},{"DealDamage":{"to":{"Target":1},"amount":1}}]"#,
    )
}

/// "Target player shuffles up to three target cards from their graveyard into their library."
const E10A_BLESSING_TARGETS: &str = r#"[{"what":"Player"},{"bounds":{"min":0,"max":3},"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}]"#;
const E10A_BLESSING_EFFECTS: &str =
    r#"[{"Move":{"what":{"Target":1},"to":{"Library":{"place":"Shuffled"}}}}]"#;

/// "Return up to three target creature cards with total mana value 3 or less from your graveyard to the battlefield."
fn e10a_patch() -> Arc<CardDefinition> {
    spell(
        "Patch",
        CardType::Sorcery,
        r#"[{"bounds":{"min":0,"max":3},"what":{"Together":{"rule":{"TotalManaValueAtMost":3},"what":{"Object":[{"Type":"Creature"},{"OwnedBy":"You"},{"In":"Graveyard"}]}}}}]"#,
        r#"[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]"#,
    )
}

/// A creature card whose mana value is `generic`, named after its seat and value.
fn e10a_card(seat: usize, generic: u32) -> Arc<CardDefinition> {
    creature(&format!("Card {seat}-{generic}"), generic, &[], 1, 1)
}

/// Seat 0 holds `spell`; every seat's graveyard holds creature cards of mana value 1 to 4.
fn e10a_table(seats: usize, spell: Arc<CardDefinition>) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).hand(0, [spell]);
    for seat in 0..seats {
        scenario = scenario.graveyard(seat, (1..=4).map(|value| e10a_card(seat, value)));
    }
    scenario
}

/// The graveyard card with this name, as a target.
fn e10a_in_graveyard(driver: &Driver, seat: usize, name: &str) -> Target {
    let state = driver.state();
    let id = state
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .expect("the card is in that graveyard");
    Target::Object(state.object(id).unwrap().reference())
}

/// The pending target decision's legal targets.
fn e10a_legal(driver: &Driver) -> Vec<Target> {
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("a target decision is pending");
    };
    options.legal.clone()
}

/// Pays if asked and passes until the stack is empty.
fn e10a_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn e10a_another_target_leaves_out_the_player_chosen_first(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(e10a_table(seats, e10a_trail()))?;
    driver.begin_cast("Trail")?;
    let first = driver.target_player(1);
    assert!(e10a_legal(&driver).contains(&first));
    driver.choose_targets(&[first])?;
    assert!(!e10a_legal(&driver).contains(&first), "another target");
    let again = driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![first])));
    assert!(again.is_err(), "the player chosen first is refused");
    let second = driver.target_player(seats - 1);
    let second = if second == first {
        driver.target_player(0)
    } else {
        second
    };
    driver.choose_targets(&[second])?;
    e10a_settle(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 18);
    let other = match second {
        Target::Player(player) => player,
        Target::Object(_) => unreachable!(),
    };
    assert_eq!(driver.state().player(other).unwrap().life(), 19);
    replays(&driver);
    Ok(())
}

#[rstest]
fn e10a_their_graveyard_is_the_one_of_the_player_chosen_first(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] chosen: usize,
) -> Result<(), ScriptError> {
    let card = spell(
        "Blessing",
        CardType::Instant,
        E10A_BLESSING_TARGETS,
        E10A_BLESSING_EFFECTS,
    );
    let mut driver = Driver::from_scenario(e10a_table(seats, card))?;
    driver.begin_cast("Blessing")?;
    driver.choose_targets(&[driver.target_player(chosen)])?;
    let legal = e10a_legal(&driver);
    assert_eq!(legal.len(), 4, "only that player's graveyard");
    let picked: Vec<Target> = [1, 3]
        .iter()
        .map(|value| e10a_in_graveyard(&driver, chosen, &format!("Card {chosen}-{value}")))
        .collect();
    assert!(picked.iter().all(|target| legal.contains(target)));
    let other = (chosen + 1) % seats;
    let elsewhere = e10a_in_graveyard(&driver, other, &format!("Card {other}-1"));
    assert!(
        !legal.contains(&elsewhere),
        "not another player's graveyard"
    );
    driver.choose_targets(&picked)?;
    e10a_settle(&mut driver)?;
    let yard = names(driver.state(), Zone::Graveyard(pid(chosen)));
    assert_eq!(
        yard.iter().filter(|name| name.starts_with("Card")).count(),
        2
    );
    assert_eq!(
        names(driver.state(), Zone::Library(pid(chosen)))
            .iter()
            .filter(|name| name.starts_with("Card"))
            .count(),
        2
    );
    replays(&driver);
    Ok(())
}

#[rstest]
fn e10a_a_mode_reads_the_first_target_of_its_own_mode(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"effects":[{{"Modal":{{"choose":{{"min":2,"max":2}},"modes":[{{"targets":[{{"what":"Player"}}],"effects":[{{"LoseLife":{{"who":{{"Target":0}},"amount":1}}}}]}},{{"targets":{E10A_BLESSING_TARGETS},"effects":{E10A_BLESSING_EFFECTS}}}]}}}}]}}}}"#
    ))
    .unwrap();
    let card = Arc::new(
        CardDefinition::new("Command")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    );
    let mut driver = Driver::from_scenario(e10a_table(seats, card))?;
    driver.begin_cast("Command")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![0, 1])))?;
    driver.choose_targets(&[driver.target_player(0)])?;
    let last = seats - 1;
    driver.choose_targets(&[driver.target_player(last)])?;
    let legal = e10a_legal(&driver);
    let theirs = e10a_in_graveyard(&driver, last, &format!("Card {last}-2"));
    let mine = e10a_in_graveyard(&driver, 0, "Card 0-2");
    assert!(
        legal.contains(&theirs),
        "the graveyard of the mode's player"
    );
    assert!(!legal.contains(&mine), "not the first mode's player");
    driver.choose_targets(&[theirs])?;
    e10a_settle(&mut driver)?;
    assert!(names(driver.state(), Zone::Library(pid(last))).contains(&format!("Card {last}-2")));
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 19);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_that_fit(&[1, 2], true)]
#[case::one_that_fits_alone(&[3], true)]
#[case::none(&[], true)]
#[case::over_the_total(&[1, 3], false)]
fn e10a_a_total_mana_value_bounds_the_set(
    #[case] values: &[u32],
    #[case] accepted: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(e10a_table(seats, e10a_patch()))?;
    driver.begin_cast("Patch")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the spell asks for its targets");
    };
    let total = options.total.clone().expect("the options say the total");
    assert_eq!(total.most, 3);
    assert_eq!(
        options.legal.len(),
        3,
        "the card of mana value 4 is over the total alone"
    );
    assert_eq!(total.values.len(), 3);
    let picked: Vec<Target> = values
        .iter()
        .map(|value| e10a_in_graveyard(&driver, 0, &format!("Card 0-{value}")))
        .collect();
    let answered = driver.answer(Answer::ChooseTargets(TargetChoice::Targets(picked.clone())));
    assert_eq!(answered.is_ok(), accepted);
    if !accepted {
        return Ok(());
    }
    e10a_settle(&mut driver)?;
    let field = names(driver.state(), Zone::Battlefield);
    for value in values {
        assert!(field.contains(&format!("Card 0-{value}")));
    }
    assert_eq!(
        field.iter().filter(|name| name.starts_with("Card")).count(),
        values.len()
    );
    replays(&driver);
    Ok(())
}
