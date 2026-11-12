//! Goal: a card with several faces has the characteristics of its zone and of the face it was cast or played as: both halves of a split card, the front face of the other two layouts, the half cast, the Adventure or the back face on the stack and the battlefield, and its front face again once it leaves them; the face is chosen at the announcement and listed with its cost in the options, an Adventure that resolves is exiled on an adventure and may be cast as the creature from exile, one that is countered or fizzles goes to the graveyard, and a back face land is played with its own mana ability.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::FaceOption;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay, resolve};
use crate::common::{name_of, names, pid};

fn game(hand: &[&str], lands: Vec<mtg_engine::PresetPermanent>) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
            .hand_named(0, hand.iter().copied())
            .battlefield(0, lands),
    )
}

fn red_lands(count: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(mountain())).collect()
}

fn faces(driver: &Driver) -> Vec<FaceOption> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority is asked");
    };
    options.faces.clone()
}

fn in_zone(driver: &Driver, zone: Zone, name: &str) -> Option<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
}

fn mana_value(driver: &Driver, id: ObjectId) -> u32 {
    driver.state().object(id).unwrap().card().mana_value()
}

#[rstest]
#[case::split("Fire // Ice", "Fire // Ice", 4, &[Color::Blue, Color::Red])]
#[case::adventure("Bonecrusher Giant // Stomp", "Bonecrusher Giant", 3, &[Color::Red])]
#[case::modal("Selfless Glyphweaver // Deadly Vanity", "Selfless Glyphweaver", 3, &[Color::White])]
fn a_card_in_a_hand_has_its_front_face_or_both_halves(
    #[case] card: &str,
    #[case] name: &str,
    #[case] value: u32,
    #[case] colors: &[Color],
) -> Result<(), ScriptError> {
    let driver = game(&[card], Vec::new())?;
    let id = in_zone(&driver, Zone::Hand(pid(0)), name).expect("in the hand under its name");
    assert_eq!(mana_value(&driver, id), value);
    let found = characteristics(driver.state(), id).unwrap();
    assert_eq!(found.colors.iter().collect::<Vec<_>>(), colors);
    Ok(())
}

#[rstest]
#[case::fire("Fire")]
#[case::ice("Ice")]
fn a_half_of_a_split_card_is_cast_for_its_own_cost_and_is_only_that_half_on_the_stack(
    #[case] half: &str,
) -> Result<(), ScriptError> {
    let lands = vec![ready(mountain()), ready(island()), ready(mountain())];
    let mut driver = game(&["Fire // Ice"], lands)?;
    let listed: Vec<(String, String)> = faces(&driver)
        .iter()
        .map(|f| (f.name.clone(), f.cost.as_ref().unwrap().to_string()))
        .collect();
    assert_eq!(
        listed,
        [
            ("Fire".to_owned(), "{1}{R}".to_owned()),
            ("Ice".to_owned(), "{1}{U}".to_owned())
        ]
    );
    driver.begin_cast("Fire // Ice")?;
    choose_way(&mut driver, half)?;
    if half == "Fire" {
        let target = driver.target_player(1);
        driver.choose_targets(&[target])?;
    } else {
        let target = driver.id_of("Mountain")?;
        driver.choose_targets(&[mtg_engine::choose::Target::Object(
            driver.state().object(target).unwrap().reference(),
        )])?;
    }
    pay(&mut driver)?;
    let spell = in_zone(&driver, Zone::Stack, half).expect("the half is on the stack");
    assert_eq!(mana_value(&driver, spell), 2);
    let colors = characteristics(driver.state(), spell).unwrap().colors;
    assert_eq!(colors.len(), 1, "one half, one color");
    resolve(&mut driver)?;
    assert!(
        in_zone(&driver, Zone::Graveyard(pid(0)), "Fire // Ice").is_some(),
        "both halves again in the graveyard"
    );
    Ok(())
}

#[test]
fn an_adventure_resolves_to_exile_and_the_creature_is_cast_from_there() -> Result<(), ScriptError> {
    let mut driver = game(&["Bonecrusher Giant // Stomp"], red_lands(5))?;
    let listed: Vec<String> = faces(&driver).iter().map(|f| f.name.clone()).collect();
    assert_eq!(listed, ["Bonecrusher Giant", "Stomp"]);
    driver.begin_cast("Bonecrusher Giant")?;
    choose_way(&mut driver, "Stomp")?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    let spell = in_zone(&driver, Zone::Stack, "Stomp").expect("Stomp is on the stack");
    assert!(
        driver
            .state()
            .object(spell)
            .unwrap()
            .base_types()
            .contains(CardType::Instant)
    );
    resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 18);
    let card = in_zone(&driver, Zone::Exile, "Bonecrusher Giant").expect("exiled on an adventure");
    assert_eq!(
        driver.state().object(card).unwrap().on_adventure(),
        Some(pid(0))
    );
    // The creature card is cast from exile, never the Adventure again.
    driver.begin_cast("Bonecrusher Giant")?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    let giant = driver.id_of("Bonecrusher Giant")?;
    let stats = characteristics(driver.state(), giant).unwrap();
    assert_eq!((stats.power, stats.toughness), (Some(4), Some(3)));
    assert_eq!(driver.state().object(giant).unwrap().on_adventure(), None);
    Ok(())
}

#[test]
fn an_adventure_that_fizzles_goes_to_the_graveyard() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
            .hand_named(0, ["Bonecrusher Giant // Stomp", "Lightning Bolt"])
            .battlefield(0, red_lands(5))
            .battlefield_named(1, ["Grizzly Bears"]),
    )?;
    driver.begin_cast("Bonecrusher Giant")?;
    choose_way(&mut driver, "Stomp")?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bears])?;
    pay(&mut driver)?;
    driver.cast_at("Lightning Bolt", &[bears])?;
    resolve(&mut driver)?;
    assert!(
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .all(|id| name_of(driver.state(), *id) != "Grizzly Bears")
    );
    resolve(&mut driver)?;
    assert!(in_zone(&driver, Zone::Graveyard(pid(0)), "Bonecrusher Giant").is_some());
    assert!(in_zone(&driver, Zone::Exile, "Bonecrusher Giant").is_none());
    Ok(())
}

#[test]
fn a_back_face_land_is_played_with_its_own_mana_ability() -> Result<(), ScriptError> {
    let mut driver = game(&["Hengegate Pathway // Mistgate Pathway"], Vec::new())?;
    let listed: Vec<(String, bool)> = faces(&driver)
        .iter()
        .map(|f| (f.name.clone(), f.land))
        .collect();
    assert_eq!(
        listed,
        [
            ("Hengegate Pathway".to_owned(), true),
            ("Mistgate Pathway".to_owned(), true)
        ]
    );
    let card = in_zone(&driver, Zone::Hand(pid(0)), "Hengegate Pathway").unwrap();
    driver.answer(Answer::Priority(PriorityAction::PlayLandFace(card, 1)))?;
    let land = driver.id_of("Mistgate Pathway")?;
    assert_eq!(driver.state().object(land).unwrap().face(), Some(1));
    driver.tap_for_mana("Mistgate Pathway")?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield),
        ["Mistgate Pathway".to_owned()]
    );
    Ok(())
}

#[rstest]
#[case::only_the_front_is_affordable(vec![plains(), plains(), plains()], &["Selfless Glyphweaver"])]
#[case::both_are_affordable(
    vec![plains(), swamp(), swamp(), swamp(), forest(), forest(), forest(), forest()],
    &["Selfless Glyphweaver", "Deadly Vanity"]
)]
fn a_modal_card_lists_the_faces_that_can_be_cast(
    #[case] lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let driver = game(
        &["Selfless Glyphweaver // Deadly Vanity"],
        lands.into_iter().map(ready).collect(),
    )?;
    let listed: Vec<String> = faces(&driver).iter().map(|f| f.name.clone()).collect();
    assert_eq!(listed, expected);
    Ok(())
}

#[test]
fn a_back_face_spell_is_that_face_on_the_battlefield_and_the_front_face_after_it_leaves()
-> Result<(), ScriptError> {
    let lands: Vec<_> = [plains(), plains(), plains()]
        .into_iter()
        .map(ready)
        .collect();
    let mut driver = game(&["Selfless Glyphweaver // Deadly Vanity"], lands)?;
    driver.cast("Selfless Glyphweaver")?;
    resolve(&mut driver)?;
    let id = driver.id_of("Selfless Glyphweaver")?;
    assert_eq!(driver.state().object(id).unwrap().face(), None);
    Ok(())
}
