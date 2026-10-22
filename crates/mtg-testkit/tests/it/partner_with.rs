//! Goal: Partner with plays as its card says at 2 to 6 seats: when the creature enters, its controller chooses a target player, who alone is asked whether to put the named card into their hand from their library and alone may identify it while they choose (no other seat is shown it); a yes puts the card into the hand and shuffles that library, a no leaves the library as it was but shuffled, a library without the card is shuffled without a question, and no other library is touched.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::sight::offered_cards;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const ROCKSTEADY: &str = "Rocksteady, Mutant Marauder";
const PARTNER: &str = "Bebop, Skull & Crossbones";

/// Every seat's library from the top down; the partner is the third card when the seat has it.
fn library(has_partner: bool) -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
    let mut cards = vec![mountain(), mountain()];
    if has_partner {
        cards.push(card(PARTNER).unwrap());
    }
    cards.extend((0..5).map(|_| mountain()));
    cards
}

/// Seat 0 holds Rocksteady with the Forests that pay for it; the seats in `with_partner` have Bebop in their library.
fn table(seats: usize, with_partner: &[usize]) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(ROCKSTEADY).unwrap()])
        .battlefield(0, (0..3).map(|_| ready(forest())));
    for seat in 0..seats {
        scenario = scenario.library(seat, library(with_partner.contains(&seat)));
    }
    scenario
}

fn in_zone(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
        .collect()
}

/// What happened: whether the target was asked, and the partner it was offered while it chose.
struct Played {
    asked_yes_no: bool,
    offered: Option<(PlayerId, Vec<ObjectId>)>,
    unseen_by_others: bool,
}

/// Casts Rocksteady, targets `target` with the trigger, answers the yes/no question with `yes` and picks the partner.
fn play(
    driver: &mut Driver,
    seats: usize,
    target: usize,
    yes: bool,
) -> Result<Played, ScriptError> {
    driver.cast_tapping(ROCKSTEADY, &["Forest", "Forest", "Forest"])?;
    let mut played = Played {
        asked_yes_no: false,
        offered: None,
        unseen_by_others: true,
    };
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_player(target);
                driver.choose_targets(&[target])?;
            }
            DecisionKind::YesNo(_) => {
                played.asked_yes_no = true;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(options) => {
                played.offered = offered_cards(driver.state());
                for seat in (0..seats).map(pid) {
                    let sees = options
                        .from
                        .iter()
                        .any(|id| driver.state().is_shown_to(seat, *id));
                    let may = played.offered.as_ref().is_some_and(|(who, _)| *who == seat);
                    played.unseen_by_others &= !sees || may;
                }
                driver.answer(Answer::ChooseCards(CardsChoice(options.from.clone())))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(played)
}

#[rstest]
#[case::it_fetches_its_own_partner(0, true, true)]
#[case::an_opponent_fetches_theirs(1, true, true)]
#[case::the_target_declines(0, true, false)]
fn the_target_player_may_put_the_partner_into_their_hand(
    #[values(2, 3, 6)] seats: usize,
    #[case] target: usize,
    #[case] has_partner: bool,
    #[case] yes: bool,
) -> Result<(), ScriptError> {
    let with = if has_partner { vec![target] } else { vec![] };
    let mut driver = Driver::from_scenario(table(seats, &with))?;
    let before: Vec<usize> = (0..seats)
        .map(|seat| driver.state().zone(Zone::Library(pid(seat))).len())
        .collect();
    let played = play(&mut driver, seats, target, yes)?;
    assert!(played.asked_yes_no);
    let hand = in_zone(&driver, Zone::Hand(pid(target)), PARTNER);
    let library = in_zone(&driver, Zone::Library(pid(target)), PARTNER);
    assert_eq!(
        (hand.len(), library.len()),
        (usize::from(yes), usize::from(!yes))
    );
    assert_eq!(
        driver.state().zone(Zone::Library(pid(target))).len(),
        before[target] - usize::from(yes)
    );
    if yes {
        let (who, cards) = played.offered.expect("the target was offered the partner");
        assert_eq!((who, cards.len()), (pid(target), 1));
        assert!(
            played.unseen_by_others,
            "no other seat was shown the library"
        );
    }
    for seat in (0..seats).filter(|seat| *seat != target) {
        assert_eq!(
            driver.state().zone(Zone::Library(pid(seat))).len(),
            before[seat],
            "seat {seat}'s library is not touched"
        );
    }
    Ok(())
}

#[rstest]
fn a_library_without_the_partner_is_shuffled_and_asks_nothing_more(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[]))?;
    let played = play(&mut driver, seats, 0, true)?;
    assert!(played.offered.is_none());
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), PARTNER).len(), 0);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 7);
    Ok(())
}
