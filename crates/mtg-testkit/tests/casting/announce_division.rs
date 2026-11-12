//! Goal: an amount divided among targets (CR 601.2d) is asked after the targets and before the payment for a spell, a mode of a spell, an activated ability and a triggered ability, a single target takes the whole amount without a question, the answer is checked and kept in the choices the cast carries, the cast can be backed out of at the division with no trace in the log (a trigger cannot), and whole bot games with divided spells keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{DivideChoice, ModeChoice};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::{Answer, EngineError, Event};
use mtg_testkit::announce_cards::{split_bolt, split_charm, split_hawk, split_pinger};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::{
    Driver, PlayOptions, Scenario, ScriptError, play_game_with, ready, split_fuzz_setup,
};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

fn scenario(card: std::sync::Arc<mtg_cards::CardDefinition>) -> Scenario {
    Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(mountain()), ready(mountain())])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))])
}

/// The options of the pending division.
fn division(driver: &Driver) -> mtg_engine::choose::DivideOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::DivideAmount(options) => options.clone(),
        other => panic!("a division was expected, got {}", other.name()),
    }
}

/// The choices of the spell cast or ability activated in these events.
fn announced(events: &[Event]) -> mtg_engine::choose::Choices {
    events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.clone()),
            Event::AbilityPutOnStack(put) => Some(put.choices.clone()),
            _ => None,
        })
        .expect("a spell was cast or an ability put on the stack")
}

#[test]
fn a_spell_divides_its_amount_after_its_targets_and_before_its_payment() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(scenario(split_bolt("Split Bolt", 3)))?;
    driver.begin_cast("Split Bolt")?;
    let (foe, bear) = (driver.target_player(1), driver.target_named("Bear")?);
    driver.choose_targets(&[foe, bear])?;
    let options = division(&driver);
    assert_eq!(options.total, 3);
    assert_eq!(options.among, [foe, bear]);
    assert!(options.can_cancel, "nothing has been paid yet");
    assert_eq!(options.text, "Divide 3 damage among the targets.");
    driver.answer(Answer::DivideAmount(DivideChoice::Division(vec![
        (foe, 2),
        (bear, 1),
    ])))?;
    driver.expect_decision(0, "pay-mana")?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let choices = announced(&events);
    assert_eq!(choices.targets, [vec![foe, bear]]);
    assert_eq!(choices.division, [(foe, 2), (bear, 1)]);
    Ok(())
}

#[rstest]
#[case::the_most_the_specification_allows(3, 3)]
#[case::capped_by_the_amount(2, 2)]
fn the_targets_offered_never_outnumber_the_amount(
    #[case] amount: i32,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let card = split_bolt("Split Bolt", amount);
    let mut driver = Driver::from_scenario(scenario(card))?;
    driver.begin_cast("Split Bolt")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("targets come first");
    };
    // Two players and a creature could be named; the amount says how many may be.
    assert_eq!(
        (options.min, options.max, options.legal.len()),
        (1, most, 3)
    );
    Ok(())
}

#[test]
fn a_single_target_takes_the_whole_amount_without_a_question() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_bolt("Split Bolt", 3)))?;
    driver.begin_cast("Split Bolt")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    driver.expect_decision(0, "pay-mana")?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(announced(&events).division, [(bear, 3)]);
    Ok(())
}

#[rstest]
#[case::too_little(vec![(1, 1), (2, 1)])]
#[case::too_much(vec![(1, 2), (2, 2)])]
#[case::a_share_of_nothing(vec![(1, 0), (2, 3)])]
#[case::a_target_left_out(vec![(1, 3)])]
#[case::a_target_twice(vec![(1, 1), (1, 2)])]
fn a_division_that_is_not_the_amount_shared_among_the_targets_is_refused_and_asked_again(
    #[case] shares: Vec<(usize, u32)>,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_bolt("Split Bolt", 3)))?;
    driver.begin_cast("Split Bolt")?;
    let (foe, bear) = (driver.target_player(1), driver.target_named("Bear")?);
    driver.choose_targets(&[foe, bear])?;
    let by_index = |index: usize| if index == 1 { foe } else { bear };
    let shares = shares
        .into_iter()
        .map(|(index, share)| (by_index(index), share))
        .collect();
    let before = driver.engine().events().len();
    let refused = driver.answer(Answer::DivideAmount(DivideChoice::Division(shares)));
    assert!(
        matches!(refused, Err(ScriptError::Engine(EngineError::Illegal(_)))),
        "{refused:?}"
    );
    assert_eq!(driver.engine().events().len(), before);
    division(&driver);
    Ok(())
}

#[test]
fn backing_out_at_the_division_logs_nothing() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_bolt("Split Bolt", 3)))?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_cast("Split Bolt")?;
    let (foe, bear) = (driver.target_player(1), driver.target_named("Bear")?);
    driver.choose_targets(&[foe, bear])?;
    driver.answer(Answer::DivideAmount(DivideChoice::Cancel))?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    driver.expect_decision(0, "priority")?;
    // The same cast can be made again from the start.
    driver.begin_cast("Split Bolt")?;
    driver.expect_decision(0, "choose-targets")?;
    Ok(())
}

#[test]
fn a_mode_with_a_divided_amount_asks_it_after_the_targets_of_the_mode() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_charm("Split Charm")))?;
    driver.begin_cast("Split Charm")?;
    driver.expect_decision(0, "choose-mode")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![0])))?;
    driver.expect_decision(0, "choose-targets")?;
    let (foe, bear) = (driver.target_player(1), driver.target_named("Bear")?);
    driver.choose_targets(&[bear, foe])?;
    assert_eq!(division(&driver).total, 2);
    driver.answer(Answer::DivideAmount(DivideChoice::Division(vec![
        (bear, 1),
        (foe, 1),
    ])))?;
    driver.expect_decision(0, "pay-mana")?;
    Ok(())
}

#[test]
fn the_mode_without_a_divided_amount_asks_none() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_charm("Split Charm")))?;
    driver.begin_cast("Split Charm")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![1])))?;
    driver.expect_decision(0, "pay-mana")?;
    Ok(())
}

#[test]
fn an_ability_with_no_mana_to_pay_goes_on_the_stack_when_its_amount_is_divided()
-> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(split_pinger("Split Pinger"))])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_activation("Split Pinger")?;
    let (foe, bear) = (driver.target_player(1), driver.target_named("Bear")?);
    driver.choose_targets(&[foe, bear])?;
    // Backing out leaves the permanent untapped and the log as it was.
    driver.answer(Answer::DivideAmount(DivideChoice::Cancel))?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    driver.begin_activation("Split Pinger")?;
    driver.choose_targets(&[foe, bear])?;
    let events = driver.answer(Answer::DivideAmount(DivideChoice::Division(vec![
        (foe, 1),
        (bear, 1),
    ])))?;
    assert_eq!(announced(&events).division, [(foe, 1), (bear, 1)]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::PermanentTapped { .. }))
    );
    assert_eq!(driver.state().stack().len(), 1);
    Ok(())
}

#[test]
fn a_triggered_ability_divides_as_it_goes_on_the_stack_and_cannot_be_backed_out_of()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_hawk("Split Hawk")))?;
    driver.cast("Split Hawk")?;
    // Both players pass and the creature resolves; its trigger then goes on the stack.
    driver.pass()?;
    driver.pass()?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the trigger asks for its targets");
    };
    assert!(options.triggered && !options.can_cancel);
    let (foe, bear) = (driver.target_player(1), driver.target_named("Bear")?);
    driver.choose_targets(&[foe, bear])?;
    let options = division(&driver);
    assert!(!options.can_cancel, "a trigger has no way out");
    let refused = driver.answer(Answer::DivideAmount(DivideChoice::Cancel));
    assert!(
        matches!(
            refused,
            Err(ScriptError::Engine(EngineError::Illegal(
                IllegalAnswer::NotOffered("cancel")
            )))
        ),
        "{refused:?}"
    );
    let events = driver.answer(Answer::DivideAmount(DivideChoice::Division(vec![
        (foe, 1),
        (bear, 1),
    ])))?;
    let put = events
        .iter()
        .find_map(|event| match event {
            Event::AbilityPutOnStack(put) => Some(put),
            _ => None,
        })
        .expect("the trigger went on the stack");
    assert_eq!(put.choices.division, [(foe, 1), (bear, 1)]);
    Ok(())
}

/// What the games used.
#[derive(Default)]
struct Seen {
    divided: u64,
    single: u64,
}

#[rstest]
#[case(2)]
#[case(4)]
fn bot_games_with_divided_spells_keep_every_invariant(#[case] players: usize) {
    let seen = std::sync::Mutex::new(Seen::default());
    let games = fuzz_games(40);
    for_each_seed(0..games, |seed| {
        let outcome = play_game_with(
            split_fuzz_setup(players, seed),
            seed,
            PlayOptions::default(),
        )
        .unwrap_or_else(|failure| panic!("{failure}"));
        let mut seen = seen.lock().unwrap();
        for event in &outcome.events {
            let choices = match event {
                Event::SpellCast { choices, .. } => choices,
                Event::AbilityPutOnStack(put) => &put.choices,
                _ => continue,
            };
            for (at, spec) in choices.targets.iter().enumerate() {
                if choices.division.is_empty() {
                    continue;
                }
                let shared = spec
                    .iter()
                    .filter(|t| choices.division.iter().any(|(named, _)| named == *t))
                    .count();
                match shared {
                    0 => {}
                    1 => seen.single += 1,
                    _ => seen.divided += 1,
                }
                let total: u32 = choices.division.iter().map(|(_, share)| share).sum();
                assert!(total >= u32::try_from(shared).unwrap_or(0), "{at}");
                assert!(choices.division.iter().all(|(_, share)| *share >= 1));
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.divided > 0, "some spell divided its amount");
    assert!(seen.single > 0, "some spell gave all to one target");
}
