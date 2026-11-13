//! Goal: the attacks of a turn and of a player's last turn decide what the r55-23 cards allow, at two to four seats:
//! Weathered Sentinels (defender) may attack only the players who attacked its controller during their last turn, and
//! is not offered after no attack or an attack on another player; Fast Forward costs {1} less for each opponent its
//! caster attacked this turn (an opponent attacked twice counts once), and attacks of no combat yet reduce nothing; and the
//! log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn r5523_seat(n: usize) -> PlayerId {
    PlayerId(u64::try_from(n).unwrap())
}

/// Answers passively until the deciding seat is to declare attackers in a turn of seat `seat`.
fn r5523_to_attacks(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..400 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::DeclareAttackers(_))
            && driver.state().turn().active().index() == seat
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck("no attack".to_owned()))
}

/// Seat 1 begins the game's first turn with a Raider; seat 0 controls Weathered Sentinels.
fn r5523_sentinels_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(card("Weathered Sentinels").unwrap())])
        .battlefield(1, [ready(free_creature("Raider", 1, 1))])
}

#[rstest]
#[case::attacked_you(Some(0), Some(vec![1]))]
#[case::near_miss_no_attack(None, None)]
#[case::near_miss_attacked_another(Some(2), None)]
fn r5523_sentinels_attack_only_last_turn_attackers(
    #[values(2, 3, 4)] seats: usize,
    #[case] attacked: Option<usize>,
    #[case] defenders: Option<Vec<usize>>,
) -> Result<(), ScriptError> {
    if attacked == Some(2) && seats == 2 {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(r5523_sentinels_table(seats))?;
    r5523_to_attacks(&mut driver, 1)?;
    match attacked {
        Some(at) => driver.attack(&[("Raider", at)])?,
        None => driver.attack(&[])?,
    };
    r5523_to_attacks(&mut driver, 0)?;
    let sentinels = driver.id_of("Weathered Sentinels")?;
    let DecisionKind::DeclareAttackers(options) = driver.decision()?.kind.clone() else {
        panic!("attackers are declared");
    };
    let offered = options
        .attackers
        .iter()
        .find(|option| option.object == sentinels)
        .map(|option| option.defenders.clone());
    let expected = defenders.map(|seats| {
        seats
            .into_iter()
            .map(|seat| AttackTarget::Player(r5523_seat(seat)))
            .collect::<Vec<_>>()
    });
    assert_eq!(offered, expected);
    if seats > 2 && expected.is_some() {
        assert!(
            driver.attack(&[("Weathered Sentinels", 2)]).is_err(),
            "another player is out of reach"
        );
    }
    if expected.is_some() {
        driver.attack(&[("Weathered Sentinels", 1)])?;
    }
    let setup = r5523_sentinels_table(seats).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

/// Seat 0 begins its turn with three Raiders, Fast Forward in hand and five Mountains.
fn r5523_fast_forward_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Fast Forward").unwrap()])
        .battlefield(
            0,
            [
                ready(free_creature("Raider", 1, 1)),
                ready(free_creature("Raider", 1, 1)),
                ready(free_creature("Raider", 1, 1)),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
}

/// The cost Fast Forward asks as it is cast now.
fn r5523_fast_forward_cost(driver: &mut Driver) -> Result<ManaCost, ScriptError> {
    driver.begin_cast("Fast Forward")?;
    Ok(driver.pay_options()?.cost.clone())
}

#[rstest]
#[case::no_attack(&[], "{4}{R}")]
#[case::one_opponent(&[1], "{3}{R}")]
#[case::one_opponent_twice(&[1, 1], "{3}{R}")]
#[case::two_opponents(&[1, 2], "{2}{R}")]
#[case::three_opponents(&[1, 2, 3], "{1}{R}")]
fn r5523_fast_forward_costs_less_per_opponent_attacked(
    #[values(2, 3, 4)] seats: usize,
    #[case] attacks: &[usize],
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    if attacks.iter().any(|seat| *seat >= seats) {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(r5523_fast_forward_table(seats))?;
    assert_eq!(
        r5523_fast_forward_cost(&mut driver)?,
        ManaCost::parse("{4}{R}").unwrap(),
        "before combat"
    );
    let mut driver = Driver::from_scenario(r5523_fast_forward_table(seats))?;
    r5523_to_attacks(&mut driver, 0)?;
    let raiders: Vec<_> = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Raider")
        })
        .collect();
    let declared: Vec<_> = raiders
        .iter()
        .zip(attacks)
        .map(|(raider, seat)| (*raider, AttackTarget::Player(r5523_seat(*seat))))
        .collect();
    driver.answer(mtg_engine::decision::Answer::DeclareAttackers(declared))?;
    for _ in 0..200 {
        if driver.state().turn().step() == Step::Main2 {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(
        r5523_fast_forward_cost(&mut driver)?,
        ManaCost::parse(cost).unwrap()
    );
    let setup = r5523_fast_forward_table(seats).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
