//! Goal: an object that has "all activated abilities of" permanents has them as the permanents have them now (CR 113.6,
//! 613.1f): one an effect gave the donor whatever the timestamps, the mana ability of a land type an effect gave it, none
//! it lost, and with "except mana abilities" none of the mana abilities.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::modify::Duration;
use mtg_core::card::Subtype;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::abilities::{AbilitySource, Grant};
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::effects::Modification;
use mtg_testkit::cards::{creature, forest, island, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "{2}: You gain 1 life." as an ability an effect gives.
fn gain_life() -> Arc<Ability> {
    Arc::new(
        serde_json::from_str(
            r#"{"Activated":{"cost":"{2}","effects":[{"GainLife":{"amount":1}}]}}"#,
        )
        .unwrap(),
    )
}

/// The abilities of `holder` that some other permanent lends as it has them now.
fn lent(driver: &Driver, holder: ObjectId) -> Vec<Arc<Ability>> {
    mtg_engine::ability_reader::abilities_of(driver.state(), holder)
        .into_iter()
        .filter(|(address, _)| {
            matches!(
                address.source,
                AbilitySource::Granted(Grant::R111Lent { .. })
            )
        })
        .map(|(_, body)| body)
        .collect()
}

/// A game of `seats` players with `holder` for seat 0 and a Wastes for the last seat.
fn game(seats: usize, holder: &str) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card(holder).unwrap()), ready(forest())])
        .battlefield(seats - 1, [ready(wastes())]);
    Driver::from_scenario(scenario)
}

#[rstest]
fn r111_an_ability_given_to_a_land_is_lent_whatever_its_timestamp(
    #[values(2, 3, 4)] seats: usize,
    #[values("Manascape Refractor", "Sharkey, Tyrant of the Shire")] holder: &str,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, holder)?;
    let id = driver.id_of(holder)?;
    assert_eq!(lent(&driver, id), Vec::<Arc<Ability>>::new());
    let wastes = driver.id_of("Wastes")?;
    let on = driver.on(wastes);
    driver.add_effect(
        on,
        Modification::GrantAbility(gain_life()),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(lent(&driver, id), vec![gain_life()]);
    // The land loses the ability it was given, and the holder no longer has it.
    let on = driver.on(wastes);
    driver.add_effect(on, Modification::LoseAbilities, Duration::UntilEndOfTurn)?;
    assert_eq!(lent(&driver, id), Vec::<Arc<Ability>>::new());
    Ok(())
}

#[rstest]
fn r111_a_land_type_an_effect_gives_lends_its_mana_ability_unless_mana_is_left_out(
    #[values(2, 3, 4)] seats: usize,
    #[values("Manascape Refractor", "Sharkey, Tyrant of the Shire")] holder: &str,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, holder)?;
    let id = driver.id_of(holder)?;
    let wastes = driver.id_of("Wastes")?;
    let on = driver.on(wastes);
    driver.add_effect(
        on,
        Modification::SetLandTypes(vec![Subtype("Island".into())]),
        Duration::UntilEndOfTurn,
    )?;
    let blue = lent(&driver, id).iter().any(|body| body.is_mana_ability());
    assert_eq!(blue, holder == "Manascape Refractor");
    if blue {
        driver.tap_for_mana_of(holder, ManaType::Blue)?;
        assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), 20);
    }
    Ok(())
}

#[rstest]
fn r111_only_the_lands_the_selector_names_lend(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let holder = "Sharkey, Tyrant of the Shire";
    let mut driver = game(seats, holder)?;
    let id = driver.id_of(holder)?;
    // A land of Sharkey's own controller lends nothing; an opponent's does.
    let own = driver.id_of("Forest")?;
    let on = driver.on(own);
    driver.add_effect(
        on,
        Modification::GrantAbility(gain_life()),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(lent(&driver, id), Vec::<Arc<Ability>>::new());
    let theirs = driver.id_of("Wastes")?;
    let on = driver.on(theirs);
    driver.add_effect(
        on,
        Modification::GrantAbility(gain_life()),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(lent(&driver, id).len(), 1);
    Ok(())
}

#[rstest]
fn r111_abilities_gained_as_an_ability_resolves_stay_as_they_were(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let elemental = "Quicksilver Elemental";
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card(elemental).unwrap()), ready(island())])
        .battlefield(seats - 1, [ready(creature("Donor", 1, &[], 1, 1))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let id = driver.id_of(elemental)?;
    let donor = driver.id_of("Donor")?;
    let on = driver.on(donor);
    driver.add_effect(
        on,
        Modification::GrantAbility(gain_life()),
        Duration::UntilEndOfTurn,
    )?;
    driver.begin_activation(elemental)?;
    let target = driver.target_named("Donor")?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(lent(&driver, id), vec![gain_life()]);
    // The donor loses the ability later; the elemental keeps what it gained as the ability resolved (CR 611.2c).
    let on = driver.on(donor);
    driver.add_effect(on, Modification::LoseAbilities, Duration::UntilEndOfTurn)?;
    assert_eq!(lent(&driver, id), vec![gain_life()]);
    Ok(())
}
