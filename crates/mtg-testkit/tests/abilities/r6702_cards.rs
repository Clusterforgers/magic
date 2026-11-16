//! Goal: the r67-02 activated, triggered and static abilities play as they read at two to four seats: Baba Lysaga counts
//! the card types among the permanents sacrificed for its cost (none to three of them, an artifact creature counting
//! twice), Galactus must attack an opponent with the most life (any of several tied) unless its controller controls
//! Silver Surfer, Koh gains the activated and triggered abilities of the last card chosen among those it exiled, and
//! Incinerator of the Guilty deals the X of the evidence collected to each creature and planeswalker of the player it
//! damaged, Jace's 0 is offered only at twenty-five loyalty and leaves each opponent the bottom card of their library,
//! and Fabrication Foundry exiles artifacts of total mana value exactly X to return one of mana value X or less.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The name of an object.
fn r6702_name(driver: &Driver, id: ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

/// Answers everything passively until the stack is empty and nothing waits.
fn r6702_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
        {
            break;
        }
        let decision = driver.decision()?.clone();
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Ok(())
}

/// Answers the card choice asked now with the cards of these names.
fn r6702_take(driver: &mut Driver, names: &[&str]) -> Result<(), ScriptError> {
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("a card choice is asked: {}", driver.decision()?.kind.name());
    };
    let mut wanted: Vec<&str> = names.to_vec();
    let mut taken: Vec<ObjectId> = Vec::new();
    for id in &options.from {
        let name = r6702_name(driver, *id);
        if let Some(at) = wanted.iter().position(|w| *w == name) {
            wanted.remove(at);
            taken.push(*id);
        }
    }
    driver.answer(Answer::ChooseCards(CardsChoice(taken)))?;
    Ok(())
}

/// Seat 0 sacrifices the named permanents to Baba Lysaga: the life of seat 0 and of seat 1, and the cards seat 0 drew.
fn r6702_baba(seats: usize, sacrificed: &[&str]) -> Result<(i32, i32, usize), ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(card("Baba Lysaga, Night Witch").unwrap()),
                    ready(mountain()),
                    ready(card("Sol Ring").unwrap()),
                    ready(card("Signal Pest").unwrap()),
                    ready(free_creature("Wolf", 2, 2)),
                ],
            )
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let held = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_activation("Baba Lysaga, Night Witch")?;
    r6702_take(&mut driver, sacrificed)?;
    r6702_settle(&mut driver)?;
    let state = driver.state();
    Ok((
        state.player(pid(0)).unwrap().life(),
        state.player(pid(1)).unwrap().life(),
        state.zone(Zone::Hand(pid(0))).len() - held,
    ))
}

#[rstest]
#[case::land_artifact_creature(2, &["Mountain", "Sol Ring", "Wolf"], true)]
#[case::an_artifact_creature_counts_twice(3, &["Mountain", "Signal Pest"], true)]
#[case::two_types(4, &["Sol Ring", "Wolf"], false)]
#[case::nothing(2, &[], false)]
fn r6702_baba_lysaga_counts_the_card_types_sacrificed(
    #[case] seats: usize,
    #[case] sacrificed: &[&str],
    #[case] rewarded: bool,
) -> Result<(), ScriptError> {
    let (mine, theirs, drawn) = r6702_baba(seats, sacrificed)?;
    if rewarded {
        assert_eq!((mine, theirs, drawn), (23, 17, 3));
    } else {
        assert_eq!((mine, theirs, drawn), (20, 20, 0));
    }
    Ok(())
}

/// Seat 0 attacks with Galactus (and with Silver Surfer beside it when `herald`), the opponents' life set as given: the
/// players Galactus may attack, and whether it must attack.
fn r6702_galactus(lives: &[i32], herald: bool) -> Result<(Vec<AttackTarget>, bool), ScriptError> {
    let seats = lives.len() + 1;
    let mut mine = vec![ready(card("Galactus, Devourer of Worlds").unwrap())];
    if herald {
        mine.push(ready(free_creature(
            "Silver Surfer, Galactus's Herald",
            2,
            2,
        )));
    }
    let scenario = lives.iter().enumerate().fold(
        Scenario::new(seats)
            .battlefield(0, mine)
            .at(1, 0, Step::Main1),
        |scenario, (at, life)| scenario.life(at + 1, *life),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("seat 0 declares attackers");
    };
    let galactus = driver.id_of("Galactus, Devourer of Worlds")?;
    let option = options
        .attackers
        .iter()
        .find(|a| a.object == galactus)
        .unwrap();
    Ok((
        option.defenders.clone(),
        options.required.contains(&galactus),
    ))
}

#[rstest]
#[case::the_richer_of_two(&[30, 50], false, &[2])]
#[case::two_tied_of_three(&[50, 20, 50], false, &[1, 3])]
#[case::the_only_one(&[40], false, &[1])]
#[case::not_with_silver_surfer(&[30, 50], true, &[1, 2])]
fn r6702_galactus_attacks_an_opponent_with_the_most_life(
    #[case] lives: &[i32],
    #[case] herald: bool,
    #[case] expected: &[usize],
) -> Result<(), ScriptError> {
    let (defenders, required) = r6702_galactus(lives, herald)?;
    let players: Vec<usize> = defenders
        .iter()
        .filter_map(|target| match target {
            AttackTarget::Player(player) => Some(player.index()),
            AttackTarget::Planeswalker(_) => None,
        })
        .collect();
    assert_eq!(players, expected);
    assert_eq!(required, !herald);
    Ok(())
}

/// Seat 0's Koh exiled the Elves; seat 0 pays 1 life to choose `chosen`: seat 0's life and the mana abilities of Koh.
fn r6702_koh(seats: usize, chosen: Option<&str>) -> Result<(i32, usize), ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .hand(0, [card("Koh, the Face Stealer").unwrap()])
            .battlefield(
                0,
                std::iter::once(ready(card("Llanowar Elves").unwrap()))
                    .chain((0..6).map(|_| ready(mtg_testkit::cards::swamp())))
                    .collect::<Vec<_>>(),
            )
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Koh, the Face Stealer")?;
    // The enters trigger exiles the Elves.
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                let elves = driver.id_of("Llanowar Elves")?;
                let target = mtg_engine::choose::Target::Object(
                    driver.state().object(elves).unwrap().reference(),
                );
                driver.choose_targets(&[target])?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    if let Some(chosen) = chosen {
        driver.begin_activation("Koh, the Face Stealer")?;
        r6702_settle_choosing(&mut driver, chosen)?;
    }
    let koh = driver.id_of("Koh, the Face Stealer")?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority after the choice");
    };
    let mana = options
        .mana_abilities
        .iter()
        .filter(|option| option.source.object == koh)
        .count();
    Ok((driver.state().player(pid(0)).unwrap().life(), mana))
}

/// Settles the stack, choosing the named card when a card choice is asked.
fn r6702_settle_choosing(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(_) => r6702_take(driver, &[name])?,
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r6702_koh_gains_the_abilities_of_the_chosen_card(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let (_, without) = r6702_koh(seats, None)?;
    let (life, with) = r6702_koh(seats, Some("Llanowar Elves"))?;
    assert_eq!(life, 19);
    // Koh has no mana ability of its own, and the Elves' once the Elves are chosen.
    assert_eq!((without, with), (0, 1));
    Ok(())
}

/// Seat 0's Incinerator deals combat damage to seat 1 and seat 0 collects evidence `x` (declines at 0) with every card of a
/// graveyard of mana values 1, 3 and 1.
fn r6702_incinerator(seats: usize, x: u32) -> Result<(Vec<String>, usize), ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(0, [ready(card("Incinerator of the Guilty").unwrap())])
            .graveyard(
                0,
                [
                    card("Lightning Bolt").unwrap(),
                    card("Rhystic Study").unwrap(),
                    card("Duress").unwrap(),
                ],
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Small", 1, 2)),
                    ready(free_creature("Big", 5, 5)),
                    ready(walker("Walker", 3)),
                ],
            )
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Incinerator of the Guilty", 1)])?;
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(x > 0)))?;
            }
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let all = options.from.clone();
                driver.answer(Answer::ChooseCards(CardsChoice(all)))?;
            }
            _ if driver.state().turn().step() == Step::Main2 => break,
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    let mut alive: Vec<String> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.controller() == PlayerId::from_index(1))
        })
        .map(|id| r6702_name(&driver, *id))
        .collect();
    alive.sort();
    let exiled = driver.state().zone(Zone::Exile).len();
    Ok((alive, exiled))
}

#[rstest]
#[case::three_kills_the_small_and_the_walker(2, 3, &["Big"], 3)]
#[case::five_four_seats(4, 5, &["Big"], 3)]
#[case::nothing_collected(3, 0, &["Big", "Small", "Walker"], 0)]
fn r6702_incinerator_deals_the_evidence_collected(
    #[case] seats: usize,
    #[case] x: u32,
    #[case] left: &[&str],
    #[case] exiled: usize,
) -> Result<(), ScriptError> {
    let (alive, gone) = r6702_incinerator(seats, x)?;
    assert_eq!(alive, left);
    assert_eq!(gone, exiled);
    Ok(())
}

/// Seat 0's Jace with this loyalty: whether its 0 ability is offered, and after it, the library sizes of the other seats.
fn r6702_jace(seats: usize, loyalty: u32) -> Result<(bool, Vec<usize>), ScriptError> {
    let jace = (*card("Jace, Reality Sculptor").unwrap())
        .clone()
        .with_loyalty(loyalty);
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(0, [ready(std::sync::Arc::new(jace))])
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![mountain(); 6]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let offered = driver
        .offered_abilities("Jace, Reality Sculptor")?
        .contains(&2);
    if offered {
        driver.begin_ability("Jace, Reality Sculptor", 2)?;
        r6702_settle(&mut driver)?;
    }
    let libraries = (1..seats)
        .map(|seat| driver.state().zone(Zone::Library(pid(seat))).len())
        .collect();
    Ok((offered, libraries))
}

#[rstest]
#[case::twenty_five_two_seats(2, 25, true)]
#[case::twenty_five_four_seats(4, 25, true)]
#[case::twenty_four(3, 24, false)]
fn r6702_jace_exiles_all_but_the_bottom_card_at_twenty_five_loyalty(
    #[case] seats: usize,
    #[case] loyalty: u32,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let (found, libraries) = r6702_jace(seats, loyalty)?;
    assert_eq!(found, offered);
    let left = if offered { 1 } else { 6 };
    assert!(
        libraries.iter().all(|cards| *cards == left),
        "{libraries:?}"
    );
    Ok(())
}

/// Seat 0 activates Fabrication Foundry for this X, exiling the named artifacts: whether the Sol Ring of its graveyard
/// came back, and how many artifacts seat 0 still controls besides the Foundry.
fn r6702_foundry(seats: usize, x: u32, exiled: &[&str]) -> Result<(bool, usize), ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(
                0,
                [
                    ready(card("Fabrication Foundry").unwrap()),
                    ready(card("Arcane Signet").unwrap()),
                    ready(card("Signal Pest").unwrap()),
                    ready(mtg_testkit::cards::plains()),
                    ready(mtg_testkit::cards::plains()),
                    ready(mtg_testkit::cards::plains()),
                ],
            )
            .graveyard(0, [card("Sol Ring").unwrap()])
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![mountain(); 6]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let abilities = driver.offered_abilities("Fabrication Foundry")?;
    let index = abilities.iter().copied().max().unwrap();
    driver.begin_ability("Fabrication Foundry", index)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseCards(_) => r6702_take(&mut driver, exiled)?,
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    let state = driver.state();
    let back = state
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| r6702_name(&driver, *id) == "Sol Ring");
    let artifacts = ["Arcane Signet", "Signal Pest", "Sol Ring"]
        .iter()
        .filter(|name| {
            state
                .zone(Zone::Battlefield)
                .iter()
                .any(|id| r6702_name(&driver, *id) == **name)
        })
        .count();
    Ok((back, artifacts))
}

#[rstest]
#[case::signet_and_pest_make_three(2, 3, &["Arcane Signet", "Signal Pest"], true, 1)]
#[case::the_signet_alone_makes_two(4, 2, &["Arcane Signet"], true, 2)]
fn r6702_fabrication_foundry_returns_an_artifact_of_the_exiled_total(
    #[case] seats: usize,
    #[case] x: u32,
    #[case] exiled: &[&str],
    #[case] back: bool,
    #[case] artifacts: usize,
) -> Result<(), ScriptError> {
    let (returned, left) = r6702_foundry(seats, x, exiled)?;
    assert_eq!(returned, back);
    assert_eq!(left, artifacts);
    Ok(())
}
