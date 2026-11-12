//! Goal: the engine pieces of pack r30-3b play at 2 to 4 seats: absorb lessens each damage event, a rule deals all damage as though its source had wither, lands that lose all abilities but mana abilities keep tapping for mana and lose the rest, a redirection shield counts its total as it begins, a chosen creature type is added to the others, the creatures a selector picks must block the attacker that names them, a sick creature attacks only the opponents of the rule's controller, and under a mana burn rule any player losing unspent mana loses that much life.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::OptionChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn with_rules(card: CardDefinition, abilities: &[&str]) -> Arc<CardDefinition> {
    abilities
        .iter()
        .fold(card, |card, json| card.with_rules(rules(json)))
        .into()
}

/// A free sorcery that deals `amount` damage to target creature.
fn bolt(name: &str, amount: u32) -> Arc<CardDefinition> {
    let spell = format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
    );
    with_rules(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[])),
        &[&spell],
    )
}

/// A permanent with these abilities and nothing else.
fn enchantment(name: &str, abilities: &[&str]) -> Arc<CardDefinition> {
    with_rules(
        CardDefinition::new(name).with_type(CardType::Enchantment),
        abilities,
    )
}

/// Answers every decision passively, paying mana automatically, until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let paying = matches!(driver.decision()?.kind, DecisionKind::PayMana(_));
        if !paying && driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match driver.decision()?.kind {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseOption(_) => {
                driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn damage(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage()
}

fn withered(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::MinusOneMinusOne)
}

/// A 0/9 creature with absorb `n`, when `n` is above zero.
fn sponge(n: u32) -> Arc<CardDefinition> {
    let card = CardDefinition::new("Sponge")
        .with_type(CardType::Creature)
        .with_power_toughness(0, 9);
    if n == 0 {
        return card.into();
    }
    card.with_rules(Ability::Keyword(Keyword::Macro {
        name: "absorb".to_owned(),
        amount: n,
    }))
    .into()
}

#[rstest]
fn absorb_lessens_each_damage_event(
    #[values(2, 3, 4)] seats: usize,
    #[values((0, 3, 2), (1, 3, 2), (2, 3, 1), (3, 2, 2))] case: (u32, u32, u32),
) -> Result<(), ScriptError> {
    let (absorb, first, second) = case;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(sponge(absorb))])
            .hand(0, [bolt("First", first), bolt("Second", second)]),
    )?;
    let sponge = driver.target_named("Sponge")?;
    driver.cast_at("First", &[sponge])?;
    settle(&mut driver)?;
    driver.cast_at("Second", &[sponge])?;
    settle(&mut driver)?;
    assert_eq!(
        damage(&driver, "Sponge"),
        first.saturating_sub(absorb) + second.saturating_sub(absorb),
        "each event loses up to {absorb}"
    );
    Ok(())
}

#[rstest]
fn a_wither_rule_deals_damage_to_creatures_as_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] ruled: bool,
    #[values(1, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let mut battlefield = vec![ready(free_creature("Bear", 0, 9))];
    if ruled {
        battlefield.push(ready(enchantment(
            "Torment",
            &[r#"{"Rule":{"rule":"DamageAsWither"}}"#],
        )));
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, battlefield)
            .hand(0, [bolt("Bolt", amount)]),
    )?;
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Bolt", &[bear])?;
    settle(&mut driver)?;
    let (marked, counters) = if ruled { (0, amount) } else { (amount, 0) };
    assert_eq!(damage(&driver, "Bear"), marked);
    assert_eq!(withered(&driver, "Bear"), counters);
    Ok(())
}

/// A Forest that also has flying and "{0}: You gain 1 life.".
fn odd_forest() -> Arc<CardDefinition> {
    with_rules(
        CardDefinition::basic_land(Some(Color::Green)),
        &[
            r#"{"Keyword":"Flying"}"#,
            r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#,
        ],
    )
}

#[rstest]
fn lands_that_lose_all_but_mana_abilities_still_tap_for_mana(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] ruled: bool,
) -> Result<(), ScriptError> {
    let mut battlefield = vec![ready(odd_forest())];
    let name = odd_forest().name.clone();
    if ruled {
        battlefield.push(ready(enchantment(
            "Sun",
            &[r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":["LoseAbilitiesExceptMana"]}}"#],
        )));
    }
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, battlefield))?;
    let id = driver.id_of(&name)?;
    assert_eq!(has_keyword(driver.state(), id, &Keyword::Flying), !ruled);
    assert_eq!(driver.offered_abilities(&name)?.is_empty(), ruled);
    driver.tap_for_mana(&name)?;
    let pool = driver
        .state()
        .player(crate::common::pid(0))
        .map(|p| p.mana().get(ManaType::Green));
    assert_eq!(pool, Some(1), "the mana ability stays");
    Ok(())
}

/// "{0}: The next `most` damage that would be dealt to this creature this turn is dealt to target creature you control
/// instead", the total an amount counted as the shield begins: the number of Markers on the battlefield.
fn kor() -> Arc<CardDefinition> {
    with_rules(
        CardDefinition::new("Kor")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 9),
        &[
            r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Shield":{"when":{"Damage":{"to":"This"}},"by":{"RedirectCounted":{"most":{"Count":[{"Named":"Marker"}]},"to":{"Target":0}}},"duration":"UntilEndOfTurn"}}]}}"#,
        ],
    )
}

fn marker() -> Arc<CardDefinition> {
    enchantment("Marker", &[])
}

#[rstest]
fn a_counted_redirection_takes_its_total_as_it_begins(
    #[values(2, 3, 4)] seats: usize,
    #[values((1, 3), (2, 1), (3, 3), (0, 2))] case: (usize, u32),
) -> Result<(), ScriptError> {
    let (markers, amount) = case;
    let mut battlefield = vec![ready(kor()), ready(free_creature("Ally", 0, 9))];
    battlefield.extend((0..markers).map(|_| ready(marker())));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, battlefield)
            .hand(0, [bolt("Bolt", amount)]),
    )?;
    driver.begin_activation("Kor")?;
    let ally = driver.target_named("Ally")?;
    driver.choose_targets(&[ally])?;
    settle(&mut driver)?;
    let kor = driver.target_named("Kor")?;
    driver.cast_at("Bolt", &[kor])?;
    settle(&mut driver)?;
    let moved = amount.min(u32::try_from(markers).unwrap());
    assert_eq!(damage(&driver, "Ally"), moved);
    assert_eq!(damage(&driver, "Kor"), amount - moved);
    Ok(())
}

/// A Sliver with "{0}: This creature becomes the creature type of your choice in addition to its other types until end
/// of turn."
fn shifter() -> Arc<CardDefinition> {
    with_rules(
        CardDefinition::new("Shifter")
            .with_type(CardType::Creature)
            .with_subtype("Sliver")
            .with_power_toughness(1, 1),
        &[
            r#"{"Activated":{"cost":"{0}","effects":[{"ChooseValue":{"kind":"CreatureType","as":"chosen"}},{"Continuous":{"affected":"This","mods":[{"AddCreatureTypeChosen":"chosen"}],"duration":"UntilEndOfTurn"}}]}}"#,
        ],
    )
}

#[rstest]
fn a_chosen_creature_type_is_added_to_the_others(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [ready(shifter()), ready(free_creature("Elf", 1, 1))]),
    )?;
    let id = driver.id_of("Shifter")?;
    let before = characteristics(driver.state(), id).unwrap().subtypes;
    driver.begin_activation("Shifter")?;
    settle(&mut driver)?;
    let after = characteristics(driver.state(), id).unwrap().subtypes;
    assert!(
        before.iter().all(|kind| after.contains(kind)),
        "the old types stay: {before:?} in {after:?}"
    );
    assert_eq!(after.len(), before.len() + 1, "a type is added: {after:?}");
    driver.idle_until(2, Step::Upkeep)?;
    let id = driver.id_of("Shifter")?;
    assert_eq!(
        characteristics(driver.state(), id).unwrap().subtypes,
        before
    );
    Ok(())
}

/// A 1/1 attacker that every Wall able to block must block.
fn priest() -> Arc<CardDefinition> {
    with_rules(
        CardDefinition::new("Priest")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
        &[
            r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"MustBeBlockedBy":[{"Subtype":"Wall"}]}}}}}"#,
        ],
    )
}

fn wall(name: &str) -> Arc<CardDefinition> {
    CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_subtype("Wall")
        .with_power_toughness(0, 4)
        .into()
}

#[rstest]
fn walls_able_to_block_the_attacker_must_and_others_need_not(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(priest()), ready(free_creature("Other", 1, 1))])
            .battlefield(1, [ready(wall("Wall")), ready(free_creature("Bear", 2, 2))]),
    )?;
    driver.attack(&[("Priest", 1), ("Other", 1)])?;
    for _ in 0..seats {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.pass()?;
    }
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("expected blockers");
    };
    let wall = driver.id_of("Wall")?;
    let bear = driver.id_of("Bear")?;
    let priest = driver.id_of("Priest")?;
    let walls = options.blockers.iter().find(|b| b.object == wall).unwrap();
    assert_eq!(
        walls.can_block,
        vec![priest],
        "the Wall may block only the Priest"
    );
    let bears = options.blockers.iter().find(|b| b.object == bear).unwrap();
    assert_eq!(bears.can_block.len(), 2, "the Bear is free");
    assert!(driver.block(&[]).is_err(), "the Wall must block");
    driver.block(&[("Wall", "Priest")])?;
    Ok(())
}

#[rstest]
fn sick_creatures_attack_only_the_opponents_of_the_rules_controller(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] ruler: usize,
) -> Result<(), ScriptError> {
    let rule = r#"{"Rule":{"rule":{"AttackAsThoughHasteAt":"Opponents"}}}"#;
    let brute = ready(enchantment("Brute", &[rule]));
    let mut mine = vec![
        sick(free_creature("Fresh", 1, 1)),
        ready(free_creature("Old", 1, 1)),
    ];
    let mut scenario = Scenario::new(seats).at(2, 0, Step::DeclareAttackers);
    if ruler == 0 {
        mine.push(brute);
    } else {
        scenario = scenario.battlefield(ruler, [brute]);
    }
    let driver = Driver::from_scenario(scenario.battlefield(0, mine))?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("expected attackers");
    };
    let fresh = driver.id_of("Fresh")?;
    let offered: Vec<AttackTarget> = options
        .attackers
        .iter()
        .find(|a| a.object == fresh)
        .map(|a| a.defenders.clone())
        .unwrap_or_default();
    let wanted: Vec<AttackTarget> = (1..seats)
        .filter(|seat| *seat != ruler)
        .map(|seat| AttackTarget::Player(crate::common::pid(seat)))
        .collect();
    assert_eq!(offered, wanted);
    let old = driver.id_of("Old")?;
    let all = options.attackers.iter().find(|a| a.object == old).unwrap();
    assert_eq!(
        all.defenders.len(),
        seats - 1,
        "a creature that isn't sick attacks anyone"
    );
    Ok(())
}

#[rstest]
fn a_player_losing_unspent_mana_loses_that_much_life_under_the_rule(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(0), Some(1))] ruler: Option<usize>,
    #[values(1, 2)] forests: usize,
) -> Result<(), ScriptError> {
    let mut mine: Vec<_> = (0..forests).map(|_| ready(forest())).collect();
    let yurlok = || ready(enchantment("Yurlok", &[r#"{"Rule":{"rule":"ManaBurn"}}"#]));
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    match ruler {
        Some(0) => mine.push(yurlok()),
        Some(seat) => scenario = scenario.battlefield(seat, [yurlok()]),
        None => {}
    }
    let mut driver = Driver::from_scenario(scenario.battlefield(0, mine))?;
    for _ in 0..forests {
        driver.tap_for_mana("Forest")?;
    }
    driver.idle_until(2, Step::BeginCombat)?;
    let lost = if ruler.is_some() { forests } else { 0 };
    driver.expect_life(0, 20 - i32::try_from(lost).unwrap())?;
    driver.expect_life(1, 20)?;
    Ok(())
}
