//! Goal: a mill puts the top cards of a library into its owner's graveyard, top card first and all in one step, at tables of two, three and four seats: only the library named loses cards, a library shorter than the count gives what it has and its owner does not lose until they draw, the cards that follow become the new top, the moves are `ZoneChanged` with the cause `Milled` in the order of the library, and what watches the graveyard sees each card go.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{names, pid, spell};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A library of `size` cards, `Card 0` on top.
fn library(size: usize) -> Vec<Arc<CardDefinition>> {
    (0..size)
        .map(|n| free_creature(&format!("Card {n}"), 1, 1))
        .collect()
}

/// An instant for `{0}`: the target player mills `count` cards.
fn grind(count: u32) -> Arc<CardDefinition> {
    spell(
        "Grind",
        CardType::Instant,
        r#"[{"what":"Player"}]"#,
        &format!(r#"[{{"Mill":{{"who":{{"Target":0}},"count":{count}}}}}]"#),
    )
}

/// Seat 0 holds `spell`; every seat has a library of `size` cards.
fn table(seats: usize, spell: Arc<CardDefinition>, size: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats).hand(0, [spell]), |scenario, seat| {
        scenario.library(seat, library(size))
    })
}

fn resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

/// The names in a library from the top down.
fn top_down(state: &GameState, seat: usize) -> Vec<String> {
    let mut cards = names(state, Zone::Library(pid(seat)));
    cards.reverse();
    cards
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[rstest]
#[case::the_caster_mills_itself(0)]
#[case::an_opponent(1)]
#[case::a_player_further_round(3)]
fn only_the_library_named_loses_its_top_cards_in_order(
    #[case] target: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let target = target % seats;
    let mut driver = Driver::from_scenario(table(seats, grind(3), 6))?;
    let begun = driver.engine().events().len();
    driver.cast_at("Grind", &[driver.target_player(target)])?;
    resolve_all(&mut driver)?;
    for seat in 0..seats {
        let (library, graveyard) = if seat == target {
            (
                strings(&["Card 3", "Card 4", "Card 5"]),
                strings(&["Card 0", "Card 1", "Card 2"]),
            )
        } else {
            (
                strings(&["Card 0", "Card 1", "Card 2", "Card 3", "Card 4", "Card 5"]),
                Vec::new(),
            )
        };
        assert_eq!(top_down(driver.state(), seat), library, "library of {seat}");
        // The first card milled goes first, so it lies under the others.
        let mut got = names(driver.state(), Zone::Graveyard(pid(seat)));
        if seat == 0 {
            // The spell itself rests in its caster's graveyard, after what it milled.
            assert_eq!(got.last().map(String::as_str), Some("Grind"));
            got.retain(|name| name != "Grind");
        }
        assert_eq!(got, graveyard, "graveyard of {seat}");
        assert!(driver.state().is_alive(pid(seat)));
    }
    // One move for each card, in the order of the library, with the cause, and all of them before one step is done.
    let moves: Vec<(Zone, Zone, ZoneCause, String)> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            Event::ZoneChanged {
                object,
                from,
                to,
                cause: ZoneCause::Milled,
            } => Some((
                *from,
                *to,
                ZoneCause::Milled,
                driver.state().object(*object).unwrap().card().name.clone(),
            )),
            _ => None,
        })
        .collect();
    let owner = pid(target);
    let want: Vec<_> = (0..3)
        .map(|n| {
            (
                Zone::Library(owner),
                Zone::Graveyard(owner),
                ZoneCause::Milled,
                format!("Card {n}"),
            )
        })
        .collect();
    assert_eq!(moves, want);
    // They are one step: nothing else is logged between the first move and the last.
    let at: Vec<usize> = driver.engine().events()[begun..]
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            matches!(
                event,
                Event::ZoneChanged {
                    cause: ZoneCause::Milled,
                    ..
                }
            )
        })
        .map(|(at, _)| at)
        .collect();
    assert!(at.windows(2).all(|pair| pair[1] == pair[0] + 1), "{at:?}");
    Ok(())
}

#[rstest]
#[case::two_cards_for_three(2, 3, &["Card 0", "Card 1"])]
#[case::exactly_enough(3, 3, &["Card 0", "Card 1", "Card 2"])]
#[case::an_empty_library(0, 3, &[])]
fn a_library_that_is_short_gives_what_it_has_and_its_owner_loses_only_by_drawing(
    #[case] size: usize,
    #[case] count: u32,
    #[case] milled: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [grind(count)])
            .library(1, library(size)),
    )?;
    driver.cast_at("Grind", &[driver.target_player(1)])?;
    resolve_all(&mut driver)?;
    driver.expect_zone(Zone::Graveyard(pid(1)), milled)?;
    let remaining: Vec<String> = (milled.len()..size).map(|n| format!("Card {n}")).collect();
    assert_eq!(top_down(driver.state(), 1), remaining);
    assert!(driver.state().is_alive(pid(1)), "milling is not drawing");
    assert_eq!(driver.state().result(), None);
    Ok(())
}

#[rstest]
fn the_card_that_follows_the_mill_is_the_next_one_drawn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let grind_and_draw = spell(
        "Grind and Draw",
        CardType::Instant,
        r#"[{"what":"Player"}]"#,
        r#"[{"Mill":{"who":{"Target":0},"count":2}},{"Draw":{"who":{"Target":0}}}]"#,
    );
    let mut driver = Driver::from_scenario(table(seats, grind_and_draw, 5))?;
    driver.cast_at("Grind and Draw", &[driver.target_player(1)])?;
    resolve_all(&mut driver)?;
    driver.expect_zone(Zone::Graveyard(pid(1)), &["Card 0", "Card 1"])?;
    driver.expect_zone(Zone::Hand(pid(1)), &["Card 2"])?;
    assert_eq!(top_down(driver.state(), 1), strings(&["Card 3", "Card 4"]));
    Ok(())
}

/// A permanent that gains its controller 1 life whenever a card its controller owns goes from a library to a graveyard.
fn ghoul() -> Arc<CardDefinition> {
    let watching: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"OwnedBy":"You"}],"from":"Library","to":"Graveyard"}},
            "effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Ghoul")
            .with_type(CardType::Artifact)
            .with_rules(watching),
    )
}

#[rstest]
#[case::its_own_cards_are_milled(0, 3)]
#[case::an_opponents_cards_are_not_its_own(1, 0)]
fn what_watches_a_graveyard_sees_each_card_milled(
    #[case] target: usize,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, grind(3), 6).battlefield(0, [ready(ghoul())]))?;
    driver.cast_at("Grind", &[driver.target_player(target)])?;
    resolve_all(&mut driver)?;
    driver.expect_life(0, 20 + gained)
}
