//! Goal: the real cards that cast or play from the top of the library (Sigarda, Font of Blessings as drafted; Future Sight; Courser of Kruphix) offer the top card at two to four seats only when the permission covers it and at its timing, the cast takes it to the battlefield, and the owner then knows the next top card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{DecisionKind, PriorityOptions};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, plains};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::fb27_reported::draft;

/// A white 2/2 costing {W} with the subtype.
fn white(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_power_toughness(2, 2)
            .with_subtype(subtype),
    )
}

/// The library's top card in a case.
#[derive(Clone, Copy, Debug)]
enum Top {
    Angel,
    Human,
    Bear,
    Land,
}

fn card(top: Top) -> Arc<CardDefinition> {
    match top {
        Top::Angel => white("Angel", "Angel"),
        Top::Human => white("Human", "Human"),
        Top::Bear => white("Bear", "Bear"),
        Top::Land => plains(),
    }
}

/// The permanent that holds the permission.
#[derive(Clone, Copy, Debug)]
enum Holder {
    Sigarda,
    FutureSight,
    Courser,
}

fn holder(holder: Holder) -> Arc<CardDefinition> {
    match holder {
        Holder::Sigarda => draft("sigarda_font_of_blessings"),
        Holder::FutureSight => corpus::card("Future Sight").unwrap(),
        Holder::Courser => corpus::card("Courser of Kruphix").unwrap(),
    }
}

fn table(seats: usize, by: Holder, top: Top, step: Step) -> Result<Driver, ScriptError> {
    let mine = vec![ready(plains()), ready(plains()), ready(holder(by))];
    Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, step)
            .library(0, [card(top), white("Under", "Angel")])
            .battlefield(0, mine),
    )
}

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("priority expected, got {}", other.name()),
    }
}

/// The library of seat 0, bottom first.
fn library(driver: &Driver) -> Vec<mtg_core::ids::ObjectId> {
    driver
        .state()
        .zone(Zone::Library(PlayerId::from_index(0)))
        .to_vec()
}

#[rstest]
#[case::sigarda_an_angel(Holder::Sigarda, Top::Angel, true, false)]
#[case::sigarda_a_human(Holder::Sigarda, Top::Human, true, false)]
#[case::sigarda_not_a_bear(Holder::Sigarda, Top::Bear, false, false)]
#[case::sigarda_not_a_land(Holder::Sigarda, Top::Land, false, false)]
#[case::future_sight_any_spell(Holder::FutureSight, Top::Bear, true, false)]
#[case::future_sight_a_land(Holder::FutureSight, Top::Land, false, true)]
#[case::courser_a_land(Holder::Courser, Top::Land, false, true)]
#[case::courser_no_spell(Holder::Courser, Top::Angel, false, false)]
fn the_covered_top_card_is_offered_in_a_main_phase(
    #[values(2, 3, 4)] seats: usize,
    #[case] by: Holder,
    #[case] top: Top,
    #[case] castable: bool,
    #[case] playable: bool,
) -> Result<(), ScriptError> {
    let driver = table(seats, by, top, Step::Main1)?;
    let library = library(&driver);
    let options = options(&driver);
    assert_eq!(
        options.castable.contains(&library[1]),
        castable,
        "{by:?} {top:?} cast"
    );
    assert_eq!(
        options.playable_lands.contains(&library[1]),
        playable,
        "{by:?} {top:?} play"
    );
    assert!(
        !options.castable.contains(&library[0]),
        "never the card under it"
    );
    // Sigarda lets its controller look at the top any time; Future Sight and Courser reveal it.
    assert_eq!(
        mtg_engine::sight::known_top(driver.state(), PlayerId::from_index(0)).first(),
        Some(&library[1])
    );
    Ok(())
}

#[rstest]
fn a_creature_on_top_waits_for_sorcery_timing(
    #[values(2, 3, 4)] seats: usize,
    #[values(Holder::Sigarda, Holder::FutureSight)] by: Holder,
) -> Result<(), ScriptError> {
    let driver = table(seats, by, Top::Angel, Step::Upkeep)?;
    let top = library(&driver)[1];
    assert!(!options(&driver).castable.contains(&top));
    Ok(())
}

#[rstest]
fn the_top_angel_is_cast_and_the_next_card_is_looked_at(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, Holder::Sigarda, Top::Angel, Step::Main1)?;
    driver.cast("Angel")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    let on_field = driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == "Angel")
    });
    assert!(on_field, "the Angel resolved");
    let next = library(&driver);
    assert_eq!(next.len(), 1);
    assert_eq!(
        mtg_engine::sight::known_top(driver.state(), PlayerId::from_index(0)),
        next
    );
    // The card under it is an Angel too, so it is offered next.
    assert!(options(&driver).castable.contains(&next[0]));
    Ok(())
}

#[rstest]
fn sigarda_cast_from_the_hand_opens_the_top_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine: Vec<_> = (0..4).map(|_| ready(plains())).collect();
    mine.extend((0..2).map(|_| ready(mtg_testkit::cards::forest())));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [draft("sigarda_font_of_blessings")])
            .library(0, [card(Top::Angel), white("Under", "Angel")])
            .battlefield(0, mine),
    )?;
    let top = library(&driver)[1];
    assert_eq!(
        mtg_engine::sight::known_top(driver.state(), PlayerId::from_index(0)),
        Vec::<mtg_core::ids::ObjectId>::new()
    );
    driver.cast("Sigarda, Font of Blessings")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(
        mtg_engine::sight::known_top(driver.state(), PlayerId::from_index(0)).first(),
        Some(&top)
    );
    assert!(options(&driver).castable.contains(&top));
    driver.cast("Angel")?;
    Ok(())
}
