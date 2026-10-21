//! Goal: the X of a spell is read where a card puts it after the announcement: the intervening "if" of an enters trigger when the trigger is detected and when it resolves, also after the permanent has left; the trigger "when you cast this spell" and the trigger of a permanent on any spell cast, also after the spell was countered; and the announcement of a spell with X in a target filter offers only the values of X that leave a target, refuses the others and cannot be cast when no value does; the log of each replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event, PresetPermanent, replay};
use mtg_testkit::cards::{bear, forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::x_cards::blaze;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A card of this type whose cost is `rest` besides `x` symbols of `{X}`, with these abilities written as data.
fn made(
    name: &str,
    kind: CardType,
    (rest, x): (&str, u8),
    abilities: &[&str],
) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(ManaCost::parse(rest).unwrap())
        .with_x_in_cost(x);
    if kind == CardType::Creature {
        card = card.with_power_toughness(3, 3);
    }
    for ability in abilities {
        card = card.with_rules(serde_json::from_str(ability).unwrap());
    }
    Arc::new(card)
}

/// A creature for `{X}`: when it enters, if X is 5 or more, its controller gains 3 life.
fn warden() -> Arc<CardDefinition> {
    made(
        "Warden",
        CardType::Creature,
        ("", 1),
        &[
            r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Compare":{"left":"X","op":"Ge","right":5}},"effects":[{"GainLife":{"amount":3}}]}}"#,
        ],
    )
}

/// A sorcery for `{X}{R}` that gains 1 life; when it is cast, its controller gains X life.
fn sparker() -> Arc<CardDefinition> {
    made(
        "Sparker",
        CardType::Sorcery,
        ("{R}", 1),
        &[
            r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#,
            r#"{"Triggered":{"when":{"SpellCast":{"what":["This"],"by":"You"}},"from":"Stack","effects":[{"GainLife":{"amount":"X"}}]}}"#,
        ],
    )
}

/// An enchantment that gains its controller X life whenever they cast a spell, X being that of the spell.
fn font() -> Arc<CardDefinition> {
    made(
        "Font",
        CardType::Enchantment,
        ("{1}", 0),
        &[
            r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"GainLife":{"amount":"X"}}]}}"#,
        ],
    )
}

fn lands(count: usize, land: fn() -> Arc<CardDefinition>) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

/// `permanents` and a Font.
fn with_font(mut permanents: Vec<PresetPermanent>) -> Vec<PresetPermanent> {
    permanents.push(ready(font()));
    permanents
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn say(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver.expect_decision(0, "choose-number")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    Ok(())
}

fn pay(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|found| *found == name)
        .count()
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// How many triggered abilities were put on the stack.
fn triggers_put(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityPutOnStack(put) if put.cause.is_some()))
        .count()
}

/// Casts `name` for `x` and lets everything resolve.
fn cast(driver: &mut Driver, name: &str, x: u32) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    say(driver, x)?;
    pay(driver)?;
    settle(driver)
}

/// The opponent destroys `name` with Doom Blade, with whatever is on the stack.
fn doom(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.expect_decision(1, "priority")?;
    let target = driver.target_named(name)?;
    driver.begin_cast("Doom Blade")?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

/// The opponent counters the spell `name` with Counterspell, with whatever is above it on the stack.
fn counter(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.expect_decision(1, "priority")?;
    let state = driver.state();
    let spell = state
        .zone(Zone::Stack)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == name)
        .map(|object| Target::Object(object.reference()))
        .ok_or_else(|| ScriptError::NoSuchCard {
            name: name.to_owned(),
            place: "the stack",
        })?;
    driver.begin_cast("Counterspell")?;
    driver.choose_targets(&[spell])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

#[rstest]
#[case::nothing(0, false)]
#[case::one(1, false)]
#[case::just_short(4, false)]
#[case::just_enough(5, true)]
#[case::more(6, true)]
#[case::large(12, true)]
fn an_enters_trigger_with_an_if_on_x_triggers_for_the_x_that_meets_it(
    #[case] x: u32,
    #[case] gains: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [warden()])
            .battlefield(0, lands(12, mountain))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    cast(&mut driver, "Warden", x)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Warden"), 1);
    assert_eq!(life(&driver, 0), if gains { 23 } else { 20 });
    // The "if" is checked as the ability would trigger: one that is not true never goes on the stack (CR 603.4).
    assert_eq!(triggers_put(&driver), usize::from(gains));
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::just_enough(5)]
#[case::more(6)]
#[case::large(12)]
fn the_if_of_an_enters_trigger_reads_the_x_of_a_permanent_that_left(
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [warden()])
            .battlefield(0, lands(12, mountain))
            .hand(1, [card("Doom Blade").unwrap()])
            .battlefield(1, lands(2, swamp))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Warden")?;
    say(&mut driver, x)?;
    pay(&mut driver)?;
    driver.pass()?;
    driver.pass()?;
    // The enters trigger waits on the stack; its controller passes and the opponent destroys the creature in response.
    driver.pass()?;
    doom(&mut driver, "Warden")?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Warden"), 0);
    assert_eq!(
        life(&driver, 0),
        23,
        "the trigger still reads the X of its source"
    );
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn a_countered_spell_with_an_enters_trigger_never_triggers() -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [warden()])
            .battlefield(0, lands(12, mountain))
            .hand(1, [card("Counterspell").unwrap()])
            .battlefield(1, lands(2, island))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Warden")?;
    say(&mut driver, 9)?;
    pay(&mut driver)?;
    driver.pass()?;
    counter(&mut driver, "Warden")?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Warden"), 0);
    assert_eq!(life(&driver, 0), 20);
    assert_eq!(triggers_put(&driver), 0);
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::nothing(0)]
#[case::one(1)]
#[case::three(3)]
#[case::large(12)]
fn the_cast_trigger_of_a_spell_reads_the_x_it_was_cast_with(
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [sparker()])
            .battlefield(0, lands(13, mountain))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    cast(&mut driver, "Sparker", x)?;
    // X from the trigger, 1 from the spell.
    assert_eq!(life(&driver, 0), 21 + i32::try_from(x).unwrap());
    assert_eq!(triggers_put(&driver), 1);
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::one(1)]
#[case::four(4)]
fn the_cast_trigger_of_a_spell_that_was_countered_still_reads_its_x(
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [sparker()])
            .battlefield(0, lands(8, mountain))
            .hand(1, [card("Counterspell").unwrap()])
            .battlefield(1, lands(2, island))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Sparker")?;
    say(&mut driver, x)?;
    pay(&mut driver)?;
    // The trigger is above the spell; the spell is countered while the trigger waits.
    driver.pass()?;
    counter(&mut driver, "Sparker")?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, Zone::Graveyard(pid(0)), "Sparker"), 1);
    assert_eq!(
        life(&driver, 0),
        20 + i32::try_from(x).unwrap(),
        "no life from the spell"
    );
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::nothing(0)]
#[case::one(1)]
#[case::three(3)]
fn a_trigger_on_any_spell_cast_reads_the_x_of_that_spell(
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [blaze().unwrap()])
            .battlefield(0, with_font(lands(4, mountain)))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Blaze")?;
    say(&mut driver, x)?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 20 + i32::try_from(x).unwrap());
    assert_eq!(life(&driver, 1), 20 - i32::try_from(x).unwrap());
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn a_trigger_on_a_spell_without_x_reads_0() -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [bear()])
            .battlefield(0, with_font(lands(2, forest)))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Grizzly Bears")?;
    settle(&mut driver)?;
    assert_eq!(triggers_put(&driver), 1);
    assert_eq!(life(&driver, 0), 20);
    Ok(())
}

/// What the cast of Disembowel does against opposing creatures of these mana costs with `lands` Swamps.
#[derive(Debug, PartialEq, Eq)]
enum Offered {
    /// A number between these bounds, less the numbers excepted.
    Between(u32, u32, Vec<u32>),
    /// X is the one value left, so the targets are asked at once.
    Only(u32),
    /// The spell is not offered.
    NotCastable,
}

fn creature_costing(name: &str, cost: &str) -> Arc<CardDefinition> {
    let mut found = (*free_creature(name, 2, 2)).clone();
    if !cost.is_empty() {
        found.mana_cost = Some(ManaCost::parse(cost).unwrap());
    }
    Arc::new(found)
}

fn disembowel_against(
    costs: &[&str],
    lands_count: usize,
) -> Result<(Driver, Offered), ScriptError> {
    let creatures: Vec<PresetPermanent> = costs
        .iter()
        .enumerate()
        .map(|(at, cost)| ready(creature_costing(&format!("Target {at}"), cost)))
        .collect();
    let scenario = Scenario::new(2)
        .hand(0, [card("Disembowel").unwrap()])
        .battlefield(0, lands(lands_count, swamp))
        .battlefield(1, creatures);
    let mut driver = Driver::from_scenario(scenario)?;
    let offered = match driver.begin_cast("Disembowel") {
        Err(ScriptError::NoSuchCard { .. }) => Offered::NotCastable,
        Err(other) => return Err(other),
        Ok(()) => match &driver.decision()?.kind {
            DecisionKind::ChooseNumber(options) => {
                Offered::Between(options.min, options.max.unwrap(), options.except.clone())
            }
            DecisionKind::ChooseTargets(_) => Offered::Only(only_x(&driver)),
            other => panic!("the cast asked {other:?}"),
        },
    };
    Ok((driver, offered))
}

/// The mana value of the creatures offered as targets: with a single X left it is the X of the cast.
fn only_x(driver: &Driver) -> u32 {
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("not a target decision");
    };
    let state = driver.state();
    let Target::Object(first) = options.legal[0] else {
        panic!("a player among the targets");
    };
    state.object(first.id).unwrap().card().mana_value()
}

#[rstest]
#[case::the_values_that_have_a_target(&["{1}", "{2}", "{4}"], 5, Offered::Between(1, 4, vec![3]))]
#[case::far_apart(&["{1}", "{1}", "{6}"], 7, Offered::Between(1, 6, vec![2, 3, 4, 5]))]
#[case::a_free_creature_and_a_dear_one(&["", "{2}"], 3, Offered::Between(0, 2, vec![1]))]
#[case::one_value_is_no_question(&["{2}", "{2}"], 4, Offered::Only(2))]
#[case::nothing_is_a_value(&[""], 3, Offered::Only(0))]
#[case::a_value_that_cannot_be_paid_is_not_offered(&["{1}", "{6}"], 4, Offered::Only(1))]
#[case::no_value_with_a_target_can_be_paid(&["{5}"], 4, Offered::NotCastable)]
#[case::no_creature(&[], 4, Offered::NotCastable)]
fn disembowel_offers_the_values_of_x_that_have_a_target(
    #[case] costs: &[&str],
    #[case] lands_count: usize,
    #[case] expected: Offered,
) -> Result<(), ScriptError> {
    let (_, offered) = disembowel_against(costs, lands_count)?;
    assert_eq!(offered, expected);
    Ok(())
}

#[rstest]
#[case::below_the_gap(1, true)]
#[case::in_the_gap(2, false)]
#[case::in_the_gap_too(3, false)]
#[case::above_the_gap(4, true)]
#[case::above_the_most(5, false)]
fn disembowel_refuses_a_value_of_x_with_no_target(
    #[case] x: u32,
    #[case] accepted: bool,
) -> Result<(), ScriptError> {
    let (mut driver, offered) = disembowel_against(&["{1}", "{4}"], 6)?;
    assert_eq!(offered, Offered::Between(1, 4, vec![2, 3]));
    let result = driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)));
    assert_eq!(result.is_ok(), accepted, "{x}: {result:?}");
    if !accepted {
        assert!(matches!(
            result,
            Err(ScriptError::Engine(EngineError::Illegal(_)))
        ));
        // The question is asked again and the cast can still be backed out of.
        assert!(driver.expect_decision(0, "choose-number").is_ok());
    }
    Ok(())
}

#[rstest]
#[case::low(1, "Target 0")]
#[case::high(4, "Target 1")]
fn disembowel_destroys_the_creature_of_the_value_chosen(
    #[case] x: u32,
    #[case] dies: &str,
) -> Result<(), ScriptError> {
    let scenario = || {
        let creatures = [("Target 0", "{1}"), ("Target 1", "{4}")]
            .map(|(name, cost)| ready(creature_costing(name, cost)));
        Scenario::new(2)
            .hand(0, [card("Disembowel").unwrap()])
            .battlefield(0, lands(6, swamp))
            .battlefield(1, creatures)
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Disembowel")?;
    say(&mut driver, x)?;
    let only = driver.decision()?;
    let DecisionKind::ChooseTargets(options) = &only.kind else {
        panic!("targets are asked after X");
    };
    assert_eq!(
        options.legal.len(),
        1,
        "only the creature of that mana value"
    );
    let wanted = driver.target_named(dies)?;
    driver.choose_targets(&[wanted])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let left = names(driver.state(), Zone::Battlefield);
    assert!(!left.contains(&dies.to_owned()));
    replays(&driver, scenario());
    Ok(())
}

/// Wargate with its three colours and `extra` Mountains for X, and a library of permanents of mana value 0, 1, 1, 1, 3
/// and 5 and an instant of mana value 1.
fn wargate(extra: usize) -> Scenario {
    let mut permanents = vec![ready(forest()), ready(plains()), ready(island())];
    permanents.extend(lands(extra, mountain));
    Scenario::new(2)
        .hand(0, [card("Wargate").unwrap()])
        .battlefield(0, permanents)
        .library_named(
            0,
            [
                "Batterskull",
                "Hall of Triumph",
                "Bonesplitter",
                "Sol Ring",
                "Llanowar Elves",
                "Forest",
                "Lightning Bolt",
            ],
        )
}

#[rstest]
#[case::nothing(0, &["Forest"])]
#[case::one(1, &["Bonesplitter", "Forest", "Llanowar Elves", "Sol Ring"])]
#[case::two(2, &["Bonesplitter", "Forest", "Llanowar Elves", "Sol Ring"])]
#[case::three(3, &["Bonesplitter", "Forest", "Hall of Triumph", "Llanowar Elves", "Sol Ring"])]
#[case::large(6, &["Batterskull", "Bonesplitter", "Forest", "Hall of Triumph", "Llanowar Elves", "Sol Ring"])]
fn wargate_finds_only_permanents_of_mana_value_x_or_less(
    #[case] x: u32,
    #[case] found: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(wargate(6))?;
    driver.begin_cast("Wargate")?;
    say(&mut driver, x)?;
    pay(&mut driver)?;
    driver.pass()?;
    driver.pass()?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("a search was expected");
    };
    let mut offered: Vec<String> = options
        .from
        .iter()
        .map(|id| common::name_of(driver.state(), *id))
        .collect();
    offered.sort();
    assert_eq!(offered, found);
    let pick = options.from[0];
    let picked = common::name_of(driver.state(), pick);
    driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
        pick,
    ])))?;
    settle(&mut driver)?;
    assert!(count(&driver, Zone::Battlefield, &picked) >= 1);
    replays(&driver, wargate(6));
    Ok(())
}

#[test]
fn a_countered_wargate_searches_for_nothing() -> Result<(), ScriptError> {
    let scenario = || {
        wargate(2)
            .hand(1, [card("Counterspell").unwrap()])
            .battlefield(1, lands(2, island))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Wargate")?;
    say(&mut driver, 2)?;
    pay(&mut driver)?;
    driver.pass()?;
    counter(&mut driver, "Wargate")?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, Zone::Graveyard(pid(0)), "Wargate"), 1);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 7);
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn a_countered_disembowel_destroys_nothing() -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Disembowel").unwrap()])
            .battlefield(0, lands(3, swamp))
            .hand(1, [card("Counterspell").unwrap()])
            .battlefield(
                1,
                [
                    ready(creature_costing("Target", "{2}")),
                    ready(island()),
                    ready(island()),
                ],
            )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Disembowel")?;
    // X is 2 with nothing to choose, so the targets come next.
    let target = driver.target_named("Target")?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    driver.pass()?;
    counter(&mut driver, "Disembowel")?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Target"), 1);
    replays(&driver, scenario());
    Ok(())
}
