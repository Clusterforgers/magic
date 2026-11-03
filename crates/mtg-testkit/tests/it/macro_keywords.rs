//! Goal: the keywords the macro table expands play whole at tables of two, three and four seats as their rules say: bushido pumps its creature by its number when it blocks or becomes blocked and not when it is unblocked, mobilize attacks with its number of tapped Warriors that are sacrificed at the next end step, endure puts counters on a creature that is still there and makes a Spirit token otherwise (of the card, a target and "it"), earthbend makes a land a creature with haste that returns tapped when it dies, and modular enters with counters and moves them to an artifact creature when it dies.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::macros;
use mtg_cards::refs::Ref;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{
    burn, forest, free_creature, keyworded, keyworded_creature, mountain, plains,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

pub(crate) fn macro_keyword(name: &str, amount: u32) -> Keyword {
    Keyword::Macro {
        name: name.to_owned(),
        amount,
    }
}

/// Seat 0 controls these attackers, ready; every other seat has a Wall that deals no damage.
pub(crate) fn table(
    seats: usize,
    attackers: Vec<Arc<CardDefinition>>,
    walls: &[Arc<CardDefinition>],
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, attackers.into_iter().map(ready).collect::<Vec<_>>());
    for seat in 1..seats {
        let own: Vec<_> = if seat == 1 {
            walls.iter().cloned().map(ready).collect()
        } else {
            vec![ready(free_creature(&format!("Wall {seat}"), 0, 4))]
        };
        scenario = scenario.battlefield(seat, own);
    }
    scenario
}

/// An answer a test gives to a decision in place of the passive one; `None` leaves the decision to the passive answer.
pub(crate) type Hook<'a> = &'a dyn Fn(&Driver, &Decision) -> Option<Answer>;

/// `play` with at most one block and the passive answers.
fn fight(
    driver: &mut Driver,
    attacks: &[&str],
    block: Option<(&str, &str)>,
    until: Step,
) -> Result<(), ScriptError> {
    play(driver, attacks, block.as_slice(), &|_, _| None, until)
}

/// Seat 0 attacks seat 1 with `attacks`; seat 1 declares `blocks` (blocker, attacker). The game goes on until `until`
/// with an empty stack; `hook` answers the other decisions first.
pub(crate) fn play(
    driver: &mut Driver,
    attacks: &[&str],
    blocks: &[(&str, &str)],
    hook: Hook<'_>,
    until: Step,
) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    let aimed: Vec<(&str, usize)> = attacks.iter().map(|name| (*name, 1)).collect();
    driver.attack(&aimed)?;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        if let Some(answer) = hook(driver, &decision) {
            driver.answer(answer)?;
            continue;
        }
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                let chosen = if decision.player == pid(1) {
                    blocks
                } else {
                    &[]
                };
                driver.block(chosen)?;
            }
            DecisionKind::Priority(_)
                if driver.state().turn().step() == until && driver.state().stack().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the game did not reach {until:?}");
}

pub(crate) fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

pub(crate) fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// How many permanents the seat controls.
pub(crate) fn permanents(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().controller() == pid(seat))
        .count()
}

#[rstest]
fn bushido_pumps_a_blocked_attacker_by_its_number(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let samurai = keyworded("Samurai", 1, 3, &[macro_keyword("bushido", amount)]);
    let mut driver =
        Driver::from_scenario(table(seats, vec![samurai], &[free_creature("Wall", 0, 4)]))?;
    fight(
        &mut driver,
        &["Samurai"],
        Some(("Wall", "Samurai")),
        Step::EndCombat,
    )?;
    let amount = i32::try_from(amount).unwrap();
    assert_eq!(size(&driver, "Samurai"), (1 + amount, 3 + amount));
    Ok(())
}

#[rstest]
fn bushido_pumps_a_blocker_by_its_number(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] amount: u32,
) -> Result<(), ScriptError> {
    let warrior = keyworded("Warrior", 0, 4, &[macro_keyword("bushido", amount)]);
    let attacker = free_creature("Attacker", 0, 4);
    let mut driver = Driver::from_scenario(table(seats, vec![attacker], &[warrior]))?;
    fight(
        &mut driver,
        &["Attacker"],
        Some(("Warrior", "Attacker")),
        Step::EndCombat,
    )?;
    let amount = i32::try_from(amount).unwrap();
    assert_eq!(size(&driver, "Warrior"), (amount, 4 + amount));
    assert_eq!(size(&driver, "Attacker"), (0, 4));
    Ok(())
}

#[rstest]
fn bushido_does_nothing_for_an_unblocked_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let samurai = keyworded("Samurai", 1, 3, &[macro_keyword("bushido", 2)]);
    let mut driver =
        Driver::from_scenario(table(seats, vec![samurai], &[free_creature("Wall", 0, 4)]))?;
    fight(&mut driver, &["Samurai"], None, Step::EndCombat)?;
    assert_eq!(size(&driver, "Samurai"), (1, 3));
    Ok(())
}

#[rstest]
fn mobilize_attacks_with_tapped_warriors_that_are_sacrificed_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let raider = keyworded("Raider", 1, 1, &[macro_keyword("mobilize", amount)]);
    let mut driver =
        Driver::from_scenario(table(seats, vec![raider], &[free_creature("Wall", 0, 4)]))?;
    fight(&mut driver, &["Raider"], None, Step::EndCombat)?;
    let tokens = usize::try_from(amount).unwrap();
    assert_eq!(permanents(&driver, 0), 1 + tokens);
    // The Raider and every Warrior attacked the same player unblocked.
    assert_eq!(life(&driver, 1), 20 - 1 - i32::try_from(amount).unwrap());
    for seat in 2..seats {
        assert_eq!(life(&driver, seat), 20, "seat {seat}");
    }
    fight_on(&mut driver, Step::End)?;
    assert_eq!(permanents(&driver, 0), 1);
    Ok(())
}

/// A creature with one triggered ability: the trigger `when`, the target specifications `targets` and the effects.
fn triggered(name: &str, when: &str, targets: &str, effects: &[Effect]) -> Arc<CardDefinition> {
    let effects = serde_json::to_string(effects).unwrap();
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{when},"targets":{targets},"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

const ATTACKS: &str = r#"{"Attacks":{"what":["This"]}}"#;
const DIES: &str = r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#;
const MY_CREATURE: &str = r#"[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]"#;
const MY_LAND: &str = r#"[{"what":{"Object":[{"Type":"Land"},{"ControlledBy":"You"}]}}]"#;

/// Answers a target question of seat 0 with the legal target of that name.
fn aim_at(name: &str) -> impl Fn(&Driver, &Decision) -> Option<Answer> + '_ {
    move |driver, decision| {
        let DecisionKind::ChooseTargets(options) = &decision.kind else {
            return None;
        };
        let chosen = options.legal.iter().find(|target| {
            matches!(target, Target::Object(object)
                if driver.state().object(object.id).is_some_and(|o| o.card().name == name))
        })?;
        Some(Answer::ChooseTargets(TargetChoice::Targets(vec![*chosen])))
    }
}

#[rstest]
fn endure_puts_counters_on_a_creature_that_is_still_there(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let endurer = triggered(
        "Endurer",
        ATTACKS,
        "[]",
        &macros::action_effects("endure", amount, &Ref::This).unwrap(),
    );
    let mut driver =
        Driver::from_scenario(table(seats, vec![endurer], &[free_creature("Wall", 0, 4)]))?;
    fight(&mut driver, &["Endurer"], None, Step::EndCombat)?;
    let amount = i32::try_from(amount).unwrap();
    assert_eq!(size(&driver, "Endurer"), (1 + amount, 1 + amount));
    assert_eq!(permanents(&driver, 0), 1);
    Ok(())
}

#[rstest]
fn endure_makes_a_spirit_token_when_the_creature_left(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let mortal = triggered(
        "Mortal",
        DIES,
        "[]",
        &macros::action_effects("endure", amount, &Ref::This).unwrap(),
    );
    let mut driver =
        Driver::from_scenario(table(seats, vec![mortal], &[free_creature("Wall", 3, 4)]))?;
    fight(
        &mut driver,
        &["Mortal"],
        Some(("Wall", "Mortal")),
        Step::EndCombat,
    )?;
    let amount = i32::try_from(amount).unwrap();
    assert_eq!(permanents(&driver, 0), 1);
    assert_eq!(size(&driver, "Spirit"), (amount, amount));
    Ok(())
}

#[rstest]
fn a_target_creature_endures(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let coach = triggered(
        "Coach",
        ATTACKS,
        MY_CREATURE,
        &macros::action_effects("endure", amount, &Ref::Target(0)).unwrap(),
    );
    let scenario = table(
        seats,
        vec![coach, free_creature("Buddy", 1, 1)],
        &[free_creature("Wall", 0, 4)],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let aim = aim_at("Buddy");
    play(&mut driver, &["Coach"], &[], &aim, Step::EndCombat)?;
    let amount = i32::try_from(amount).unwrap();
    assert_eq!(size(&driver, "Buddy"), (1 + amount, 1 + amount));
    assert_eq!(size(&driver, "Coach"), (1, 1));
    Ok(())
}

#[rstest]
fn earthbend_makes_a_land_a_creature_that_returns_tapped_when_it_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let bender = triggered(
        "Bender",
        ATTACKS,
        MY_LAND,
        &macros::action_effects("earthbend", amount, &Ref::Target(0)).unwrap(),
    );
    let scenario = table(seats, Vec::new(), &[free_creature("Wall", 0, 4)])
        .battlefield(0, [ready(bender), ready(forest()), ready(mountain())])
        .hand(0, [burn("Zap", i32::try_from(amount).unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let aim = aim_at("Forest");
    play(&mut driver, &["Bender"], &[], &aim, Step::EndCombat)?;
    let forest = driver.id_of("Forest")?;
    let chars = characteristics(driver.state(), forest).unwrap();
    let amount_i = i32::try_from(amount).unwrap();
    assert_eq!(
        (chars.power, chars.toughness),
        (Some(amount_i), Some(amount_i))
    );
    assert!(chars.types.contains(CardType::Land) && chars.types.contains(CardType::Creature));
    assert!(keywords_of(driver.state(), forest).contains(&&Keyword::Haste));
    fight_on(&mut driver, Step::Main2)?;
    let target = driver.target_named("Forest")?;
    driver.cast_at("Zap", &[target])?;
    fight_on(&mut driver, Step::Main2)?;
    let back = driver.id_of("Forest")?;
    let returned = driver.state().object(back).unwrap();
    assert!(returned.permanent().unwrap().tapped());
    assert!(!characteristics(driver.state(), back).unwrap().is_creature());
    assert_eq!(names(&driver, Zone::Graveyard(pid(0))), ["Zap"]);
    Ok(())
}

#[rstest]
fn modular_moves_its_counters_to_an_artifact_creature_when_it_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
    #[values(true, false)] says_yes: bool,
) -> Result<(), ScriptError> {
    let module = keyworded("Module", 0, 4, &[macro_keyword("modular", amount)]);
    let golem = Arc::new(
        CardDefinition::new("Golem")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    );
    let scenario = table(seats, Vec::new(), &[free_creature("Wall", 9, 9)]).battlefield(
        0,
        [
            with_counters(ready(module), CounterKind::PlusOnePlusOne, amount),
            ready(golem),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let hook = |_: &Driver, decision: &Decision| match &decision.kind {
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(says_yes))),
        _ => None,
    };
    play(
        &mut driver,
        &["Module"],
        &[("Wall", "Module")],
        &hook,
        Step::EndCombat,
    )?;
    let amount = i32::try_from(amount).unwrap();
    let moved = if says_yes { amount } else { 0 };
    assert_eq!(size(&driver, "Golem"), (1 + moved, 1 + moved));
    assert!(driver.id_of("Module").is_err());
    Ok(())
}

#[rstest]
fn modular_enters_with_its_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let module = keyworded_creature("Module", 1, &[], 0, 0, &[macro_keyword("modular", amount)]);
    let scenario = table(seats, vec![plains()], &[free_creature("Wall", 0, 4)]).hand(0, [module]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Module")?;
    fight_on(&mut driver, Step::Main1)?;
    let amount = i32::try_from(amount).unwrap();
    assert_eq!(size(&driver, "Module"), (amount, amount));
    Ok(())
}

/// The names of the cards in a zone, sorted.
fn names(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut found: Vec<String> = driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    found.sort();
    found
}

/// Passes on until `until` with an empty stack.
pub(crate) fn fight_on(driver: &mut Driver, until: Step) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if let DecisionKind::Priority(_) = &driver.decision()?.kind
            && driver.state().turn().step() == until
            && driver.state().stack().is_empty()
        {
            return Ok(());
        }
        if let DecisionKind::Priority(_) = &driver.decision()?.kind {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    panic!("the game did not reach {until:?}");
}
