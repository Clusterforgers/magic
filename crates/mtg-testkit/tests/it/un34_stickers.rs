//! Goal: stickers (CR 123) play at tables of two to four seats: a seat puts a sticker of one of its sheets on a permanent it owns for the sticker's tickets, a name sticker adds its words to the name the game reads (layer 3), an ability sticker gives its ability (layer 6) and a power/toughness sticker sets both (layer 7b); a sticker is used once, one the seat cannot pay for is not offered, the stickers go when the permanent leaves, the other seats are untouched and the log replays to the same state. Claiming the prize of an Attraction triggers its prize ability and "whenever you claim the prize", and each Attraction visited triggers "whenever you visit an Attraction".
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::ability::{Ability, Sticker};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::attraction::Lights;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{
    un33_opener, un34_fan, un34_prize_attraction, un34_sheet, un34_stickerer,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn tickets(count: u32) -> (CounterKind, u32) {
    (CounterKind::Other("ticket".to_owned()), count)
}

/// One sticker of each kind: a name sticker for one ticket, a flying sticker for two and a 5/5 for three.
fn sheet() -> Vec<Sticker> {
    vec![
        Sticker::Name {
            words: "Wicked".to_owned(),
            tickets: 1,
        },
        Sticker::Ability {
            ability: Box::new(Ability::Keyword(Keyword::Flying)),
            tickets: 2,
        },
        Sticker::PowerToughness {
            power: 5,
            toughness: 5,
            tickets: 3,
        },
    ]
}

/// Seat 0 brings one sheet, has `held` tickets, a Bear and a Stickerer in hand; every other seat has a Bear too.
fn table(seats: usize, held: u32) -> Scenario {
    let (kind, count) = tickets(held);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .stickers(0, [un34_sheet("Fun Sheet", sheet())])
        .counters(0, kind, count)
        .hand(0, [un34_stickerer("Stickerer")]);
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, [ready(mtg_testkit::cards::bear())]);
    }
    scenario
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        driver.answer_passively()?;
    }
    Ok(())
}

fn bear_of(driver: &Driver, seat: usize) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().owner() == pid(seat))
        .unwrap()
}

/// Casts the Stickerer and picks the sticker whose words begin with `pick`, when asked.
fn put(driver: &mut Driver, pick: &str) -> Result<(), ScriptError> {
    driver.cast("Stickerer")?;
    while !driver.state().stack().is_empty() {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseOption(options) = &decision.kind {
            let at = options
                .options
                .iter()
                .position(|option| format!("{option:?}").contains(pick))
                .expect("the sticker is offered");
            driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Ok(())
}

#[rstest]
fn a_sticker_changes_the_permanent_it_is_put_on(
    #[values(2, 3, 4)] seats: usize,
    #[values("Wicked", "Flying", "5/5")] pick: &str,
) -> Result<(), ScriptError> {
    let scenario = table(seats, 3);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = bear_of(&driver, 0);
    put(&mut driver, pick)?;
    let state = driver.state();
    let [at] = state.stickers_on(bear) else {
        panic!("one sticker on the Bear");
    };
    assert!(state.sticker_used(*at));
    let paid = state.sticker(*at).unwrap().tickets();
    assert_eq!(
        state.player(pid(0)).unwrap().counters_of(&tickets(0).0),
        3 - paid
    );
    let chars = mtg_engine::characteristics::characteristics(state, bear).unwrap();
    let name = state.name_of(bear).unwrap();
    match pick {
        "Wicked" => assert_eq!(name, "Wicked Bear"),
        "Flying" => {
            assert_eq!(name, "Bear");
            assert!(mtg_engine::keywords::has_keyword(
                state,
                bear,
                &Keyword::Flying
            ));
        }
        _ => assert_eq!((chars.power, chars.toughness), (Some(5), Some(5))),
    }
    for seat in 1..seats {
        let other = bear_of(&driver, seat);
        assert!(driver.state().stickers_on(other).is_empty());
        assert_eq!(driver.state().name_of(other).unwrap(), "Bear");
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
#[case::one_ticket(1, 1)]
#[case::two_tickets(2, 2)]
#[case::none(0, 0)]
fn only_the_stickers_the_seat_can_pay_for_are_offered(
    #[case] held: u32,
    #[case] offered: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, held))?;
    driver.cast("Stickerer")?;
    let mut asked = None;
    while !driver.state().stack().is_empty() {
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            asked = Some(options.options.len());
        }
        driver.answer_passively()?;
    }
    let bear = bear_of(&driver, 0);
    let on = driver.state().stickers_on(bear).len();
    match offered {
        0 => assert_eq!(on, 0, "nothing to pay for, nothing put on"),
        1 => assert_eq!((asked, on), (None, 1), "the one sticker is taken unasked"),
        count => assert_eq!((asked, on), (Some(count), 1)),
    }
    Ok(())
}

#[test]
fn a_sticker_is_used_once_and_leaves_with_the_permanent() -> Result<(), ScriptError> {
    let scenario = table(2, 3).hand(
        0,
        [un34_stickerer("Stickerer"), un34_stickerer("Stickerer")],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = bear_of(&driver, 0);
    put(&mut driver, "Wicked")?;
    put(&mut driver, "Flying")?;
    let state = driver.state();
    assert_eq!(state.stickers_on(bear).len(), 2);
    assert_eq!(state.name_of(bear).unwrap(), "Wicked Bear");
    // The name sticker is used: a third put offers only the 5/5, which the seat cannot pay for any more.
    let used: Vec<_> = state
        .sticker_sheets(pid(0))
        .into_iter()
        .flat_map(|(sheet, card)| {
            (0..mtg_engine::un34_stickers::stickers_of(card).count()).map(move |sticker| {
                mtg_engine::un34_stickers::StickerAt {
                    player: pid(0),
                    sheet,
                    sticker,
                }
            })
        })
        .filter(|at| state.sticker_used(*at))
        .collect();
    assert_eq!(used.len(), 2);
    let mut moved = driver.state().clone();
    mtg_engine::fold::apply_event(
        &mut moved,
        &Event::ZoneChanged {
            object: bear,
            from: Zone::Battlefield,
            to: Zone::Hand(pid(0)),
            cause: mtg_engine::events::ZoneCause::Moved,
        },
    )
    .unwrap();
    assert!(moved.stickers_on(bear).is_empty());
    Ok(())
}

#[rstest]
fn a_visit_that_claims_the_prize_triggers_the_prize_and_the_fans(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let deck = (0..2).map(|n| {
        (
            un34_prize_attraction(&format!("Booth {n}")),
            Lights::of(1..=6),
        )
    });
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Upkeep)
        .attractions(0, deck)
        .hand(0, [un33_opener("Open Two", Some(2))])
        .battlefield(0, [ready(un34_fan("Fan"))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Open Two")?;
    settle(&mut driver)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.pass_until(Step::Main1)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    settle(&mut driver)?;
    let claims = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::PrizeClaimed { .. }))
        .count();
    assert_eq!(
        claims, 2,
        "each Attraction lit up is visited and claims its prize"
    );
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + 2,
        "each prize draws a card"
    );
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        life + 2 * 1 + 2 * 2,
        "the fan gains 1 for each visit and 2 for each prize"
    );
    for seat in 1..seats {
        assert_eq!(driver.state().player(pid(seat)).unwrap().life(), 20);
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
