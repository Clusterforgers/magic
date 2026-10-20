//! Goal: a spell with flash is castable whenever its controller has priority, as an instant is (in its controller's turn and in another player's, in any step, with a spell on the stack), at tables of two, three and four seats, while a spell without flash keeps the timing of its type; a flash spell still needs its mana, and a creature cast with flash in the middle of combat can block.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature, instant, keyworded_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A sorcery for `{G}` that draws a card, with the keywords given.
fn sorcery(name: &str, keywords: &[Keyword]) -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap();
    let card = CardDefinition::new(name)
        .with_type(CardType::Sorcery)
        .with_mana_cost(cost(0, &[Color::Green]))
        .with_rules(spell);
    Arc::new(keywords.iter().fold(card, |card, keyword| {
        card.with_rules(Ability::Keyword(keyword.clone()))
    }))
}

/// A creature for `{G}`, with flash or without.
fn bear(name: &str, keywords: &[Keyword]) -> Arc<CardDefinition> {
    keyworded_creature(name, 0, &[Color::Green], 2, 2, keywords)
}

/// What the holder's hand offers in the tables: every kind of spell for `{G}`.
fn hand() -> Vec<Arc<CardDefinition>> {
    vec![
        bear("Flash Bear", &[Keyword::Flash]),
        bear("Bear", &[]),
        instant("Trick", 0, &[Color::Green]),
        sorcery("Flash Sorcery", &[Keyword::Flash]),
        sorcery("Sorcery", &[]),
    ]
}

/// The names of the spells `holder` may cast at its first priority decision, after the seats before it have passed
/// (and, with `stacked`, after the active player cast a `{0}` instant that waits on the stack).
fn castable(
    seats: usize,
    active: usize,
    holder: usize,
    step: Step,
    stacked: bool,
) -> Result<Vec<String>, ScriptError> {
    let mut mine = hand();
    let mut scenario = Scenario::new(seats)
        .at(1, active, step)
        .battlefield(holder, [ready(forest())]);
    if stacked {
        let trick = instant("Stack Filler", 0, &[]);
        if active == holder {
            mine.push(trick);
        } else {
            scenario = scenario.hand(active, [trick]);
        }
    }
    let mut driver = Driver::from_scenario(scenario.hand(holder, mine))?;
    if stacked {
        driver.cast("Stack Filler")?;
        assert_eq!(driver.state().stack().len(), 1);
    }
    for _ in 0..seats {
        if driver.decision()?.player.index() == holder {
            break;
        }
        driver.pass()?;
    }
    let decision = driver.decision()?;
    assert_eq!(decision.player.index(), holder, "the holder has priority");
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("the holder has priority");
    };
    let mut names: Vec<String> = options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    names.sort();
    Ok(names)
}

const EVERYTHING: [&str; 5] = ["Bear", "Flash Bear", "Flash Sorcery", "Sorcery", "Trick"];
const AT_INSTANT_SPEED: [&str; 3] = ["Flash Bear", "Flash Sorcery", "Trick"];

#[rstest]
// The holder's own turn: at sorcery speed everything can be cast, otherwise only what has flash or is an instant.
#[case::own_main_phase(true, Step::Main1, false, &EVERYTHING)]
#[case::own_second_main_phase(true, Step::Main2, false, &EVERYTHING)]
#[case::own_main_phase_with_a_spell_waiting(true, Step::Main1, true, &AT_INSTANT_SPEED)]
#[case::own_upkeep(true, Step::Upkeep, false, &AT_INSTANT_SPEED)]
#[case::own_beginning_of_combat(true, Step::BeginCombat, false, &AT_INSTANT_SPEED)]
#[case::own_end_step(true, Step::End, false, &AT_INSTANT_SPEED)]
// Another player's turn: the same, in every step.
#[case::their_main_phase(false, Step::Main1, false, &AT_INSTANT_SPEED)]
#[case::their_main_phase_with_a_spell_waiting(false, Step::Main1, true, &AT_INSTANT_SPEED)]
#[case::their_upkeep(false, Step::Upkeep, false, &AT_INSTANT_SPEED)]
#[case::their_draw_step(false, Step::Draw, false, &AT_INSTANT_SPEED)]
#[case::their_beginning_of_combat(false, Step::BeginCombat, false, &AT_INSTANT_SPEED)]
#[case::their_second_main_phase(false, Step::Main2, false, &AT_INSTANT_SPEED)]
#[case::their_end_step(false, Step::End, false, &AT_INSTANT_SPEED)]
fn flash_casts_at_the_timing_of_an_instant(
    #[case] own_turn: bool,
    #[case] step: Step,
    #[case] stacked: bool,
    #[case] want: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let holder = seats - 1;
    let active = if own_turn { holder } else { 0 };
    assert_eq!(castable(seats, active, holder, step, stacked)?, want);
    Ok(())
}

#[rstest]
#[case::two_for_a_two_mana_spell(2, true)]
#[case::one_for_a_two_mana_spell(1, false)]
fn a_spell_with_flash_still_needs_its_mana(
    #[case] forests: usize,
    #[case] castable_now: bool,
    #[values(true, false)] own_turn: bool,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let holder = seats - 1;
    let active = if own_turn { holder } else { 0 };
    let stalker = keyworded_creature("Stalker", 1, &[Color::Green], 2, 2, &[Keyword::Flash]);
    let lands: Vec<_> = (0..forests).map(|_| ready(forest())).collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, active, Step::End)
            .battlefield(holder, lands)
            .hand(holder, [stalker]),
    )?;
    for _ in 0..seats {
        if driver.decision()?.player.index() == holder {
            break;
        }
        driver.pass()?;
    }
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("the holder has priority");
    };
    assert_eq!(!options.castable.is_empty(), castable_now);
    if castable_now {
        driver.cast("Stalker")?;
        while !driver.state().stack().is_empty() {
            driver.pass()?;
        }
        let mut board = vec!["Forest"; forests];
        board.push("Stalker");
        driver.expect_zone(Zone::Battlefield, &board)?;
        driver.expect_zone(Zone::Hand(pid(holder)), &[])?;
    }
    Ok(())
}

#[rstest]
fn a_creature_cast_with_flash_in_combat_blocks(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("Raider", 2, 2))])
            .battlefield(1, [ready(forest())])
            .hand(1, [bear("Ambusher", &[Keyword::Flash])]),
    )?;
    driver.attack(&[("Raider", 1)])?;
    // Everyone passes in turn until the defender has priority, which is when the ambush is cast.
    while driver.decision()?.player.index() != 1 {
        driver.pass()?;
    }
    driver.cast("Ambusher")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    driver.expect_zone(Zone::Battlefield, &["Raider", "Forest", "Ambusher"])?;
    while !matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
        driver.pass()?;
    }
    driver.block(&[("Ambusher", "Raider")])?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 20)?;
    driver.expect_zone(Zone::Graveyard(pid(0)), &["Raider"])?;
    driver.expect_zone(Zone::Graveyard(pid(1)), &["Ambusher"])
}
