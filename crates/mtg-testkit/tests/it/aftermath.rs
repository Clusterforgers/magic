//! Goal: the half of a split card with aftermath is cast from its owner's graveyard and from no other zone, the other half from a hand and never from the graveyard, a spell of it that was cast from a graveyard is exiled wherever it goes from the stack (resolved, countered, copied, under Rest in Peace) and the half cast from the hand goes to the graveyard where the other half follows, the X of the half is chosen as for any spell, at tables of two, three and four seats, and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{self, keep_targets_until_done};
use common::{name_of, pid};
use mtg_engine::choose::{NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, GameState, replay};
use mtg_testkit::cards::{forest, island, mountain, swamp};
use mtg_testkit::corpus::{self, card};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn in_zone(state: &GameState, zone: Zone, name: &str) -> bool {
    state
        .zone(zone)
        .iter()
        .any(|id| name_of(state, *id) == name)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let state = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// The faces the pending priority decision lists, by name.
fn faces(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority is asked");
    };
    options.faces.iter().map(|f| f.name.clone()).collect()
}

/// Whether the card is among the spells the pending priority decision offers.
fn castable(driver: &Driver, name: &str) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return false;
    };
    options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

fn lands(swamps: usize, mountains: usize, islands: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..swamps)
        .map(|_| ready(swamp()))
        .chain((0..mountains).map(|_| ready(mountain())))
        .chain((0..islands).map(|_| ready(island())))
        .collect()
}

/// Seat 0 holds `hand` and has `graveyard` in its graveyard and the lands; every library is filled.
fn scenario(
    seats: usize,
    hand: &[&str],
    graveyard: &[&str],
    lands: Vec<mtg_engine::PresetPermanent>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .graveyard_named(0, graveyard.iter().copied())
        .battlefield(0, lands)
        .battlefield_named(1, ["Grizzly Bears"]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    scenario
}

/// Seat 0 casts the Ribbons of Cut // Ribbons with X and pays with Swamps, so that other lands are left.
fn cast_ribbons(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver.begin_cast("Cut // Ribbons")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    driver.pay_tapping(&vec!["Swamp"; x as usize + 2], 0)?;
    Ok(())
}

#[rstest]
#[case::cut_ribbons("Cut // Ribbons", "Cut", "Ribbons", lands(2, 2, 0))]
#[case::consign_oblivion("Consign // Oblivion", "Consign", "Oblivion", lands(5, 0, 2))]
#[case::spring_mind("Spring // Mind", "Spring", "Mind", lands(0, 0, 6))]
fn the_half_with_aftermath_is_cast_from_the_graveyard_only_and_the_other_half_from_the_hand_only(
    #[case] card: &str,
    #[case] front: &str,
    #[case] back: &str,
    #[case] lands: Vec<mtg_engine::PresetPermanent>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Spring costs {2}{G}: the Islands pay the blue half and a Forest the green one.
    let mut lands = lands;
    lands.push(ready(forest()));
    lands.push(ready(forest()));
    lands.push(ready(forest()));
    let in_hand = Driver::from_scenario(scenario(seats, &[card], &[], lands.clone()))?;
    assert!(castable(&in_hand, card));
    assert_eq!(faces(&in_hand), [front], "only the front half from a hand");
    let in_graveyard = Driver::from_scenario(scenario(seats, &[], &[card], lands))?;
    assert!(castable(&in_graveyard, card));
    assert_eq!(
        faces(&in_graveyard),
        [back],
        "only the aftermath half from a graveyard"
    );
    Ok(())
}

#[test]
fn a_half_with_aftermath_is_not_offered_without_its_mana() -> Result<(), ScriptError> {
    // Ribbons costs {X}{B}{B}: two Mountains pay for Cut but not for Ribbons.
    let driver = Driver::from_scenario(scenario(2, &[], &["Cut // Ribbons"], lands(0, 2, 0)))?;
    assert!(!castable(&driver, "Cut // Ribbons"));
    assert_eq!(faces(&driver), [] as [String; 0]);
    Ok(())
}

#[rstest]
fn the_half_from_the_graveyard_chooses_x_as_any_spell_does(
    #[values(0, 1, 3)] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || scenario(seats, &[], &["Cut // Ribbons"], lands(5, 0, 0));
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    driver.begin_cast("Cut // Ribbons")?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked: {}", driver.decision()?.kind.name());
    };
    assert_eq!(
        (options.min, options.max),
        (0, Some(3)),
        "five Swamps, two for {{B}}{{B}}"
    );
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    common::casting::pay(&mut driver)?;
    copying::run(&mut driver, seats)?;
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 20 - i32::try_from(x).unwrap());
    }
    assert_eq!(life(&driver, 0), 20);
    assert_replays(&driver, &setup);
    Ok(())
}

/// How the aftermath spell leaves the stack.
#[derive(Debug, Clone, Copy)]
enum Route {
    Resolves,
    Countered,
    Copied,
    RestInPeace,
}

#[rstest]
fn a_spell_cast_from_the_graveyard_is_exiled_wherever_it_goes_from_the_stack(
    #[values(Route::Resolves, Route::Countered, Route::Copied, Route::RestInPeace)] route: Route,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        let copying = matches!(route, Route::Copied);
        let hand: &[&str] = if copying { &["Twincast"] } else { &[] };
        let scenario = scenario(
            seats,
            hand,
            &["Cut // Ribbons"],
            lands(4, 0, if copying { 2 } else { 0 }),
        );
        match route {
            Route::Resolves | Route::Copied => scenario,
            Route::Countered => scenario
                .hand(1, [card("Counterspell").unwrap()])
                .battlefield(1, lands(0, 0, 2)),
            Route::RestInPeace => scenario.battlefield(1, [ready(card("Rest in Peace").unwrap())]),
        }
    };
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    cast_ribbons(&mut driver, 2)?;
    let ribbons = driver.state().stack()[0];
    assert!(
        driver
            .state()
            .object(ribbons)
            .unwrap()
            .cast_from_graveyard(),
        "the spell remembers where it was cast from"
    );
    match route {
        Route::Resolves | Route::RestInPeace => {}
        Route::Countered => {
            let at = Target::Object(driver.state().object(ribbons).unwrap().reference());
            driver.pass()?;
            assert_eq!(driver.decision()?.player, pid(1));
            driver.cast_at("Counterspell", &[at])?;
        }
        Route::Copied => {
            let at = Target::Object(driver.state().object(ribbons).unwrap().reference());
            driver.begin_cast("Twincast")?;
            driver.choose_targets(&[at])?;
            common::casting::pay(&mut driver)?;
        }
    }
    keep_targets_until_done(&mut driver, seats)?;
    let lost = match route {
        Route::Resolves | Route::RestInPeace => 2,
        Route::Countered => 0,
        Route::Copied => 4,
    };
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 20 - lost, "seat {seat}");
    }
    assert!(in_zone(driver.state(), Zone::Exile, "Cut // Ribbons"));
    for seat in 0..seats {
        assert!(!in_zone(
            driver.state(),
            Zone::Graveyard(pid(seat)),
            "Cut // Ribbons"
        ));
    }
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_front_half_goes_to_the_graveyard_and_the_back_half_is_cast_from_there_and_exiled(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        scenario(seats, &["Cut // Ribbons"], &[], lands(4, 2, 0))
            .battlefield_named(1, ["Grizzly Bears"])
    };
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    assert!(!castable_back_half(&driver));
    let bears = driver.target_named("Grizzly Bears")?;
    driver.begin_cast("Cut // Ribbons")?;
    driver.choose_targets(&[bears])?;
    common::casting::pay(&mut driver)?;
    keep_targets_until_done(&mut driver, seats)?;
    assert!(!in_zone(driver.state(), Zone::Battlefield, "Grizzly Bears"));
    assert!(
        in_zone(driver.state(), Zone::Graveyard(pid(0)), "Cut // Ribbons"),
        "a half cast from the hand goes to the graveyard"
    );
    assert!(!in_zone(driver.state(), Zone::Exile, "Cut // Ribbons"));
    assert!(castable_back_half(&driver));
    assert_eq!(faces(&driver), ["Ribbons"]);
    cast_ribbons(&mut driver, 1)?;
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 19);
    assert!(in_zone(driver.state(), Zone::Exile, "Cut // Ribbons"));
    assert!(!in_zone(
        driver.state(),
        Zone::Graveyard(pid(0)),
        "Cut // Ribbons"
    ));
    assert_replays(&driver, &setup);
    Ok(())
}

fn castable_back_half(driver: &Driver) -> bool {
    castable(driver, "Cut // Ribbons") && faces(driver) == ["Ribbons"]
}

#[test]
fn the_cards_are_verified() {
    for name in ["Cut // Ribbons", "Consign // Oblivion", "Spring // Mind"] {
        let support = corpus::corpus().unwrap().support(name).unwrap();
        assert_eq!(
            support.support,
            mtg_cards::support::Support::Verified,
            "{name}"
        );
    }
}
