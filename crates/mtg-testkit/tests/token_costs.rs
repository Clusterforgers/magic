//! Goal: Clue (`{2}`, sacrifice: draw a card) and Food (`{2}`, `{T}`, sacrifice: gain 3 life) are activated at tables of two, three and four seats through the payment decision of an ability: offered only while the mana can be paid, paid with the lands of the player (never the token itself), backed out of for free, with the sacrifice and the tap among the events that announce the ability and the mana after them, so the token has ceased to exist when its ability waits on the stack and the ability still resolves; the log replays to the live state and an engine rebuilt at each decision asks the same thing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Makes a Food when it enters.
fn cook() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Named":"food"}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Cook")
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules),
    )
}

fn resolve_the_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// The token named `name` on the battlefield.
fn token_named(driver: &Driver, name: &str) -> ObjectId {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.kind() == ObjectKind::Token && o.card().name == name)
        })
        .expect("the token is on the battlefield")
}

/// The position of the first event that satisfies `wanted`.
fn position(events: &[Event], wanted: impl Fn(&Event) -> bool) -> usize {
    events.iter().position(wanted).expect("the event is there")
}

#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// Seat 0 controls Tireless Tracker and `lands` Forests and holds a Forest to play, with Islands to draw.
fn clue_table(seats: usize, lands: usize) -> Scenario {
    let mut mine = vec![ready(card("Tireless Tracker").unwrap())];
    mine.extend((0..lands).map(|_| ready(forest())));
    Scenario::new(seats)
        .hand(0, [forest()])
        .battlefield(0, mine)
        .library(0, [island(), island(), island()])
}

/// Plays the Forest, which makes a Clue, and returns the Clue.
fn make_a_clue(driver: &mut Driver) -> Result<ObjectId, ScriptError> {
    driver.play_land("Forest")?;
    resolve_the_stack(driver)?;
    Ok(token_named(driver, "Clue"))
}

#[rstest]
fn a_clue_is_paid_with_two_lands_and_sacrificed_to_draw_a_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = clue_table(seats, 2).setup().unwrap();
    let mut driver = Driver::from_scenario(clue_table(seats, 2))?;
    let clue = make_a_clue(&mut driver)?;
    let hands: Vec<usize> = (0..seats).map(|seat| hand(&driver, seat)).collect();

    driver.begin_activation("Clue")?;
    let options = driver.pay_options()?;
    assert_eq!(options.cost.to_string(), "{2}");
    assert!(options.can_cancel, "nothing is paid until the payment is");
    assert_eq!(
        options
            .sources
            .iter()
            .filter(|s| s.source.object == clue)
            .count(),
        0,
        "the Clue is no source of its own mana"
    );
    assert_eq!(options.sources.len(), 3, "the three Forests");

    let events = driver.pay_tapping(&["Forest", "Forest"], 0)?;
    // The sacrifice announces the ability with the activation; the mana is paid after.
    let sacrificed = position(
        &events,
        |e| matches!(e, Event::ZoneChanged { object, cause: ZoneCause::Sacrificed, .. } if *object == clue),
    );
    let activated = position(&events, |e| matches!(e, Event::AbilityActivated { .. }));
    let on_stack = position(&events, |e| matches!(e, Event::AbilityPutOnStack(_)));
    let first_tap = position(&events, |e| matches!(e, Event::PermanentTapped { .. }));
    let spent = position(&events, |e| matches!(e, Event::ManaSpent { .. }));
    assert!(
        sacrificed < activated && activated < on_stack && on_stack < first_tap && first_tap < spent
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::PermanentTapped { object } if *object == clue)),
        "a Clue has no {{T}}"
    );
    assert!(driver.state().has_ceased(clue));
    // The Tracker's trigger for the sacrifice goes on the stack above the ability of the Clue, which waits below it.
    let [ability, trigger] = driver.state().stack() else {
        panic!("the ability and the Tracker's trigger wait on the stack");
    };
    let ability = driver.state().object(*ability).unwrap();
    assert_eq!(ability.kind(), ObjectKind::Ability);
    assert_eq!(name_of(driver.state(), ability.id()), "Clue");
    assert_eq!(name_of(driver.state(), *trigger), "Tireless Tracker");

    resolve_the_stack(&mut driver)?;
    assert_eq!(hand(&driver, 0), hands[0] + 1, "the Clue drew a card");
    for (seat, before) in hands.iter().enumerate().skip(1) {
        assert_eq!(hand(&driver, seat), *before, "nobody else drew");
    }
    assert_replays(&driver, &setup)
}

#[rstest]
fn backing_out_of_a_clue_activation_is_free(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = clue_table(seats, 2).setup().unwrap();
    let mut driver = Driver::from_scenario(clue_table(seats, 2))?;
    let clue = make_a_clue(&mut driver)?;
    let (hash, logged) = (driver.state().state_hash(), driver.engine().events().len());
    driver.begin_activation("Clue")?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.state().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), logged, "no event");
    assert!(driver.state().object(clue).is_some() && !driver.state().has_ceased(clue));
    // And it can still be paid afterwards, automatically.
    driver.begin_activation("Clue")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(driver.state().has_ceased(clue));
    resolve_the_stack(&mut driver)?;
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_clue_is_not_offered_without_the_two_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Playing the one Forest leaves a single land: {2} cannot be paid.
    let mut driver = Driver::from_scenario(clue_table(seats, 0))?;
    make_a_clue(&mut driver)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert_eq!(options.abilities.len(), 0);
    assert!(driver.begin_activation("Clue").is_err());
    Ok(())
}

/// Seat 0 holds a Cook and controls three Forests.
fn food_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .hand(0, [cook()])
        .battlefield(0, [ready(forest()), ready(forest()), ready(forest())])
}

#[rstest]
fn food_is_paid_with_two_lands_tapped_and_sacrificed_to_gain_three_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = food_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(food_table(seats))?;
    driver.cast_tapping("Cook", &["Forest"])?;
    resolve_the_stack(&mut driver)?;
    let food = token_named(&driver, "Food");
    let lives: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();

    driver.begin_activation("Food")?;
    let options = driver.pay_options()?;
    assert_eq!(options.cost.to_string(), "{2}");
    assert_eq!(options.sources.len(), 2, "the two Forests that are left");
    let events = driver.pay_tapping(&["Forest", "Forest"], 0)?;
    let tapped = position(
        &events,
        |e| matches!(e, Event::PermanentTapped { object } if *object == food),
    );
    let sacrificed = position(
        &events,
        |e| matches!(e, Event::ZoneChanged { object, cause: ZoneCause::Sacrificed, .. } if *object == food),
    );
    let activated = position(&events, |e| matches!(e, Event::AbilityActivated { .. }));
    let spent = position(&events, |e| matches!(e, Event::ManaSpent { .. }));
    assert!(tapped < sacrificed && sacrificed < activated && activated < spent);
    assert!(driver.state().has_ceased(food));

    resolve_the_stack(&mut driver)?;
    assert_eq!(life(&driver, 0), lives[0] + 3);
    for (seat, before) in lives.iter().enumerate().skip(1) {
        assert_eq!(life(&driver, seat), *before);
    }
    assert_replays(&driver, &setup)
}

#[rstest]
fn food_cannot_be_paid_for_twice_from_one_pool_of_lands(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Three Forests pay the Cook and one Food; a second Food has nothing left to pay with.
    let mut driver = Driver::from_scenario(food_table(seats))?;
    driver.cast_tapping("Cook", &["Forest"])?;
    resolve_the_stack(&mut driver)?;
    driver.begin_activation("Food")?;
    driver.pay_tapping(&["Forest", "Forest"], 0)?;
    resolve_the_stack(&mut driver)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert_eq!(
        options.abilities.len(),
        0,
        "the Food is gone and so is the mana"
    );
    Ok(())
}
