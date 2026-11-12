//! Goal: previewing a pending attack or block gives the forecast the real answer then leaves behind (single, double and chump blocks, no blocks, attacks that are lethal), refuses what the real answer refuses, previews nothing else, and leaves the game exactly as it was.

#![allow(clippy::expect_used)]

use crate::common;

use common::combat::{Fighter, creatures, to_blockers};
use common::{name_of, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::forecast::{CombatForecast, pending_combat_forecast};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, Engine, EngineError, PreviewError};
use mtg_testkit::cards::{bear, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;

/// What an engine shows from outside; a preview must leave all of it as it was.
#[derive(Debug, PartialEq, Eq)]
struct Outside {
    hash: u64,
    events: usize,
    decision: Option<Decision>,
}

fn outside(engine: &Engine) -> Outside {
    Outside {
        hash: engine.state_hash(),
        events: engine.events().len(),
        decision: engine.decision().cloned(),
    }
}

/// Player 0 is about to declare attackers; `attackers` are theirs, `blockers` belong to player 1.
fn facing_attack(
    attackers: &[Fighter],
    blockers: &[Fighter],
    life: i32,
) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, creatures(attackers))
            .battlefield(1, creatures(blockers))
            .life(1, life),
    )
}

/// Player 0 attacked player 1 with every attacker, and player 1 now faces the blockers decision.
fn facing_blocks(
    attackers: &[Fighter],
    blockers: &[Fighter],
    life: i32,
) -> Result<Driver, ScriptError> {
    let mut driver = facing_attack(attackers, blockers, life)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|fighter| (fighter.0, 1)).collect();
    driver.attack(&attacks)?;
    to_blockers(&mut driver, 1)?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::DeclareBlockers(_)
    ));
    Ok(driver)
}

fn id(driver: &Driver, name: &str) -> ObjectId {
    driver.id_of(name).expect("a creature of that name")
}

/// The answer that makes these (blocker, attacker) blocks.
fn blocks(driver: &Driver, pairs: &[(&str, &str)]) -> Answer {
    Answer::DeclareBlockers(
        pairs
            .iter()
            .map(|(blocker, attacker)| (id(driver, blocker), id(driver, attacker)))
            .collect(),
    )
}

/// The answer that sends these (attacker, defending seat) attacks.
fn attacks(driver: &Driver, pairs: &[(&str, usize)]) -> Answer {
    Answer::DeclareAttackers(
        pairs
            .iter()
            .map(|(attacker, seat)| (id(driver, attacker), AttackTarget::Player(pid(*seat))))
            .collect(),
    )
}

/// Previews `answer` as `seat`, checks that the game is as it was, then gives the answer for real and
/// checks that the forecast it leaves behind is the one previewed.
fn preview_and_declare(driver: &mut Driver, seat: usize, answer: Answer) -> CombatForecast {
    let before = outside(driver.engine());
    let previewed = driver
        .engine()
        .preview_forecast(pid(seat), &answer)
        .expect("a forecast");
    assert_eq!(
        outside(driver.engine()),
        before,
        "the preview moved the game"
    );
    driver.answer(answer).expect("the answer is legal");
    let real = pending_combat_forecast(driver.state()).expect("combat is pending after the answer");
    assert_eq!(previewed, real, "the preview and the real forecast differ");
    previewed
}

fn doomed(driver: &Driver, forecast: &CombatForecast) -> Vec<String> {
    let mut names: Vec<String> = forecast
        .dying()
        .map(|object| name_of(driver.state(), object))
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::trade(vec![("A", 2, 2, 0)], vec![("B", 2, 2, 0)], vec![("B", "A")], 20, vec!["A", "B"], 0, false)]
#[case::blocker_dies(vec![("A", 3, 3, 0)], vec![("B", 1, 1, 0)], vec![("B", "A")], 20, vec!["B"], 0, false)]
#[case::attacker_dies(vec![("A", 1, 1, 0)], vec![("B", 3, 3, 0)], vec![("B", "A")], 20, vec!["A"], 0, false)]
#[case::walls(vec![("A", 1, 5, 0)], vec![("B", 1, 5, 0)], vec![("B", "A")], 20, vec![], 0, false)]
#[case::chump_block_stops_lethal_damage(vec![("A", 10, 10, 0)], vec![("B", 1, 1, 0)], vec![("B", "A")], 3, vec!["B"], 0, false)]
#[case::no_blocks(vec![("A", 3, 3, 0)], vec![("B", 1, 1, 0)], vec![], 20, vec![], 3, false)]
#[case::no_blocks_lethal(vec![("A", 5, 5, 0)], vec![("B", 1, 1, 0)], vec![], 5, vec![], 5, true)]
#[case::double_block_kills_both_blockers(vec![("A", 5, 5, 0)], vec![("B1", 2, 2, 0), ("B2", 2, 2, 0)], vec![("B1", "A"), ("B2", "A")], 20, vec!["B1", "B2"], 0, false)]
#[case::double_block_kills_the_attacker_and_one_blocker(vec![("A", 4, 4, 0)], vec![("B1", 2, 3, 0), ("B2", 2, 3, 0)], vec![("B1", "A"), ("B2", "A")], 20, vec!["A", "B1"], 0, false)]
#[case::triple_block(vec![("A", 6, 6, 0)], vec![("B1", 1, 1, 0), ("B2", 1, 1, 0), ("B3", 1, 1, 0)], vec![("B1", "A"), ("B2", "A"), ("B3", "A")], 20, vec!["B1", "B2", "B3"], 0, false)]
#[case::one_blocked_the_other_lethal(vec![("A1", 4, 4, 0), ("A2", 5, 1, 0)], vec![("B", 1, 1, 0)], vec![("B", "A1")], 5, vec!["B"], 5, true)]
#[case::two_pairs(vec![("A1", 2, 2, 0), ("A2", 4, 4, 0)], vec![("B1", 2, 2, 0), ("B2", 1, 1, 0)], vec![("B1", "A1"), ("B2", "A2")], 20, vec!["A1", "B1", "B2"], 0, false)]
#[case::damage_already_marked(vec![("A", 2, 4, 2)], vec![("B", 2, 2, 0)], vec![("B", "A")], 20, vec!["A", "B"], 0, false)]
fn a_preview_of_blocks_says_who_dies_and_equals_the_forecast_after_declaring_them(
    #[case] attackers: Vec<Fighter>,
    #[case] blockers: Vec<Fighter>,
    #[case] pairs: Vec<(&str, &str)>,
    #[case] life: i32,
    #[case] dies: Vec<&str>,
    #[case] damage: u32,
    #[case] lethal: bool,
) -> Result<(), ScriptError> {
    let mut driver = facing_blocks(&attackers, &blockers, life)?;
    let answer = blocks(&driver, &pairs);
    let forecast = preview_and_declare(&mut driver, 1, answer);

    assert_eq!(doomed(&driver, &forecast), dies);
    let [player] = forecast.players.as_slice() else {
        panic!("one defending player expected: {:?}", forecast.players);
    };
    assert_eq!(player.player, pid(1));
    assert_eq!(player.damage, damage);
    assert_eq!(player.life, life);
    assert_eq!(
        i64::from(player.life_after),
        i64::from(life) - i64::from(damage)
    );
    assert_eq!(player.lethal, lethal);
    Ok(())
}

#[rstest]
#[case::one_attacker_lethal(vec![("A", 6, 6, 0)], 5, vec!["A"], 6, true)]
#[case::exactly_lethal(vec![("A", 5, 5, 0)], 5, vec!["A"], 5, true)]
#[case::two_attackers_lethal_together(vec![("A1", 3, 3, 0), ("A2", 2, 2, 0)], 5, vec!["A1", "A2"], 5, true)]
#[case::one_short_of_lethal(vec![("A1", 3, 3, 0), ("A2", 2, 2, 0)], 6, vec!["A1", "A2"], 5, false)]
#[case::only_some_attack(vec![("A1", 3, 3, 0), ("A2", 5, 5, 0)], 5, vec!["A1"], 3, false)]
#[case::no_power_no_damage(vec![("A", 0, 4, 0)], 5, vec!["A"], 0, false)]
fn a_preview_of_an_attack_says_how_much_gets_through_and_equals_the_forecast_after_declaring_it(
    #[case] offered: Vec<Fighter>,
    #[case] life: i32,
    #[case] attacking: Vec<&str>,
    #[case] damage: u32,
    #[case] lethal: bool,
) -> Result<(), ScriptError> {
    let mut driver = facing_attack(&offered, &[("B", 1, 1, 0)], life)?;
    let pairs: Vec<(&str, usize)> = attacking.iter().map(|name| (*name, 1)).collect();
    let answer = attacks(&driver, &pairs);
    let forecast = preview_and_declare(&mut driver, 0, answer);

    assert_eq!(forecast.attackers.len(), attacking.len());
    assert!(
        forecast
            .attackers
            .iter()
            .all(|a| !a.blocked && a.receives.is_empty())
    );
    assert_eq!(doomed(&driver, &forecast), Vec::<String>::new());
    let [player] = forecast.players.as_slice() else {
        panic!("one defending player expected: {:?}", forecast.players);
    };
    assert_eq!(
        (player.damage, player.life, player.lethal),
        (damage, life, lethal)
    );
    Ok(())
}

#[test]
fn an_attack_on_two_players_forecasts_each_of_them() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, creatures(&[("A1", 4, 4, 0), ("A2", 2, 2, 0)]))
            .life(1, 4)
            .life(2, 9),
    )?;
    let answer = attacks(&driver, &[("A1", 1), ("A2", 2)]);
    let forecast = preview_and_declare(&mut driver, 0, answer);
    let by_seat = |seat: usize| {
        forecast
            .players
            .iter()
            .find(|p| p.player == pid(seat))
            .map(|p| (p.damage, p.lethal))
    };
    assert_eq!(by_seat(1), Some((4, true)));
    assert_eq!(by_seat(2), Some((2, false)));
    Ok(())
}

#[test]
fn the_first_defenders_preview_leaves_the_other_attackers_unblocked() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, creatures(&[("A1", 3, 3, 0), ("A2", 3, 3, 0)]))
            .battlefield(1, creatures(&[("B1", 1, 1, 0)]))
            .battlefield(2, creatures(&[("B2", 1, 1, 0)])),
    )?;
    driver.attack(&[("A1", 1), ("A2", 2)])?;
    to_blockers(&mut driver, 2)?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::DeclareBlockers(_)
    ));
    let seat = driver.decision()?.player.index();
    assert_eq!(seat, 1, "the first defender in turn order decides first");
    let answer = blocks(&driver, &[("B1", "A1")]);
    let forecast = preview_and_declare(&mut driver, seat, answer);
    let blocked = |name: &str| {
        forecast
            .attackers
            .iter()
            .find(|a| a.attacker == id(&driver, name))
            .map(|a| a.blocked)
    };
    assert_eq!(blocked("A1"), Some(true));
    assert_eq!(blocked("A2"), Some(false), "player 2 has not declared yet");
    Ok(())
}

#[test]
fn a_preview_differs_from_the_forecast_without_the_block() -> Result<(), ScriptError> {
    let driver = facing_blocks(&[("A", 10, 10, 0)], &[("B", 1, 1, 0)], 3)?;
    let unblocked = pending_combat_forecast(driver.state()).expect("the table forecast");
    assert!(unblocked.players.iter().all(|p| p.lethal));
    assert_eq!(unblocked.dying().count(), 0);

    let answer = blocks(&driver, &[("B", "A")]);
    let chump = driver
        .engine()
        .preview_forecast(pid(1), &answer)
        .expect("a forecast");
    assert_ne!(chump, unblocked);
    assert!(chump.players.iter().all(|p| !p.lethal && p.damage == 0));
    assert_eq!(doomed(&driver, &chump), ["B"]);

    // Asking for no blocks at all gives the forecast the table already shows.
    let none = driver
        .engine()
        .preview_forecast(pid(1), &Answer::DeclareBlockers(Vec::new()))
        .expect("a forecast");
    assert_eq!(none, unblocked);
    Ok(())
}

#[test]
fn an_attack_with_nobody_has_no_combat_to_forecast() -> Result<(), ScriptError> {
    let mut driver = facing_attack(&[("A", 3, 3, 0)], &[("B", 1, 1, 0)], 20)?;
    let before = outside(driver.engine());
    let none = Answer::DeclareAttackers(Vec::new());
    assert_eq!(
        driver.engine().preview_forecast(pid(0), &none),
        Err(PreviewError::NoCombat)
    );
    assert_eq!(outside(driver.engine()), before);
    // The real answer agrees: there is no pending forecast afterwards.
    driver.answer(none)?;
    assert_eq!(pending_combat_forecast(driver.state()), None);
    Ok(())
}

/// A table where plenty of attacks and blocks are illegal: a sick and a tapped creature on each side.
fn crowded() -> Scenario {
    Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(free_creature("A", 2, 2)),
                sick(free_creature("S", 2, 2)),
                tapped(free_creature("T", 2, 2)),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("B", 2, 2)),
                tapped(free_creature("U", 2, 2)),
            ],
        )
}

/// An answer a refusal case gives, described by card names and built once the creatures exist.
enum Bad {
    /// Attacks: (attacker name, defending seat).
    Attack(&'static [(&'static str, usize)]),
    /// Blocks: (blocker name, attacker name).
    Block(&'static [(&'static str, &'static str)]),
    /// An answer that names no card.
    Raw(Answer),
}

impl Bad {
    fn answer(self, driver: &Driver) -> Answer {
        match self {
            Bad::Attack(pairs) => attacks(driver, pairs),
            Bad::Block(pairs) => blocks(driver, pairs),
            Bad::Raw(answer) => answer,
        }
    }
}

#[rstest]
#[case::summoning_sick_attacker(Bad::Attack(&[("S", 1)]), IllegalAnswer::NotOffered("attacker"))]
#[case::tapped_attacker(Bad::Attack(&[("T", 1)]), IllegalAnswer::NotOffered("attacker"))]
#[case::someone_elses_creature(Bad::Attack(&[("B", 1)]), IllegalAnswer::NotOffered("attacker"))]
#[case::attacker_twice(Bad::Attack(&[("A", 1), ("A", 1)]), IllegalAnswer::Duplicate("attacker"))]
#[case::attacking_yourself(Bad::Attack(&[("A", 0)]), IllegalAnswer::NotOffered("defender"))]
#[case::attacking_a_seat_that_is_not_there(Bad::Attack(&[("A", 7)]), IllegalAnswer::NotOffered("defender"))]
#[case::block_answer_to_an_attack(Bad::Raw(Answer::DeclareBlockers(Vec::new())), IllegalAnswer::WrongKind { expected: "declare-attackers", got: "declare-blockers" })]
fn an_illegal_attack_is_refused_exactly_as_the_real_answer_would_be(
    #[case] bad: Bad,
    #[case] illegal: IllegalAnswer,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(crowded())?;
    let answer = bad.answer(&driver);
    refused_the_same_way(&mut driver, 0, answer, &illegal);
    Ok(())
}

#[rstest]
#[case::tapped_blocker(Bad::Block(&[("U", "A")]), IllegalAnswer::NotOffered("blocker"))]
#[case::the_attackers_own_creature(Bad::Block(&[("A", "A")]), IllegalAnswer::NotOffered("blocker"))]
#[case::blocker_twice(Bad::Block(&[("B", "A"), ("B", "A")]), IllegalAnswer::Duplicate("blocker"))]
#[case::blocking_a_creature_that_does_not_attack(Bad::Block(&[("B", "T")]), IllegalAnswer::NotOffered("attacker to block"))]
#[case::a_blocker_that_does_not_exist(Bad::Raw(Answer::DeclareBlockers(vec![(ObjectId(u64::MAX), ObjectId(1))])), IllegalAnswer::NotOffered("blocker"))]
#[case::priority_answer_to_a_block(Bad::Raw(Answer::Priority(PriorityAction::Pass)), IllegalAnswer::WrongKind { expected: "declare-blockers", got: "priority" })]
fn an_illegal_block_is_refused_exactly_as_the_real_answer_would_be(
    #[case] bad: Bad,
    #[case] illegal: IllegalAnswer,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(crowded())?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    let answer = bad.answer(&driver);
    refused_the_same_way(&mut driver, 1, answer, &illegal);
    Ok(())
}

/// The preview refuses `answer` with `illegal`, the real answer is refused with the same reason, and neither moves the game.
fn refused_the_same_way(driver: &mut Driver, seat: usize, answer: Answer, illegal: &IllegalAnswer) {
    let before = outside(driver.engine());
    assert_eq!(
        driver.engine().preview_forecast(pid(seat), &answer),
        Err(PreviewError::Illegal(illegal.clone()))
    );
    assert_eq!(outside(driver.engine()), before);
    assert_eq!(
        driver.answer(answer),
        Err(ScriptError::Engine(EngineError::Illegal(illegal.clone())))
    );
    assert_eq!(outside(driver.engine()), before);
}

#[test]
fn only_attacks_and_blocks_can_be_previewed() -> Result<(), ScriptError> {
    // Priority.
    let driver = Driver::from_scenario(Scenario::new(2).at(2, 0, Step::Main1))?;
    let pass = Answer::Priority(PriorityAction::Pass);
    assert_eq!(
        driver.engine().preview_forecast(pid(0), &pass),
        Err(PreviewError::NotPreviewable("priority"))
    );
    // Even an attack answer is nothing to preview when no attack is being declared.
    assert_eq!(
        driver
            .engine()
            .preview_forecast(pid(0), &Answer::DeclareAttackers(Vec::new())),
        Err(PreviewError::NotPreviewable("priority"))
    );

    // Paying for a spell.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, [bear()])
            .battlefield(0, [ready(forest()), ready(forest())]),
    )?;
    driver.begin_cast("Grizzly Bears")?;
    assert_eq!(
        driver
            .engine()
            .preview_forecast(pid(0), &Answer::PayMana(PayChoice::Auto)),
        Err(PreviewError::NotPreviewable("pay-mana"))
    );

    // Dividing damage: the forecast exists then, but the decision has no outcome left to choose.
    let mut driver = facing_blocks(&[("A", 5, 5, 0)], &[("B1", 2, 2, 0), ("B2", 2, 2, 0)], 20)?;
    driver.block(&[("B1", "A"), ("B2", "A")])?;
    driver.pass()?;
    driver.pass()?;
    driver.expect_decision(0, "assign-damage")?;
    assert!(pending_combat_forecast(driver.state()).is_some());
    let split = Answer::AssignDamage(vec![(id(&driver, "B1"), 2), (id(&driver, "B2"), 3)]);
    assert_eq!(
        driver.engine().preview_forecast(pid(0), &split),
        Err(PreviewError::NotPreviewable("assign-damage"))
    );
    Ok(())
}

#[test]
fn only_the_seat_that_faces_the_decision_can_preview_it() -> Result<(), ScriptError> {
    let driver = facing_blocks(&[("A", 3, 3, 0)], &[("B", 1, 1, 0)], 20)?;
    let answer = blocks(&driver, &[("B", "A")]);
    let before = outside(driver.engine());
    assert_eq!(
        driver.engine().preview_forecast(pid(0), &answer),
        Err(PreviewError::NotYourDecision(pid(0)))
    );
    assert_eq!(
        driver.engine().preview_forecast(pid(5), &answer),
        Err(PreviewError::NotYourDecision(pid(5)))
    );
    assert!(driver.engine().preview_forecast(pid(1), &answer).is_ok());
    assert_eq!(outside(driver.engine()), before);
    Ok(())
}

#[test]
fn nothing_can_be_previewed_once_the_game_is_over() -> Result<(), ScriptError> {
    let mut driver = facing_blocks(&[("A", 3, 3, 0)], &[("B", 1, 1, 0)], 20)?;
    let answer = blocks(&driver, &[("B", "A")]);
    driver.concede(pid(0))?;
    assert!(driver.engine().decision().is_none());
    assert_eq!(
        driver.engine().preview_forecast(pid(1), &answer),
        Err(PreviewError::NoDecision)
    );
    Ok(())
}

#[test]
fn previews_are_invisible_to_the_game_that_follows() -> Result<(), ScriptError> {
    // The same fight with and without a flood of previews ends in the same log and the same state.
    let play = |previews: bool| -> Result<(u64, usize), ScriptError> {
        let mut driver = facing_blocks(&[("A", 5, 5, 0)], &[("B1", 2, 2, 0), ("B2", 2, 2, 0)], 20)?;
        if previews {
            let each = [
                vec![],
                vec![("B1", "A")],
                vec![("B1", "A"), ("B2", "A")],
                vec![("B2", "A")],
            ];
            for _ in 0..25 {
                for pairs in &each {
                    let answer = blocks(&driver, pairs);
                    driver
                        .engine()
                        .preview_forecast(pid(1), &answer)
                        .expect("a forecast");
                }
            }
        }
        driver.block(&[("B1", "A"), ("B2", "A")])?;
        driver.pass()?;
        driver.pass()?;
        driver.assign_damage(&[("B1", 2), ("B2", 3)])?;
        Ok((driver.engine().state_hash(), driver.engine().events().len()))
    };
    assert_eq!(play(false)?, play(true)?);
    Ok(())
}
