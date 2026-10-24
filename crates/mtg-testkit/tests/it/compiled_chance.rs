//! Goal: a die with its table, a coin flip with a branch for each result, a vote with its outcome or its count for each name and a repeat for each opponent, as the compiler reads them from Oracle text, play whole at two, three and four seats: the seeded roll or flip picks the row or the branch and no other, each vote is asked of the controller first and then in turn order and decides the outcome by its count, a repeat does its steps once for each opponent, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::compile::compile;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_core::card::CardType;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::rng::GameRng;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, free_creature, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A creature for `{0}` with the abilities the compiler reads from this text; the card must read whole.
fn compiled(name: &str, text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: "12345678-1234-1234-1234-123456789abf".to_owned(),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            type_line: "Creature \u{2014} Elemental".to_owned(),
            oracle_text: text.to_owned(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let compiled = compile(&printed);
    assert!(
        compiled.clauses.iter().all(|clause| clause.read),
        "{compiled:?}"
    );
    let card = compiled.card.expect("the card compiles");
    let mut definition = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_mana_cost(cost(0, &[]));
    for ability in &card.faces[0].abilities {
        definition = definition.with_rules(ability.clone());
    }
    Arc::new(definition)
}

/// Answers every question passively, and the votes by `vote(seat)`, until the stack is empty and priority is asked again.
fn play_out(driver: &mut Driver, vote: impl Fn(usize) -> usize) -> Result<Vec<usize>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseOption(_) => {
                asked.push(decision.player.index());
                driver.answer(Answer::ChooseOption(OptionChoice(vote(
                    decision.player.index(),
                ))))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn stream_at(seats: usize) -> u64 {
    Driver::from_scenario(Scenario::new(seats))
        .unwrap()
        .state()
        .rng_index()
}

/// The first seed whose roll of a die of `sides` faces from the substream `index` is `face`.
fn die_seed(index: u64, sides: u64, face: u64) -> u64 {
    (0u64..100_000)
        .find(|seed| GameRng::substream(*seed, index).below(sides) + 1 == face)
        .unwrap()
}

/// The first seed whose substream `index` wins the flip (an even draw) or loses it.
fn coin_seed(index: u64, win: bool) -> u64 {
    (0u64..100_000)
        .find(|seed| (GameRng::substream(*seed, index).below(2) == 0) == win)
        .unwrap()
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn count(driver: &Driver, seat: usize, prefix: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat) && object.card().name.starts_with(prefix))
        .count()
}

const ROLLER: &str = "When this creature enters, roll a d20.\n1\u{2014}9 | Each player sacrifices a permanent of their choice.\n10\u{2014}19 | Each opponent sacrifices a permanent of their choice.\n20 | Each opponent sacrifices two permanents of their choice.";

/// The table of a die as the compiler reads it: the totals of the rows pick what the roll does.
#[rstest]
#[case::the_first_row(1, 1, 1)]
#[case::the_end_of_the_first_row(9, 1, 1)]
#[case::the_second_row(10, 0, 1)]
#[case::the_end_of_the_second_row(19, 0, 1)]
#[case::the_last_row(20, 0, 2)]
fn a_die_table_read_by_the_compiler_does_the_row_of_the_roll(
    #[values(2, 3, 4)] seats: usize,
    #[case] face: u64,
    #[case] mine_lost: usize,
    #[case] theirs_lost: usize,
) -> Result<(), ScriptError> {
    let start = stream_at(seats);
    let mut scenario = Scenario::new(seats)
        .seed(die_seed(start, 20, face))
        .hand(0, [compiled("Roller", ROLLER)])
        .battlefield(
            0,
            (0..3).map(|n| ready(free_creature(&format!("Mine {n}"), 1, 1))),
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            (0..3).map(|n| ready(free_creature(&format!("Pawn {seat}-{n}"), 1, 1))),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Roller")?;
    play_out(&mut driver, |_| 0)?;
    let rolled = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::DieRolled { sides: 20, .. }))
        .count();
    assert_eq!(rolled, 1);
    // Seat 0 holds the Roller and three creatures; the first row takes one permanent of theirs.
    assert_eq!(
        count(&driver, 0, "Mine") + count(&driver, 0, "Roller"),
        4 - mine_lost,
        "seat 0"
    );
    for seat in 1..seats {
        assert_eq!(count(&driver, seat, "Pawn"), 3 - theirs_lost, "seat {seat}");
    }
    assert_eq!(
        driver.state().rng_index(),
        start + 1,
        "one roll used one draw"
    );
    replays(&driver);
    Ok(())
}

const GAMBLER: &str = "{R}, {T}: Flip a coin. If you win the flip, you gain 3 life. If you lose the flip, each opponent gains 1 life.";

/// A flip with a branch for each result as the compiler reads it.
#[rstest]
fn a_flip_read_by_the_compiler_does_the_branch_of_its_result(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] win: bool,
) -> Result<(), ScriptError> {
    let start = stream_at(seats);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .seed(coin_seed(start, win))
            .battlefield(0, [ready(compiled("Gambler", GAMBLER)), ready(mountain())]),
    )?;
    driver.begin_activation("Gambler")?;
    driver.pay_tapping(&["Mountain"], 0)?;
    play_out(&mut driver, |_| 0)?;
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let base = life(seats - 1);
    assert_eq!(
        life(0) - base,
        if win { 3 } else { -1 },
        "3 for me, or 1 for each opponent"
    );
    for seat in 1..seats {
        assert_eq!(life(seat), base, "the opponents are level");
    }
    let flipped = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(flipped, [win]);
    replays(&driver);
    Ok(())
}

const COUNCIL: &str = "When this creature enters, starting with you, each player votes for time or knowledge. If time gets more votes, draw two cards. If knowledge gets more votes or the vote is tied, you gain 3 life.";

/// A vote with its outcome as the compiler reads it: the controller votes first, then turn order; time needs more votes.
#[rstest]
#[case::two_seats_time(2, &[0, 0], true)]
#[case::two_seats_tied(2, &[0, 1], false)]
#[case::three_seats_time_ahead(3, &[1, 0, 0], true)]
#[case::three_seats_knowledge_ahead(3, &[0, 1, 1], false)]
#[case::four_seats_tied(4, &[0, 0, 1, 1], false)]
#[case::four_seats_time_ahead(4, &[0, 0, 0, 1], true)]
fn a_vote_read_by_the_compiler_is_decided_by_its_count(
    #[case] seats: usize,
    #[case] votes: &[usize],
    #[case] time: bool,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).hand(0, [compiled("Council", COUNCIL)]))?;
    let hand_before = driver.state().zone(Zone::Hand(pid(0))).len();
    let life_before = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Council")?;
    let asked = play_out(&mut driver, |seat| votes[seat])?;
    assert_eq!(
        asked,
        (0..seats).collect::<Vec<_>>(),
        "the controller first, then turn order"
    );
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let life = driver.state().player(pid(0)).unwrap().life();
    if time {
        assert_eq!((hand, life), (hand_before - 1 + 2, life_before));
    } else {
        assert_eq!((hand, life), (hand_before - 1, life_before + 3));
    }
    replays(&driver);
    Ok(())
}

const EACH_VOTE: &str = "When this creature enters, starting with you, each player votes for evidence or bribery. For each evidence vote, draw a card. For each bribery vote, you gain 2 life.";

/// "For each A vote" does its steps once for each player who voted for A.
#[rstest]
#[case::all_evidence(&[0, 0, 0], 3, 0)]
#[case::all_bribery(&[1, 1, 1], 0, 3)]
#[case::split(&[0, 1, 0], 2, 1)]
fn a_count_for_each_name_read_by_the_compiler_does_the_steps_once_for_each_vote(
    #[case] votes: &[usize],
    #[case] cards: usize,
    #[case] bribes: i32,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(3).hand(0, [compiled("Tally", EACH_VOTE)]))?;
    let hand_before = driver.state().zone(Zone::Hand(pid(0))).len();
    let life_before = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Tally")?;
    play_out(&mut driver, |seat| votes[seat])?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand_before - 1 + cards
    );
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        life_before + 2 * bribes
    );
    replays(&driver);
    Ok(())
}

const LEVY: &str = "When this creature enters, for each opponent, you create a 1/1 white Human Soldier creature token.";

/// "For each opponent, you create a token" makes one token for each opponent, for the controller.
#[rstest]
fn a_repeat_for_each_opponent_read_by_the_compiler_makes_a_token_for_each(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [compiled("Levy", LEVY)])
            .battlefield(0, [ready(island())]),
    )?;
    driver.cast("Levy")?;
    play_out(&mut driver, |_| 0)?;
    assert_eq!(count(&driver, 0, "Human Soldier"), seats - 1);
    for seat in 1..seats {
        assert_eq!(count(&driver, seat, "Human Soldier"), 0, "seat {seat}");
    }
    replays(&driver);
    Ok(())
}
