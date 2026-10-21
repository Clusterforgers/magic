//! Goal: the keywords that decide who may attack and block (flying, reach, menace, defender, haste, vigilance) allow and refuse exactly what the rules say, over tables of attacker and blocker mixes, and a refused declaration changes nothing.

#![allow(clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::combat::to_blockers;
use common::keywords::{Unit, attack_with_all, card, duel, keyword, ready_units};
use common::name_of;
use mtg_cards::AbilityDef;
use mtg_cards::CardDefinition;
use mtg_cards::ManaAbility;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::mana::ManaType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{free_creature, pinger};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

/// The (blocker, attacker) pairs the pending blockers decision offers, by name.
fn offered_blocks(driver: &Driver) -> Vec<(String, String)> {
    let Ok(decision) = driver.decision() else {
        return Vec::new();
    };
    let DecisionKind::DeclareBlockers(options) = &decision.kind else {
        return Vec::new();
    };
    let mut pairs: Vec<(String, String)> = options
        .blockers
        .iter()
        .flat_map(|blocker| {
            blocker.can_block.iter().map(|attacker| {
                (
                    name_of(driver.state(), blocker.object),
                    name_of(driver.state(), *attacker),
                )
            })
        })
        .collect();
    pairs.sort();
    pairs
}

fn pair(blocker: &str, attacker: &str) -> (String, String) {
    (blocker.to_owned(), attacker.to_owned())
}

#[rstest]
#[case::flying_over_nothing("flying", "", false)]
#[case::flying_over_a_deathtouch_wall("flying", "deathtouch", false)]
#[case::flying_over_menace_and_vigilance("flying", "menace vigilance", false)]
#[case::flying_against_flying("flying", "flying", true)]
#[case::flying_against_reach("flying", "reach", true)]
#[case::flying_against_reach_among_other_words("flying", "vigilance reach lifelink", true)]
#[case::flying_against_flying_and_reach("flying", "flying reach", true)]
#[case::the_ground_against_nothing("", "", true)]
#[case::the_ground_against_flying("", "flying", true)]
#[case::the_ground_against_reach("", "reach", true)]
#[case::reach_attacks_like_the_ground("reach", "", true)]
#[case::reach_attacking_against_flying("reach", "flying", true)]
#[case::flying_with_haste_is_still_flying("flying haste", "", false)]
fn flying_can_be_blocked_only_by_flying_or_reach(
    #[case] attacker: &'static str,
    #[case] blocker: &'static str,
    #[case] blocks: bool,
) -> Result<(), ScriptError> {
    let attackers: [Unit; 1] = [("A", 2, 2, attacker)];
    let mut driver = duel(&attackers, &[("B", 2, 2, blocker)])?;
    attack_with_all(&mut driver, &attackers)?;
    let want = if blocks {
        vec![pair("B", "A")]
    } else {
        Vec::new()
    };
    assert_eq!(offered_blocks(&driver), want);
    if !blocks {
        driver.expect_event("no blocker to ask", |e| {
            matches!(e, Event::NothingToDo { .. })
        })?;
    }
    Ok(())
}

#[test]
fn a_refused_block_of_a_flying_attacker_changes_nothing() -> Result<(), ScriptError> {
    let attackers: [Unit; 2] = [("Flyer", 2, 2, "flying"), ("Walker", 2, 2, "")];
    let mut driver = duel(&attackers, &[("Wall", 0, 5, ""), ("Spider", 1, 3, "reach")])?;
    attack_with_all(&mut driver, &attackers)?;
    assert_eq!(
        offered_blocks(&driver),
        [
            pair("Spider", "Flyer"),
            pair("Spider", "Walker"),
            pair("Wall", "Walker")
        ]
    );
    let flyer = driver.id_of("Flyer")?;
    let wall = driver.id_of("Wall")?;
    let before = driver.engine().state_hash();
    let refused = driver.answer(mtg_engine::Answer::DeclareBlockers(vec![(wall, flyer)]));
    assert_eq!(
        refused.err(),
        Some(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::NotOffered("attacker to block")
        )))
    );
    assert_eq!(driver.engine().state_hash(), before);
    driver.block(&[("Spider", "Flyer"), ("Wall", "Walker")])?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 20)
}

/// What one declaration of blocks came to.
#[derive(Debug, PartialEq, Eq)]
enum Declared {
    Accepted,
    Refused { needed: usize, got: usize },
}

#[rstest]
// With one creature that could block, a menace attacker cannot be blocked at all.
#[case::one_potential_blocker(&[("B1", 2, 2, "")], &[], vec![], None)]
#[case::two_potential_blockers_are_offered(&[("B1", 2, 2, ""), ("B2", 2, 2, "")], &[("B1", "A"), ("B2", "A")], vec![pair("B1", "A"), pair("B2", "A")], Some(Declared::Accepted))]
#[case::one_of_two_is_refused(&[("B1", 2, 2, ""), ("B2", 2, 2, "")], &[("B1", "A")], vec![pair("B1", "A"), pair("B2", "A")], Some(Declared::Refused { needed: 2, got: 1 }))]
#[case::no_block_is_always_legal(&[("B1", 2, 2, ""), ("B2", 2, 2, "")], &[], vec![pair("B1", "A"), pair("B2", "A")], Some(Declared::Accepted))]
#[case::three_may_block_with_two(&[("B1", 1, 1, ""), ("B2", 1, 1, ""), ("B3", 1, 1, "")], &[("B1", "A"), ("B3", "A")], vec![pair("B1", "A"), pair("B2", "A"), pair("B3", "A")], Some(Declared::Accepted))]
#[case::three_may_block_with_three(&[("B1", 1, 1, ""), ("B2", 1, 1, ""), ("B3", 1, 1, "")], &[("B1", "A"), ("B2", "A"), ("B3", "A")], vec![pair("B1", "A"), pair("B2", "A"), pair("B3", "A")], Some(Declared::Accepted))]
fn menace_can_be_blocked_only_by_two_or_more(
    #[case] blockers: &[Unit],
    #[case] blocks: &[(&str, &str)],
    #[case] offered: Vec<(String, String)>,
    #[case] declared: Option<Declared>,
) -> Result<(), ScriptError> {
    let attackers: [Unit; 1] = [("A", 3, 3, "menace")];
    let mut driver = duel(&attackers, blockers)?;
    attack_with_all(&mut driver, &attackers)?;
    assert_eq!(offered_blocks(&driver), offered);
    let Some(declared) = declared else {
        return Ok(());
    };
    let before = driver.engine().state_hash();
    let result = match driver.block(blocks) {
        Ok(_) => Declared::Accepted,
        Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::TooFewBlockers {
            needed,
            got,
        }))) => Declared::Refused { needed, got },
        Err(other) => return Err(other),
    };
    assert_eq!(result, declared);
    if matches!(declared, Declared::Refused { .. }) {
        assert_eq!(driver.engine().state_hash(), before);
        driver.expect_decision(1, "declare-blockers")?;
    }
    Ok(())
}

#[rstest]
// Only the creatures that could block the menace attacker count towards its two: three creatures, one that can.
#[case::a_flying_menace_attacker_needs_two_flying_or_reach(("A", 3, 3, "menace flying"), &[("G1", 2, 2, ""), ("G2", 2, 2, ""), ("F", 2, 2, "flying")], vec![])]
#[case::two_creatures_that_can_block_it(("A", 3, 3, "menace flying"), &[("R", 2, 2, "reach"), ("F", 2, 2, "flying")], vec![pair("F", "A"), pair("R", "A")])]
fn menace_counts_only_the_creatures_that_can_block_the_attacker(
    #[case] attacker: Unit,
    #[case] blockers: &[Unit],
    #[case] offered: Vec<(String, String)>,
) -> Result<(), ScriptError> {
    let mut driver = duel(&[attacker], blockers)?;
    attack_with_all(&mut driver, &[attacker])?;
    assert_eq!(offered_blocks(&driver), offered);
    Ok(())
}

#[test]
fn the_options_say_how_many_blockers_a_menace_attacker_needs() -> Result<(), ScriptError> {
    let attackers: [Unit; 2] = [("Menace", 2, 2, "menace"), ("Plain", 2, 2, "")];
    let mut driver = duel(&attackers, &[("B1", 1, 4, ""), ("B2", 1, 4, "")])?;
    attack_with_all(&mut driver, &attackers)?;
    let menace = driver.id_of("Menace")?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("blockers expected");
    };
    assert_eq!(options.min_blockers, [(menace, 2)]);
    // With one creature left that could block, the attacker is not offered and needs nobody.
    let attackers: [Unit; 2] = [("Menace", 2, 2, "menace"), ("Plain", 2, 2, "")];
    let mut driver = duel(&attackers, &[("B1", 1, 4, "")])?;
    attack_with_all(&mut driver, &attackers)?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("blockers expected");
    };
    assert_eq!(options.min_blockers, []);
    assert_eq!(options.blockers.len(), 1);
    assert_eq!(
        options.blockers[0].can_block.len(),
        1,
        "only the plain attacker"
    );
    Ok(())
}

#[test]
fn menace_and_a_plain_attacker_are_blocked_separately() -> Result<(), ScriptError> {
    let attackers: [Unit; 2] = [("Menace", 2, 2, "menace"), ("Plain", 2, 2, "")];
    let mut driver = duel(&attackers, &[("B1", 1, 4, ""), ("B2", 1, 4, "")])?;
    attack_with_all(&mut driver, &attackers)?;
    // One blocker on each is legal for the plain attacker only.
    let refused = driver.block(&[("B1", "Menace"), ("B2", "Plain")]);
    assert_eq!(
        refused.err(),
        Some(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::TooFewBlockers { needed: 2, got: 1 }
        )))
    );
    driver.block(&[("B1", "Plain")])?;
    driver.idle_until(2, Step::EndCombat)?;
    // The menace attacker was unblocked.
    driver.expect_life(1, 18)
}

fn attackers_offered(driver: &Driver) -> Vec<String> {
    let Ok(decision) = driver.decision() else {
        return Vec::new();
    };
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        return Vec::new();
    };
    options
        .attackers
        .iter()
        .map(|a| name_of(driver.state(), a.object))
        .collect()
}

#[rstest]
#[case::defender_cannot_attack("defender", false)]
#[case::defender_beats_haste("defender haste", false)]
#[case::defender_with_other_words("vigilance defender lifelink", false)]
#[case::a_plain_creature_can("", true)]
#[case::vigilance_can("vigilance", true)]
#[case::reach_can("reach", true)]
fn defender_cannot_attack(
    #[case] words: &'static str,
    #[case] attacks: bool,
) -> Result<(), ScriptError> {
    let driver = duel(
        &[("A", 2, 2, words), ("Other", 1, 1, "")],
        &[("B", 1, 1, "")],
    )?;
    let offered = attackers_offered(&driver);
    assert_eq!(offered.contains(&"A".to_owned()), attacks, "{offered:?}");
    assert!(offered.contains(&"Other".to_owned()));
    Ok(())
}

#[test]
fn a_creature_with_defender_blocks_and_a_table_of_defenders_has_nothing_to_attack_with()
-> Result<(), ScriptError> {
    let driver = duel(&[("Wall", 0, 4, "defender")], &[("B", 1, 1, "")])?;
    // The only creature cannot attack, so the declaration is skipped and says so.
    driver.expect_event("the skipped attack", |e| {
        matches!(e, Event::NothingToDo { .. })
    })?;
    assert_eq!(attackers_offered(&driver).len(), 0);
    // The wall still blocks on the other player's turn.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, ready_units(&[("A", 2, 2, "")]))
            .battlefield(1, ready_units(&[("Wall", 0, 4, "defender")])),
    )?;
    attack_with_all(&mut driver, &[("A", 2, 2, "")])?;
    assert_eq!(offered_blocks(&driver), [pair("Wall", "A")]);
    driver.block(&[("Wall", "A")])?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 20)
}

#[rstest]
#[case::sick_creature_cannot_attack("", true, false)]
#[case::haste_ignores_sickness("haste", true, true)]
#[case::haste_among_other_words("deathtouch haste", true, true)]
#[case::other_words_do_not("vigilance trample", true, false)]
#[case::not_sick_attacks("", false, true)]
#[case::haste_not_sick_attacks("haste", false, true)]
fn haste_lets_a_creature_attack_the_turn_it_arrives(
    #[case] words: &'static str,
    #[case] is_sick: bool,
    #[case] attacks: bool,
) -> Result<(), ScriptError> {
    let unit: Unit = ("A", 2, 2, words);
    let permanent = if is_sick {
        sick(card(unit))
    } else {
        ready(card(unit))
    };
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [permanent, ready(free_creature("Filler", 1, 1))]),
    )?;
    assert_eq!(
        attackers_offered(&driver).contains(&"A".to_owned()),
        attacks
    );
    Ok(())
}

/// A 1/1 pinger (`{T}: 1 damage to any target`) with these keywords.
fn pinger_with(words: &[Keyword]) -> Arc<CardDefinition> {
    let mut card = (*pinger("Pinger", 0)).clone();
    for word in words {
        card = card.with_rules(Ability::Keyword(word.clone()));
    }
    Arc::new(card)
}

/// A 1/1 elf with `{T}: Add {G}` and these keywords.
fn elf_with(words: &[Keyword]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Elf")
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_ability(AbilityDef::Mana(ManaAbility {
            produces: ManaType::Green,
        }));
    for word in words {
        card = card.with_rules(Ability::Keyword(word.clone()));
    }
    Arc::new(card)
}

#[rstest]
#[case::sick_pinger_waits(&[], true, false)]
#[case::hasty_sick_pinger_pings(&[Keyword::Haste], true, true)]
#[case::pinger_with_only_other_words_waits(&[Keyword::Vigilance], true, false)]
#[case::ready_pinger_pings(&[], false, true)]
fn haste_also_frees_tap_abilities(
    #[case] words: &[Keyword],
    #[case] is_sick: bool,
    #[case] taps: bool,
) -> Result<(), ScriptError> {
    let pinger = pinger_with(words);
    let permanent = if is_sick { sick(pinger) } else { ready(pinger) };
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .battlefield(0, [permanent])
            .battlefield(1, [ready(free_creature("B", 1, 1))]),
    )?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority expected");
    };
    assert_eq!(!options.abilities.is_empty(), taps);
    assert_eq!(options.can_act, taps);
    Ok(())
}

#[rstest]
#[case::sick_elf_cannot_tap(&[], true, false)]
#[case::hasty_sick_elf_taps_for_mana(&[Keyword::Haste], true, true)]
#[case::ready_elf_taps(&[], false, true)]
fn haste_also_frees_mana_abilities(
    #[case] words: &[Keyword],
    #[case] is_sick: bool,
    #[case] taps: bool,
) -> Result<(), ScriptError> {
    let elf = elf_with(words);
    let permanent = if is_sick { sick(elf) } else { ready(elf) };
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .battlefield(0, [permanent]),
    )?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority expected");
    };
    assert_eq!(!options.mana_abilities.is_empty(), taps);
    Ok(())
}

#[rstest]
#[case::attackers_tap("", true)]
#[case::vigilance_stays_untapped("vigilance", false)]
#[case::vigilance_among_others("first-strike vigilance", false)]
#[case::reach_taps("reach", true)]
fn vigilance_attacks_without_tapping(
    #[case] words: &'static str,
    #[case] tapped: bool,
) -> Result<(), ScriptError> {
    let attackers: [Unit; 2] = [("A", 2, 2, words), ("Plain", 1, 1, "")];
    let mut driver = duel(&attackers, &[("B", 1, 1, "")])?;
    driver.attack(&[("A", 1), ("Plain", 1)])?;
    let state = driver.state();
    let tapped_now = |name: &str| -> Option<bool> {
        let id = state
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| name_of(state, *id) == name)?;
        state
            .object(id)?
            .permanent()
            .map(mtg_engine::object::PermanentState::tapped)
    };
    assert_eq!(tapped_now("A"), Some(tapped));
    assert_eq!(
        tapped_now("Plain"),
        Some(true),
        "a creature without vigilance taps"
    );
    Ok(())
}

#[test]
fn a_vigilant_attacker_can_still_use_its_tap_ability_after_attacking() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(pinger_with(&[Keyword::Vigilance]))])
            .battlefield(1, [ready(free_creature("B", 1, 1))]),
    )?;
    driver.attack(&[("Pinger", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[])?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.pass()?;
    driver.pass_until(Step::Main2)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority expected");
    };
    assert_eq!(options.abilities.len(), 1, "the untapped attacker can ping");
    driver.expect_life(1, 19)
}

#[test]
fn the_declared_attackers_of_a_vigilant_table_are_logged_without_taps() -> Result<(), ScriptError> {
    let attackers: [Unit; 2] = [("V1", 1, 1, "vigilance"), ("V2", 1, 1, "vigilance")];
    let mut driver = duel(&attackers, &[("B", 1, 1, "")])?;
    let events = driver.attack(&[("V1", 1), ("V2", 1)])?;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::AttackersDeclared { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::PermanentTapped { .. }))
    );
    // The keyword names resolve to the rules value the words stand for.
    assert_eq!(keyword("vigilance"), Keyword::Vigilance);
    Ok(())
}
