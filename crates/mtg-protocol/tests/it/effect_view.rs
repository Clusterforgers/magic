//! Goal: the view of an object carries what the continuous effects make of it: the derived power and toughness, the keywords an effect grants after the printed ones, and no summoning sickness once haste is granted; the effect ends with its duration and the view goes back to the card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::turn::Step;
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::cards::keyworded;
use mtg_testkit::{Driver, Scenario, sick};

fn view_of(driver: &Driver, id: ObjectId) -> ObjectView {
    StateView::for_player(driver.state(), PlayerId(1))
        .objects
        .into_iter()
        .find(|o| o.id == id)
        .unwrap()
}

#[test]
fn a_viewer_sees_the_pump_the_granted_keywords_and_the_end_of_both() {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [sick(keyworded("Scout", 2, 2, &[Keyword::Vigilance]))]),
    )
    .unwrap();
    let id = driver.id_of("Scout").unwrap();
    let plain = view_of(&driver, id);
    assert_eq!((plain.power, plain.toughness), (Some(2), Some(2)));
    assert_eq!(plain.keywords, ["Vigilance"]);
    assert!(plain.permanent.as_ref().unwrap().summoning_sick);

    for modification in [
        Modification::ModifyPowerToughness {
            power: 3,
            toughness: 3,
        },
        Modification::AddKeyword(Keyword::Haste),
        Modification::AddKeyword(Keyword::Flying),
    ] {
        driver
            .add_effect(driver.on(id), modification, Duration::UntilEndOfTurn)
            .unwrap();
    }
    let changed = view_of(&driver, id);
    assert_eq!((changed.power, changed.toughness), (Some(5), Some(5)));
    assert_eq!(changed.keywords, ["Vigilance", "Haste", "Flying"]);
    assert!(!changed.permanent.as_ref().unwrap().summoning_sick);

    driver.idle_until(2, Step::Upkeep).unwrap();
    let after = view_of(&driver, id);
    assert_eq!((after.power, after.toughness), (Some(2), Some(2)));
    assert_eq!(after.keywords, ["Vigilance"]);
}
