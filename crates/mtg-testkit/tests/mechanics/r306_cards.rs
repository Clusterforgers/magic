//! Goal: the r30-6 cards play their whole text at two to four seats: a control change to the one player with the most
//! cards in hand and none on a tie, a reveal repeated while lands come up, lands put down by each player and a draw for
//! each opponent who put none, a sacrifice any opponent may make before the creature transforms, three different players
//! for three gifts, an X held to the cap of its payment, a payment shrunk by domain and paid with no question at five
//! types, and a dance of discards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

fn bears(prefix: &str, count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|at| free_creature(&format!("{prefix} {at}"), 2, 2))
        .collect()
}

fn named_land(name: &str) -> Arc<CardDefinition> {
    let mut land = (*forest()).clone();
    name.clone_into(&mut land.name);
    Arc::new(land)
}

fn id(driver: &Driver, name: &str) -> ObjectId {
    driver.id_of(name).unwrap()
}

fn controller(driver: &Driver, name: &str) -> PlayerId {
    mtg_engine::characteristics::controller_of(driver.state(), id(driver, name)).unwrap()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// Answers with `policy` where it has an answer and passively elsewhere, until `turn` reaches `step`; then checks the log
/// replays to the same state.
fn run_until(
    driver: &mut Driver,
    turn: u32,
    step: Step,
    policy: &dyn Fn(&Driver, &Decision) -> Option<Answer>,
) -> Result<(), ScriptError> {
    for _ in 0..400 {
        let state = driver.state();
        if state.turn().number() == turn && state.turn().step() == step {
            let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
            assert_eq!(replayed.state_hash(), driver.state().state_hash());
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        let answer = policy(driver, &decision).unwrap_or_else(|| Driver::passive_answer(&decision));
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck(format!("{step:?}")))
}

fn passive(_: &Driver, _: &Decision) -> Option<Answer> {
    None
}

#[rstest]
#[case::the_most_takes_it(2, &[0, 3], Some(1))]
#[case::a_tie_keeps_it(2, &[2, 2], None)]
#[case::a_four_seat_leader(4, &[1, 2, 5, 4], Some(2))]
#[case::a_four_seat_tie(4, &[1, 4, 4, 0], None)]
#[case::you_lead(4, &[6, 2, 5, 4], Some(0))]
fn sokenzan_renegade_goes_to_the_one_player_with_the_most_cards(
    #[case] seats: usize,
    #[case] hands: &[usize],
    #[case] taker: Option<usize>,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Untap)
        .battlefield(0, [ready(card("Sokenzan Renegade"))]);
    for (seat, count) in hands.iter().enumerate() {
        scenario = scenario
            .hand(seat, bears(&format!("Card {seat}"), *count))
            .library(seat, bears(&format!("Deck {seat}"), 3));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    run_until(&mut driver, 2, Step::Draw, &passive)?;
    assert_eq!(
        controller(&driver, "Sokenzan Renegade"),
        pid(taker.unwrap_or(0))
    );
    Ok(())
}

#[rstest]
#[case::lands_then_a_spell(&[true, true, false, true], 2)]
#[case::a_spell_on_top(&[false, true], 0)]
#[case::only_lands(&[true, true, true], 3)]
fn countryside_crusher_mills_lands_until_another_card(
    #[case] top: &[bool],
    #[case] milled: usize,
) -> Result<(), ScriptError> {
    let library: Vec<Arc<CardDefinition>> = top
        .iter()
        .enumerate()
        .map(|(at, land)| {
            if *land {
                named_land(&format!("Land {at}"))
            } else {
                free_creature(&format!("Spell {at}"), 1, 1)
            }
        })
        .collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Untap)
            .battlefield(0, [ready(card("Countryside Crusher"))])
            .library(0, library)
            .library(1, bears("Deck", 3)),
    )?;
    run_until(&mut driver, 2, Step::Draw, &passive)?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    let expected: Vec<String> = (0..milled).map(|at| format!("Land {at}")).collect();
    assert_eq!(graveyard, expected);
    let crusher = driver
        .state()
        .object(id(&driver, "Countryside Crusher"))
        .unwrap();
    let counters = crusher
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(
        counters as usize, milled,
        "each land milled is a land card put into the graveyard"
    );
    Ok(())
}

fn pay_yes(_: &Driver, decision: &Decision) -> Option<Answer> {
    match &decision.kind {
        DecisionKind::YesNo(o) => Some(Answer::YesNo(YesNoChoice(!o.only_no))),
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::ChooseNumber(o) => Some(Answer::ChooseNumber(NumberChoice::Number(
            o.max.unwrap_or(o.min),
        ))),
        _ => None,
    }
}

#[rstest]
#[case::two_seats_both(2, &[true, true])]
#[case::two_seats_none(2, &[false, false])]
#[case::four_seats_some(4, &[true, false, true, false])]
fn kynaios_and_tiro_draws_for_each_opponent_who_put_no_land(
    #[case] seats: usize,
    #[case] puts: &[bool],
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main2)
        .battlefield(0, [ready(card("Kynaios and Tiro of Meletis"))]);
    for seat in 0..seats {
        let land = named_land(&format!("Land {seat}"));
        scenario = scenario
            .hand(seat, [land])
            .library(seat, bears(&format!("Deck {seat}"), 3));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<usize> = (0..seats).map(|seat| hand(&driver, seat)).collect();
    let wants = puts.to_vec();
    let policy = move |_: &Driver, decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(o) => {
            let seat = (0..wants.len())
                .find(|seat| pid(*seat) == decision.player)
                .unwrap();
            let take = if wants[seat] {
                o.max as usize
            } else {
                o.min as usize
            };
            Some(Answer::ChooseCards(CardsChoice(
                o.from.iter().copied().take(take).collect(),
            )))
        }
        _ => None,
    };
    run_until(&mut driver, 3, Step::Upkeep, &policy)?;
    for seat in 0..seats {
        let field = names(driver.state(), Zone::Battlefield);
        assert_eq!(
            field.contains(&format!("Land {seat}")),
            puts[seat],
            "seat {seat}"
        );
        let drew = usize::from(seat == 0) + usize::from(seat != 0 && !puts[seat]);
        let put = usize::from(puts[seat]);
        assert_eq!(
            hand(&driver, seat),
            before[seat] + drew - put,
            "seat {seat}"
        );
    }
    Ok(())
}

#[rstest]
#[case::nobody_two_seats(2, &[], true)]
#[case::the_opponent_two_seats(2, &[1], false)]
#[case::nobody_four_seats(4, &[], true)]
#[case::one_of_three(4, &[2], false)]
#[case::two_of_three(4, &[1, 3], false)]
fn innocent_traveler_transforms_unless_an_opponent_sacrifices(
    #[case] seats: usize,
    #[case] givers: &[usize],
    #[case] transforms: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Untap)
        .battlefield(0, [ready(card("Innocent Traveler // Malicious Invader"))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, bears(&format!("Deck {seat}"), 3));
        if seat > 0 {
            scenario =
                scenario.battlefield(seat, [ready(free_creature(&format!("Bear {seat}"), 2, 2))]);
        }
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let chosen = givers.to_vec();
    let policy = move |_: &Driver, decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(o) => {
            let gives = chosen.iter().any(|seat| pid(*seat) == decision.player);
            let take = if gives { 1 } else { o.min as usize };
            Some(Answer::ChooseCards(CardsChoice(
                o.from.iter().copied().take(take).collect(),
            )))
        }
        _ => None,
    };
    run_until(&mut driver, 2, Step::Draw, &policy)?;
    let name = if transforms {
        "Malicious Invader"
    } else {
        "Innocent Traveler"
    };
    assert!(
        names(driver.state(), Zone::Battlefield).contains(&name.to_owned()),
        "{name}"
    );
    for seat in 1..seats {
        let gone =
            names(driver.state(), Zone::Graveyard(pid(seat))).contains(&format!("Bear {seat}"));
        assert_eq!(gone, givers.contains(&seat), "seat {seat}");
    }
    Ok(())
}

#[rstest]
#[case::two_seats(2, 0)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 2)]
fn gluntch_gives_three_different_players_a_gift(
    #[case] seats: usize,
    #[case] treasures: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main2)
        .battlefield(0, [ready(card("Gluntch, the Bestower"))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, bears(&format!("Deck {seat}"), 3));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<usize> = (0..seats).map(|seat| hand(&driver, seat)).collect();
    run_until(&mut driver, 3, Step::Upkeep, &passive)?;
    let gluntch = driver
        .state()
        .object(id(&driver, "Gluntch, the Bestower"))
        .unwrap();
    assert_eq!(
        gluntch
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        2
    );
    let drawn: usize = (0..seats)
        .map(|seat| hand(&driver, seat) - before[seat])
        .sum();
    assert_eq!(drawn, 1, "one player draws");
    assert_eq!(
        hand(&driver, 0),
        before[0],
        "the first player is not the second"
    );
    let made = driver
        .state()
        .objects()
        .filter(|o| o.card().name == "Treasure" && o.zone() == Zone::Battlefield)
        .count();
    assert_eq!(made, treasures);
    Ok(())
}

const CAPPED: &str = r#"{"Activated":{"cost":[],"effects":[{"Pay":{"cost":[{"Mana":"{X}"},{"XAtMost":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"paid":[{"Draw":{"count":"X"}}]}}]}}"#;

#[rstest]
#[case::the_cap_is_lower(3, 1, 1)]
#[case::the_mana_is_lower(1, 4, 1)]
#[case::a_cap_of_none(2, 0, 0)]
fn an_x_is_held_to_the_cap_of_its_payment(
    #[case] lands: usize,
    #[case] creatures: usize,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let rules: Ability = serde_json::from_str(CAPPED).unwrap();
    let capped = Arc::new(
        CardDefinition::new("Capped")
            .with_type(CardType::Artifact)
            .with_rules(rules),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                std::iter::once(ready(capped))
                    .chain((0..lands).map(|_| ready(forest())))
                    .chain(
                        bears("Bear", creatures.saturating_sub(1))
                            .into_iter()
                            .map(ready),
                    )
                    .chain(
                        (0..usize::from(creatures > 0)).map(|_| ready(free_creature("Last", 1, 1))),
                    ),
            )
            .library(0, bears("Deck", 6)),
    )?;
    driver.begin_activation("Capped")?;
    let before = hand(&driver, 0);
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(o) => {
                assert_eq!(o.max, Some(most));
                let refused = driver.answer(Answer::ChooseNumber(NumberChoice::Number(most + 1)));
                assert!(refused.is_err(), "an X past the cap is refused");
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(most)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                let answer = pay_yes(&driver, &decision)
                    .unwrap_or_else(|| Driver::passive_answer(&decision));
                driver.answer(answer)?;
            }
        }
    }
    assert_eq!(hand(&driver, 0), before + most as usize);
    Ok(())
}

#[rstest]
#[case::five_types_cost_nothing(vec![plains(), island(), swamp(), mountain(), forest()], false, true)]
#[case::three_types_paid(vec![plains(), island(), swamp(), swamp(), swamp(), swamp()], true, true)]
#[case::three_types_short(vec![plains(), island(), swamp()], true, false)]
#[case::declined(vec![plains(), island(), swamp(), swamp(), swamp(), swamp()], false, false)]
fn draco_costs_ten_less_two_for_each_basic_land_type(
    #[case] lands: Vec<Arc<CardDefinition>>,
    #[case] yes: bool,
    #[case] stays: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Untap)
            .battlefield(
                0,
                std::iter::once(ready(card("Draco"))).chain(lands.into_iter().map(ready)),
            )
            .library(0, bears("Deck", 3))
            .library(1, bears("Deck", 3)),
    )?;
    let policy: &dyn Fn(&Driver, &Decision) -> Option<Answer> =
        if yes { &pay_yes } else { &passive };
    run_until(&mut driver, 2, Step::Draw, policy)?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(!graveyard.contains(&"Draco".to_owned()), stays);
    Ok(())
}

#[rstest]
#[case::creature_and_spell(2, &[Some(true), Some(false)], 1, 1)]
#[case::nobody(2, &[None, None], 0, 0)]
#[case::only_creatures(4, &[None, Some(true), None, Some(true)], 1, 0)]
fn mog_dances_with_each_discard(
    #[case] seats: usize,
    #[case] discards: &[Option<bool>],
    #[case] tokens: usize,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main2)
        .battlefield(0, [ready(card("Mog, Moogle Warrior"))]);
    for (seat, discard) in discards.iter().enumerate().take(seats) {
        let kept: Arc<CardDefinition> = match discard {
            Some(false) => {
                Arc::new(CardDefinition::new(format!("Spell {seat}")).with_type(CardType::Sorcery))
            }
            _ => free_creature(&format!("Spell {seat}"), 1, 1),
        };
        scenario = scenario
            .hand(seat, [kept])
            .library(seat, bears(&format!("Deck {seat}"), 3));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<usize> = (0..seats).map(|seat| hand(&driver, seat)).collect();
    let wants = discards.to_vec();
    let policy = move |_: &Driver, decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(o) => {
            let seat = (0..wants.len())
                .find(|seat| pid(*seat) == decision.player)
                .unwrap();
            let take = if wants[seat].is_some() {
                1
            } else {
                o.min as usize
            };
            Some(Answer::ChooseCards(CardsChoice(
                o.from.iter().copied().take(take).collect(),
            )))
        }
        _ => None,
    };
    run_until(&mut driver, 3, Step::Upkeep, &policy)?;
    let moogles: Vec<ObjectId> = driver
        .state()
        .objects()
        .filter(|o| o.card().name == "Moogle" && o.zone() == Zone::Battlefield)
        .map(mtg_engine::object::GameObject::id)
        .collect();
    assert_eq!(
        moogles.len(),
        tokens,
        "{:?}",
        names(driver.state(), Zone::Battlefield)
    );
    let mog = driver
        .state()
        .object(id(&driver, "Mog, Moogle Warrior"))
        .unwrap();
    assert_eq!(
        mog.permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        counters
    );
    for (seat, held) in before.iter().enumerate() {
        // A discard is followed by a draw, so each hand keeps its size.
        assert_eq!(hand(&driver, seat), *held, "seat {seat}");
    }
    Ok(())
}
