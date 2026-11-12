//! Goal: the triggers of pack l29-0 play as their cards read at tables of two to four seats: an exhaust activation
//! triggers "whenever you activate an exhaust ability" and no other activation does; a player chosen as a target
//! triggers "whenever you become the target", filtered by who controls the spell or ability, and "that spell or ability"
//! and "its controller" name what targeted them; an attack with the card makes a token for each opponent; the attack
//! draw needs the most life; a keyword comes with enough power.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::{names, pid};

/// An artifact with "{0}: You gain 1 life.", once each game when `exhaust`.
fn engine_card(exhaust: bool) -> Arc<CardDefinition> {
    let limit = if exhaust {
        r#","limit":"OncePerGame""#
    } else {
        ""
    };
    let rule = format!(
        r#"{{"Activated":{{"cost":[]{limit},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Engine")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// An artifact with "{0}: Target player gains 10 life."
fn healer() -> Arc<CardDefinition> {
    let rule = r#"{"Activated":{"cost":[],"targets":[{"what":"Player"}],"effects":[{"GainLife":{"who":{"Target":0},"amount":10}}]}}"#;
    Arc::new(
        CardDefinition::new("Healer")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(rule).unwrap()),
    )
}

/// An enchantment with "Whenever `who` become(s) the target of a spell or ability `by`, you gain 100 life."
fn watcher(who: &str, by: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Triggered":{{"when":{{"PlayerBecomesTarget":{{"who":"{who}","by":{by}}}}},"effects":[{{"GainLife":{{"amount":100}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// Answers passively until nothing is on the stack and nothing waits.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
#[case::an_exhaust_ability(true, 1)]
#[case::another_ability(false, 0)]
fn rangers_refueler_draws_for_an_exhaust_activation_only(
    #[case] exhaust: bool,
    #[case] drawn: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .library(0, (0..5).map(|_| engine_card(false)))
            .battlefield(
                0,
                [
                    ready(card("Rangers' Refueler").unwrap()),
                    ready(engine_card(exhaust)),
                ],
            ),
    )?;
    let before = hand(&driver, 0);
    driver.begin_ability("Engine", 0)?;
    settle(&mut driver)?;
    assert_eq!(hand(&driver, 0) - before, drawn);
    Ok(())
}

#[rstest]
#[case::you_by_anyone("You", "[]", 0, 100)]
#[case::you_by_you("You", r#"[{"ControlledBy":"You"}]"#, 0, 100)]
#[case::you_by_an_opponent("You", r#"[{"ControlledBy":"Opponents"}]"#, 0, 0)]
#[case::you_not_by_a_spell("You", r#"["Spell"]"#, 0, 0)]
#[case::another_player_targeted("You", "[]", 1, 0)]
#[case::an_opponent_targeted("Opponents", "[]", 1, 100)]
fn becoming_the_target_triggers_for_the_player_and_the_source_named(
    #[case] who: &str,
    #[case] by: &str,
    #[case] target: usize,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [ready(healer()), ready(watcher(who, by))]),
    )?;
    let before = life(&driver, 0);
    driver.begin_ability("Healer", 0)?;
    let player = driver.target_player(target);
    driver.choose_targets(&[player])?;
    settle(&mut driver)?;
    let healed = if target == 0 { 10 } else { 0 };
    assert_eq!(life(&driver, 0) - before, gained + healed);
    Ok(())
}

#[rstest]
fn amulet_of_safekeeping_counters_what_targets_you_unless_its_controller_pays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // The amulet's controller is seat 1; seat 0 targets seat 1 and has no mana to pay.
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(healer())])
            .battlefield(1, [ready(card("Amulet of Safekeeping").unwrap())]),
    )?;
    let before = life(&driver, 1);
    driver.begin_ability("Healer", 0)?;
    let player = driver.target_player(1);
    driver.choose_targets(&[player])?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), before, "the ability is countered");
    Ok(())
}

#[rstest]
#[case::the_strike_leader("Ainok Strike Leader", true)]
#[case::another_creature("Bear", false)]
fn ainok_strike_leader_makes_a_goblin_attacking_each_opponent(
    #[case] attacker: &str,
    #[case] triggers: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let bear = Arc::new(
        CardDefinition::new("Bear")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [ready(card("Ainok Strike Leader").unwrap()), ready(bear)],
            ),
    )?;
    driver.attack(&[(attacker, 1)])?;
    settle(&mut driver)?;
    let goblins = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Goblin")
        .count();
    assert_eq!(goblins, if triggers { seats - 1 } else { 0 });
    Ok(())
}

#[rstest]
#[case::the_most_life(20, 1)]
#[case::tied_for_most(40, 1)]
#[case::less_life(10, 0)]
fn preacher_draws_on_attack_while_you_have_the_most_life(
    #[case] mine: i32,
    #[case] drawn: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .life(0, mine)
        .library(0, (0..5).map(|_| engine_card(false)))
        .battlefield(0, [ready(card("Preacher of the Schism").unwrap())]);
    for seat in 1..seats {
        scenario = scenario.life(seat, if seat == 1 { 40 } else { 15 });
    }
    if mine == 20 {
        scenario = scenario.life(1, 15);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before = (hand(&driver, 0), life(&driver, 0));
    driver.attack(&[("Preacher of the Schism", 1)])?;
    settle(&mut driver)?;
    assert_eq!(hand(&driver, 0) - before.0, drawn);
    assert_eq!(before.1 - life(&driver, 0), i32::try_from(drawn).unwrap());
    Ok(())
}

#[rstest]
#[case::power_ten(9, true)]
#[case::power_nine(8, false)]
fn witch_maw_nephilim_gains_trample_with_power_ten(
    #[case] counters: u32,
    #[case] tramples: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let nephilim = card("Witch-Maw Nephilim").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [with_counters(
                    ready(nephilim),
                    CounterKind::PlusOnePlusOne,
                    counters,
                )],
            ),
    )?;
    driver.attack(&[("Witch-Maw Nephilim", 1)])?;
    settle(&mut driver)?;
    let id = driver.id_of("Witch-Maw Nephilim")?;
    assert_eq!(
        has_keyword(driver.state(), id, &mtg_cards::keyword::Keyword::Trample),
        tramples
    );
    Ok(())
}

/// A card for `{0}` of the type with these rules (JSON), a 2/2 when it is a creature.
fn free(name: &str, ty: CardType, legendary: bool, rules: &[&str]) -> Arc<CardDefinition> {
    let mut definition = CardDefinition::new(name)
        .with_type(ty)
        .with_mana_cost(mtg_testkit::cards::cost(0, &[]));
    if ty == CardType::Creature {
        definition = definition.with_power_toughness(2, 2);
    }
    if legendary {
        definition = definition.with_supertype(mtg_core::card::Supertype::Legendary);
    }
    for rule in rules {
        definition = definition.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(definition)
}

fn count_named(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|n| *n == name)
        .count()
}

#[rstest]
#[case::the_holder_attacked(1, 1)]
#[case::the_holder_not_attacked(2, 0)]
fn loot_dispute_makes_treasure_when_you_attack_the_initiative_holder(
    #[case] holder: usize,
    #[case] treasures: usize,
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let gift = free(
        "Gift",
        CardType::Instant,
        false,
        &[
            r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"TakeInitiative":{"who":{"Target":0}}}]}}"#,
        ],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [gift])
            .battlefield(
                0,
                [
                    ready(card("Loot Dispute").unwrap()),
                    ready(free("Bear", CardType::Creature, false, &[])),
                ],
            ),
    )?;
    let player = driver.target_player(holder);
    driver.cast_at("Gift", &[player])?;
    settle(&mut driver)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let before = count_named(&driver, "Treasure");
    driver.attack(&[("Bear", 1)])?;
    settle(&mut driver)?;
    assert_eq!(count_named(&driver, "Treasure") - before, treasures);
    Ok(())
}

#[rstest]
#[case::a_targeted_ability(true, 20)]
#[case::an_untargeted_ability(false, 1)]
fn ertha_jo_copies_an_ability_that_targets_a_player(
    #[case] targeted: bool,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let source = if targeted {
        healer()
    } else {
        engine_card(false)
    };
    let name = if targeted { "Healer" } else { "Engine" };
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Ertha Jo, Frontier Mentor").unwrap()),
            ready(source),
        ],
    ))?;
    let before = life(&driver, 0);
    driver.begin_ability(name, 0)?;
    if targeted {
        let player = driver.target_player(0);
        driver.choose_targets(&[player])?;
    }
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0) - before, gained);
    Ok(())
}

#[rstest]
#[case::no_keimi_yet(false)]
#[case::keimi_already(true)]
fn tatsunari_makes_keimi_only_while_you_control_none(
    #[case] already: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Tatsunari, Toad Rider").unwrap())];
    if already {
        mine.push(ready(free("Keimi", CardType::Creature, true, &[])));
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [free("Charm", CardType::Enchantment, false, &[])])
            .battlefield(0, mine),
    )?;
    driver.cast("Charm")?;
    settle(&mut driver)?;
    let created = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, mtg_engine::events::Event::TokenCreated(token) if token.name == "Keimi"))
        .count();
    assert_eq!(created, usize::from(!already));
    Ok(())
}

#[rstest]
#[case::a_legendary_creature(true, 1)]
#[case::another_creature(false, 0)]
fn campsite_cuisine_makes_food_for_a_legendary_creature(
    #[case] legendary: bool,
    #[case] foods: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [free("Hero", CardType::Creature, legendary, &[])])
            .battlefield(0, [ready(card("Campsite Cuisine").unwrap())]),
    )?;
    driver.cast("Hero")?;
    settle(&mut driver)?;
    assert_eq!(count_named(&driver, "Food"), foods);
    Ok(())
}

#[rstest]
fn phantom_steed_attacks_with_an_illusion_copy_of_the_exiled_card_until_end_of_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let island = || ready(mtg_testkit::cards::island());
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [card("Phantom Steed").unwrap()])
            .battlefield(
                0,
                [
                    island(),
                    island(),
                    island(),
                    island(),
                    ready(free("Bear", CardType::Creature, false, &[])),
                ],
            ),
    )?;
    driver.cast_tapping("Phantom Steed", &["Island", "Island", "Island", "Island"])?;
    settle(&mut driver)?;
    assert_eq!(count_named(&driver, "Bear"), 0, "the bear is exiled");
    let turn = 2 + u32::try_from(seats).unwrap();
    driver.idle_until(turn, Step::DeclareAttackers)?;
    driver.attack(&[("Phantom Steed", 1)])?;
    settle(&mut driver)?;
    let state = driver.state();
    let copy = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == "Bear"))
        .expect("the copy attacks");
    let values = state.object(copy).unwrap().card();
    assert!(
        values
            .subtypes
            .iter()
            .any(|subtype| subtype.0 == "Illusion"),
        "the copy is an Illusion"
    );
    driver.idle_until(turn, Step::End)?;
    assert_eq!(count_named(&driver, "Bear"), 0, "the copy is sacrificed");
    Ok(())
}

#[rstest]
fn mirror_style_master_copies_each_attacking_modified_creature_until_end_of_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let creature = |name: &str| ready(free(name, CardType::Creature, false, &[]));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(card("Mirror-Style Master").unwrap()),
                    with_counters(creature("Bear"), CounterKind::PlusOnePlusOne, 1),
                    creature("Wolf"),
                ],
            ),
    )?;
    driver.attack(&[("Mirror-Style Master", 1), ("Bear", 1), ("Wolf", 1)])?;
    settle(&mut driver)?;
    assert_eq!(
        (count_named(&driver, "Bear"), count_named(&driver, "Wolf")),
        (2, 1)
    );
    driver.idle_until(2, Step::End)?;
    assert_eq!(count_named(&driver, "Bear"), 1, "the copy is exiled");
    Ok(())
}

#[rstest]
#[case::in_its_band(true, true)]
#[case::attacking_apart(false, false)]
fn icatian_skirmishers_give_first_strike_to_the_creatures_banded_with_it(
    #[case] banded: bool,
    #[case] strikes_first: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(card("Icatian Skirmishers").unwrap()),
                    ready(free("Bear", CardType::Creature, false, &[])),
                    ready(free("Wolf", CardType::Creature, false, &[])),
                ],
            ),
    )?;
    let attacks = [("Icatian Skirmishers", 1), ("Bear", 1), ("Wolf", 1)];
    if banded {
        driver.attack_in_bands(&attacks, &[&["Icatian Skirmishers", "Bear"]])?;
    } else {
        driver.attack(&attacks)?;
    }
    settle(&mut driver)?;
    let first_strike = |name: &str| {
        let id = driver.id_of(name).unwrap();
        has_keyword(
            driver.state(),
            id,
            &mtg_cards::keyword::Keyword::FirstStrike,
        )
    };
    assert_eq!(first_strike("Bear"), strikes_first);
    assert!(!first_strike("Wolf"), "a creature outside the band");
    Ok(())
}

#[rstest]
#[case::with_another_wall("Stone Wall", true)]
#[case::with_a_bear("Bear", false)]
fn wall_of_caltrops_gains_banding_when_only_walls_block_with_it(
    #[case] partner: &str,
    #[case] bands: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let wall = Arc::new(
        CardDefinition::new("Stone Wall")
            .with_type(CardType::Creature)
            .with_subtype("Wall")
            .with_power_toughness(0, 8),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 1, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(card("Wall of Caltrops").unwrap()),
                    ready(wall),
                    ready(free("Bear", CardType::Creature, false, &[])),
                ],
            )
            .battlefield(1, [ready(free("Raider", CardType::Creature, false, &[]))]),
    )?;
    driver.attack(&[("Raider", 0)])?;
    for _ in 0..20 {
        if matches!(
            driver.decision()?.kind,
            mtg_engine::decision::DecisionKind::DeclareBlockers(_)
        ) {
            break;
        }
        driver.pass()?;
    }
    driver.block(&[("Wall of Caltrops", "Raider"), (partner, "Raider")])?;
    settle(&mut driver)?;
    let id = driver.id_of("Wall of Caltrops")?;
    assert_eq!(
        has_keyword(driver.state(), id, &mtg_cards::keyword::Keyword::Banding),
        bands
    );
    Ok(())
}
