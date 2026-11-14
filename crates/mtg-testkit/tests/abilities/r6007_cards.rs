//! Goal: the r60-07 cards play as printed at two to four seats: River Song's controller draws from the bottom of their
//! library while the other players draw from the top; Breathstealer's Crypt reveals each card drawn and a creature card is
//! discarded unless its drawer pays 3 life; Zur's Weirding reveals the card instead and another player may pay 2 life to
//! mill it, or it is drawn; "Creatures don't untap during target player's next untap step" holds every creature down in
//! that one step and then ends; Theorist's Sanctum enters untapped only when a Jace is beheld; Way of the Mind Sculptor
//! draws for a loyalty activation that removed two or more counters; Jace, Reality Sculptor empowers Jace by the Islands.
//! Each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

fn r6007_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers every pending question, `yes` to each yes-or-no, until the stack and the waiting triggers are empty.
fn r6007_settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let state = driver.state();
        let busy = !state.stack().is_empty() || !state.pending_triggers().is_empty();
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) | DecisionKind::ChooseTargets(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// An artifact sacrificed for these steps (JSON), at no mana.
fn r6007_gadget(name: &str, effects: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["SacrificeSelf"],"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

fn r6007_use(driver: &mut Driver, name: &str, yes: bool) -> Result<(), ScriptError> {
    driver.begin_ability(name, 0)?;
    r6007_settle(driver, yes)
}

/// A library of named cards, top first.
fn r6007_pages(seat: usize) -> Vec<Arc<CardDefinition>> {
    ["Top", "Middle", "Bottom"]
        .iter()
        .map(|place| free_creature(&format!("{place} {seat}"), 1, 1))
        .collect()
}

fn r6007_hand(driver: &Driver, seat: usize) -> Vec<String> {
    common::names(driver.state(), Zone::Hand(pid(seat)))
}

fn r6007_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn r6007_river_song_draws_from_the_bottom(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] count: u32,
) -> Result<(), ScriptError> {
    let draw = format!(r#"[{{"Draw":{{"who":"EachPlayer","count":{count}}}}}]"#);
    let mut scenario = Scenario::new(seats).at(3, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("River Song").unwrap()),
            ready(r6007_gadget("Study", &draw)),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, r6007_pages(seat));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    r6007_use(&mut driver, "Study", true)?;
    let mut mine = r6007_hand(&driver, 0);
    mine.sort();
    let expected: Vec<String> = if count == 1 {
        vec!["Bottom 0".into()]
    } else {
        vec!["Bottom 0".into(), "Middle 0".into()]
    };
    assert_eq!(mine, expected);
    for seat in 1..seats {
        assert!(
            r6007_hand(&driver, seat).contains(&format!("Top {seat}")),
            "seat {seat}"
        );
        assert!(!r6007_hand(&driver, seat).contains(&format!("Bottom {seat}")));
    }
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_breathstealers_crypt_discards_a_drawn_creature_unless_life_is_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] creature: bool,
    #[values(false, true)] pay: bool,
) -> Result<(), ScriptError> {
    let top = if creature {
        free_creature("Drawn", 2, 2)
    } else {
        plains()
    };
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Breathstealer's Crypt").unwrap()),
                ready(r6007_gadget("Study", r#"[{"Draw":{}}]"#)),
            ],
        )
        .library(0, [top, plains()]);
    let mut driver = Driver::from_scenario(scenario)?;
    r6007_use(&mut driver, "Study", pay)?;
    let kept = !creature || pay;
    assert_eq!(r6007_hand(&driver, 0).len(), usize::from(kept));
    assert_eq!(
        common::names(driver.state(), Zone::Graveyard(pid(0))).len(),
        // The sacrificed gadget, and a creature card discarded.
        1 + usize::from(!kept)
    );
    assert_eq!(
        r6007_life(&driver, 0),
        if creature && pay { 17 } else { 20 }
    );
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_zurs_weirding_mills_the_card_when_another_player_pays(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] pay: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Zur's Weirding").unwrap()),
                ready(r6007_gadget("Study", r#"[{"Draw":{}}]"#)),
            ],
        )
        .library(0, r6007_pages(0));
    let mut driver = Driver::from_scenario(scenario)?;
    r6007_use(&mut driver, "Study", pay)?;
    let graveyard = common::names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(graveyard.contains(&"Top 0".to_owned()), pay);
    assert_eq!(
        r6007_hand(&driver, 0),
        if pay {
            Vec::new()
        } else {
            vec!["Top 0".to_owned()]
        }
    );
    // The drawing player is never asked; the first other player in turn order pays.
    assert_eq!(r6007_life(&driver, 0), 20);
    assert_eq!(r6007_life(&driver, 1), if pay { 18 } else { 20 });
    for seat in 2..seats {
        assert_eq!(r6007_life(&driver, seat), 20);
    }
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_creatures_stay_tapped_through_that_players_next_untap_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hold = r#"[{"Rule":{"rule":{"R6007NoUntapDuring":{"what":[{"Type":"Creature"}],"whose":"Opponents"}},"duration":"NextUntapStep"}}]"#;
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(r6007_gadget("Beam", hold))])
        .battlefield(
            1,
            [tapped(free_creature("Sleeper", 2, 2)), tapped(island())],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    r6007_use(&mut driver, "Beam", true)?;
    driver.idle_until(4, Step::Upkeep)?;
    let is_tapped = |driver: &Driver, name: &str| {
        let id = driver.id_of(name).unwrap();
        driver
            .state()
            .object(id)
            .and_then(|object| object.permanent())
            .is_some_and(|permanent| permanent.tapped())
    };
    assert!(
        is_tapped(&driver, "Sleeper"),
        "held down in its controller's untap step"
    );
    assert!(!is_tapped(&driver, "Island"), "a land untaps");
    assert!(
        driver.state().rule_effects().is_empty(),
        "the rule ended with the step"
    );
    let next = 4 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Upkeep)?;
    assert!(!is_tapped(&driver, "Sleeper"), "untaps in the step after");
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_theorists_sanctum_enters_tapped_unless_a_jace_is_beheld(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] jace: u8,
    #[values(false, true)] behold: bool,
) -> Result<(), ScriptError> {
    let jace_card = || {
        Arc::new(
            CardDefinition::new("Jace Card")
                .with_type(CardType::Planeswalker)
                .with_subtype("Jace"),
        )
    };
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Theorist's Sanctum").unwrap()]);
    // 0: no Jace; 1: a Jace card in hand; 2: a Jace permanent.
    if jace == 1 {
        scenario = scenario.hand(0, [card("Theorist's Sanctum").unwrap(), jace_card()]);
    }
    if jace == 2 {
        scenario = scenario.battlefield(0, [ready(jace_card())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.play_land("Theorist's Sanctum")?;
    r6007_settle(&mut driver, behold)?;
    let id = driver.id_of("Theorist's Sanctum")?;
    let entered_tapped = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(|permanent| permanent.tapped());
    assert_eq!(entered_tapped, !(behold && jace > 0));
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
#[case::plus_draws_nothing(0, 0)]
#[case::minus_three_draws(1, 1)]
fn r6007_way_of_the_mind_sculptor_draws_for_two_or_more_removed(
    #[values(2, 3, 4)] seats: usize,
    #[case] ability: usize,
    #[case] draws: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Way of the Mind Sculptor").unwrap()),
                with_counters(
                    ready(card("Jace, Reality Sculptor").unwrap()),
                    CounterKind::Loyalty,
                    5,
                ),
            ],
        )
        .library(0, r6007_pages(0));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Jace, Reality Sculptor", ability)?;
    r6007_settle(&mut driver, true)?;
    // The +1 empowers a new Jace token with the Islands (none); the -3 waits for an attack.
    assert_eq!(r6007_hand(&driver, 0).len(), draws);
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_jace_empowers_jace_by_the_islands(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] islands: usize,
) -> Result<(), ScriptError> {
    let mut permanents = vec![with_counters(
        ready(card("Jace, Reality Sculptor").unwrap()),
        CounterKind::Loyalty,
        5,
    )];
    permanents.extend((0..islands).map(|_| ready(island())));
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, permanents);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Jace, Reality Sculptor", 0)?;
    r6007_settle(&mut driver, true)?;
    let token = driver.id_of("Jace")?;
    let loyalty = driver
        .state()
        .object(token)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::Loyalty));
    assert_eq!(loyalty, u32::try_from(islands).unwrap());
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_each_player_keeps_creatures_with_total_power_four_or_less(
    #[values(2, 3, 4)] seats: usize,
    #[values("Destined Confrontation", "Slaughter the Strong")] name: &str,
    #[values(false, true)] keep: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card(name).unwrap()])
        .battlefield(0, [(); 4].map(|()| ready(plains())));
    for seat in 0..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Big {seat}"), 3, 3)),
                ready(free_creature(&format!("Mid {seat}"), 2, 2)),
                ready(free_creature(&format!("Small {seat}"), 1, 1)),
                ready(free_creature(&format!("Weak {seat}"), -1, 1)),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(name)?;
    for _ in 0..80 {
        let state = driver.state();
        let busy = !state.stack().is_empty();
        match &driver.decision()?.kind {
            // Each pick takes the first creature offered while keeping, none otherwise.
            DecisionKind::ChooseCards(options) => {
                let first = options.from.first().copied().filter(|_| keep);
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                    first.into_iter().collect(),
                )))?;
            }
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let field = common::names(driver.state(), Zone::Battlefield);
    for seat in 0..seats {
        let kept: Vec<&str> = ["Big", "Mid", "Small", "Weak"]
            .into_iter()
            .filter(|kind| field.contains(&format!("{kind} {seat}")))
            .collect();
        // Taking the first offered: Big (3), then Mid no longer fits, Small (4), Weak (-1 makes 3), and Mid still not.
        let expected = if keep {
            vec!["Big", "Small", "Weak"]
        } else {
            Vec::new()
        };
        assert_eq!(kept, expected, "seat {seat}");
    }
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_elvish_refueler_exhausts_again_until_one_is_activated_this_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Elvish Refueler").unwrap())])
        .battlefield(0, [(); 6].map(|()| ready(mtg_testkit::cards::forest())));
    let mut driver = Driver::from_scenario(scenario)?;
    let grown = |driver: &Driver| {
        let id = driver.id_of("Elvish Refueler").unwrap();
        driver
            .state()
            .object(id)
            .and_then(|object| object.permanent())
            .map_or(0, |permanent| {
                permanent.counters_of(&CounterKind::PlusOnePlusOne)
            })
    };
    driver.begin_ability("Elvish Refueler", 1)?;
    r6007_settle(&mut driver, true)?;
    assert_eq!(grown(&driver), 1);
    // Activated an exhaust ability this turn: not again.
    assert!(driver.begin_ability("Elvish Refueler", 1).is_err());
    // On the next turn of its controller it may be activated once more, though it was activated before.
    let next = 3 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1)?;
    driver.begin_ability("Elvish Refueler", 1)?;
    r6007_settle(&mut driver, true)?;
    assert_eq!(grown(&driver), 2);
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_dragons_disciple_grows_for_a_dragon_revealed_or_controlled(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] dragon: u8,
    #[values(false, true)] reveal: bool,
) -> Result<(), ScriptError> {
    let dragon_card = || {
        Arc::new(
            CardDefinition::new("Wyrm")
                .with_type(CardType::Creature)
                .with_subtype("Dragon"),
        )
    };
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [(); 3].map(|()| ready(mtg_testkit::cards::mountain())));
    // 0: no Dragon; 1: a Dragon card in hand; 2: a Dragon permanent.
    scenario = match dragon {
        1 => scenario.hand(0, [card("Dragon's Disciple").unwrap(), dragon_card()]),
        2 => scenario
            .hand(0, [card("Dragon's Disciple").unwrap()])
            .battlefield(0, [ready(dragon_card())]),
        _ => scenario.hand(0, [card("Dragon's Disciple").unwrap()]),
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Dragon's Disciple")?;
    r6007_settle(&mut driver, reveal)?;
    let id = driver.id_of("Dragon's Disciple")?;
    let counters = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    let grows = dragon == 2 || (dragon == 1 && reveal);
    assert_eq!(counters, u32::from(grows));
    r6007_replays(&driver);
    Ok(())
}
