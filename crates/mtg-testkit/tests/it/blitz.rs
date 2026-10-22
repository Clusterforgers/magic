//! Goal: blitz is an alternative cost from the hand: the creature has haste, draws its controller a card when it dies and is sacrificed at the next end step only when it was cast for its blitz cost; a creature cast for its mana cost has none of it; it is offered only with the mana to pay it; a creature that died or left another way before the end step is not sacrificed again and draws once at most; a copy is no blitzed creature; the creature's own abilities work; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{name_of, on_battlefield, pid};
use mtg_cards::keyword::Keyword;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{forest, island, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

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

/// Answers the orders of triggers and passes priority until nothing is on the stack and a player holds priority.
fn settle_all(driver: &mut Driver) -> Result<(), ScriptError> {
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        || !driver.state().stack().is_empty()
    {
        driver.answer_passively()?;
    }
    Ok(())
}

/// Seat 0 casts `name` for its blitz cost (asked beside the mana cost when both can be paid) and lets it resolve.
fn blitz(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, "Blitz")?;
    }
    pay(driver)?;
    settle(driver)
}

/// Seat 0 casts `name` for its mana cost and lets it resolve.
fn cast_for_mana(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, "Cast for")?;
    }
    pay(driver)?;
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

/// The cards in the hand of seat 0.
fn hand(driver: &Driver) -> usize {
    driver.state().zone(Zone::Hand(pid(0))).len()
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

/// Seat 1 casts `spell` at `name` while seat 0 holds priority with an empty stack, and the spell resolves.
fn opponent_casts_at(driver: &mut Driver, spell: &str, name: &str) -> Result<(), ScriptError> {
    driver.pass()?;
    let at = driver.target_named(name)?;
    driver.cast_at(spell, &[at])?;
    settle(driver)
}

#[rstest]
#[case::treasure_maker("Riveteers Requisitioner", vec![mountain(), mountain(), mountain()], "Treasure")]
#[case::opponents_shrink("Night Clubber", vec![swamp(), swamp(), swamp()], "Night Clubber")]
#[case::menace_attacker("Mayhem Patrol", vec![mountain(), mountain()], "Mayhem Patrol")]
fn a_blitzed_creature_has_haste_draws_when_it_dies_and_is_sacrificed_at_the_end_step(
    #[case] card: &str,
    #[case] lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] _leaves: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(game(seats, &[card], lands.into_iter().map(ready).collect()))?;
    blitz(&mut driver, card)?;
    let id = on_battlefield(driver.state(), card).expect("the creature entered");
    assert!(has_keyword(driver.state(), id, &Keyword::Haste));
    assert_eq!(hand(&driver), 0);
    driver.idle_until(1, Step::End)?;
    settle_all(&mut driver)?;
    assert!(on_battlefield(driver.state(), card).is_none());
    assert_eq!(count(driver.state(), Zone::Graveyard(pid(0)), card), 1);
    assert_eq!(hand(&driver), 1, "it drew a card when it died");
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_creature_cast_for_its_mana_cost_stays_and_draws_nothing_when_it_dies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = game(seats, &["Riveteers Requisitioner"], lands(2, mountain))
        .hand_named(1, ["Lightning Bolt"])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    cast_for_mana(&mut driver, "Riveteers Requisitioner")?;
    let id = on_battlefield(driver.state(), "Riveteers Requisitioner").unwrap();
    assert!(!has_keyword(driver.state(), id, &Keyword::Haste));
    opponent_casts_at(&mut driver, "Lightning Bolt", "Riveteers Requisitioner")?;
    settle_all(&mut driver)?;
    assert_eq!(
        count(
            driver.state(),
            Zone::Graveyard(pid(0)),
            "Riveteers Requisitioner"
        ),
        1
    );
    assert_eq!(hand(&driver), 0, "no card was drawn");
    assert_eq!(count(driver.state(), Zone::Battlefield, "Treasure"), 1);
    Ok(())
}

#[rstest]
fn a_blitzed_creature_that_died_early_drew_once_and_is_not_sacrificed_again(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = game(seats, &["Riveteers Requisitioner"], lands(3, mountain))
        .hand_named(1, ["Lightning Bolt"])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    blitz(&mut driver, "Riveteers Requisitioner")?;
    opponent_casts_at(&mut driver, "Lightning Bolt", "Riveteers Requisitioner")?;
    settle_all(&mut driver)?;
    assert_eq!(hand(&driver), 1, "it drew a card when it died");
    driver.idle_until(1, Step::End)?;
    settle_all(&mut driver)?;
    assert_eq!(hand(&driver), 1, "nothing more was drawn");
    assert_eq!(
        count(
            driver.state(),
            Zone::Graveyard(pid(0)),
            "Riveteers Requisitioner"
        ),
        1
    );
    assert_eq!(driver.state().stack(), []);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::both_ways(3, &["Blitz", "Cast for"])]
#[case::the_mana_cost_only(2, &["Cast for"])]
#[case::nothing(1, &[])]
fn the_blitz_cost_is_offered_beside_the_mana_cost_only_when_it_can_be_paid(
    #[case] mountains: usize,
    #[case] offered: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(
        2,
        &["Riveteers Requisitioner"],
        lands(mountains, mountain),
    ))?;
    if offered.is_empty() {
        assert!(driver.begin_cast("Riveteers Requisitioner").is_err());
        return Ok(());
    }
    driver.begin_cast("Riveteers Requisitioner")?;
    if let [only] = offered {
        assert!(
            !matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)),
            "the only way, {only}, is not asked"
        );
        return Ok(());
    }
    let labels = ways(&driver)?;
    assert_eq!(labels.len(), offered.len(), "{labels:?}");
    for start in offered {
        assert!(
            labels.iter().any(|label| label.starts_with(start)),
            "{labels:?} has {start}"
        );
    }
    Ok(())
}

#[test]
fn blitz_is_not_offered_on_the_turn_of_another_player() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        game(2, &["Riveteers Requisitioner"], lands(3, mountain)).at(1, 1, Step::Main1),
    )?;
    assert!(driver.begin_cast("Riveteers Requisitioner").is_err());
    Ok(())
}

#[test]
fn a_copy_of_the_blitzed_creature_has_no_haste_and_is_not_sacrificed() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(
        2,
        &["Riveteers Requisitioner", "Cackling Counterpart"],
        [mountain, island, island, island, island, island]
            .map(|land| ready(land()))
            .into(),
    ))?;
    blitz(&mut driver, "Riveteers Requisitioner")?;
    let original = on_battlefield(driver.state(), "Riveteers Requisitioner").unwrap();
    let at = driver.target_named("Riveteers Requisitioner")?;
    driver.cast_at("Cackling Counterpart", &[at])?;
    settle(&mut driver)?;
    let copy = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| *id != original && name_of(driver.state(), *id) == "Riveteers Requisitioner")
        .expect("the copy entered");
    assert!(has_keyword(driver.state(), original, &Keyword::Haste));
    assert!(!has_keyword(driver.state(), copy, &Keyword::Haste));
    driver.idle_until(1, Step::End)?;
    settle_all(&mut driver)?;
    assert_eq!(
        on_battlefield(driver.state(), "Riveteers Requisitioner"),
        Some(copy)
    );
    assert_eq!(hand(&driver), 1, "the original drew a card when it died");
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::bounced_and_cast_again(5, Zone::Battlefield, 0)]
#[case::bounced_only(3, Zone::Hand(pid(0)), 1)]
fn a_blitzed_creature_bounced_by_an_opponent_is_not_sacrificed_and_draws_nothing(
    #[case] mountains: usize,
    #[case] end: Zone,
    #[case] hand_size: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = game(
        seats,
        &["Riveteers Requisitioner"],
        lands(mountains, mountain),
    )
    .hand_named(1, ["Cyclonic Rift"])
    .battlefield(1, [ready(island()), ready(island())]);
    let mut driver = Driver::from_scenario(scenario)?;
    blitz(&mut driver, "Riveteers Requisitioner")?;
    opponent_casts_at(&mut driver, "Cyclonic Rift", "Riveteers Requisitioner")?;
    assert!(on_battlefield(driver.state(), "Riveteers Requisitioner").is_none());
    if end == Zone::Battlefield {
        cast_for_mana(&mut driver, "Riveteers Requisitioner")?;
    }
    driver.idle_until(1, Step::End)?;
    settle_all(&mut driver)?;
    assert_eq!(
        count(driver.state(), end, "Riveteers Requisitioner"),
        1,
        "the card is in {end:?}"
    );
    assert_eq!(hand(&driver), hand_size);
    assert_eq!(
        count(
            driver.state(),
            Zone::Graveyard(pid(0)),
            "Riveteers Requisitioner"
        ),
        0
    );
    assert_replays(&driver);
    Ok(())
}
