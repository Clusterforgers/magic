//! Goal: a creature cast with delve remembers the cards its delve exiled, and Soulflayer has each listed keyword one of
//! the creature cards among them has (with its variant), none of a noncreature card's, none of a card that is not listed
//! and none when it was cast without delve; at 2 to 4 seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::keywords::keywords_of;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{keyworded, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An enchantment card with flying: no creature card.
fn flying_enchantment() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Sky Charm")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(r#"{"Keyword":"Flying"}"#).unwrap()),
    )
}

#[rstest]
#[case::flying_and_trample(&["Flyer", "Trampler", "Filler", "Filler"], &[Keyword::Flying, Keyword::Trample])]
#[case::not_from_an_enchantment(&["Sky Charm", "Filler", "Filler", "Filler"], &[])]
#[case::not_banding(&["Bander", "Filler", "Filler", "Filler"], &[])]
#[case::without_delve(&[], &[])]
fn r5511_soulflayer_has_the_keywords_of_the_creature_cards_it_delved(
    #[case] delve: &[&str],
    #[case] has: &[Keyword],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let graveyard = vec![
        keyworded("Flyer", 1, 1, &[Keyword::Flying]),
        keyworded("Trampler", 1, 1, &[Keyword::Trample]),
        keyworded("Bander", 1, 1, &[Keyword::Banding]),
        flying_enchantment(),
        keyworded("Filler", 1, 1, &[]),
        keyworded("Filler", 1, 1, &[]),
        keyworded("Filler", 1, 1, &[]),
    ];
    let lands = if delve.is_empty() { 6 } else { 2 };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, vec![card("Soulflayer").unwrap()])
            .graveyard(0, graveyard)
            .battlefield(0, (0..lands).map(|_| ready(swamp()))),
    )?;
    driver.begin_cast("Soulflayer")?;
    let tapped: Vec<&str> = vec!["Swamp"; lands];
    driver.pay_helped(&tapped, &[], delve, 0)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let soulflayer = driver.id_of("Soulflayer")?;
    let held: Vec<Keyword> = keywords_of(driver.state(), soulflayer)
        .into_iter()
        .cloned()
        .collect();
    for keyword in [
        Keyword::Flying,
        Keyword::Trample,
        Keyword::Reach,
        Keyword::Banding,
    ] {
        assert_eq!(
            held.contains(&keyword),
            has.contains(&keyword),
            "{keyword:?} in {held:?}"
        );
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
