//! Goal: a manual grant of an ability gives a permanent a keyword the engine applies or a line of text for the table through the ability layer, until end of turn or for good: the keyword is read like a printed one (deathtouch kills in combat, flying keeps ground blockers out), an end-of-turn grant is gone in the next turn and a lasting one is not, text is listed while it holds, and an empty or long line and a missing object are refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use common::combat::{duel, survivors, to_blockers};
use common::pid;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::keywords::has_keyword;
use mtg_engine::manual::{ManualAction, ManualRefusal};
use mtg_engine::manual_grant::{ManualGrant, ManualKeyword, granted_text};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, EngineError};
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

fn give(driver: &mut Driver, name: &str, grant: ManualGrant, until_end_of_turn: bool) {
    let object = driver.id_of(name).unwrap();
    let action = ManualAction::GrantAbility {
        object,
        grant,
        until_end_of_turn,
    };
    driver.manual_by(pid(0), &action).unwrap();
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
}

fn has(driver: &Driver, name: &str, keyword: ManualKeyword) -> bool {
    let id = driver.id_of(name).unwrap();
    has_keyword(driver.state(), id, &keyword.keyword())
}

fn next_turn(driver: &mut Driver) -> Result<(), ScriptError> {
    let turn = driver.state().turn().number();
    while driver.state().turn().number() == turn {
        driver.answer_passively()?;
    }
    Ok(())
}

#[rstest]
#[case::flying(ManualKeyword::Flying)]
#[case::reach(ManualKeyword::Reach)]
#[case::first_strike(ManualKeyword::FirstStrike)]
#[case::double_strike(ManualKeyword::DoubleStrike)]
#[case::deathtouch(ManualKeyword::Deathtouch)]
#[case::trample(ManualKeyword::Trample)]
#[case::lifelink(ManualKeyword::Lifelink)]
#[case::vigilance(ManualKeyword::Vigilance)]
#[case::haste(ManualKeyword::Haste)]
#[case::menace(ManualKeyword::Menace)]
#[case::defender(ManualKeyword::Defender)]
#[case::indestructible(ManualKeyword::Indestructible)]
#[case::hexproof(ManualKeyword::Hexproof)]
#[case::shroud(ManualKeyword::Shroud)]
#[case::flash(ManualKeyword::Flash)]
fn a_granted_keyword_is_held_until_its_end(
    #[case] keyword: ManualKeyword,
    #[values(true, false)] until_end_of_turn: bool,
) -> Result<(), ScriptError> {
    let mut driver = duel((1, 1), (1, 1))?;
    assert!(!has(&driver, "A", keyword));
    give(
        &mut driver,
        "A",
        ManualGrant::Keyword(keyword),
        until_end_of_turn,
    );
    assert!(has(&driver, "A", keyword));
    assert!(!has(&driver, "B", keyword));
    next_turn(&mut driver)?;
    assert_eq!(has(&driver, "A", keyword), !until_end_of_turn);
    Ok(())
}

/// Attacker `A` against blocker `B`; whether each is dead after combat damage.
#[rstest]
#[case::deathtouch_kills_the_big_blocker(Some(ManualKeyword::Deathtouch), (1, 1), (5, 5), true, true)]
#[case::without_it_the_blocker_lives(None, (1, 1), (5, 5), true, false)]
#[case::first_strike_wins_the_trade(Some(ManualKeyword::FirstStrike), (2, 2), (2, 2), false, true)]
#[case::indestructible_survives(Some(ManualKeyword::Indestructible), (1, 1), (5, 5), false, false)]
fn a_granted_keyword_works_in_combat(
    #[case] keyword: Option<ManualKeyword>,
    #[case] attacker: (i32, i32),
    #[case] blocker: (i32, i32),
    #[case] attacker_dead: bool,
    #[case] blocker_dead: bool,
) -> Result<(), ScriptError> {
    let mut driver = duel(attacker, blocker)?;
    if let Some(keyword) = keyword {
        give(&mut driver, "A", ManualGrant::Keyword(keyword), true);
    }
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    driver.pass_until(Step::EndCombat)?;
    let alive = survivors(&driver);
    assert_eq!(!alive.contains(&"A".to_owned()), attacker_dead);
    assert_eq!(!alive.contains(&"B".to_owned()), blocker_dead);
    Ok(())
}

#[rstest]
#[case::flying(Some(ManualKeyword::Flying), false)]
#[case::reach_is_not_evasion(Some(ManualKeyword::Reach), true)]
#[case::nothing(None, true)]
fn a_granted_flying_keeps_the_ground_blocker_out(
    #[case] keyword: Option<ManualKeyword>,
    #[case] can_block: bool,
) -> Result<(), ScriptError> {
    let mut driver = duel((1, 1), (1, 1))?;
    if let Some(keyword) = keyword {
        give(&mut driver, "A", ManualGrant::Keyword(keyword), true);
    }
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    let b = driver.id_of("B")?;
    // With no legal blocker the declaration is skipped.
    let offered = match &driver.decision()?.kind {
        DecisionKind::DeclareBlockers(options) => options
            .blockers
            .iter()
            .any(|blocker| blocker.object == b && !blocker.can_block.is_empty()),
        _ => false,
    };
    assert_eq!(offered, can_block);
    Ok(())
}

#[rstest]
#[case::until_end_of_turn(true)]
#[case::for_good(false)]
fn text_is_listed_while_it_holds(#[case] until_end_of_turn: bool) -> Result<(), ScriptError> {
    let mut driver = duel((1, 1), (1, 1))?;
    let a = driver.id_of("A")?;
    give(
        &mut driver,
        "A",
        ManualGrant::Text("  Cannot be countered  ".into()),
        until_end_of_turn,
    );
    give(
        &mut driver,
        "A",
        ManualGrant::Text("Gets +1/+1 for each Elf".into()),
        false,
    );
    assert_eq!(
        granted_text(driver.state(), a),
        ["Cannot be countered", "Gets +1/+1 for each Elf"]
    );
    assert!(granted_text(driver.state(), driver.id_of("B")?).is_empty());
    next_turn(&mut driver)?;
    assert_eq!(
        granted_text(driver.state(), a).len(),
        if until_end_of_turn { 1 } else { 2 }
    );
    Ok(())
}

fn refusal(driver: &mut Driver, action: ManualAction) -> ManualRefusal {
    let hash = driver.state().state_hash();
    let error = driver.manual_by(pid(0), &action).unwrap_err();
    assert_eq!(driver.state().state_hash(), hash);
    let ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal))) = error else {
        panic!("not a manual refusal: {error:?}");
    };
    refusal
}

#[rstest]
#[case::empty("   ")]
#[case::too_long(&"x".repeat(81))]
#[case::a_line_break("two\nlines")]
fn a_bad_text_is_refused(#[case] text: &str) -> Result<(), ScriptError> {
    let mut driver = duel((1, 1), (1, 1))?;
    let object = driver.id_of("A")?;
    let action = ManualAction::GrantAbility {
        object,
        grant: ManualGrant::Text(text.to_owned()),
        until_end_of_turn: false,
    };
    assert!(matches!(
        refusal(&mut driver, action),
        ManualRefusal::OutOfRange(_)
    ));
    Ok(())
}

#[test]
fn a_missing_object_is_refused() -> Result<(), ScriptError> {
    let mut driver = duel((1, 1), (1, 1))?;
    let action = ManualAction::GrantAbility {
        object: ObjectId(u64::MAX),
        grant: ManualGrant::Keyword(ManualKeyword::Flying),
        until_end_of_turn: true,
    };
    assert!(matches!(
        refusal(&mut driver, action),
        ManualRefusal::Unknown(_)
    ));
    Ok(())
}
