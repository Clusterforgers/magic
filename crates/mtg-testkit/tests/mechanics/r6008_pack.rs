//! Goal: the r60-08 pieces run in play at two to four seats: a granted optional move into a library (Pulmonic Sliver)
//! asks the Sliver's controller and follows the answer, and friend or foe (Virtus's Maneuver) has each friend return a
//! creature card and each foe sacrifice a creature, and an attacker may fight Grothama, whose leaving draws each player the
//! damage their sources dealt it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::swamp;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Priority passes until the stack is empty; payments are automatic, a yes-or-no question gets `yes` (and is
/// recorded with its seat), a choice of option gets the next of `options`, and other questions a passive answer.
fn r6008_settle(driver: &mut Driver, yes: bool, options: &[usize]) -> Vec<usize> {
    let mut asked = Vec::new();
    let mut next = options.iter();
    for _ in 0..120 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::YesNo(_) => {
                asked.push(r6008_seat(driver, decision.player));
                driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
            }
            DecisionKind::ChooseOption(_) => {
                let pick = *next.next().expect("an option for every question");
                driver
                    .answer(Answer::ChooseOption(OptionChoice(pick)))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    asked
}

/// The seat of a player.
fn r6008_seat(driver: &Driver, player: mtg_core::ids::PlayerId) -> usize {
    driver
        .state()
        .players()
        .iter()
        .position(|p| p.id() == player)
        .unwrap()
}

/// The names of the cards in a zone of the seat's.
fn r6008_names(
    driver: &Driver,
    seat: usize,
    zone: fn(mtg_core::ids::PlayerId) -> Zone,
) -> Vec<String> {
    let state = driver.state();
    let player = state.players()[seat].id();
    state
        .zone(zone(player))
        .iter()
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .collect()
}

/// The names of the permanents the seat controls.
fn r6008_permanents(driver: &Driver, seat: usize) -> Vec<String> {
    let state = driver.state();
    let player = state.players()[seat].id();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| mtg_engine::characteristics::controller_of(state, **id) == Some(player))
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .collect()
}

#[rstest]
#[case::kept_on_top(true)]
#[case::let_go(false)]
fn r6008_a_destroyed_sliver_may_go_on_top_of_its_library(
    #[values(2, 3, 4)] seats: usize,
    #[case] yes: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(swamp()), ready(swamp())])
        .battlefield(1, [ready(card("Pulmonic Sliver").unwrap())])
        .hand(0, [card("Doom Blade").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Pulmonic Sliver").unwrap();
    driver.cast_at("Doom Blade", &[target]).unwrap();
    let asked = r6008_settle(&mut driver, yes, &[]);
    assert_eq!(asked, [1], "the Sliver's controller decides");
    let library = r6008_names(&driver, 1, Zone::Library);
    let graveyard = r6008_names(&driver, 1, Zone::Graveyard);
    let on_top = library.last().map(String::as_str) == Some("Pulmonic Sliver")
        || library.first().map(String::as_str) == Some("Pulmonic Sliver");
    assert_eq!(on_top, yes);
    assert_eq!(graveyard.contains(&"Pulmonic Sliver".to_owned()), !yes);
}

#[rstest]
#[case::all_foes(&[1, 1, 1, 1])]
#[case::caster_friend(&[0, 1, 1, 1])]
#[case::all_friends(&[0, 0, 0, 0])]
fn r6008_friends_return_a_creature_card_and_foes_sacrifice(
    #[values(2, 3, 4)] seats: usize,
    #[case] picks: &[usize],
) {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(swamp()), ready(swamp()), ready(swamp())])
        .hand(0, [card("Virtus's Maneuver").unwrap()]);
    for seat in 0..seats {
        scenario = scenario
            .battlefield(seat, [ready(card("Grizzly Bears").unwrap())])
            .graveyard(seat, [card("Llanowar Elves").unwrap()]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Virtus's Maneuver").unwrap();
    r6008_settle(&mut driver, false, &picks[..seats]);
    for (seat, pick) in picks[..seats].iter().enumerate() {
        let friend = *pick == 0;
        let hand = r6008_names(&driver, seat, Zone::Hand);
        let field = r6008_permanents(&driver, seat);
        assert_eq!(
            hand.contains(&"Llanowar Elves".to_owned()),
            friend,
            "seat {seat}"
        );
        assert_eq!(
            field.contains(&"Grizzly Bears".to_owned()),
            friend,
            "seat {seat}"
        );
    }
}

#[rstest]
#[case::fought(true, 2)]
#[case::declined(false, 0)]
fn r6008_an_attacker_may_fight_grothama_and_its_controller_draws_that_damage(
    #[values(2, 3, 4)] seats: usize,
    #[case] fight: bool,
    #[case] drawn: usize,
) {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Grothama, All-Devouring").unwrap()),
                ready(card("Grizzly Bears").unwrap()),
                ready(swamp()),
                ready(swamp()),
            ],
        )
        .hand(0, [card("Doom Blade").unwrap()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| swamp()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Grizzly Bears", 1)]).unwrap();
    r6008_settle(&mut driver, fight, &[]);
    let graveyard = r6008_names(&driver, 0, Zone::Graveyard);
    assert_eq!(graveyard.contains(&"Grizzly Bears".to_owned()), fight);
    driver.pass_until(Step::Main2).unwrap();
    let before: Vec<usize> = (0..seats)
        .map(|seat| r6008_names(&driver, seat, Zone::Hand).len())
        .collect();
    let target = driver.target_named("Grothama, All-Devouring").unwrap();
    driver.cast_at("Doom Blade", &[target]).unwrap();
    r6008_settle(&mut driver, false, &[]);
    for seat in 0..seats {
        let now = r6008_names(&driver, seat, Zone::Hand).len();
        // Seat 0 cast Doom Blade from its hand, then drew the damage its Bears dealt to Grothama.
        let want = if seat == 0 {
            before[0] - 1 + drawn
        } else {
            before[seat]
        };
        assert_eq!(now, want, "seat {seat}");
    }
}
