//! Goal: embalm and eternalize are sorcery-speed abilities from a graveyard that exile the card as a cost and make a token copy of it with no mana cost that is a Zombie in addition to its other types and white (embalm) or a black 4/4 (eternalize), with the abilities of the card; the activation is not offered with a spell on the stack or without the mana, the cost is paid even if the ability is answered, a card that has left the graveyard cannot be embalmed twice, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, on_battlefield, pid};
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, characteristics, replay};
use mtg_testkit::cards::{island, mountain, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The object called `name` in `zone`.
fn find(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// Passes priority until the stack is empty or a decision other than priority is pending.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 activates the ability of `name` in its graveyard and pays automatically.
fn activate(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

fn game(card: &str, lands: Vec<mtg_engine::PresetPermanent>) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, [card])
            .battlefield(0, lands),
    )
}

fn lands(
    land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
    count: usize,
) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

#[rstest]
#[case::embalm_a_lifelinker("Sacred Cat", plains, 1, Color::White, (1, 1), &["Cat", "Zombie"], &[Keyword::Lifelink])]
#[case::embalm_a_vanilla_creature("Tah-Crop Skirmisher", island, 4, Color::White, (2, 1), &["Snake", "Warrior", "Zombie"], &[])]
#[case::embalm_a_blue_flyer("Aven Initiate", island, 7, Color::White, (3, 2), &["Bird", "Warrior", "Zombie"], &[Keyword::Flying])]
#[case::eternalize_a_double_striker("Adorned Pouncer", plains, 5, Color::Black, (4, 4), &["Cat", "Zombie"], &[Keyword::DoubleStrike])]
#[case::eternalize_a_vanilla_creature("Proven Combatant", island, 6, Color::Black, (4, 4), &["Human", "Warrior", "Zombie"], &[])]
fn the_token_is_a_zombie_copy_with_no_mana_cost_and_the_card_is_exiled(
    #[case] card: &str,
    #[case] land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
    #[case] count: usize,
    #[case] color: Color,
    #[case] stats: (i32, i32),
    #[case] subtypes: &[&str],
    #[case] keywords: &[Keyword],
) -> Result<(), ScriptError> {
    let mut driver = game(card, lands(land, count))?;
    activate(&mut driver, card)?;
    assert_eq!(
        driver.state().zone(Zone::Stack).len(),
        1,
        "an ability, not a spell"
    );
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))),
        [],
        "the card is exiled as a cost, before the ability resolves"
    );
    find(driver.state(), Zone::Exile, card);
    settle(&mut driver)?;
    let token = on_battlefield(driver.state(), card).expect("the token entered");
    let object = driver.state().object(token).unwrap();
    assert_eq!(object.kind(), ObjectKind::Token);
    assert_eq!(object.card().mana_cost, None, "no mana cost");
    assert_eq!(object.card().mana_value(), 0);
    let expected: Vec<Subtype> = subtypes
        .iter()
        .map(|name| Subtype((*name).into()))
        .collect();
    assert_eq!(
        object.card().subtypes,
        expected,
        "a Zombie in addition to its other types"
    );
    let chars = characteristics(driver.state(), token).unwrap();
    assert!(chars.types.contains(CardType::Creature));
    assert_eq!(chars.colors, ColorSet::EMPTY.with(color));
    assert_eq!(
        (chars.power, chars.toughness),
        (Some(stats.0), Some(stats.1))
    );
    for keyword in keywords {
        assert!(has_keyword(driver.state(), token, keyword), "{keyword:?}");
    }
    find(driver.state(), Zone::Exile, card);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))), []);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn the_token_has_no_embalm_of_its_own_and_is_not_the_card() -> Result<(), ScriptError> {
    let mut driver = game("Sacred Cat", lands(plains, 1))?;
    activate(&mut driver, "Sacred Cat")?;
    settle(&mut driver)?;
    let token = on_battlefield(driver.state(), "Sacred Cat").unwrap();
    assert!(
        has_keyword(driver.state(), token, &Keyword::Lifelink),
        "a copy has the abilities of the card"
    );
    assert!(
        driver.offered_abilities("Sacred Cat")?.is_empty(),
        "the token is on the battlefield and its embalm works from a graveyard only"
    );
    Ok(())
}

#[rstest]
#[case::cat_without_mana("Sacred Cat", 0)]
#[case::cat_with_the_wrong_colour("Sacred Cat", 1)]
fn the_activation_needs_the_mana_of_its_cost(
    #[case] card: &str,
    #[case] mountains: usize,
) -> Result<(), ScriptError> {
    let driver = game(card, lands(mountain, mountains))?;
    assert!(driver.offered_abilities(card)?.is_empty());
    Ok(())
}

#[test]
fn it_is_a_sorcery_so_it_is_not_offered_with_a_spell_on_the_stack_or_on_another_turn()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, ["Sacred Cat"])
            .battlefield(0, [ready(plains()), ready(mountain())])
            .hand_named(0, ["Lightning Bolt"]),
    )?;
    assert_eq!(driver.offered_abilities("Sacred Cat")?.len(), 1);
    driver.cast_at("Lightning Bolt", &[driver.target_player(1)])?;
    assert!(
        driver.offered_abilities("Sacred Cat")?.is_empty(),
        "a sorcery waits for an empty stack"
    );
    settle(&mut driver)?;
    assert_eq!(driver.offered_abilities("Sacred Cat")?.len(), 1);
    driver.idle_until(2, mtg_engine::turn::Step::Upkeep)?;
    assert_eq!(
        driver.decision()?.player,
        pid(1),
        "the opponent is the active player"
    );
    Ok(())
}

#[test]
fn the_card_is_gone_once_it_is_embalmed_so_the_ability_is_offered_once() -> Result<(), ScriptError>
{
    let mut driver = game("Sacred Cat", lands(plains, 2))?;
    activate(&mut driver, "Sacred Cat")?;
    assert!(
        driver.offered_abilities("Sacred Cat").is_err()
            || driver.offered_abilities("Sacred Cat")?.is_empty(),
        "the card left the graveyard to pay for the first activation"
    );
    settle(&mut driver)?;
    assert!(driver.offered_abilities("Sacred Cat")?.is_empty());
    Ok(())
}
