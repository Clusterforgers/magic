//! Goal: an enchantment that becomes an Aura with enchant creature as it enters (Cloudform, Lightform) manifests the top card of its controller's library and attaches itself to it at tables of two to four seats, the manifested creature has what the Aura grants, the Aura stays a Aura of the card for good, an empty library leaves it unattached and put into the graveyard (CR 704.5m), and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::card::Subtype;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::FaceDown;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{free_creature, island, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus};
use rstest::rstest;

use crate::common::casting::{lands, pay};
use crate::common::pid;

/// Seat 0 holds `card` with three lands of its colour; every library holds `cards` plain creatures.
fn table(seats: usize, card: &str, cards: usize) -> Scenario {
    let land = if card == "Cloudform" { island } else { plains };
    let library = || (0..cards).map(|i| free_creature(&format!("Pad {i}"), 1, 1));
    (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [corpus::card(card).unwrap()])
            .battlefield(0, lands(3, land)),
        |scenario, seat| scenario.library(seat, library()),
    )
}

/// Casts the card and lets the stack resolve.
fn cast_and_settle(driver: &mut Driver, card: &str) -> Result<(), ScriptError> {
    driver.begin_cast(card)?;
    pay(driver)?;
    for _ in 0..20 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::cloudform("Cloudform", Keyword::Hexproof)]
#[case::lightform("Lightform", Keyword::Lifelink)]
fn the_enchantment_becomes_an_aura_on_the_manifested_top_card(
    #[case] card: &str,
    #[case] granted: Keyword,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, card, 5).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, card, 5))?;
    let top = driver.state().zone(Zone::Library(pid(0))).last().copied();
    cast_and_settle(&mut driver, card)?;
    let state = driver.state();
    let aura = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == card)
        .expect("the enchantment stays on the battlefield");
    let host = state.host_of(aura).expect("attached");
    let manifested = state.object(host).unwrap();
    assert_eq!(manifested.face_down(), Some(FaceDown::Manifest));
    assert_eq!(Some(host), top, "the top card of the caster's library");
    assert_eq!(manifested.controller(), pid(0));
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), 4);
    for seat in 1..seats {
        assert_eq!(state.zone(Zone::Library(pid(seat))).len(), 5);
    }
    let subtypes = characteristics(state, aura).unwrap().subtypes;
    assert!(
        subtypes.contains(&Subtype("Aura".to_owned())),
        "{subtypes:?}"
    );
    let keywords = keywords_of(state, host);
    assert!(keywords.contains(&&Keyword::Flying), "{keywords:?}");
    assert!(keywords.contains(&&granted), "{keywords:?}");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn with_an_empty_library_the_aura_is_attached_to_nothing_and_put_into_the_graveyard(
    #[values("Cloudform", "Lightform")] card: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, card, 0).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, card, 0))?;
    cast_and_settle(&mut driver, card)?;
    let state = driver.state();
    let graveyard: Vec<&str> = state
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .map(|id| state.object(*id).unwrap().card().name.as_str())
        .collect();
    assert_eq!(graveyard, [card]);
    replays(&driver, &setup);
    Ok(())
}
