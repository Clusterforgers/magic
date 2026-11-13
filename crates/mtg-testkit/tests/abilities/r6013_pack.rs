//! Goal: the r60-13 pack's abilities play as printed at two to four seats: Argentum Masticore's upkeep trigger
//! destroys a permanent only up to the discarded card's mana value and is sacrificed when nothing is discarded; Grist's
//! +1 repeats while it mills Insect cards, gaining loyalty each time; Primeval Spawn is exiled when it would enter
//! without being cast and enters when cast with mana.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

/// An artifact of mana value `value`.
fn r6013_relic(name: &str, value: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(value, &[])),
    )
}

/// A creature card with the subtype.
fn r6013_kin(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(1, 1),
    )
}

/// Answers passively until the stack is empty and nothing resolves, choosing `discard` when a card may be discarded
/// and `target` when a target is asked.
fn r6013_settle(
    driver: &mut Driver,
    discard: Option<&str>,
    target: Option<&str>,
) -> Result<(), ScriptError> {
    for _ in 0..300 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            if let Ok(decision) = driver.decision()
                && matches!(decision.kind, DecisionKind::Priority(_))
            {
                return Ok(());
            }
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) if discard.is_some() => {
                let id = r6013_find(driver, discard.unwrap());
                let picked = if options.from.contains(&id) {
                    vec![id]
                } else {
                    Vec::new()
                };
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            DecisionKind::Discard(_) if discard.is_some() => {
                let id = r6013_find(driver, discard.unwrap());
                driver.answer(Answer::Discard(vec![id]))?;
            }
            DecisionKind::ChooseTargets(_) if target.is_some() => {
                let chosen: Target = driver.target_named(target.unwrap())?;
                driver.choose_targets(&[chosen])?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The object named `name` in any zone.
fn r6013_find(driver: &Driver, name: &str) -> ObjectId {
    let state = driver.state();
    let mut zones = vec![Zone::Battlefield, Zone::Exile, Zone::Stack];
    for seat in 0..state.players().len() {
        zones.extend([Zone::Hand(pid(seat)), Zone::Graveyard(pid(seat)), Zone::Library(pid(seat))]);
    }
    zones
        .into_iter()
        .flat_map(|zone| state.zone(zone).iter().copied())
        .find(|id| state.object(*id).is_some_and(|object| object.card().name == name))
        .unwrap_or_else(|| panic!("{name} is somewhere"))
}

fn r6013_zone_of(driver: &Driver, name: &str) -> Zone {
    let id = r6013_find(driver, name);
    driver.state().object(id).unwrap().zone()
}

/// Casts `name`, choosing `target` when a target is asked, and pays automatically.
fn r6013_cast(driver: &mut Driver, name: &str, target: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    for _ in 0..10 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[target.expect("a target is asked")])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                return Ok(());
            }
            _ => return Ok(()),
        }
    }
    Ok(())
}

#[rstest]
#[case::cheaper_is_destroyed(3, "Cheap Relic", true)]
#[case::equal_is_destroyed(4, "Dear Relic", true)]
#[case::dearer_is_kept(3, "Dear Relic", false)]
fn r6013_masticore_destroys_up_to_the_discarded_value(
    #[values(2, 3, 4)] seats: usize,
    #[case] discarded_value: u32,
    #[case] target: &str,
    #[case] destroyed: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Argentum Masticore").unwrap())])
        .hand(0, [r6013_relic("Fodder", discarded_value)])
        .battlefield(
            1,
            [
                ready(r6013_relic("Cheap Relic", 2)),
                ready(r6013_relic("Dear Relic", 4)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    let attempt = r6013_settle(&mut driver, Some("Fodder"), Some(target));
    if destroyed {
        attempt?;
        assert!(matches!(r6013_zone_of(&driver, target), Zone::Graveyard(_)));
    } else {
        // The dearer relic is no legal target: it stays whatever was tried.
        let _ = attempt;
        assert_eq!(r6013_zone_of(&driver, target), Zone::Battlefield);
    }
    assert_eq!(r6013_zone_of(&driver, "Fodder"), Zone::Graveyard(pid(0)));
    assert_eq!(
        r6013_zone_of(&driver, "Argentum Masticore"),
        Zone::Battlefield
    );
    Ok(())
}

#[rstest]
fn r6013_masticore_is_sacrificed_without_a_discard(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Argentum Masticore").unwrap())])
        .battlefield(1, [ready(r6013_relic("Cheap Relic", 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    r6013_settle(&mut driver, None, None)?;
    assert_eq!(
        r6013_zone_of(&driver, "Argentum Masticore"),
        Zone::Graveyard(pid(0))
    );
    assert_eq!(r6013_zone_of(&driver, "Cheap Relic"), Zone::Battlefield);
    Ok(())
}

#[rstest]
#[case::no_insect(&["Rock", "Stone"], 1, 4)]
#[case::one_insect(&["Beetle", "Rock", "Stone"], 2, 5)]
#[case::two_insects(&["Beetle", "Mantis", "Rock", "Stone"], 3, 6)]
fn r6013_grist_repeats_while_insects_are_milled(
    #[values(2, 3, 4)] seats: usize,
    #[case] library: &[&str],
    #[case] tokens: usize,
    #[case] loyalty: u32,
) -> Result<(), ScriptError> {
    let cards: Vec<Arc<CardDefinition>> = library
        .iter()
        .map(|name| match *name {
            "Beetle" | "Mantis" => r6013_kin(name, "Insect"),
            _ => r6013_kin(name, "Elemental"),
        })
        .collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Grist, the Hunger Tide").unwrap())])
        .library(0, cards);
    let mut driver = Driver::from_scenario(scenario)?;
    let grist = driver.id_of("Grist, the Hunger Tide")?;
    let plus = driver
        .offered_abilities("Grist, the Hunger Tide")?
        .into_iter()
        .min()
        .unwrap();
    driver.begin_ability("Grist, the Hunger Tide", plus)?;
    r6013_settle(&mut driver, None, None)?;
    let insects = common::names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.as_str() == "Insect")
        .count();
    assert_eq!(insects, tokens);
    let counters = driver
        .state()
        .object(grist)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::Loyalty));
    assert_eq!(counters, Some(loyalty));
    Ok(())
}

/// An instant that returns every creature card from its controller's graveyard to the battlefield.
fn r6013_return_all() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"to":{"Battlefield":{}}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Return All")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

#[rstest]
fn r6013_spawn_put_onto_the_battlefield_is_exiled(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .graveyard(0, [card("Primeval Spawn").unwrap()])
        .hand(0, [r6013_return_all()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Return All")?;
    r6013_settle(&mut driver, None, None)?;
    assert_eq!(r6013_zone_of(&driver, "Primeval Spawn"), Zone::Exile);
    Ok(())
}

#[rstest]
fn r6013_spawn_cast_with_mana_enters(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let lands: Vec<_> = [plains(), island(), swamp(), mountain(), forest()]
        .into_iter()
        .cycle()
        .take(10)
        .map(ready)
        .collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, lands)
        .hand(0, [card("Primeval Spawn").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Primeval Spawn")?;
    r6013_settle(&mut driver, None, None)?;
    assert_eq!(r6013_zone_of(&driver, "Primeval Spawn"), Zone::Battlefield);
    Ok(())
}

/// A free instant with this spell ability.
fn r6013_instant(name: &str, spell: &str) -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(spell).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

#[rstest]
#[case::an_opponent_searches_the_top_four(1, 4)]
#[case::its_controller_searches_everything(0, 6)]
fn r6013_mindcensor_limits_an_opponents_search(
    #[values(2, 3, 4)] seats: usize,
    #[case] searcher: usize,
    #[case] offered: usize,
) -> Result<(), ScriptError> {
    let library: Vec<_> = (1..=6)
        .map(|at| r6013_kin(&format!("Deep {at}"), "Elf"))
        .collect();
    let tutor = r6013_instant(
        "Tutor",
        r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Creature"}],"as":"found"}}]}}"#,
    );
    let scenario = Scenario::new(seats)
        .at(1, searcher, Step::Main1)
        .battlefield(0, [ready(card("Aven Mindcensor").unwrap())])
        .library(searcher, library)
        .hand(searcher, [tutor]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Tutor")?;
    let mut seen = None;
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                seen = Some(
                    options
                        .from
                        .iter()
                        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
                        .collect::<Vec<_>>(),
                );
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let seen = seen.expect("the search asks for a card");
    assert_eq!(seen.len(), offered, "{seen:?}");
    // The top of the library is the first card given.
    assert!(seen.iter().all(|name| {
        let at: usize = name.trim_start_matches("Deep ").parse().unwrap();
        at <= offered
    }));
    Ok(())
}

#[rstest]
#[case::one_hit(1, 1, false)]
#[case::many_hits(4, 1, false)]
#[case::one_big_hit_is_lethal(1, 5, true)]
fn r6013_wolverine_heals_before_each_damage(
    #[values(2, 3, 4)] seats: usize,
    #[case] hits: usize,
    #[case] each: i32,
    #[case] dies: bool,
) -> Result<(), ScriptError> {
    let zap = format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{each}}}}}]}}}}"#
    );
    let zaps: Vec<_> = (0..hits)
        .map(|at| r6013_instant(&format!("Zap {at}"), &zap))
        .collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Wolverine, Fierce Fighter").unwrap())])
        .hand(0, zaps);
    let mut driver = Driver::from_scenario(scenario)?;
    let wolverine = driver.id_of("Wolverine, Fierce Fighter")?;
    for at in 0..hits {
        let target = driver.target_named("Wolverine, Fierce Fighter")?;
        r6013_cast(&mut driver, &format!("Zap {at}"), Some(target))?;
        r6013_settle(&mut driver, None, None)?;
    }
    let object = driver.state().object(wolverine).unwrap();
    if dies {
        assert!(matches!(object.zone(), Zone::Graveyard(_)));
    } else {
        assert_eq!(object.zone(), Zone::Battlefield);
        assert_eq!(
            object.permanent().unwrap().damage(),
            u32::try_from(each).unwrap()
        );
    }
    Ok(())
}

#[rstest]
fn r6013_winds_hold_a_permanent_with_wind_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let wind = CounterKind::Other("wind".to_owned());
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Freyalise's Winds").unwrap())])
        .battlefield(
            1,
            [
                with_counters(tapped(r6013_kin("Windswept", "Elf")), wind.clone(), 2),
                tapped(r6013_kin("Calm", "Elf")),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Upkeep)?;
    let held = driver.state().object(driver.id_of("Windswept")?).unwrap();
    let held = held.permanent().unwrap();
    assert!(held.tapped());
    assert_eq!(held.counters_of(&wind), 0);
    let calm = driver.state().object(driver.id_of("Calm")?).unwrap();
    assert!(!calm.permanent().unwrap().tapped());
    Ok(())
}

/// An instant that does what the ability at `path` (face, then index) of the card file does.
fn r6013_effects_of(slug: &str, face: usize, index: usize, kind: &str) -> Arc<CardDefinition> {
    let path = format!(
        "{}/../mtg-cards/cards/{slug}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let effects = file["faces"][face]["abilities"][index][kind]["effects"].clone();
    assert!(effects.is_array(), "{slug} has {kind} effects");
    r6013_instant(
        "Borrowed",
        &serde_json::json!({ "Spell": { "effects": effects } }).to_string(),
    )
}

#[rstest]
fn r6013_ajani_minus_four_keeps_one_of_each_type(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let borrowed = r6013_effects_of("ajani_nacatl_pariah_ajani_nacatl_avenger", 1, 2, "Loyalty");
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [borrowed])
        .battlefield(0, [ready(r6013_relic("Mine", 1))]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(r6013_relic(&format!("Relic {seat}a"), 1)),
                ready(r6013_relic(&format!("Relic {seat}b"), 2)),
                ready(r6013_kin(&format!("Bear {seat}"), "Bear")),
                ready(forest()),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Borrowed")?;
    r6013_settle(&mut driver, None, None)?;
    assert_eq!(r6013_zone_of(&driver, "Mine"), Zone::Battlefield);
    for seat in 1..seats {
        let kept = common::names(driver.state(), Zone::Battlefield)
            .into_iter()
            .filter(|name| name.starts_with(&format!("Relic {seat}")))
            .count();
        assert_eq!(kept, 1, "seat {seat} keeps one artifact");
        assert_eq!(
            r6013_zone_of(&driver, &format!("Bear {seat}")),
            Zone::Battlefield
        );
    }
    assert_eq!(
        common::names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| name.as_str() == "Forest")
            .count(),
        seats - 1,
        "lands are never sacrificed"
    );
    Ok(())
}

#[rstest]
#[case(7, 14)]
#[case(20, 40)]
fn r6013_enforcer_doubles_its_controllers_life(
    #[values(2, 3, 4)] seats: usize,
    #[case] before: i32,
    #[case] after: i32,
) -> Result<(), ScriptError> {
    let borrowed = r6013_effects_of("enduring_angel_angelic_enforcer", 1, 3, "Triggered");
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .life(0, before)
        .hand(0, [borrowed]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Borrowed")?;
    r6013_settle(&mut driver, None, None)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), after);
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20);
    Ok(())
}

#[rstest]
fn r6013_mana_clash_repeats_until_both_coins_come_up_heads(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4, 5)] seed: u64,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(mountain())])
        .hand(0, [card("Mana Clash").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let begun = driver.engine().events().len();
    let foe = driver.target_player(1);
    r6013_cast(&mut driver, "Mana Clash", Some(foe))?;
    r6013_settle(&mut driver, None, None)?;
    let flips: Vec<(usize, bool)> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            mtg_engine::Event::CoinFlipped { player, side, .. } => {
                Some((player.index(), *side == mtg_engine::choose::CoinSide::Heads))
            }
            _ => None,
        })
        .collect();
    let mine: Vec<bool> = flips
        .iter()
        .filter(|(who, _)| *who == 0)
        .map(|(_, heads)| *heads)
        .collect();
    let theirs: Vec<bool> = flips
        .iter()
        .filter(|(who, _)| *who == 1)
        .map(|(_, heads)| *heads)
        .collect();
    assert_eq!(mine.len(), theirs.len());
    let pairs: Vec<(bool, bool)> = mine.iter().copied().zip(theirs.iter().copied()).collect();
    let (last, before) = pairs.split_last().unwrap();
    assert_eq!(*last, (true, true));
    assert!(before.iter().all(|pair| *pair != (true, true)));
    let tails =
        |side: &[bool]| i32::try_from(side.iter().filter(|heads| !**heads).count()).unwrap();
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        20 - tails(&mine)
    );
    assert_eq!(
        driver.state().player(pid(1)).unwrap().life(),
        20 - tails(&theirs)
    );
    Ok(())
}

#[rstest]
#[case::paid(true)]
#[case::declined(false)]
fn r6013_valentin_exiles_an_opponents_dying_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] pay: bool,
) -> Result<(), ScriptError> {
    let zap = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":5}}]}}"#;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Valentin, Dean of the Vein // Lisette, Dean of the Root").unwrap()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .battlefield(1, [ready(r6013_kin("Victim", "Bear"))])
        .hand(0, [r6013_instant("Zap", zap)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let victim = driver.target_named("Victim")?;
    r6013_cast(&mut driver, "Zap", Some(victim))?;
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(pay)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.pay_tapping(&["Forest", "Forest"], 0)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(r6013_zone_of(&driver, "Victim"), Zone::Exile);
    let pests = common::names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.as_str() == "Pest")
        .count();
    assert_eq!(pests, usize::from(pay));
    Ok(())
}

