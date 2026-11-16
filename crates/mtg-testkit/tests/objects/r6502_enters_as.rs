//! Goal: a card returned to the battlefield as something else ("It's an Aura enchantment with enchant creature you
//! control and ...") is that from the moment it enters, at 2 to 4 seats: it enchants a permanent it may enchant as the
//! Aura it is (CR 303.4g) and has the abilities the effect gives instead of its own, it stays in the graveyard when there
//! is nothing it could enchant, it is never a creature on the battlefield, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::{CardType, Subtype};
use mtg_engine::ability_reader::abilities_of;
use mtg_engine::characteristics::characteristics;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{replays, settle};
use crate::common::{names, on_battlefield, pid};

/// Seat 0 controls `dying`, a Mountain and, when `host` names one, that permanent; seat 1 controls a creature.
fn table(seats: usize, dying: &str, host: Option<&str>) -> Scenario {
    let mut mine = vec![ready(card(dying).unwrap()), ready(mountain())];
    match host {
        Some(name) => mine.push(ready(free_creature(name, 2, 2))),
        None => {}
    }
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 10)])
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Theirs", 2, 2))])
}

#[rstest]
#[case::lion_on_my_creature("Bronzehide Lion", Some("Mine"), (1, 0))]
#[case::lion_with_nothing_to_enchant("Bronzehide Lion", None, (0, 0))]
#[case::lion_beside_another_creature_of_mine("Bronzehide Lion", Some("Partner"), (1, 0))]
fn a_card_returns_as_the_aura_it_is_made(
    #[values(2, 3, 4)] seats: usize,
    #[case] dying: &str,
    #[case] host: Option<&str>,
    #[case] (own, granted): (usize, usize),
) -> Result<(), ScriptError> {
    let setup = table(seats, dying, host).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, dying, host))?;
    let host_id = host.map(|name| driver.id_of(name)).transpose()?;
    let before = host_id.map_or(0, |id| abilities_of(driver.state(), id).len());
    let target = driver.target_named(dying)?;
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver)?;
    let state = driver.state();
    let Some(host_id) = host_id else {
        // Nothing it could enchant: it stays where it died (CR 303.4g).
        assert!(on_battlefield(state, dying).is_none());
        assert!(names(state, Zone::Graveyard(pid(0))).contains(&dying.to_owned()));
        replays(&driver, &setup);
        return Ok(());
    };
    let aura = on_battlefield(state, dying).expect("it returned");
    let object = state.object(aura).unwrap();
    assert_eq!(
        object
            .permanent()
            .unwrap()
            .attached_to()
            .map(|host| host.id),
        Some(host_id)
    );
    let chars = characteristics(state, aura).unwrap();
    assert!(!chars.is_creature());
    assert!(chars.types.contains(CardType::Enchantment));
    assert!(chars.has_subtype(&object.card().subtypes, &Subtype("Aura".into())));
    // Theirs is never enchanted, and the host gains what the Aura grants.
    assert_ne!(
        Some(driver.id_of("Theirs")?),
        object.permanent().unwrap().attached_to().map(|h| h.id)
    );
    // Its own abilities are those the effect gives (the Lion's are gone), and the host keeps its own.
    assert_eq!(abilities_of(state, aura).len(), own);
    assert_eq!(abilities_of(state, host_id).len(), before + granted);
    replays(&driver, &setup);
    Ok(())
}
