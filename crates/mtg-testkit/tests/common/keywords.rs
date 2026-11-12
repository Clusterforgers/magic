//! Goal: shared setup for the keyword test files: creatures written as words in a table (this is not a test module itself).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::cards::keyworded;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

use super::combat::to_blockers;

/// A creature of a keyword table: name, power, toughness and its keywords as words ("flying first-strike").
pub type Unit = (&'static str, i32, i32, &'static str);

/// The keyword a word names.
pub fn keyword(word: &str) -> Keyword {
    match word {
        "flying" => Keyword::Flying,
        "reach" => Keyword::Reach,
        "menace" => Keyword::Menace,
        "defender" => Keyword::Defender,
        "haste" => Keyword::Haste,
        "vigilance" => Keyword::Vigilance,
        "first-strike" => Keyword::FirstStrike,
        "double-strike" => Keyword::DoubleStrike,
        "deathtouch" => Keyword::Deathtouch,
        "trample" => Keyword::Trample,
        "lifelink" => Keyword::Lifelink,
        "indestructible" => Keyword::Indestructible,
        other => panic!("no combat keyword is called {other:?}"),
    }
}

/// The keywords a line of words names.
pub fn keywords(words: &str) -> Vec<Keyword> {
    words.split_whitespace().map(keyword).collect()
}

/// The card of a unit.
pub fn card(unit: Unit) -> Arc<CardDefinition> {
    keyworded(unit.0, unit.1, unit.2, &keywords(unit.3))
}

/// The units as permanents that start untapped and ready to act.
pub fn ready_units(units: &[Unit]) -> Vec<PresetPermanent> {
    units.iter().map(|unit| ready(card(*unit))).collect()
}

/// Two players, player 0 about to declare attackers with `attackers`, player 1 holding `blockers`.
pub fn duel(attackers: &[Unit], blockers: &[Unit]) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, ready_units(attackers))
            .battlefield(1, ready_units(blockers)),
    )
}

/// Attacks player 1 with every attacker of `attackers` and passes to player 1's blocker decision (or past it).
pub fn attack_with_all(driver: &mut Driver, attackers: &[Unit]) -> Result<(), ScriptError> {
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|unit| (unit.0, 1)).collect();
    driver.attack(&attacks)?;
    to_blockers(driver, 1)
}
