//! Goal: foretelling is a special action from the hand, offered with priority at any time during its owner's turn for {2} whatever the cost modifiers say; it exiles the card face down without the stack, only its owner may look at it, the cast for the foretell cost opens on a later turn than the one it was foretold in (cost modifiers apply to it), the spell is face up and remembers the way, a card that leaves exile is a new object that is no longer foretold, and a replay of the log rebuilds the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::FaceDown;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, apply_event};
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{pay, resolve};
use crate::common::{name_of, names, pid};

/// The id of the card called `name` in a zone.
fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// The cards the holder of priority may foretell now.
fn foretellable(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options
        .foretellable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

/// The cards the holder of priority may cast now.
fn castable(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

/// Foretells the card called `name` from seat 0's hand, paying by tapping the named lands.
fn foretell(driver: &mut Driver, name: &str, lands: &[&str]) -> Result<Vec<Event>, ScriptError> {
    let id = find(driver, Zone::Hand(pid(0)), name);
    driver.answer(Answer::Priority(PriorityAction::Foretell(id)))?;
    driver.pay_tapping(lands, 0)
}

/// Lands for seat 0: two Forests, two Islands and a Mountain, untapped.
fn lands() -> Vec<PresetPermanent> {
    vec![
        ready(forest()),
        ready(forest()),
        ready(island()),
        ready(island()),
        ready(mountain()),
    ]
}

/// Seat 0 holds `cards` and `lands()`, in the first main phase of turn 1; seat 1 holds `against` on its battlefield.
fn game(cards: &[&str], against: &[&str]) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
            .hand_named(0, cards.iter().copied())
            .battlefield(0, lands())
            .battlefield_named(1, against.iter().copied()),
    )
}

/// The number of tapped permanents seat 0 controls.
fn tapped(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(0) && o.permanent().is_some_and(|p| p.tapped()))
        .count()
}

#[rstest]
#[case::instant_that_scries("Behold the Multiverse")]
#[case::counterspell("Saw It Coming")]
#[case::burn("Demon Bolt")]
#[case::creature("Augury Raven")]
fn foretelling_exiles_the_card_face_down_for_two_and_never_uses_the_stack(
    #[case] card: &str,
) -> Result<(), ScriptError> {
    let mut driver = game(&[card, "Lightning Bolt"], &[])?;
    assert_eq!(foretellable(&driver), [card]);
    let events = foretell(&mut driver, card, &["Forest", "Forest"])?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ZoneChanged {
            from: Zone::Hand(_),
            to: Zone::Exile,
            cause: ZoneCause::Foretold,
            ..
        }
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::SpellCast { .. })),
        "a special action does not use the stack"
    );
    let id = find(&driver, Zone::Exile, card);
    assert_eq!(
        driver.state().object(id).unwrap().face_down(),
        Some(FaceDown::Foretold { turn: 1 })
    );
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        ["Lightning Bolt"]
    );
    assert_eq!(driver.state().zone(Zone::Stack), []);
    assert_eq!(tapped(&driver), 2, "{{2}} was paid");
    assert_eq!(
        driver.decision()?.player,
        pid(0),
        "the owner keeps priority"
    );
    assert!(foretellable(&driver).is_empty());
    Ok(())
}

#[rstest]
#[case::main_phase(1, 0, Step::Main1, 5, true)]
#[case::upkeep(1, 0, Step::Upkeep, 5, true)]
#[case::beginning_of_combat(1, 0, Step::BeginCombat, 5, true)]
#[case::end_step(1, 0, Step::End, 5, true)]
#[case::exactly_two_lands(1, 0, Step::Main1, 2, true)]
#[case::one_land(1, 0, Step::Main1, 1, false)]
#[case::no_land(1, 0, Step::Main1, 0, false)]
fn a_card_is_foretold_during_its_owners_turn_for_two(
    #[case] turn: u32,
    #[case] active: usize,
    #[case] step: Step,
    #[case] lands: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .at(turn, active, step)
            .library(0, vec![forest(); 6])
            .hand_named(0, ["Behold the Multiverse", "Lightning Bolt"])
            .battlefield(0, (0..lands).map(|_| ready(island())).collect::<Vec<_>>()),
    )?;
    assert_eq!(driver.decision()?.player, pid(0));
    assert_eq!(
        foretellable(&driver),
        if offered {
            vec!["Behold the Multiverse".to_owned()]
        } else {
            Vec::new()
        },
        "a card without foretell is never offered"
    );
    Ok(())
}

#[test]
fn a_card_is_not_foretold_on_the_turn_of_another_player() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 1, Step::Main1)
            .library(0, vec![forest(); 6])
            .hand_named(0, ["Behold the Multiverse"])
            .battlefield(0, vec![ready(island()), ready(island())]),
    )?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert_eq!(driver.state().turn().active(), pid(1));
    assert!(foretellable(&driver).is_empty());
    let id = find(&driver, Zone::Hand(pid(0)), "Behold the Multiverse");
    let refused = driver.answer(Answer::Priority(PriorityAction::Foretell(id)));
    assert!(
        refused.is_err(),
        "the action is refused outside the owner's turn"
    );
    assert_eq!(names(driver.state(), Zone::Exile), Vec::<String>::new());
    Ok(())
}

#[test]
fn a_card_is_foretold_with_a_spell_on_the_stack_and_in_response_to_nothing_else()
-> Result<(), ScriptError> {
    let mut driver = game(&["Behold the Multiverse", "Lightning Bolt"], &[])?;
    let target = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[target])?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
    assert_eq!(foretellable(&driver), ["Behold the Multiverse"]);
    foretell(&mut driver, "Behold the Multiverse", &["Forest", "Forest"])?;
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1, "the bolt waits");
    Ok(())
}

#[test]
fn the_payment_of_foretell_can_be_backed_out_of() -> Result<(), ScriptError> {
    let mut driver = game(&["Saw It Coming"], &[])?;
    let id = find(&driver, Zone::Hand(pid(0)), "Saw It Coming");
    let before = driver.engine().state_hash();
    driver.answer(Answer::Priority(PriorityAction::Foretell(id)))?;
    assert_eq!(driver.decision()?.kind.name(), "pay-mana");
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.decision()?.kind.name(), "priority");
    assert_eq!(driver.engine().state_hash(), before, "nothing was done");
    Ok(())
}

#[test]
fn a_foretold_card_cannot_be_cast_on_the_turn_it_was_foretold_in() -> Result<(), ScriptError> {
    let mut driver = game(&["Behold the Multiverse"], &[])?;
    foretell(&mut driver, "Behold the Multiverse", &["Forest", "Forest"])?;
    assert!(
        castable(&driver).is_empty(),
        "not for its foretell cost nor for its mana cost"
    );
    Ok(())
}

#[rstest]
#[case::instant_in_the_next_turn("Behold the Multiverse", 2, Step::Upkeep, &["Island", "Island"])]
#[case::burn_in_the_next_turn("Demon Bolt", 2, Step::Upkeep, &["Mountain"])]
#[case::instant_two_turns_later("Behold the Multiverse", 4, Step::Upkeep, &["Island", "Island"])]
#[case::creature_in_the_turn_after("Augury Raven", 3, Step::Main1, &["Island", "Island"])]
fn a_foretold_card_is_cast_for_its_foretell_cost_from_a_later_turn(
    #[case] card: &str,
    #[case] turn: u32,
    #[case] step: Step,
    #[case] lands: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = game(&[card], &["Serra Angel"])?;
    foretell(&mut driver, card, &["Forest", "Forest"])?;
    driver.idle_until(turn, step)?;
    // The decision is the owner's only when it is their turn or they have priority: pass to them.
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert_eq!(castable(&driver), [card]);
    let id = find(&driver, Zone::Exile, card);
    assert_eq!(
        driver.state().object(id).unwrap().face_down(),
        Some(FaceDown::Foretold { turn: 1 })
    );
    let tapped_before = tapped(&driver);
    driver.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
    if card == "Demon Bolt" {
        let angel = driver.target_named("Serra Angel")?;
        driver.choose_targets(&[angel])?;
    }
    driver.pay_tapping(lands, 0)?;
    assert_eq!(
        tapped(&driver) - tapped_before,
        lands.len(),
        "only the foretell cost was paid"
    );
    let cast = driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        })
        .expect("the spell was cast");
    assert_eq!(cast, Some(CastWay::Foretold));
    let spell = driver.state().object(id).unwrap();
    assert_eq!(spell.zone(), Zone::Stack, "the card is on the stack");
    assert_eq!(spell.face_down(), None, "and face up");
    resolve(&mut driver)?;
    Ok(())
}

#[test]
fn the_cast_for_the_foretell_cost_makes_a_creature_that_remembers_nothing_face_down()
-> Result<(), ScriptError> {
    let mut driver = game(&["Augury Raven"], &[])?;
    foretell(&mut driver, "Augury Raven", &["Forest", "Forest"])?;
    driver.idle_until(3, Step::Main1)?;
    let id = find(&driver, Zone::Exile, "Augury Raven");
    driver.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    let raven = driver.id_of("Augury Raven")?;
    let object = driver.state().object(raven).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(object.face_down(), None);
    Ok(())
}

#[test]
fn a_foretold_card_may_be_cast_for_its_foretell_cost_only_by_its_owner() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
            .hand_named(0, ["Behold the Multiverse"])
            .battlefield(0, lands())
            .battlefield(1, vec![ready(island()), ready(island())]),
    )?;
    foretell(&mut driver, "Behold the Multiverse", &["Forest", "Forest"])?;
    driver.idle_until(2, Step::Upkeep)?;
    while driver.decision()?.player != pid(1) {
        driver.pass()?;
    }
    assert!(
        castable(&driver).is_empty(),
        "an opponent with the mana cannot cast the card"
    );
    Ok(())
}

#[rstest]
#[case::foretell_is_not_taxed("Behold the Multiverse", &["Forest", "Forest"], &["Island", "Island", "Mountain"])]
fn cost_increases_tax_the_cast_and_not_the_special_action(
    #[case] card: &str,
    #[case] foretold_with: &[&str],
    #[case] cast_with: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = game(&[card], &["Thalia, Guardian of Thraben"])?;
    // The special action costs {2} though noncreature spells cost {1} more.
    foretell(&mut driver, card, foretold_with)?;
    driver.idle_until(2, Step::Upkeep)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let id = find(&driver, Zone::Exile, card);
    driver.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
    let DecisionKind::PayMana(options) = &driver.decision()?.kind else {
        panic!("the payment is asked");
    };
    assert_eq!(
        options.cost.to_string(),
        "{2}{U}",
        "the foretell cost {{1}}{{U}} and the tax of {{1}}"
    );
    driver.pay_tapping(cast_with, 0)?;
    Ok(())
}

#[test]
fn two_lands_foretell_under_a_cost_increase() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .library(0, vec![forest(); 6])
            .hand_named(0, ["Saw It Coming"])
            .battlefield(0, vec![ready(island()), ready(island())])
            .battlefield_named(1, ["Thalia, Guardian of Thraben"]),
    )?;
    assert_eq!(foretellable(&driver), ["Saw It Coming"]);
    foretell(&mut driver, "Saw It Coming", &["Island", "Island"])?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Saw It Coming"]);
    Ok(())
}

/// Seat 0 has foretold the card called `name` in turn 1; the state after it.
fn foretold(name: &str) -> Result<(Driver, ObjectId), ScriptError> {
    let mut driver = game(&[name], &[])?;
    foretell(&mut driver, name, &["Forest", "Forest"])?;
    let id = find(&driver, Zone::Exile, name);
    Ok((driver, id))
}

#[rstest]
#[case::to_the_graveyard(Zone::Graveyard(pid(0)), ZoneCause::Moved)]
#[case::to_the_hand(Zone::Hand(pid(0)), ZoneCause::Moved)]
#[case::to_the_library(Zone::Library(pid(0)), ZoneCause::Moved)]
fn a_foretold_card_that_leaves_exile_is_a_new_object_that_is_not_foretold(
    #[case] to: Zone,
    #[case] cause: ZoneCause,
) -> Result<(), ScriptError> {
    let (driver, id) = foretold("Saw It Coming")?;
    let mut state = driver.state().clone();
    let counter = state.object(id).unwrap().counter();
    apply_event(
        &mut state,
        &Event::ZoneChanged {
            object: id,
            from: Zone::Exile,
            to,
            cause,
        },
    )
    .unwrap();
    assert_eq!(state.object(id).unwrap().face_down(), None);
    assert_eq!(state.object(id).unwrap().counter(), counter + 1);
    // Exiled again, it is face up and no longer foretold.
    apply_event(
        &mut state,
        &Event::ZoneChanged {
            object: id,
            from: to,
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        },
    )
    .unwrap();
    let again = state.object(id).unwrap();
    assert_eq!(again.zone(), Zone::Exile);
    assert_eq!(again.face_down(), None);
    assert_eq!(again.counter(), counter + 2);
    Ok(())
}

#[test]
fn a_replay_of_the_log_rebuilds_a_game_with_a_foretold_card() -> Result<(), ScriptError> {
    let (mut driver, id) = foretold("Behold the Multiverse")?;
    driver.idle_until(3, Step::Main1)?;
    let rebuilt = mtg_engine::Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
    assert_eq!(
        rebuilt.state().object(id).unwrap().face_down(),
        Some(FaceDown::Foretold { turn: 1 })
    );
    Ok(())
}

#[test]
fn a_replay_rebuilds_the_cast_of_a_foretold_card() -> Result<(), ScriptError> {
    let (mut driver, id) = foretold("Behold the Multiverse")?;
    driver.idle_until(2, Step::Upkeep)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    driver.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    let rebuilt = mtg_engine::Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
    Ok(())
}
