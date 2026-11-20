//! Goal: a secret choice of an opponent, at 2 to 4 seats: made as a trigger resolves (Stalking Leonin) or as the
//! permanent enters (Emissary of Grudges), asked only with two or more opponents, kept on the permanent and logged
//! secretly; the ability whose cost reveals it acts only on the chosen player's objects, logs the reveal and is offered
//! once; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::Chosen;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, on_battlefield, pid};

/// Answers the questions until the stack is empty: the secret choice by the seat `pick`, everything else passively.
/// Returns the options the secret choice offered, if it was asked.
fn resolve_all(driver: &mut Driver, pick: usize) -> Result<Option<Vec<ChoiceOption>>, ScriptError> {
    let mut asked = None;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::ChooseOption(options) if options.text.starts_with("Secretly choose") => {
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::SecretPlayer(pid(pick)))
                    .unwrap();
                asked = Some(options.options.clone());
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The table: seat 0 with `name` in hand and eight untapped lands, every other seat with a creature named "Raider <seat>".
fn table(seats: usize, name: &str) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(name).unwrap()])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(plains()),
                ready(plains()),
                ready(plains()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [ready(free_creature(&format!("Raider {seat}"), 2, 2))],
        );
    }
    scenario
}

/// The secret choice the permanent named keeps, if any.
fn kept(driver: &Driver, name: &str) -> Option<(Chosen, bool)> {
    let permanent = driver
        .state()
        .object(on_battlefield(driver.state(), name)?)?
        .permanent()?;
    Some((permanent.chosen()?.clone(), permanent.r110_secret()))
}

#[rstest]
#[case::the_first_opponent(1)]
#[case::the_last_opponent(usize::MAX)]
fn stalking_leonin_exiles_only_an_attacker_of_the_chosen_player(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
) -> Result<(), ScriptError> {
    let pick = pick.min(seats - 1);
    let setup = table(seats, "Stalking Leonin").setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Stalking Leonin"))?;
    driver.cast("Stalking Leonin")?;
    let asked = resolve_all(&mut driver, pick)?;
    // Only a choice among two or more opponents is a question, and it offers each opponent face down.
    assert_eq!(
        asked.map(|options| options.len()),
        (seats > 2).then_some(seats - 1)
    );
    assert_eq!(
        kept(&driver, "Stalking Leonin"),
        Some((Chosen::Player(pid(pick)), true))
    );
    assert!(driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::R110ChoseSecretly { chosen: Chosen::Player(p), .. } if *p == pid(pick))));
    // Seat 1 attacks seat 0 on its turn; seat 0 reveals and aims the ability at the attacker.
    driver.idle_until(2, Step::BeginCombat)?;
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        driver.pass()?;
    }
    driver.attack(&[("Raider 1", 0)])?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    driver.begin_activation("Stalking Leonin")?;
    let raider = driver.target_named("Raider 1")?;
    driver.choose_targets(&[raider])?;
    resolve_all(&mut driver, pick)?;
    let exiled = names(driver.state(), Zone::Exile).contains(&"Raider 1".to_owned());
    assert_eq!(exiled, pick == 1);
    assert_eq!(
        kept(&driver, "Stalking Leonin"),
        Some((Chosen::Player(pid(pick)), false))
    );
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::R110ChoiceRevealed { .. }))
    );
    // Activate only once: the revealed choice pays for nothing more.
    if driver.decision()?.player == pid(0)
        && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        assert_eq!(
            driver.offered_abilities("Stalking Leonin")?,
            Vec::<usize>::new()
        );
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn emissary_of_grudges_chooses_secretly_as_it_enters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let pick = seats - 1;
    let setup = table(seats, "Emissary of Grudges").setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Emissary of Grudges"))?;
    driver.cast("Emissary of Grudges")?;
    let asked = resolve_all(&mut driver, pick)?;
    assert_eq!(
        asked.map(|options| options.len()),
        (seats > 2).then_some(seats - 1)
    );
    assert_eq!(
        kept(&driver, "Emissary of Grudges"),
        Some((Chosen::Player(pid(pick)), true))
    );
    // The enter event names no choice; the secret one follows it.
    let log = driver.engine().events();
    assert!(log.iter().all(|event| !matches!(
        event,
        Event::Entered {
            chosen: Some(_),
            ..
        }
    )));
    assert!(
        log.iter()
            .any(|event| matches!(event, Event::R110ChoseSecretly { .. }))
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn guardian_archon_fixes_the_chosen_player_into_the_protection(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let pick = seats - 1;
    let setup = table(seats, "Guardian Archon").setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Guardian Archon"))?;
    driver.cast("Guardian Archon")?;
    resolve_all(&mut driver, pick)?;
    driver.begin_activation("Guardian Archon")?;
    let archon = driver.target_named("Guardian Archon")?;
    driver.choose_targets(&[archon])?;
    resolve_all(&mut driver, pick)?;
    let seat = u8::try_from(pick).unwrap();
    let id = driver.id_of("Guardian Archon")?;
    let keywords = keywords_of(driver.state(), id);
    let fixed = Keyword::Protection(Selector(vec![Pred::ControlledBy(Ref::R110Seat(seat))]));
    assert!(keywords.contains(&&fixed), "{keywords:?}");
    // The player's protection names the same seat, fixed as the rule began.
    let rule = driver
        .engine()
        .events()
        .iter()
        .find(|event| matches!(event, Event::RuleCreated { .. }))
        .map(|event| format!("{event:?}"))
        .unwrap();
    assert!(rule.contains(&format!("R110Seat({seat})")), "{rule}");
    assert_eq!(
        driver.offered_abilities("Guardian Archon")?,
        Vec::<usize>::new()
    );
    replays(&driver, &setup);
    Ok(())
}
