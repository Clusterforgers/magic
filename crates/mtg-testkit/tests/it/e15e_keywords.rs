//! Goal: recover, ripple and epic play whole at tables of two, three and four seats: recover asks its owner for the cost when another creature they own dies and returns the card to the hand when paid and exiles it otherwise, and sees neither a creature of another player nor its own death; ripple may reveal the top cards, casts each with the spell's name for free on a yes (whose own ripple triggers in turn) and puts the rest on the bottom; epic stops its controller casting spells for the rest of the game and copies the spell, without its epic, at each of their upkeeps with its targets; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::Targetable;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::cards::{
    burn, cost, damage, forest, free_creature, gain_life, mountain, one_target,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

fn yes_no(yes: bool) -> Answer {
    Answer::YesNo(YesNoChoice(yes))
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)]
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// Answers each yes-or-no question by `yes` (its text and how many were asked before) and everything else passively, until
/// the stack is empty at a priority decision; the questions asked, in order.
fn settle(
    driver: &mut Driver,
    yes: impl Fn(&str, usize) -> bool,
) -> Result<Vec<String>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(question) => {
                let text = question.text.clone();
                let answer = yes(&text, asked.len()) && !question.only_no;
                asked.push(text);
                driver.answer(yes_no(answer))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("an empty stack".into()))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn has(driver: &Driver, zone: Zone, name: &str) -> bool {
    names(driver.state(), zone).iter().any(|held| held == name)
}

/// The other seats hold a creature each, so the table has its seats.
fn table(seats: usize) -> Scenario {
    let mut table = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 1..seats {
        table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    table
}

// Recover.

/// A sorcery for `{R}` that gains 1 life, with recover {R}{R}.
fn recall() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Recall")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(1)],
            })
            .with_rules(Ability::Keyword(Keyword::Recover(Cost::mana(
                "{R}{R}".parse().unwrap(),
            )))),
    )
}

/// Seat 0 holds Recall in its graveyard, a Bolt in hand, `lands` Mountains and a creature of its own; `beast` is a
/// creature with recover of its own on the battlefield too.
fn recover_table(seats: usize, lands: usize) -> Scenario {
    let beast = Arc::new(
        CardDefinition::new("Recovering Beast")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Keyword(Keyword::Recover(Cost::mana(
                "{R}".parse().unwrap(),
            )))),
    );
    table(seats)
        .graveyard(0, [recall()])
        .hand(0, [burn("Bolt", 3)])
        .battlefield(
            0,
            [
                vec![ready(free_creature("Mine", 1, 1)), ready(beast)],
                vec![ready(mountain()); lands],
            ]
            .concat(),
        )
}

/// How seat 0 answers recover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Pays,
    Declines,
    Cannot,
}

#[rstest]
fn recover_returns_the_card_when_paid_and_exiles_it_otherwise(
    #[values(2, 3, 4)] seats: usize,
    #[values(Reply::Pays, Reply::Declines, Reply::Cannot)] reply: Reply,
) -> Result<(), ScriptError> {
    let lands = if reply == Reply::Cannot { 1 } else { 3 };
    let setup = recover_table(seats, lands).setup()?;
    let mut driver = Driver::from_scenario(recover_table(seats, lands))?;
    let mine = driver.target_named("Mine")?;
    driver.cast_at("Bolt", &[mine])?;
    let asked = settle(&mut driver, |_, _| reply == Reply::Pays)?;
    assert_eq!(asked, ["Pay {R}{R}?"], "one question, of seat 0");
    let paid = reply == Reply::Pays;
    assert_eq!(has(&driver, Zone::Hand(pid(0)), "Recall"), paid);
    assert_eq!(has(&driver, Zone::Exile, "Recall"), !paid);
    assert!(!has(&driver, Zone::Graveyard(pid(0)), "Recall"));
    assert_replays(&driver, &setup)
}

/// A creature of another player is not seat 0's; the recover creature does not see its own death, though Recall does.
#[rstest]
#[case::a_creature_of_another_player("Bystander", &[])]
#[case::the_recover_creature_itself("Recovering Beast", &["Pay {R}{R}?"])]
fn recover_does_not_see_this_death(
    #[values(2, 3, 4)] seats: usize,
    #[case] dies: &str,
    #[case] wanted: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(recover_table(seats, 3))?;
    let target = driver.target_named(dies)?;
    driver.cast_at("Bolt", &[target])?;
    let asked = settle(&mut driver, |_, _| false)?;
    assert_eq!(asked, wanted);
    assert_eq!(
        has(&driver, Zone::Graveyard(pid(0)), "Recall"),
        wanted.is_empty()
    );
    if dies == "Recovering Beast" {
        assert!(has(&driver, Zone::Graveyard(pid(0)), "Recovering Beast"));
    }
    Ok(())
}

// Ripple.

/// An instant for `{R}` that gains 2 life, with ripple 4.
fn surge() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Surge")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Keyword(Keyword::Ripple(4)))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(2)],
            }),
    )
}

/// Seat 0 holds a Surge over a Mountain; its library, top first: Surge, Forest, Surge, Bear, then two deep cards.
fn ripple_table(seats: usize) -> Scenario {
    table(seats)
        .hand(0, [surge()])
        .battlefield(0, [ready(mountain())])
        .library(
            0,
            vec![
                surge(),
                forest(),
                surge(),
                free_creature("Bear", 2, 2),
                free_creature("Deep", 1, 1),
                free_creature("Deeper", 1, 1),
            ],
        )
}

/// The names in seat 0's library, top first.
fn library(driver: &Driver) -> Vec<String> {
    let mut library = names(driver.state(), Zone::Library(pid(0)));
    library.reverse();
    library
}

#[rstest]
#[case::not_revealed(&[false], 22, &["Surge", "Forest", "Surge", "Bear"])]
#[case::revealed_and_none_cast(&[true, false, false], 22, &["Deep", "Deeper"])]
#[case::revealed_and_both_cast(&[true, true, true], 26, &["Deep", "Deeper"])]
fn ripple_casts_the_cards_of_the_same_name_and_puts_the_rest_on_the_bottom(
    #[values(2, 3, 4)] seats: usize,
    #[case] answers: &[bool],
    #[case] gained: i32,
    #[case] top: &[&str],
) -> Result<(), ScriptError> {
    let setup = ripple_table(seats).setup()?;
    let mut driver = Driver::from_scenario(ripple_table(seats))?;
    driver.cast("Surge")?;
    // The questions after the first answers (the ripples of the Surges cast) are declined.
    let asked = settle(&mut driver, |_, at| {
        answers.get(at).copied().unwrap_or(false)
    })?;
    assert!(asked.len() >= answers.len(), "asked {asked:?}");
    assert_eq!(life(&driver, 0), gained);
    let now = library(&driver);
    assert_eq!(&now[..top.len()], top, "the library, top first: {now:?}");
    assert_replays(&driver, &setup)
}

// Epic.

/// A sorcery for `{R}` that deals 2 damage to any target, with epic.
fn epic_burn() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Epic Flames")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(mtg_cards::refs::Ref::Target(0), 2)],
            })
            .with_rules(Ability::Keyword(Keyword::Epic)),
    )
}

fn epic_table(seats: usize) -> Scenario {
    table(seats)
        .hand(0, [epic_burn(), burn("Bolt", 3)])
        .battlefield(0, vec![ready(mountain()); 2])
}

/// The first turn of seat 0 after the opening turn, `rounds` rounds on.
fn turn_of_seat_0(seats: usize, rounds: u32) -> u32 {
    1 + rounds * u32::try_from(seats).unwrap()
}

#[rstest]
fn epic_stops_casting_and_copies_the_spell_at_each_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = epic_table(seats).setup()?;
    let mut driver = Driver::from_scenario(epic_table(seats))?;
    let foe = driver.target_player(1);
    driver.cast_at("Epic Flames", &[foe])?;
    settle(&mut driver, |_, _| false)?;
    assert_eq!(life(&driver, 1), 18);
    assert!(
        driver.begin_cast("Bolt").is_err(),
        "seat 0 can't cast spells any more"
    );
    for round in 1..=2 {
        driver.idle_until(turn_of_seat_0(seats, round), Step::Upkeep)?;
        let mark = driver.engine().events().len();
        let asked = settle(&mut driver, |_, _| false)?;
        assert_eq!(asked.len(), 1, "the copy asks for new targets: {asked:?}");
        let logged = &driver.engine().events()[mark..];
        assert!(
            logged
                .iter()
                .any(|event| matches!(event, Event::SpellCopied(_))),
            "the spell is copied"
        );
        assert!(
            !logged
                .iter()
                .any(|event| matches!(event, Event::SpellCast { .. })),
            "a copy is not cast"
        );
        assert_eq!(life(&driver, 1), 18 - 2 * i32::try_from(round).unwrap());
        assert_eq!(driver.state().delayed().len(), 1, "a copy sets nothing up");
    }
    assert!(has(&driver, Zone::Hand(pid(0)), "Bolt"));
    assert_replays(&driver, &setup)
}

#[rstest]
fn the_copy_of_epic_may_take_a_new_target(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(epic_table(seats))?;
    let foe = driver.target_player(1);
    driver.cast_at("Epic Flames", &[foe])?;
    settle(&mut driver, |_, _| false)?;
    driver.idle_until(turn_of_seat_0(seats, 1), Step::Upkeep)?;
    for _ in 0..50 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(yes_no(true))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let bystander = driver.target_named("Bystander")?;
                driver.choose_targets(&[bystander])?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(life(&driver, 1), 18, "the copy hit another target");
    assert!(has(&driver, Zone::Graveyard(pid(1)), "Bystander"));
    Ok(())
}
