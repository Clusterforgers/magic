//! Goal: cascade plays whole through Bloodbraid Elf and a cascade creature of the tests: the trigger exiles from the top until a nonland card of lesser mana value (each exiled card is open to every seat), the controller may cast it without paying its mana cost with every question of the cast asked inside the resolution, the spell goes on the stack above the trigger and resolves first, and every exiled card not cast goes to the bottom in a random order that the seed fixes; a library without a card of lesser value, a card that cannot be cast (no legal target) and a declined cast leave all cards at the bottom, an X spell is cast with X 0, a modal spell asks its modes, a cost increase is still paid, and a cascade card found by a cascade cascades in its turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::discover;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, forest, keyworded_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::resolve;
use crate::common::{names, pid};

/// Seat 0 holds Bloodbraid Elf over two Mountains and two Forests; its library is `library`, top first.
fn elf_game(library: Vec<Arc<CardDefinition>>) -> Scenario {
    let lands = [mountain(), mountain(), forest(), forest()].map(ready);
    Scenario::new(2)
        .hand_named(0, ["Bloodbraid Elf"])
        .battlefield(0, lands)
        .library(0, library)
}

fn bear(name: &str, generic: u32) -> Arc<CardDefinition> {
    creature(name, generic, &[], 2, 2)
}

/// Casts the Elf and lets the cascade trigger resolve as far as its first question.
fn cascade(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.cast("Bloodbraid Elf")?;
    resolve(driver)
}

fn say(driver: &mut Driver, yes: bool) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::YesNo(YesNoChoice(yes)))
}

/// The names in the library, top card first.
fn library_top_first(driver: &Driver, seat: usize) -> Vec<String> {
    let mut library = names(driver.state(), Zone::Library(pid(seat)));
    library.reverse();
    library
}

#[test]
fn the_first_card_of_lesser_value_is_cast_for_nothing_and_the_rest_go_to_the_bottom()
-> Result<(), ScriptError> {
    let library = vec![
        forest(),
        mountain(),
        bear("Three", 3),
        bear("Four", 4),
        bear("Deep", 1),
    ];
    let mut driver = Driver::from_scenario(elf_game(library))?;
    cascade(&mut driver)?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("the cast is asked: {:?}", driver.decision()?.kind.name());
    };
    assert_eq!(question.text, "Cast Three without paying its mana cost?");
    assert_eq!(
        names(driver.state(), Zone::Exile),
        ["Forest", "Mountain", "Three"],
        "every card exiled is open on the table"
    );
    let events = say(&mut driver, true)?;
    assert!(events.iter().any(|event| matches!(event, Event::SpellCast { .. })));
    assert_eq!(
        names(driver.state(), Zone::Stack),
        ["Bloodbraid Elf", "Three"],
        "the spell is above the elf, whose trigger is gone"
    );
    assert!(names(driver.state(), Zone::Exile).is_empty());
    let library = library_top_first(&driver, 0);
    assert_eq!(library[..2], ["Four", "Deep"]);
    let mut bottom = library[2..].to_vec();
    bottom.sort();
    assert_eq!(bottom, ["Forest", "Mountain"]);
    resolve(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Stack), ["Bloodbraid Elf"]);
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Three".to_owned()));
    Ok(())
}

/// A library from a spec: `land` is a Forest, a number a creature of that mana value, anything else a corpus card.
fn library_of(spec: &[&str]) -> Vec<Arc<CardDefinition>> {
    spec.iter()
        .enumerate()
        .map(|(at, what)| match *what {
            "land" => forest(),
            number if number.parse::<u32>().is_ok() => bear(
                &format!("Card {at} costs {number}"),
                number.parse().unwrap_or(0),
            ),
            name => card(name).unwrap(),
        })
        .collect()
}

#[rstest]
#[case::lands_only(&["land", "land", "land"])]
#[case::equal_value_is_not_less(&["land", "4", "land"])]
#[case::only_greater_values(&["5", "6", "7"])]
#[case::an_empty_library(&[])]
#[case::no_legal_target_for_the_card_found(&["land", "Doom Blade"])]
fn nothing_is_asked_when_nothing_can_be_cast_and_every_card_goes_to_the_bottom(
    #[case] spec: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(elf_game(library_of(spec)))?;
    let before = {
        let mut cards = names(driver.state(), Zone::Library(pid(0)));
        cards.sort();
        cards
    };
    cascade(&mut driver)?;
    assert_eq!(driver.decision()?.kind.name(), "priority");
    assert_eq!(names(driver.state(), Zone::Stack), ["Bloodbraid Elf"]);
    assert!(names(driver.state(), Zone::Exile).is_empty());
    let mut after = names(driver.state(), Zone::Library(pid(0)));
    after.sort();
    assert_eq!(after, before, "no card is lost and none is cast");
    Ok(())
}

#[test]
fn a_cast_that_is_declined_leaves_the_card_at_the_bottom_with_the_others() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(elf_game(library_of(&["land", "2", "9"])))?;
    cascade(&mut driver)?;
    driver.expect_decision(0, "yes-no")?;
    say(&mut driver, false)?;
    assert_eq!(names(driver.state(), Zone::Stack), ["Bloodbraid Elf"]);
    let library = library_top_first(&driver, 0);
    assert_eq!(library[0], "Card 2 costs 9");
    let mut bottom = library[1..].to_vec();
    bottom.sort();
    assert_eq!(bottom, ["Card 1 costs 2", "Forest"]);
    Ok(())
}

#[test]
fn a_spell_with_a_target_asks_it_inside_the_resolution_and_resolves_before_the_elf()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(elf_game(library_of(&["land", "Lightning Bolt", "9"])))?;
    cascade(&mut driver)?;
    say(&mut driver, true)?;
    driver.expect_decision(0, "choose-targets")?;
    assert_eq!(
        names(driver.state(), Zone::Exile),
        ["Forest", "Lightning Bolt"],
        "the cards stay in exile while the cast is announced"
    );
    let foe = driver.target_player(1);
    let events = driver.choose_targets(&[foe])?;
    let cast = events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.clone()),
            _ => None,
        })
        .expect("the spell is cast");
    assert_eq!(cast.way, Some(CastWay::Free));
    assert_eq!(cast.targets.len(), 1);
    assert_eq!(names(driver.state(), Zone::Stack), ["Bloodbraid Elf", "Lightning Bolt"]);
    resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    assert_eq!(names(driver.state(), Zone::Stack), ["Bloodbraid Elf"]);
    Ok(())
}

#[test]
fn an_x_spell_is_cast_with_x_0_and_a_modal_spell_asks_its_modes() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(elf_game(library_of(&["Fireball", "9"])))?;
    cascade(&mut driver)?;
    say(&mut driver, true)?;
    driver.expect_decision(0, "choose-targets")?;
    let foe = driver.target_player(1);
    let events = driver.choose_targets(&[foe])?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::SpellCast { choices, .. } if choices.way == Some(CastWay::Free) && choices.x.unwrap_or(0) == 0
    )));
    resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20, "X is 0");

    let mut driver = Driver::from_scenario(elf_game(library_of(&["Izzet Charm", "9"])))?;
    cascade(&mut driver)?;
    say(&mut driver, true)?;
    driver.expect_decision(0, "choose-mode")?;
    Ok(())
}

#[test]
fn a_cost_increase_is_still_paid_and_the_payment_cannot_be_backed_out_of() -> Result<(), ScriptError> {
    let lands = [mountain(), mountain(), forest(), forest(), mountain()].map(ready);
    let mut battlefield = lands.to_vec();
    battlefield.push(ready(card("Thalia, Guardian of Thraben").unwrap()));
    let scenario = Scenario::new(2)
        .hand_named(0, ["Bloodbraid Elf"])
        .battlefield(0, battlefield)
        .library(0, library_of(&["Lightning Bolt", "9"]));
    let mut driver = Driver::from_scenario(scenario)?;
    cascade(&mut driver)?;
    say(&mut driver, true)?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    let options = driver.pay_options()?;
    assert_eq!(options.cost.to_string(), "{1}", "Thalia taxes the free spell");
    assert!(!options.can_cancel);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(names(driver.state(), Zone::Stack), ["Bloodbraid Elf", "Lightning Bolt"]);
    Ok(())
}

#[test]
fn nothing_is_asked_when_the_tax_cannot_be_paid() -> Result<(), ScriptError> {
    let mut battlefield = vec![ready(card("Thalia, Guardian of Thraben").unwrap())];
    battlefield.extend([mountain(), mountain(), forest(), forest()].map(ready));
    let scenario = Scenario::new(2)
        .hand_named(0, ["Bloodbraid Elf"])
        .battlefield(0, battlefield)
        .library(0, library_of(&["Lightning Bolt", "9"]));
    let mut driver = Driver::from_scenario(scenario)?;
    cascade(&mut driver)?;
    assert_eq!(driver.decision()?.kind.name(), "priority");
    assert_eq!(names(driver.state(), Zone::Stack), ["Bloodbraid Elf"]);
    Ok(())
}

#[test]
fn a_cascade_card_that_is_cast_by_a_cascade_cascades_in_its_turn() -> Result<(), ScriptError> {
    let chain = keyworded_creature("Chain", 3, &[], 1, 1, &[Keyword::Cascade]);
    let mut library = vec![chain];
    library.extend(library_of(&["land", "2", "9"]));
    let mut driver = Driver::from_scenario(elf_game(library))?;
    cascade(&mut driver)?;
    say(&mut driver, true)?;
    assert_eq!(
        names(driver.state(), Zone::Stack),
        ["Bloodbraid Elf", "Chain", "Chain"],
        "the cascade trigger of the new spell is on top"
    );
    // The cascade of the new spell goes on the stack once the first trigger is done, and finds a card of value 2.
    driver.expect_decision(0, "priority")?;
    resolve(&mut driver)?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("the second cascade asks: {:?}", driver.decision()?.kind.name());
    };
    assert_eq!(question.text, "Cast Card 1 costs 2 without paying its mana cost?");
    say(&mut driver, true)?;
    assert_eq!(
        names(driver.state(), Zone::Stack),
        ["Bloodbraid Elf", "Chain", "Card 1 costs 2"]
    );
    Ok(())
}

/// The order of the cards at the bottom of the library of seat 0 after a cascade over a library of `count` cards of value 5,
/// none of which can be found, played with `seed`.
fn bottom_order(seed: u64, count: usize) -> Result<(Vec<String>, Driver), ScriptError> {
    let library = (0..count)
        .map(|at| bear(&format!("Heavy {at}"), 5))
        .collect();
    let mut driver = Driver::from_scenario(elf_game(library).seed(seed))?;
    cascade(&mut driver)?;
    Ok((names(driver.state(), Zone::Library(pid(0))), driver))
}

#[test]
fn the_cards_go_to_the_bottom_in_an_order_the_seed_fixes_and_a_replay_repeats() -> Result<(), ScriptError> {
    let mut orders = std::collections::BTreeSet::new();
    for seed in 0..24 {
        let (order, driver) = bottom_order(seed, 6)?;
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, (0..6).map(|at| format!("Heavy {at}")).collect::<Vec<_>>());
        assert_eq!(bottom_order(seed, 6)?.0, order, "the same seed, the same order");
        let rebuilt = mtg_engine::Engine::resume(
            driver.engine().setup().clone(),
            driver.engine().events().to_vec(),
        )
        .unwrap();
        assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
        assert_eq!(rebuilt.state().rng_index(), driver.state().rng_index());
        orders.insert(order);
    }
    assert!(orders.len() > 12, "the order varies with the seed: {} orders", orders.len());
    Ok(())
}

#[test]
fn a_game_rebuilt_from_its_log_in_the_middle_of_the_cast_asks_the_same_question() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(elf_game(library_of(&["land", "Lightning Bolt", "9"])))?;
    cascade(&mut driver)?;
    say(&mut driver, true)?;
    driver.expect_decision(0, "choose-targets")?;
    let rebuilt = mtg_engine::Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.decision(), driver.engine().decision());
    assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
    Ok(())
}

/// A creature for nothing that discovers 4 when it enters.
fn discoverer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Discoverer")
            .with_type(mtg_core::card::CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(1, 1)
            .with_rules(Ability::Triggered {
                when: TriggerPattern::ZoneChange {
                    what: Selector::this(),
                    from: None,
                    to: Some(ZoneKind::Battlefield),
                },
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: discover(Amount::Fixed(4)),
            }),
    )
}

#[rstest]
#[case::cast_for_nothing(&["land", "4", "9"], Some(true), &["Card 1 costs 4"], &[])]
#[case::kept_in_the_hand_when_declined(&["land", "4", "9"], Some(false), &[], &["Card 1 costs 4"])]
#[case::a_card_that_cannot_be_cast_goes_to_the_hand(&["Counterspell", "9"], None, &[], &["Counterspell"])]
#[case::nothing_of_value_4_or_less(&["5", "land"], None, &[], &[])]
fn discover_casts_the_card_found_or_puts_it_into_the_hand(
    #[case] spec: &[&str],
    #[case] answer: Option<bool>,
    #[case] stack: &[&str],
    #[case] hand: &[&str],
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .hand(0, [discoverer()])
        .library(0, library_of(spec));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Discoverer")?;
    resolve(&mut driver)?;
    resolve(&mut driver)?;
    if let Some(yes) = answer {
        driver.expect_decision(0, "yes-no")?;
        say(&mut driver, yes)?;
    } else {
        assert_eq!(driver.decision()?.kind.name(), "priority");
    }
    assert_eq!(names(driver.state(), Zone::Stack), stack);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), hand);
    assert!(names(driver.state(), Zone::Exile).is_empty());
    Ok(())
}

