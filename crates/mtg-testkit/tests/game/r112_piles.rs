//! Goal: piles, secret numbers, guesses and stored dice at 2 to 4 seats. Goblin Game takes each player's secret number
//! and then half the life of those who chose the fewest; Expert-Level Safe opens on matching secret choices of 1 to 3
//! and exiles one more card otherwise; The Toymaker's Trap never offers a number it chose before and pays out on a
//! wrong guess; Whims of the Fates has each player separate their permanents into three piles and sacrifice one at
//! random; Centaur of Attention stores five dice, rerolls the ones asked and gets +X/+X for its most common result; The
//! Celestial Toymaker hands its controller the pile the defending player chose and costs each opponent 2 life at the
//! end step for each spell or ability that grouped cards into piles this turn; Raging River lets only fliers and the
//! pile with the chosen label block each attacker; Ferris Wheel holds a creature out until a visit roll of 3 or less and
//! never takes it again. Each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, NumberChoice, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::casting::{lands, pay};
use crate::common::{names, on_battlefield, pid};

/// The log replays to the live state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Answers until the stack is empty and priority is back, `pick` first and passively when it gives nothing. Returns
/// the events.
fn settle(
    driver: &mut Driver,
    mut pick: impl FnMut(&Driver, &Decision) -> Option<Answer>,
) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..400 {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return events;
        }
        let answer = pick(driver, &decision);
        events.extend(match answer {
            Some(answer) => driver.answer(answer).unwrap(),
            None => driver.answer_passively().unwrap(),
        });
    }
    panic!("the stack never emptied");
}

/// Casts the card from seat 0's hand, paying with its lands.
fn cast(driver: &mut Driver, name: &str) {
    driver.cast(name).unwrap();
    if driver.decision().unwrap().kind.name() == "pay-mana" {
        pay(driver).unwrap();
    }
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn r112_goblin_game_costs_each_number_and_half_for_the_fewest(#[values(2, 3, 4)] seats: usize) {
    let chosen = [3_u32, 1, 2, 1];
    let scenario = Scenario::new(seats)
        .hand(0, [card("Goblin Game").unwrap()])
        .battlefield(0, lands(7, mountain));
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    cast(&mut driver, "Goblin Game");
    let mut asked = Vec::new();
    settle(&mut driver, |_, decision| match &decision.kind {
        DecisionKind::ChooseNumber(options) => {
            assert!(options.secret);
            assert_eq!((options.min, options.max), (1, None));
            asked.push(decision.player.index());
            Some(Answer::ChooseNumber(NumberChoice::Number(
                chosen[decision.player.index()],
            )))
        }
        _ => None,
    });
    assert_eq!(
        asked,
        (0..seats).collect::<Vec<_>>(),
        "the caster first, then turn order"
    );
    let fewest = chosen[..seats].iter().min().copied().unwrap();
    for seat in 0..seats {
        let after = 20 - i32::try_from(chosen[seat]).unwrap();
        let expected = if chosen[seat] == fewest {
            after - (after + 1) / 2
        } else {
            after
        };
        assert_eq!(life(&driver, seat), expected, "seat {seat}");
    }
    replays(&scenario, &driver);
}

#[rstest]
fn r112_expert_level_safe_opens_on_a_match(
    #[values(true, false)] matching: bool,
    #[values(2, 3)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .hand(0, [card("Expert-Level Safe").unwrap()])
        .battlefield(0, lands(3, plains))
        .library(0, vec![island(), island(), island(), island(), island()]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    cast(&mut driver, "Expert-Level Safe");
    settle(&mut driver, |_, _| None);
    assert_eq!(
        driver.state().zone(Zone::Exile).len(),
        2,
        "two cards exiled face down"
    );
    driver.begin_activation("Expert-Level Safe").unwrap();
    driver
        .choose_targets(&[driver.target_player(seats - 1)])
        .unwrap();
    pay(&mut driver).unwrap();
    let mut asked = Vec::new();
    settle(&mut driver, |_, decision| match &decision.kind {
        DecisionKind::ChooseNumber(options) => {
            assert!(options.secret);
            assert_eq!((options.min, options.max), (1, Some(3)));
            asked.push(decision.player.index());
            let mine = decision.player.index() == 0;
            Some(Answer::ChooseNumber(NumberChoice::Number(
                if matching || mine { 2 } else { 3 },
            )))
        }
        _ => None,
    });
    assert_eq!(
        asked,
        vec![0, seats - 1],
        "only the controller and the target choose"
    );
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    if matching {
        assert!(on_battlefield(driver.state(), "Expert-Level Safe").is_none());
        assert_eq!((hand, driver.state().zone(Zone::Exile).len()), (2, 0));
    } else {
        assert!(on_battlefield(driver.state(), "Expert-Level Safe").is_some());
        assert_eq!((hand, driver.state().zone(Zone::Exile).len()), (0, 3));
    }
    replays(&scenario, &driver);
}

#[rstest]
fn r112_the_toymakers_trap_offers_only_fresh_numbers(
    #[values(true, false)] right: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .at(1, seats - 1, Step::End)
        .battlefield(0, [ready(card("The Toymaker's Trap").unwrap())])
        .library(0, vec![swamp(), swamp(), swamp(), swamp()]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let mut excluded = Vec::new();
    let mut guesser = None;
    let start = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.idle_until(2, Step::Upkeep).unwrap();
    settle(&mut driver, |_, decision| match &decision.kind {
        DecisionKind::ChooseNumber(options) if options.secret => {
            assert_eq!(decision.player, pid(0));
            excluded.clone_from(&options.except);
            Some(Answer::ChooseNumber(NumberChoice::Number(3)))
        }
        DecisionKind::ChooseNumber(options) => {
            assert_eq!((options.min, options.max), (1, Some(5)));
            guesser = Some(decision.player.index());
            Some(Answer::ChooseNumber(NumberChoice::Number(if right {
                3
            } else {
                4
            })))
        }
        DecisionKind::ChooseOption(options) if options.text == "Choose an opponent to guess." => {
            Some(Answer::ChooseOption(OptionChoice(
                options.options.len() - 1,
            )))
        }
        _ => None,
    });
    assert!(excluded.is_empty(), "nothing chosen before");
    assert_eq!(guesser, Some(seats - 1));
    if right {
        assert!(on_battlefield(driver.state(), "The Toymaker's Trap").is_none());
        assert_eq!(life(&driver, seats - 1), 20);
    } else {
        assert!(on_battlefield(driver.state(), "The Toymaker's Trap").is_some());
        assert_eq!(life(&driver, seats - 1), 16);
        assert_eq!(
            driver.state().zone(Zone::Hand(pid(0))).len(),
            start + 1,
            "the Trap draws in the upkeep"
        );
        // The next upkeep of seat 0 leaves out the 3.
        driver
            .idle_until(2 + u32::try_from(seats).unwrap(), Step::Upkeep)
            .unwrap();
        settle(&mut driver, |_, decision| match &decision.kind {
            DecisionKind::ChooseNumber(options) if options.secret => {
                excluded.clone_from(&options.except);
                Some(Answer::ChooseNumber(NumberChoice::Number(1)))
            }
            DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(NumberChoice::Number(2))),
            _ => None,
        });
        assert_eq!(excluded, vec![3]);
    }
    replays(&scenario, &driver);
}

#[rstest]
fn r112_whims_of_the_fates_sacrifices_one_pile_each(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) {
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .hand(0, [card("Whims of the Fates").unwrap()])
        .battlefield(0, lands(6, mountain));
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            ["A", "B", "C"].map(|name| ready(free_creature(&format!("{name}{seat}"), 1, 1))),
        );
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    cast(&mut driver, "Whims of the Fates");
    let mut separators = Vec::new();
    settle(&mut driver, |driver, decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            separators.push(decision.player.index());
            assert!(options.text.contains("3 piles"), "{}", options.text);
            // One card a pile for the others; the caster puts all six Mountains in the first pile.
            let take = if decision.player == pid(0) {
                options.from.len()
            } else {
                1
            };
            let _ = driver;
            Some(Answer::ChooseCards(CardsChoice(
                options.from[..take].to_vec(),
            )))
        }
        _ => None,
    });
    let mut expected = vec![0];
    for seat in 1..seats {
        expected.extend([seat, seat]);
    }
    assert_eq!(
        separators, expected,
        "the caster first; two piles asked of each, the third is the rest"
    );
    let lands_left = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().controller() == pid(0))
        .count();
    assert!(
        lands_left == 0 || lands_left == 6,
        "the caster's pile goes whole or stays whole"
    );
    for seat in 1..seats {
        let left = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| driver.state().object(**id).unwrap().controller() == pid(seat))
            .count();
        assert_eq!(left, 2, "seat {seat} sacrificed one pile of one creature");
    }
    replays(&scenario, &driver);
}

#[rstest]
fn r112_centaur_of_attention_counts_its_most_common_result(
    #[values(2, 3)] seats: usize,
    #[values(true, false)] reroll: bool,
) {
    let scenario = Scenario::new(seats)
        .hand(0, [card("Centaur of Attention").unwrap()])
        .battlefield(0, lands(5, forest));
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    cast(&mut driver, "Centaur of Attention");
    let events = settle(&mut driver, |_, _| None);
    let rolled: Vec<u32> = events
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect();
    assert_eq!(rolled.len(), 5);
    let most = |results: &[u32]| {
        (1..=6)
            .map(|face| results.iter().filter(|r| **r == face).count())
            .max()
            .unwrap()
    };
    let id = driver.id_of("Centaur of Attention").unwrap();
    let pt = |driver: &Driver| {
        let chars = characteristics(driver.state(), id).unwrap();
        (chars.power.unwrap(), chars.toughness.unwrap())
    };
    let x = i32::try_from(most(&rolled)).unwrap();
    assert_eq!(pt(&driver), (3 + x, 3 + x));
    driver.pass_until(Step::BeginCombat).unwrap();
    let mut asked = 0;
    let events = settle(&mut driver, |_, decision| match &decision.kind {
        DecisionKind::YesNo(options) if options.text.starts_with("Reroll stored result") => {
            asked += 1;
            Some(Answer::YesNo(YesNoChoice(reroll && asked % 2 == 1)))
        }
        _ => None,
    });
    assert_eq!(asked, 5, "each stored result is asked about");
    let rerolled: Vec<u32> = events
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect();
    let mut stored = rolled;
    if reroll {
        assert_eq!(rerolled.len(), 3);
        for (at, result) in [0, 2, 4].into_iter().zip(&rerolled) {
            stored[at] = *result;
        }
    } else {
        assert_eq!(rerolled, Vec::<u32>::new());
    }
    let x = i32::try_from(most(&stored)).unwrap();
    assert_eq!(pt(&driver), (3 + x, 3 + x));
    replays(&scenario, &driver);
}

#[rstest]
fn r112_the_celestial_toymaker_hands_over_the_chosen_pile(
    #[values(2, 3)] seats: usize,
    #[values(0, 1)] pick: usize,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("The Celestial Toymaker").unwrap())])
        .library(0, vec![island(), swamp(), plains(), forest()]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    let toymaker = driver.id_of("The Celestial Toymaker").unwrap();
    driver
        .answer(Answer::DeclareAttackers(vec![(
            toymaker,
            mtg_engine::combat::AttackTarget::Player(pid(1)),
        )]))
        .unwrap();
    let mut labels = Vec::new();
    let mut chooser = None;
    settle(&mut driver, |driver, decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            // The Island goes face down, the rest face up.
            let island = options
                .from
                .iter()
                .copied()
                .find(|id| crate::common::name_of(driver.state(), *id) == "Island");
            Some(Answer::ChooseCards(CardsChoice(
                island.into_iter().collect(),
            )))
        }
        DecisionKind::ChooseOption(options) if options.text == "Choose a pile." => {
            chooser = Some(decision.player.index());
            labels.clone_from(&options.options);
            Some(Answer::ChooseOption(OptionChoice(pick)))
        }
        _ => None,
    });
    assert_eq!(chooser, Some(1), "the defending player chooses");
    let text = format!("{labels:?}");
    assert!(
        text.contains("1 face-down card") && !text.contains("Island"),
        "{text}"
    );
    let in_hand = driver.state().zone(Zone::Hand(pid(0))).len() - hand;
    assert_eq!(in_hand, if pick == 0 { 1 } else { 2 });
    assert_eq!(driver.state().zone(Zone::Exile).len(), 3 - in_hand);
    let _ = names(driver.state(), Zone::Exile);
    // One ability grouped cards into piles this turn: each opponent loses 2 at the end step, the defender also took 2.
    driver.idle_until(2, Step::End).unwrap();
    settle(&mut driver, |_, _| None);
    for seat in 1..seats {
        assert_eq!(
            life(&driver, seat),
            if seat == 1 { 16 } else { 18 },
            "seat {seat}"
        );
    }
    assert_eq!(life(&driver, 0), 20);
    replays(&scenario, &driver);
}

#[rstest]
fn r112_raging_river_lets_only_fliers_and_the_chosen_pile_block(#[values(2, 3)] seats: usize) {
    use mtg_cards::keyword::Keyword;
    use mtg_testkit::cards::keyworded;
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Raging River").unwrap()),
                ready(free_creature("Left Raider", 2, 2)),
                ready(free_creature("Right Raider", 2, 2)),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Lefty", 1, 1)),
                ready(free_creature("Righty", 1, 1)),
                ready(keyworded("Bird", 1, 1, &[Keyword::Flying])),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    let raiders = ["Left Raider", "Right Raider"].map(|name| driver.id_of(name).unwrap());
    driver
        .answer(Answer::DeclareAttackers(
            raiders
                .iter()
                .map(|id| (*id, mtg_engine::combat::AttackTarget::Player(pid(1))))
                .collect(),
        ))
        .unwrap();
    let mut separated = Vec::new();
    let mut labelled = Vec::new();
    settle(&mut driver, |driver, decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            separated.push(decision.player.index());
            let names: Vec<String> = options
                .from
                .iter()
                .map(|id| crate::common::name_of(driver.state(), *id))
                .collect();
            assert!(
                !names.contains(&"Bird".to_owned()) && !names.iter().any(|n| n.contains("Raider")),
                "{names:?}"
            );
            Some(Answer::ChooseCards(CardsChoice(vec![
                driver.id_of("Lefty").unwrap(),
            ])))
        }
        DecisionKind::ChooseOption(options)
            if options.options.len() == 2 && decision.player == pid(0) =>
        {
            labelled.push(());
            // The attackers are asked in turn: the first goes left, the second right.
            Some(Answer::ChooseOption(OptionChoice(usize::from(
                labelled.len() == 2,
            ))))
        }
        _ => None,
    });
    assert_eq!(
        (separated, labelled.len()),
        (vec![1], 2),
        "only the defending player divides"
    );
    driver.pass_until(Step::DeclareBlockers).unwrap();
    let DecisionKind::DeclareBlockers(options) = driver.decision().unwrap().kind.clone() else {
        panic!("blockers are asked");
    };
    let can = |name: &str| {
        let id = driver.id_of(name).unwrap();
        let mut blocks: Vec<String> = options
            .blockers
            .iter()
            .find(|option| option.object == id)
            .map(|option| {
                option
                    .can_block
                    .iter()
                    .map(|a| crate::common::name_of(driver.state(), *a))
                    .collect()
            })
            .unwrap_or_default();
        blocks.sort();
        blocks
    };
    let lefty = can("Lefty");
    let righty = can("Righty");
    assert_eq!(lefty.len() + righty.len(), 2, "{lefty:?} {righty:?}");
    assert_ne!(lefty, righty, "each pile blocks one raider");
    assert_eq!(
        can("Bird"),
        vec!["Left Raider".to_owned(), "Right Raider".to_owned()]
    );
    replays(&scenario, &driver);
}

#[rstest]
fn r112_ferris_wheel_holds_a_creature_out_until_a_low_visit_roll(
    #[values(2, 3)] seats: usize,
    #[values(1, 2, 3, 4)] seed: u64,
) {
    use mtg_core::attraction::Lights;
    use mtg_engine::choose::TargetChoice;
    use mtg_engine::phasing::is_phased_out;
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Upkeep)
        .attractions(0, [(card("Ferris Wheel").unwrap(), Lights::of(1..=6))])
        .hand(0, [mtg_testkit::cards::un33_opener("Open One", Some(1))])
        .battlefield(
            1,
            [
                ready(free_creature("Victim", 2, 2)),
                ready(free_creature("Other", 1, 1)),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 30]);
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    cast(&mut driver, "Open One");
    let victim = driver.id_of("Victim").unwrap();
    let other = driver.id_of("Other").unwrap();
    let mut offered: Vec<Vec<mtg_core::ids::ObjectId>> = Vec::new();
    let mut rolls = Vec::new();
    let mut back = None;
    // Each of seat 0's precombat main phases rolls to visit the Wheel (lit at every number).
    for _ in 0..3000 {
        if back.is_some_and(|visits| offered.len() > visits) || driver.state().turn().number() > 40
        {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let events = match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                let legal: Vec<_> = options
                    .legal
                    .iter()
                    .filter_map(|t| t.object().map(|o| o.id))
                    .collect();
                offered.push(legal.clone());
                let pick = if legal.contains(&victim) {
                    victim
                } else {
                    other
                };
                match options
                    .legal
                    .iter()
                    .find(|t| t.object().is_some_and(|o| o.id == pick))
                {
                    Some(target) => driver
                        .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![*target])))
                        .unwrap(),
                    None => driver.answer_passively().unwrap(),
                }
            }
            _ => driver.answer_passively().unwrap(),
        };
        for event in &events {
            if let Event::RolledToVisit { player, result, .. } = event
                && *player == pid(0)
            {
                rolls.push(*result);
                // From the second roll on the Victim is out until a roll of 3 or less brings it back.
                if rolls.len() >= 2 && back.is_none() {
                    if *result <= 3 {
                        assert!(!is_phased_out(driver.state(), victim), "rolls {rolls:?}");
                        back = Some(offered.len());
                    } else {
                        assert!(is_phased_out(driver.state(), victim), "rolls {rolls:?}");
                    }
                }
            }
        }
    }
    assert!(
        !offered.is_empty() && offered[0].contains(&victim),
        "{offered:?}"
    );
    assert!(
        offered.iter().skip(1).all(|legal| !legal.contains(&victim)),
        "{offered:?}"
    );
    assert!(back.is_some(), "a low roll came: {rolls:?}");
    replays(&scenario, &driver);
}
