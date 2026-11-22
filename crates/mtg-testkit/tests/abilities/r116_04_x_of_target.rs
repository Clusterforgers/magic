//! Goal: an X an activated ability's text defines from its target (`CostItem::XIs` over `Target(0)`, CR 601.2b, 601.2f) plays at tables of two to four seats: Reflecting Mirror asks no X, its cost is {X} with X twice the mana value of the target spell, worked out once the target is chosen; a spell with a single target that is the Mirror's controller gets another player as its target, one aimed elsewhere keeps its target, the Mirror is not usable when the doubled mana value can't be paid, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 1 holds a Lightning Bolt and a Mountain; seat 0 controls Reflecting Mirror, a Bear and `lands` Plains.
fn table(seats: usize, lands: usize) -> Scenario {
    let mut mine = vec![ready(card("Reflecting Mirror").unwrap()), ready(bear())];
    mine.extend((0..lands).map(|_| ready(plains())));
    Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, [ready(mountain())])
        .hand(1, [card("Lightning Bolt").unwrap()])
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

/// Whether the deciding player may activate Reflecting Mirror now.
fn mirror_offered(driver: &Driver) -> bool {
    driver
        .offered_abilities("Reflecting Mirror")
        .is_ok_and(|offered| !offered.is_empty())
}

/// Passes priority until the Mirror's controller holds it.
fn to_mirror(driver: &mut Driver) -> Result<(), ScriptError> {
    while driver.decision()?.player != PlayerId::from_index(0) {
        driver.pass()?;
    }
    Ok(())
}

/// Answers until the stack is empty: the target of a Mirror activation is the Bolt, a new target is seat `to`, mana
/// is paid automatically and counted.
fn settle(driver: &mut Driver, to: usize, paid: &mut Vec<u32>) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseTargets(options)
                if options.source == driver.id_of("Reflecting Mirror")? =>
            {
                let bolt = driver.state().zone(Zone::Stack)[0];
                let bolt = driver.state().object(bolt).unwrap().reference();
                driver.choose_targets(&[mtg_engine::choose::Target::Object(bolt)])?;
            }
            DecisionKind::ChooseTargets(_) => {
                let to = driver.target_player(to);
                driver.choose_targets(&[to])?;
            }
            DecisionKind::PayMana(options) => {
                paid.push(options.cost.mana_value());
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

#[rstest]
fn a_bolt_at_the_mirror_turns_to_another_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] at_me: bool,
) -> Result<(), ScriptError> {
    let scenario = table(seats, 2);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let at = if at_me {
        driver.target_player(0)
    } else {
        driver.target_named("Grizzly Bears")?
    };
    driver.cast_at("Lightning Bolt", &[at])?;
    to_mirror(&mut driver)?;
    assert!(
        mirror_offered(&driver),
        "the Mirror is offered with a spell on the stack"
    );
    driver.begin_activation("Reflecting Mirror")?;
    let to = seats - 1;
    let mut paid = Vec::new();
    settle(&mut driver, to, &mut paid)?;
    assert_eq!(paid, vec![2], "X is twice the Bolt's mana value");
    if at_me {
        assert_eq!(
            life(&driver, 0),
            20,
            "the Bolt left the Mirror's controller"
        );
        assert_eq!(life(&driver, to), 17, "another player took it");
    } else {
        assert_eq!(life(&driver, 0), 20);
        assert!(
            driver.state().zone(Zone::Battlefield).len() < 2 + 2 + 1,
            "the Bolt kept its target, the Bear"
        );
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn without_the_doubled_mana_value_the_mirror_does_nothing(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 1))?;
    let me = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[me])?;
    to_mirror(&mut driver)?;
    if mirror_offered(&driver) {
        driver.begin_activation("Reflecting Mirror")?;
        let mut paid = Vec::new();
        settle(&mut driver, 1, &mut paid)?;
        assert!(paid.is_empty(), "X = 2 is not paid with one land: {paid:?}");
    } else {
        settle(&mut driver, 1, &mut Vec::new())?;
    }
    assert_eq!(life(&driver, 0), 17, "the Bolt hit the Mirror's controller");
    Ok(())
}
