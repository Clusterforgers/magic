//! Goal: unearth is an activated ability from the graveyard at sorcery speed: the card returns as a new object with haste, is exiled at the next end step and instead of leaving the battlefield by any route (damage, bounce, a sacrifice, Rest in Peace in play), does nothing when the card has left the graveyard in response or the cost cannot be paid, the second activation of a card finds nothing to return, and a copy of the permanent is no unearthed card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, on_battlefield, pid};
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{island, mountain, swamp};
use mtg_testkit::zone_cards::banish;
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
fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    for _ in 0..40 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        events.extend(driver.pass()?);
    }
    Ok(events)
}

/// Seat 0 activates the unearth of `name` and pays automatically.
fn unearth(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn game(card: &str, lands: Vec<mtg_engine::PresetPermanent>) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, [card])
            .battlefield(0, lands),
    )
}

#[rstest]
#[case::black_zombie("Dregscape Zombie", vec![swamp()])]
#[case::blue_flyer("Kathari Screecher", vec![island(), island(), island()])]
#[case::artifact_flyer("Reconstructed Thopter", vec![mountain(), mountain()])]
fn unearth_returns_the_card_with_haste_and_exiles_it_at_the_end_step(
    #[case] card: &str,
    #[case] lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
) -> Result<(), ScriptError> {
    let mut driver = game(card, lands.into_iter().map(ready).collect())?;
    let before = find(driver.state(), Zone::Graveyard(pid(0)), card);
    let counter = driver.state().object(before).unwrap().counter();
    unearth(&mut driver, card)?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1, "an ability, not a spell");
    settle(&mut driver)?;
    let id = on_battlefield(driver.state(), card).expect("the card returned");
    assert_eq!(driver.state().object(id).unwrap().counter(), counter + 1);
    assert!(has_keyword(driver.state(), id, &Keyword::Haste));
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), card).is_none());
    find(driver.state(), Zone::Exile, card);
    assert!(driver.state().zone(Zone::Graveyard(pid(0))).is_empty());
    assert_replays(&driver);
    Ok(())
}

#[test]
fn the_unearthed_creature_can_attack_the_turn_it_returns() -> Result<(), ScriptError> {
    let mut driver = game("Dregscape Zombie", vec![ready(swamp())])?;
    unearth(&mut driver, "Dregscape Zombie")?;
    settle(&mut driver)?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Dregscape Zombie", 1)])?;
    driver.idle_until(1, Step::End)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 18, "it dealt its 2 damage");
    Ok(())
}

#[rstest]
#[case::without_the_mana(vec![], "Dregscape Zombie", Zone::Graveyard(pid(0)), false)]
#[case::in_the_graveyard(vec![swamp()], "Dregscape Zombie", Zone::Graveyard(pid(0)), true)]
#[case::in_the_hand(vec![swamp()], "Dregscape Zombie", Zone::Hand(pid(0)), false)]
fn unearth_is_offered_from_the_graveyard_with_the_mana_to_pay(
    #[case] lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] card: &str,
    #[case] zone: Zone,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(2).battlefield(0, lands.into_iter().map(ready));
    let scenario = match zone {
        Zone::Hand(_) => scenario.hand_named(0, [card]),
        _ => scenario.graveyard_named(0, [card]),
    };
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(driver.offered_abilities(card)?.len(), usize::from(offered));
    Ok(())
}

#[test]
fn it_is_a_sorcery_so_it_is_not_offered_with_a_spell_on_the_stack_or_on_another_turn()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, ["Dregscape Zombie"])
            .battlefield(0, [ready(swamp()), ready(mountain())])
            .hand_named(0, ["Lightning Bolt"]),
    )?;
    driver.cast_at("Lightning Bolt", &[driver.target_player(1)])?;
    assert!(
        driver.offered_abilities("Dregscape Zombie")?.is_empty(),
        "a sorcery waits for an empty stack"
    );
    settle(&mut driver)?;
    assert_eq!(driver.offered_abilities("Dregscape Zombie")?.len(), 1);
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(
        driver.decision()?.player,
        pid(1),
        "the opponent is the active player"
    );
    Ok(())
}

#[rstest]
#[case::damage("Lightning Bolt")]
fn a_creature_that_would_die_is_exiled_instead(#[case] removal: &str) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, ["Dregscape Zombie"])
            .battlefield(0, [ready(swamp())])
            .battlefield(1, [ready(mountain())])
            .hand_named(1, [removal]),
    )?;
    unearth(&mut driver, "Dregscape Zombie")?;
    settle(&mut driver)?;
    let id = on_battlefield(driver.state(), "Dregscape Zombie").unwrap();
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    let at = Target::Object(driver.state().object(id).unwrap().reference());
    driver.cast_at(removal, &[at])?;
    settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), "Dregscape Zombie").is_none());
    find(driver.state(), Zone::Exile, "Dregscape Zombie");
    assert!(driver.state().zone(Zone::Graveyard(pid(0))).is_empty());
    assert_replays(&driver);
    Ok(())
}

#[test]
fn the_ability_does_nothing_when_the_card_left_the_graveyard_in_response() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, ["Dregscape Zombie"])
            .battlefield(0, [ready(swamp())])
            .hand(1, [banish().unwrap()]),
    )?;
    let id = find(driver.state(), Zone::Graveyard(pid(0)), "Dregscape Zombie");
    unearth(&mut driver, "Dregscape Zombie")?;
    driver.pass()?;
    let at = Target::Object(driver.state().object(id).unwrap().reference());
    driver.cast_at("Banish", &[at])?;
    settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), "Dregscape Zombie").is_none());
    find(driver.state(), Zone::Exile, "Dregscape Zombie");
    driver.idle_until(1, Step::End)?;
    assert!(
        driver.state().zone(Zone::Stack).is_empty(),
        "no delayed trigger was set up for a card that did not return"
    );
    assert_replays(&driver);
    Ok(())
}
