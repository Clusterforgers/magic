//! Goal: dash is an alternative cost from the hand: the creature has haste and returns to its owner's hand at the next end step only when it was cast for its dash cost; a creature cast for its mana cost stays; it is offered only with the mana to pay it and at sorcery speed; a creature that left by another route (damage, exile, Rest in Peace, a bounce) is not moved again; a copy is no dashed creature; it can be dashed again on the next turn; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{name_of, on_battlefield, pid};
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The life total every seat starts with.
const LIFE: i32 = 20;

/// Passes priority until the stack is empty or a decision other than priority is pending.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 casts `name` for its dash cost (asked beside the mana cost when both can be paid) and lets it resolve.
fn dash(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, "Dash")?;
    }
    pay(driver)?;
    assert!(
        !driver.state().stack().is_empty(),
        "the spell is on the stack"
    );
    settle(driver)
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// The cards called `name` in `zone`.
fn count(state: &GameState, zone: Zone, name: &str) -> usize {
    state
        .zone(zone)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

/// Seat 0 has `cards` in hand and the lands, with every seat holding a library.
fn game(seats: usize, cards: &[&str], lands: Vec<mtg_engine::PresetPermanent>) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .hand_named(0, cards.iter().copied())
            .battlefield(0, lands),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

#[rstest]
#[case::red_vanilla("Mardu Scout", vec![mountain(), mountain()])]
#[case::black_vanilla("Kolaghan Skirmisher", vec![swamp(), swamp(), swamp()])]
#[case::attack_trigger("Mardu Shadowspear", vec![swamp(), swamp()])]
#[case::token_maker("Mardu Strike Leader", vec![swamp(), swamp(), swamp(), swamp()])]
fn a_dashed_creature_has_haste_and_returns_to_the_hand_at_the_end_step(
    #[case] card: &str,
    #[case] lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(game(seats, &[card], lands.into_iter().map(ready).collect()))?;
    dash(&mut driver, card)?;
    let id = on_battlefield(driver.state(), card).expect("the creature entered");
    assert!(has_keyword(driver.state(), id, &Keyword::Haste));
    assert_eq!(count(driver.state(), Zone::Hand(pid(0)), card), 0);
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), card).is_none());
    assert_eq!(count(driver.state(), Zone::Hand(pid(0)), card), 1);
    assert_eq!(count(driver.state(), Zone::Graveyard(pid(0)), card), 0);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn the_dashed_creature_attacks_the_turn_it_enters_with_its_trigger(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(
        seats,
        &["Mardu Shadowspear"],
        vec![ready(swamp()), ready(swamp())],
    ))?;
    dash(&mut driver, "Mardu Shadowspear")?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Mardu Shadowspear", 1)])?;
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    assert_eq!(life(1), LIFE - 2, "the trigger and the damage");
    for seat in 2..seats {
        assert_eq!(life(seat), LIFE - 1, "the trigger reaches each opponent");
    }
    assert_eq!(
        count(driver.state(), Zone::Hand(pid(0)), "Mardu Shadowspear"),
        1
    );
    Ok(())
}

#[rstest]
#[case::mana_cost_of_one_red_card("Mardu Scout", vec![mountain(), mountain()], 1)]
#[case::dash_only_with_other_colors("Mardu Scout", vec![mountain(), swamp()], 1)]
#[case::too_little_mana("Mardu Scout", vec![mountain()], 0)]
#[case::the_wrong_color("Mardu Scout", vec![swamp(), swamp()], 0)]
fn the_dash_cost_is_offered_beside_the_mana_cost_only_when_it_can_be_paid(
    #[case] card: &str,
    #[case] lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] dashes: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(game(2, &[card], lands.into_iter().map(ready).collect()))?;
    if driver.begin_cast(card).is_err() {
        assert_eq!(dashes, 0, "nothing can be paid");
        return Ok(());
    }
    let offered = match &driver.decision()?.kind {
        DecisionKind::ChooseOption(_) => ways(&driver)?,
        _ => vec!["Dash".to_owned()],
    };
    assert_eq!(
        offered.iter().filter(|way| way.starts_with("Dash")).count(),
        dashes,
        "{offered:?}"
    );
    Ok(())
}

#[test]
fn a_creature_cast_for_its_mana_cost_has_no_haste_and_stays() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(
        2,
        &["Mardu Scout"],
        lands(2, mountain).into_iter().collect(),
    ))?;
    driver.begin_cast("Mardu Scout")?;
    choose_way(&mut driver, "Cast for")?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let id = on_battlefield(driver.state(), "Mardu Scout").expect("it entered");
    assert!(!has_keyword(driver.state(), id, &Keyword::Haste));
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    assert_eq!(on_battlefield(driver.state(), "Mardu Scout"), Some(id));
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))), []);
    Ok(())
}

#[test]
fn dash_is_not_offered_at_instant_speed_or_from_the_graveyard() -> Result<(), ScriptError> {
    let mut in_the_graveyard = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, ["Mardu Scout"])
            .battlefield(0, [ready(mountain()), ready(mountain())]),
    )?;
    assert!(in_the_graveyard.begin_cast("Mardu Scout").is_err());
    let mut on_their_turn =
        Driver::from_scenario(game(2, &["Mardu Scout"], lands(2, mountain)).at(1, 1, Step::Main1))?;
    assert!(on_their_turn.begin_cast("Mardu Scout").is_err());
    Ok(())
}

/// How a dashed creature is taken off the battlefield before the end step, and where its card is then.
#[rstest]
#[case::dies_to_damage(Some("Lightning Bolt"), false, Zone::Graveyard(pid(0)))]
#[case::dies_with_rest_in_peace(Some("Lightning Bolt"), true, Zone::Exile)]
#[case::exiled(Some("Swords to Plowshares"), false, Zone::Exile)]
#[case::nothing_else_happens(None, false, Zone::Hand(pid(0)))]
fn a_dashed_creature_that_left_by_another_route_is_not_returned(
    #[case] removal: Option<&str>,
    #[case] rest_in_peace: bool,
    #[case] end: Zone,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut seat_one = vec![ready(mountain()), ready(plains())];
    if rest_in_peace {
        seat_one.push(ready(mtg_testkit::corpus::card("Rest in Peace").unwrap()));
    }
    let scenario = game(seats, &["Mardu Scout"], lands(2, mountain))
        .hand_named(1, removal)
        .battlefield(1, seat_one);
    let mut driver = Driver::from_scenario(scenario)?;
    dash(&mut driver, "Mardu Scout")?;
    if let Some(name) = removal {
        // Seat 0 passes with an empty stack, so seat 1 holds priority.
        driver.pass()?;
        let id = on_battlefield(driver.state(), "Mardu Scout").unwrap();
        let at = mtg_engine::choose::Target::Object(driver.state().object(id).unwrap().reference());
        driver.cast_at(name, &[at])?;
        settle(&mut driver)?;
        assert!(on_battlefield(driver.state(), "Mardu Scout").is_none());
    }
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    let zone_of_card = |zone: Zone| count(driver.state(), zone, "Mardu Scout");
    assert_eq!(zone_of_card(end), 1, "the card is in {end:?}");
    assert_eq!(
        [Zone::Hand(pid(0)), Zone::Graveyard(pid(0)), Zone::Exile]
            .into_iter()
            .map(zone_of_card)
            .sum::<usize>(),
        1,
        "once, and in no other zone"
    );
    assert_eq!(driver.state().stack(), []);
    assert_replays(&driver);
    Ok(())
}

#[test]
fn a_copy_of_the_dashed_creature_has_no_haste_and_stays_after_the_end_step()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(
        2,
        &["Mardu Scout", "Cackling Counterpart"],
        vec![
            ready(mountain()),
            ready(mountain()),
            ready(island()),
            ready(island()),
            ready(island()),
        ],
    ))?;
    dash(&mut driver, "Mardu Scout")?;
    let original = on_battlefield(driver.state(), "Mardu Scout").unwrap();
    let at = driver.target_named("Mardu Scout")?;
    driver.cast_at("Cackling Counterpart", &[at])?;
    settle(&mut driver)?;
    let scouts: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(driver.state(), *id) == "Mardu Scout")
        .collect();
    assert_eq!(scouts.len(), 2, "the original and its copy");
    let copy = scouts.into_iter().find(|id| *id != original).unwrap();
    assert!(has_keyword(driver.state(), original, &Keyword::Haste));
    assert!(!has_keyword(driver.state(), copy, &Keyword::Haste));
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    assert_eq!(on_battlefield(driver.state(), "Mardu Scout"), Some(copy));
    assert_eq!(count(driver.state(), Zone::Hand(pid(0)), "Mardu Scout"), 1);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_returned_creature_can_be_dashed_again_on_the_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let turn = u32::try_from(seats).unwrap() + 1;
    let mut driver = Driver::from_scenario(game(seats, &["Mardu Scout"], lands(2, mountain)))?;
    dash(&mut driver, "Mardu Scout")?;
    driver.idle_until(turn, Step::Main1)?;
    assert_eq!(count(driver.state(), Zone::Hand(pid(0)), "Mardu Scout"), 1);
    dash(&mut driver, "Mardu Scout")?;
    let id = on_battlefield(driver.state(), "Mardu Scout").expect("it entered again");
    assert!(has_keyword(driver.state(), id, &Keyword::Haste));
    driver.idle_until(turn, Step::End)?;
    settle(&mut driver)?;
    assert_eq!(count(driver.state(), Zone::Hand(pid(0)), "Mardu Scout"), 1);
    assert_replays(&driver);
    Ok(())
}
