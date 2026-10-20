//! Goal: an activated ability that costs mana is offered only when the whole price can be paid (the lands it needs in the colours it needs, and an untapped, not summoning-sick source for a `{T}`), asks for its targets and then its payment, can be backed out of at the payment without a trace, taps the lands and the source and puts the ability on the stack with its source as it was, and resolves; the active seat and any other seat can do it at tables of two, three and four seats; a rebuilt engine asks for priority again at a half-finished activation, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{name_of, pid};
use mtg_cards::cost::{Cost, CostItem};
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::pay_cards::{fountain, mana_pinger, price};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

fn pass_until_held_by(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(seat)
        || !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

fn passes_until_done(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    options
        .abilities
        .iter()
        .map(|ability| name_of(driver.state(), ability.object))
        .collect()
}

fn tapped(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .filter(|id| state.object(**id).unwrap().permanent().unwrap().tapped())
        .count()
}

/// Seat `who` controls a Pinger and `lands` (the others nothing); the seat is the one holding priority.
fn pinger_table(seats: usize, who: usize, lands: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(
            who,
            [
                vec![ready(mana_pinger("Pinger"))],
                (0..lands).map(|_| ready(mountain())).collect(),
            ]
            .concat(),
        ),
    )?;
    pass_until_held_by(&mut driver, who)?;
    Ok(driver)
}

#[rstest]
fn a_pinger_is_activated_by_paying_one_with_a_land_and_tapping_itself(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] who: usize,
) -> Result<(), ScriptError> {
    let mut driver = pinger_table(seats, who, 2)?;
    assert_eq!(offered(&driver), ["Pinger"]);
    driver.begin_activation("Pinger")?;
    // The targets come first, then the payment.
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseTargets(_)
    ));
    let foe = driver.target_player(seats - 1);
    driver.choose_targets(&[foe])?;
    assert_eq!(driver.decision()?.player, pid(who));
    let options = driver.pay_options()?.clone();
    assert_eq!(options.cost.to_string(), "{1}");
    assert!(options.can_cancel);
    assert_eq!(
        options.sources.len(),
        2,
        "the Pinger is not one of the sources"
    );
    assert_eq!(
        options.spell,
        driver.id_of("Pinger")?,
        "the payment is for the Pinger"
    );
    let events = driver.pay_tapping(&["Mountain"], 0)?;
    let story: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentTapped { object } if *object == driver.id_of("Pinger").unwrap() => {
                Some("tap the Pinger")
            }
            Event::AbilityActivated { .. } => Some("activated"),
            Event::AbilityPutOnStack(_) => Some("on the stack"),
            Event::PermanentTapped { .. } => Some("tap a land"),
            Event::ManaAdded { .. } => Some("mana"),
            Event::ManaSpent { .. } => Some("spent"),
            _ => None,
        })
        .collect();
    assert_eq!(
        story,
        [
            "tap the Pinger",
            "activated",
            "on the stack",
            "tap a land",
            "mana",
            "spent"
        ]
    );
    assert_eq!(
        (tapped(&driver, "Pinger"), tapped(&driver, "Mountain")),
        (1, 1)
    );
    // The ability is on the stack and resolves with the source tapped.
    assert_eq!(driver.state().stack().len(), 1);
    passes_until_done(&mut driver)?;
    assert_eq!(life(&driver, seats - 1), 19);
    Ok(())
}

#[rstest]
#[case::no_land(0, "Pinger", false)]
#[case::one_land(1, "Pinger", true)]
fn a_pinger_is_offered_only_with_a_land_to_pay_with(
    #[case] lands: usize,
    #[case] name: &str,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let driver = pinger_table(3, 0, lands)?;
    assert_eq!(offered(&driver).contains(&name.to_owned()), expected);
    Ok(())
}

#[test]
fn a_creature_that_is_summoning_sick_cannot_pay_a_tap_cost_whatever_its_player_has()
-> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            sick(mana_pinger("Pinger")),
            ready(mountain()),
            ready(mountain()),
        ],
    ))?;
    assert!(offered(&driver).is_empty());
    Ok(())
}

#[rstest]
#[case::forests_only(&["Forest", "Forest"], false)]
#[case::an_island_and_a_forest(&["Island", "Forest"], true)]
#[case::an_island_alone(&["Island"], false)]
fn the_colour_of_the_price_decides_whether_a_fountain_is_offered(
    #[case] lands: &[&str],
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let land = |name: &str| ready(if name == "Island" { island() } else { forest() });
    let mut driver = Driver::from_scenario(
        Scenario::new(3).battlefield(
            0,
            [
                vec![ready(fountain("Fountain", price(1, &[Color::Blue])))],
                lands.iter().map(|l| land(l)).collect(),
            ]
            .concat(),
        ),
    )?;
    assert_eq!(offered(&driver).contains(&"Fountain".to_owned()), expected);
    if expected {
        driver.begin_activation("Fountain")?;
        // Paying with the Forest alone would not be {1}{U}.
        assert!(driver.pay_tapping(&["Forest"], 0).is_err());
        driver.pay_tapping(&["Island", "Forest"], 0)?;
        passes_until_done(&mut driver)?;
        assert_eq!(life(&driver, 0), 22);
        assert_eq!(
            (tapped(&driver, "Island"), tapped(&driver, "Forest")),
            (1, 1)
        );
        assert_eq!(tapped(&driver, "Fountain"), 0, "no {{T}} in its cost");
    }
    Ok(())
}

#[test]
fn backing_out_of_an_activation_at_the_payment_leaves_no_trace() -> Result<(), ScriptError> {
    let mut driver = pinger_table(2, 0, 1)?;
    let before = driver.engine().events().len();
    let hash = driver.engine().state_hash();
    driver.begin_activation("Pinger")?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.engine().events().len(), before, "nothing was logged");
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(
        offered(&driver),
        ["Pinger"],
        "the ability can be activated again"
    );
    Ok(())
}

#[test]
fn a_free_tap_ability_still_needs_no_payment() -> Result<(), ScriptError> {
    let chalice = fountain("Chalice", Cost(vec![CostItem::Tap]));
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(chalice)]))?;
    driver.begin_activation("Chalice")?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    passes_until_done(&mut driver)?;
    assert_eq!(life(&driver, 0), 22);
    Ok(())
}

#[test]
fn an_engine_rebuilt_at_a_half_finished_activation_asks_for_priority_again_and_the_log_replays()
-> Result<(), ScriptError> {
    let scenario = Scenario::new(3).battlefield(
        0,
        [
            ready(mana_pinger("Pinger")),
            ready(mountain()),
            ready(mountain()),
        ],
    );
    let setup: GameSetup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.decision()?.clone();
    driver.begin_activation("Pinger")?;
    let foe = driver.target_player(2);
    driver.choose_targets(&[foe])?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    // The activation is outside the state until it is paid for: a rebuilt engine stands at the priority before it.
    let resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec())?;
    assert_eq!(resumed.decision(), Some(&before));
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let mut resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec())?;
    assert_eq!(resumed.state_hash(), driver.engine().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    // Both finish the ability the same way.
    let pass = Answer::Priority(PriorityAction::Pass);
    let mut points = vec![(driver.engine().events().len(), driver.engine().state_hash())];
    while !driver.state().stack().is_empty() {
        let live = driver.answer(pass.clone())?;
        assert_eq!(resumed.apply(pass.clone())?, live);
        points.push((driver.engine().events().len(), driver.engine().state_hash()));
    }
    assert_eq!(resumed.state_hash(), driver.engine().state_hash());
    let log = driver.engine().events().to_vec();
    for (length, hash) in points {
        assert_eq!(
            replay(&setup, &log[..length])?.state_hash(),
            hash,
            "after {length} events"
        );
    }
    assert_eq!(life(&driver, 2), 19);
    Ok(())
}
