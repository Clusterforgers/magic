//! Goal: the cost parts Damage to you, Exile top, Unattach, Collect evidence, Blight, Behold, To library, Forage and an exile from any graveyard are offered exactly when the payer can pay them, ask the payer for the objects only when there is a real choice, refuse an answer that does not pay, and pay with the events of the rules (damage, exile, counters, a reveal, a library order), so a game can never stick on a cost it offered.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::{creature, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use common::{name_of, names, pid};

/// An artifact with the ability "<cost>: You gain 1 life."
fn gadget(cost: &str) -> Arc<CardDefinition> {
    let ability =
        format!(r#"{{"Activated":{{"cost":[{cost}],"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#);
    Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&ability).unwrap()),
    )
}

/// A creature card with this mana value.
fn valued(name: &str, value: u32) -> Arc<CardDefinition> {
    creature(name, value, &[], 1, 1)
}

/// A Food artifact.
fn food(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Food"),
    )
}

/// A Goat creature card.
fn goat(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Goat")
            .with_power_toughness(1, 1),
    )
}

fn scenario(cost: &str) -> Scenario {
    Scenario::new(2).battlefield(0, [ready(gadget(cost))])
}

fn object(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|found| found.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap()
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn zone(driver: &Driver, zone: Zone) -> Vec<String> {
    names(driver.state(), zone)
}

/// The names the pending question offers, sorted, and the fewest and most it takes.
fn asked(driver: &Driver) -> (Vec<String>, u32, u32) {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a choice of cards was expected");
    };
    let mut found: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    found.sort();
    (found, options.min, options.max)
}

fn pick(driver: &mut Driver, chosen: &[&str]) -> Result<Vec<Event>, ScriptError> {
    let ids: Vec<ObjectId> = chosen
        .iter()
        .map(|name| object(driver.state(), name))
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(ids)))
}

fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut log = Vec::new();
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(log);
        }
        log.extend(driver.answer_passively()?);
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Activates the Gadget, answering each question about cards with the next of `picks`; every event of it.
fn activate(driver: &mut Driver, picks: &[&[&str]]) -> Result<Vec<Event>, ScriptError> {
    let mut log = driver.begin_activation("Gadget")?;
    let mut picks = picks.iter();
    while matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        let chosen = picks.next().expect("a question that was not expected");
        log.extend(pick(driver, chosen)?);
    }
    assert!(picks.next().is_none(), "a question that was not asked");
    log.extend(settle(driver)?);
    Ok(log)
}

fn offered(driver: &Driver) -> bool {
    !driver.offered_abilities("Gadget").unwrap().is_empty()
}

// Damage to you

#[rstest]
#[case::one(1)]
#[case::three(3)]
#[case::down_to_one_life(19)]
fn damage_to_you_is_dealt_by_the_source(#[case] amount: i32) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(&format!(r#"{{"DamageToYou":{amount}}}"#)))?;
    assert!(offered(&driver));
    let log = activate(&mut driver, &[])?;
    assert!(log.iter().any(|event| matches!(
        event,
        Event::DamageDealt { source, amount: dealt, .. }
            if *source == object(driver.state(), "Gadget") && i32::try_from(*dealt) == Ok(amount)
    )));
    assert_eq!(life(&driver), 20 - amount + 1);
    Ok(())
}

// Exile top

#[rstest]
#[case::enough(3, 2, true)]
#[case::exactly(2, 2, true)]
#[case::one_short(1, 2, false)]
#[case::empty(0, 1, false)]
fn exile_top_needs_the_cards_in_the_library(
    #[case] library: usize,
    #[case] amount: u32,
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let cards: Vec<_> = (0..library)
        .map(|at| free_creature(&format!("Card {at}"), 1, 1))
        .collect();
    let mut driver =
        Driver::from_scenario(scenario(&format!(r#"{{"ExileTop":{amount}}}"#)).library(0, cards))?;
    assert_eq!(offered(&driver), pays);
    if pays {
        activate(&mut driver, &[])?;
        let wanted: Vec<String> = (0..amount).map(|at| format!("Card {at}")).collect();
        assert_eq!(zone(&driver, Zone::Exile), wanted, "the top cards, face up");
        assert_eq!(
            zone(&driver, Zone::Library(pid(0))).len(),
            library - wanted.len()
        );
    }
    Ok(())
}

// Unattach

#[rstest]
#[case::attached(true)]
#[case::loose(false)]
fn unattach_is_offered_only_while_the_source_is_attached(
    #[case] attached: bool,
) -> Result<(), ScriptError> {
    let equipment = ready(Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Activated":{"cost":["Unattach"],"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    ));
    let equipment = if attached {
        attached_to(equipment, 0, 1)
    } else {
        equipment
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [equipment, ready(free_creature("Bear", 2, 2))]),
    )?;
    assert_eq!(offered(&driver), attached);
    if attached {
        let log = activate(&mut driver, &[])?;
        assert!(
            log.iter()
                .any(|event| matches!(event, Event::Unattached { .. }))
        );
        let id = object(driver.state(), "Gadget");
        let found = driver.state().object(id).unwrap().permanent().unwrap();
        assert!(found.attached_to().is_none());
        assert!(!offered(&driver), "nothing left to unattach");
    }
    Ok(())
}

// Collect evidence

#[rstest]
#[case::exactly_enough(4, &[4], true)]
#[case::two_cards_together(4, &[2, 2], true)]
#[case::more_than_needed(3, &[5, 1], true)]
#[case::one_short(5, &[2, 2], false)]
#[case::empty_graveyard(1, &[], false)]
#[case::lands_count_for_nothing(1, &[0, 0], false)]
fn collect_evidence_is_offered_while_the_graveyard_has_the_value(
    #[case] needed: u32,
    #[case] values: &[u32],
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let cards: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(at, value)| valued(&format!("Dead {at}"), *value))
        .collect();
    let mut driver = Driver::from_scenario(
        scenario(&format!(r#"{{"CollectEvidence":{needed}}}"#)).graveyard(0, cards),
    )?;
    assert_eq!(offered(&driver), pays);
    if !pays {
        return Ok(());
    }
    driver.begin_activation("Gadget")?;
    let everything: u32 = values.iter().sum();
    if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        // A choice exists only when some card can be left out, and an answer short of the value is refused.
        let before = driver.state().state_hash();
        let (offers, min, max) = asked(&driver);
        assert_eq!((offers.len(), min), (values.len(), 1));
        assert_eq!(usize::try_from(max), Ok(values.len()));
        assert!(everything > needed, "{values:?} leave no choice");
        let smallest = values
            .iter()
            .enumerate()
            .min_by_key(|(_, value)| **value)
            .map(|(at, _)| format!("Dead {at}"))
            .unwrap();
        if values.iter().min().copied().unwrap() < needed {
            let error = pick(&mut driver, &[&smallest]).unwrap_err();
            assert!(
                matches!(
                    error,
                    ScriptError::Engine(EngineError::Illegal(IllegalAnswer::TooLittleValue { .. }))
                ),
                "{error:?}"
            );
            assert_eq!(driver.state().state_hash(), before);
        }
        let all: Vec<String> = (0..values.len()).map(|at| format!("Dead {at}")).collect();
        let all: Vec<&str> = all.iter().map(String::as_str).collect();
        pick(&mut driver, &all)?;
    }
    settle(&mut driver)?;
    assert_eq!(zone(&driver, Zone::Exile).len(), values.len());
    assert_eq!(zone(&driver, Zone::Graveyard(pid(0))).len(), 0);
    assert_eq!(life(&driver), 21);
    Ok(())
}

#[test]
fn collect_evidence_exiles_only_the_cards_chosen() -> Result<(), ScriptError> {
    let cards = vec![valued("Big", 5), valued("Small", 1), valued("Middle", 3)];
    let mut driver =
        Driver::from_scenario(scenario(r#"{"CollectEvidence":4}"#).graveyard(0, cards))?;
    activate(&mut driver, &[&["Big"]])?;
    assert_eq!(zone(&driver, Zone::Exile), vec!["Big".to_owned()]);
    assert_eq!(zone(&driver, Zone::Graveyard(pid(0))).len(), 2);
    Ok(())
}

// Blight

#[rstest]
#[case::no_creature(&[], false)]
#[case::one_creature(&["Bear"], true)]
#[case::two_creatures(&["Bear", "Wolf"], true)]
fn blight_puts_counters_on_a_creature_the_payer_picks(
    #[case] creatures: &[&str],
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let mut permanents = vec![ready(gadget(r#"{"Blight":2}"#))];
    permanents.extend(
        creatures
            .iter()
            .map(|name| ready(free_creature(name, 3, 3))),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, permanents)
            .battlefield(1, [ready(free_creature("Ogre", 4, 4))]),
    )?;
    assert_eq!(offered(&driver), pays);
    if !pays {
        return Ok(());
    }
    let chosen = *creatures.last().unwrap();
    let only = [chosen];
    let picks: Vec<&[&str]> = if creatures.len() > 1 {
        vec![&only[..]]
    } else {
        Vec::new()
    };
    activate(&mut driver, &picks)?;
    let counters = |name: &str| {
        let id = object(driver.state(), name);
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters()
            .map(|(_, count)| count)
            .sum::<u32>()
    };
    assert_eq!(counters(chosen), 2);
    for other in &creatures[..creatures.len() - 1] {
        assert_eq!(counters(other), 0);
    }
    Ok(())
}

// Behold

#[rstest]
#[case::nothing(false, false, false)]
#[case::in_hand(false, true, true)]
#[case::on_the_battlefield(true, false, true)]
#[case::both(true, true, true)]
fn behold_takes_a_permanent_or_a_card_of_the_hand(
    #[case] on_the_battlefield: bool,
    #[case] in_hand: bool,
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let mut permanents = vec![ready(gadget(
        r#"{"Behold":{"filter":[{"Subtype":"Goat"}]}}"#,
    ))];
    if on_the_battlefield {
        permanents.push(ready(goat("Field Goat")));
    }
    permanents.push(ready(free_creature("Bear", 2, 2)));
    let mut hand = vec![free_creature("Held Bear", 2, 2)];
    if in_hand {
        hand.push(goat("Held Goat"));
    }
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, permanents).hand(0, hand))?;
    assert_eq!(offered(&driver), pays);
    if !pays {
        return Ok(());
    }
    let both = on_the_battlefield && in_hand;
    let choice = if on_the_battlefield {
        "Field Goat"
    } else {
        "Held Goat"
    };
    let log = if both {
        driver.begin_activation("Gadget")?;
        assert_eq!(
            asked(&driver).0,
            ["Field Goat", "Held Goat"],
            "a real choice"
        );
        let mut log = pick(&mut driver, &[choice])?;
        log.extend(settle(&mut driver)?);
        log
    } else {
        activate(&mut driver, &[])?
    };
    let revealed = log.iter().any(|event| matches!(event, Event::Revealed(_)));
    assert_eq!(
        revealed, !on_the_battlefield,
        "only a card of the hand is shown"
    );
    assert_eq!(
        zone(&driver, Zone::Hand(pid(0))).len(),
        1 + usize::from(in_hand)
    );
    assert!(!zone(&driver, Zone::Graveyard(pid(0))).contains(&choice.to_owned()));
    Ok(())
}

// To library

#[rstest]
#[case::hand_top("Hand", false)]
#[case::hand_bottom("Hand", true)]
#[case::graveyard_top("Graveyard", false)]
#[case::graveyard_bottom("Graveyard", true)]
#[case::battlefield_top("Battlefield", false)]
#[case::battlefield_bottom("Battlefield", true)]
fn to_library_puts_the_picked_card_on_top_or_at_the_bottom(
    #[case] from: &str,
    #[case] bottom: bool,
) -> Result<(), ScriptError> {
    let cost = format!(
        r#"{{"ToLibrary":{{"from":"{from}","filter":[{{"Type":"Creature"}}],"bottom":{bottom}}}}}"#
    );
    let named = |prefix: &str| {
        [
            free_creature(&format!("{prefix} A"), 1, 1),
            free_creature(&format!("{prefix} B"), 1, 1),
        ]
    };
    let mut battlefield = vec![ready(gadget(&cost))];
    battlefield.extend(named("Bear").map(ready));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, battlefield)
            .hand(0, named("Held"))
            .graveyard(0, named("Dead"))
            .library(
                0,
                [
                    free_creature("Old Top", 1, 1),
                    free_creature("Old Bottom", 1, 1),
                ],
            ),
    )?;
    assert!(offered(&driver));
    let picked = match from {
        "Hand" => "Held A",
        "Graveyard" => "Dead A",
        _ => "Bear A",
    };
    activate(&mut driver, &[&[picked]])?;
    let library = zone(&driver, Zone::Library(pid(0)));
    // A library lists its bottom card first and its top card last.
    let place = if bottom {
        library.first()
    } else {
        library.last()
    };
    assert_eq!(place.map(String::as_str), Some(picked));
    assert_eq!(library.len(), 3);
    Ok(())
}

#[rstest]
#[case::no_card_in_the_hand("Hand", 0, false)]
#[case::a_card_in_the_hand("Hand", 1, true)]
#[case::no_card_in_the_graveyard("Graveyard", 0, false)]
#[case::a_card_in_the_graveyard("Graveyard", 1, true)]
fn to_library_is_offered_only_with_a_card_to_put(
    #[case] from: &str,
    #[case] cards: usize,
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let cost = format!(r#"{{"ToLibrary":{{"from":"{from}","filter":[]}}}}"#);
    let held: Vec<_> = (0..cards)
        .map(|at| free_creature(&format!("Card {at}"), 1, 1))
        .collect();
    let scenario = scenario(&cost);
    let scenario = if from == "Hand" {
        scenario.hand(0, held)
    } else {
        scenario.graveyard(0, held)
    };
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(offered(&driver), pays);
    Ok(())
}

// Forage

#[rstest]
#[case::nothing(0, 0, false)]
#[case::two_cards_are_too_few(0, 2, false)]
#[case::three_cards(0, 3, true)]
#[case::a_food(1, 0, true)]
#[case::a_food_and_two_cards(1, 2, true)]
#[case::a_food_and_cards(1, 4, true)]
#[case::two_foods(2, 0, true)]
fn forage_is_offered_with_a_food_or_three_cards(
    #[case] foods: usize,
    #[case] cards: usize,
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let mut permanents = vec![ready(gadget(r#""Forage""#))];
    permanents.extend((0..foods).map(|at| ready(food(&format!("Food {at}")))));
    let graveyard: Vec<_> = (0..cards)
        .map(|at| valued(&format!("Dead {at}"), 1))
        .collect();
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, permanents)
            .graveyard(0, graveyard),
    )?;
    assert_eq!(offered(&driver), pays);
    Ok(())
}

#[test]
fn forage_with_one_food_and_no_other_way_sacrifices_it_unasked() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(gadget(r#""Forage""#)), ready(food("Snack"))]),
    )?;
    activate(&mut driver, &[])?;
    assert_eq!(zone(&driver, Zone::Graveyard(pid(0))), ["Snack"]);
    Ok(())
}

#[test]
fn forage_with_exactly_three_cards_exiles_them_unasked() -> Result<(), ScriptError> {
    let graveyard: Vec<_> = (0..3).map(|at| valued(&format!("Dead {at}"), 1)).collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(gadget(r#""Forage""#))])
            .graveyard(0, graveyard),
    )?;
    activate(&mut driver, &[])?;
    assert_eq!(zone(&driver, Zone::Exile).len(), 3);
    Ok(())
}

#[rstest]
#[case::the_food(&["Snack"], true)]
#[case::three_cards(&["Dead 0", "Dead 1", "Dead 2"], true)]
#[case::two_cards(&["Dead 0", "Dead 1"], false)]
#[case::a_card_alone(&["Dead 0"], false)]
#[case::the_food_and_a_card(&["Snack", "Dead 0"], false)]
#[case::the_food_and_three_cards(&["Snack", "Dead 0", "Dead 1", "Dead 2"], false)]
fn forage_asks_for_a_food_or_three_cards_and_refuses_anything_else(
    #[case] chosen: &[&str],
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let graveyard: Vec<_> = (0..4).map(|at| valued(&format!("Dead {at}"), 1)).collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(gadget(r#""Forage""#)), ready(food("Snack"))])
            .graveyard(0, graveyard),
    )?;
    driver.begin_activation("Gadget")?;
    let (offers, min, max) = asked(&driver);
    assert_eq!(offers.len(), 5, "the Food and every card of the graveyard");
    assert_eq!((min, max), (1, 3));
    let before = driver.state().state_hash();
    let result = pick(&mut driver, chosen);
    if pays {
        result?;
        settle(&mut driver)?;
        let gone = if chosen == ["Snack"] {
            zone(&driver, Zone::Graveyard(pid(0))).contains(&"Snack".to_owned())
        } else {
            zone(&driver, Zone::Exile).len() == 3
        };
        assert!(gone);
    } else {
        // Too many or too few is refused by the question's bounds, any other mix by the cost; either way nothing happened.
        assert!(result.is_err());
        assert_eq!(driver.state().state_hash(), before);
        assert_eq!(asked(&driver).1, 1, "the same question is still pending");
    }
    Ok(())
}

// Exile from any graveyard

#[rstest]
#[case::own(1, 0, true, &["Mine"])]
#[case::theirs(0, 1, true, &["Theirs"])]
#[case::both_is_a_choice(1, 1, true, &["Theirs"])]
#[case::neither(0, 0, false, &[])]
fn exile_from_any_graveyard_may_take_an_opponents_card(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] pays: bool,
    #[case] chosen: &[&str],
) -> Result<(), ScriptError> {
    let own = (0..mine).map(|_| valued("Mine", 1)).collect::<Vec<_>>();
    let other = (0..theirs).map(|_| valued("Theirs", 1)).collect::<Vec<_>>();
    let mut driver = Driver::from_scenario(
        scenario(r#"{"ExileFromGraveyard":{"filter":[],"any_player":true}}"#)
            .graveyard(0, own)
            .graveyard(1, other),
    )?;
    assert_eq!(offered(&driver), pays);
    if pays {
        let picks: Vec<&[&str]> = if mine + theirs > 1 {
            vec![chosen]
        } else {
            Vec::new()
        };
        activate(&mut driver, &picks)?;
        assert_eq!(zone(&driver, Zone::Exile), chosen);
    }
    Ok(())
}

#[test]
fn exile_from_a_graveyard_stays_the_payers_own_without_the_flag() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        scenario(r#"{"ExileFromGraveyard":{"filter":[]}}"#).graveyard(1, [valued("Theirs", 1)]),
    )?;
    assert!(!offered(&driver));
    Ok(())
}
