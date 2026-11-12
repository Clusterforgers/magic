//! Goal: abilities that work from a zone other than the battlefield are played as the rules say, with real cards: an activated ability is offered from a graveyard or a hand to its owner alone and only with its price; a cost that exiles or discards its source is paid, and logged, before the ability goes on the stack, which remembers the card as it was (CR 113.7a, 400.7); an ability whose card has left the zone does nothing and one that triggered from it resolves from its snapshot; a triggered ability triggers from a graveyard, a hand and the stack and looks back at a creature that died (CR 603.10a); a static ability applies from a graveyard while its condition holds; and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, on_battlefield, pid};
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{Target, YesNoChoice};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityOptions};
use mtg_engine::events::ZoneCause;
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event, GameState, replay};
use mtg_testkit::cards::{forest, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::zone_cards::{banish, discard_watcher, dragon_whelp, echo_wraith, wary_hound};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("priority was expected, got {}", other.name()),
    }
}

/// The object called `name` in `zone`.
fn in_zone(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// The object as a target.
fn target(state: &GameState, id: ObjectId) -> Target {
    Target::Object(state.object(id).unwrap().reference())
}

/// Passes priority while there is something on the stack and the decision is a priority one, so the stack resolves
/// until an effect asks or it is empty.
fn pass_while_resolving(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
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

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[derive(Debug, Clone, Copy)]
enum Place {
    Graveyard,
    Hand,
    Battlefield,
}

fn skeleton_game(place: Place, swamps: usize) -> Result<Driver, ScriptError> {
    let skeleton = card("Reassembling Skeleton").unwrap();
    let lands: Vec<_> = (0..swamps).map(|_| ready(swamp())).collect();
    let mut scenario = Scenario::new(2);
    scenario = match place {
        Place::Graveyard => scenario.graveyard(0, [skeleton]).battlefield(0, lands),
        Place::Hand => scenario.hand(0, [skeleton]).battlefield(0, lands),
        Place::Battlefield => scenario.battlefield(0, lands.into_iter().chain([ready(skeleton)])),
    };
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::in_the_graveyard_with_the_mana(Place::Graveyard, 2, true)]
#[case::in_the_graveyard_without_the_mana(Place::Graveyard, 1, false)]
#[case::in_the_hand(Place::Hand, 2, false)]
#[case::on_the_battlefield(Place::Battlefield, 2, false)]
fn a_graveyard_ability_is_offered_only_from_the_graveyard_and_with_its_price(
    #[case] place: Place,
    #[case] swamps: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = skeleton_game(place, swamps)?;
    let options = options(&driver);
    assert_eq!(options.abilities.len(), usize::from(offered));
    assert!(!offered || options.can_act, "an ability is something to do");
    Ok(())
}

#[test]
fn the_skeleton_pays_and_returns_tapped_as_a_new_object() -> Result<(), ScriptError> {
    let mut driver = skeleton_game(Place::Graveyard, 2)?;
    let id = in_zone(
        driver.state(),
        Zone::Graveyard(pid(0)),
        "Reassembling Skeleton",
    );
    driver.begin_activation("Reassembling Skeleton")?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let Some(Event::AbilityPutOnStack(put)) = events
        .iter()
        .find(|e| matches!(e, Event::AbilityPutOnStack(_)))
    else {
        panic!("the ability went on the stack");
    };
    assert_eq!((put.source.id, put.source.counter), (id, 0));
    assert_eq!(put.snapshot.zone, Zone::Graveyard(pid(0)));
    assert_eq!(put.controller, pid(0));
    let before = driver.state().object(id).unwrap().counter();
    pass_while_resolving(&mut driver)?;
    let skeleton = driver.state().object(id).unwrap();
    assert_eq!(skeleton.zone(), Zone::Battlefield);
    assert!(skeleton.permanent().unwrap().tapped());
    assert_eq!(skeleton.counter(), before + 1, "a new object (CR 400.7)");
    assert!(
        driver.state().zone(Zone::Stack).iter().all(|id| driver
            .state()
            .object(*id)
            .unwrap()
            .kind()
            != ObjectKind::Ability)
    );
    assert_replays(&driver);
    Ok(())
}

#[test]
fn the_ability_does_nothing_when_its_card_has_left_the_graveyard_by_the_time_it_resolves()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard(0, [card("Reassembling Skeleton").unwrap()])
            .battlefield(0, [ready(swamp()), ready(swamp())])
            .hand(1, [banish().unwrap()]),
    )?;
    let id = in_zone(
        driver.state(),
        Zone::Graveyard(pid(0)),
        "Reassembling Skeleton",
    );
    driver.begin_activation("Reassembling Skeleton")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1), "the opponent responds");
    let at = target(driver.state(), id);
    driver.cast_at("Banish", &[at])?;
    let events = pass_while_resolving(&mut driver)?;
    assert_eq!(driver.state().object(id).unwrap().zone(), Zone::Exile);
    assert!(
        events.iter().all(|e| !matches!(
            e,
            Event::ZoneChanged {
                to: Zone::Battlefield,
                ..
            }
        )),
        "the new object in exile is not the card the ability named"
    );
    assert_eq!(driver.state().stack(), []);
    assert_replays(&driver);
    Ok(())
}

#[test]
fn exiling_the_source_is_paid_before_the_ability_goes_on_the_stack_and_the_ability_remembers_the_card()
-> Result<(), ScriptError> {
    let soul = card("Soul of Zendikar").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard(0, [soul])
            .battlefield(0, (0..5).map(|_| ready(forest()))),
    )?;
    let id = in_zone(driver.state(), Zone::Graveyard(pid(0)), "Soul of Zendikar");
    let offered = options(&driver).abilities;
    assert_eq!(offered.len(), 1, "only the ability of the graveyard");
    assert_eq!((offered[0].object, offered[0].ability), (id, 2));
    driver.begin_activation("Soul of Zendikar")?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(matches!(
        events[0],
        Event::ZoneChanged {
            from: Zone::Graveyard(_),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
            ..
        }
    ));
    assert!(matches!(events[1], Event::AbilityActivated { .. }));
    let Event::AbilityPutOnStack(put) = &events[2] else {
        panic!("the ability goes on the stack as soon as the source is exiled");
    };
    assert_eq!((put.source.id, put.source.counter), (id, 0));
    assert_eq!(put.snapshot.zone, Zone::Graveyard(pid(0)), "as it was");
    assert!(
        events[3..]
            .iter()
            .any(|e| matches!(e, Event::PermanentTapped { .. })),
        "the mana is paid after"
    );
    pass_while_resolving(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Soul of Zendikar"]);
    assert!(on_battlefield(driver.state(), "Beast").is_some());
    assert_replays(&driver);
    Ok(())
}

#[test]
fn discarding_the_source_is_a_discard_that_the_watchers_of_discards_see() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [card("Elemental Masterpiece").unwrap()])
            .battlefield(
                0,
                [
                    ready(island()),
                    ready(mountain()),
                    ready(discard_watcher().unwrap()),
                ],
            ),
    )?;
    let id = in_zone(driver.state(), Zone::Hand(pid(0)), "Elemental Masterpiece");
    assert_eq!(options(&driver).abilities.len(), 1);
    driver.begin_activation("Elemental Masterpiece")?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(matches!(events[0], Event::CardDiscarded { object, .. } if object == id));
    assert!(matches!(events[1], Event::AbilityActivated { .. }));
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::AbilityTriggered(t) if driver.state().object(t.source.id).is_some_and(|o| o.card().name == "Discard Watcher")
        )),
        "a discard as a cost is a discard"
    );
    pass_while_resolving(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Elemental Masterpiece"]
    );
    assert!(on_battlefield(driver.state(), "Treasure").is_some());
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    assert_replays(&driver);
    Ok(())
}

/// A game in which seat 0 may burn a Dragon with a Scourge in `place` waiting, seat `owner` controlling the Dragon.
fn scourge_game(place: Place, owner: usize) -> Result<Driver, ScriptError> {
    let scourge = card("Boneyard Scourge").unwrap();
    let bolt = card("Lightning Bolt").unwrap();
    let mut mine = vec![ready(mountain()), ready(swamp()), ready(swamp())];
    let mut theirs = Vec::new();
    if owner == 0 {
        mine.push(ready(dragon_whelp()));
    } else {
        theirs.push(ready(dragon_whelp()));
    }
    let scenario = Scenario::new(2).battlefield(1, theirs);
    let scenario = match place {
        Place::Graveyard => scenario
            .battlefield(0, mine)
            .hand(0, [bolt])
            .graveyard(0, [scourge]),
        Place::Hand => scenario.battlefield(0, mine).hand(0, [bolt, scourge]),
        Place::Battlefield => {
            mine.push(ready(scourge));
            scenario.battlefield(0, mine).hand(0, [bolt])
        }
    };
    Driver::from_scenario(scenario)
}

#[test]
fn a_creature_that_dies_triggers_the_graveyard_card_that_waits_for_it_and_the_card_returns_when_paid()
-> Result<(), ScriptError> {
    let mut driver = scourge_game(Place::Graveyard, 0)?;
    let scourge = in_zone(driver.state(), Zone::Graveyard(pid(0)), "Boneyard Scourge");
    let whelp = in_zone(driver.state(), Zone::Battlefield, "Dragon Whelp");
    let at = target(driver.state(), whelp);
    let mut events = driver.cast_at("Lightning Bolt", &[at])?;
    events.extend(pass_while_resolving(&mut driver)?);
    let triggered: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::AbilityTriggered(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(triggered.len(), 1, "{triggered:?}");
    assert_eq!(triggered[0].source.id, scourge);
    assert_eq!(triggered[0].snapshot.zone, Zone::Graveyard(pid(0)));
    assert_eq!(triggered[0].cause.subjects, [at], "the Dragon, as it died");
    // "You may pay {1}{B}. If you do, return this card."
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    pass_while_resolving(&mut driver)?;
    assert_eq!(
        driver.state().object(scourge).unwrap().zone(),
        Zone::Battlefield
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Lightning Bolt", "Dragon Whelp"]
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_dragon_of_the_opponent(Place::Graveyard, 1)]
#[case::the_card_in_the_hand(Place::Hand, 0)]
#[case::the_card_on_the_battlefield(Place::Battlefield, 0)]
fn only_the_card_in_the_graveyard_that_waits_for_a_dragon_of_its_controller_triggers(
    #[case] place: Place,
    #[case] owner: usize,
) -> Result<(), ScriptError> {
    let mut driver = scourge_game(place, owner)?;
    let whelp = in_zone(driver.state(), Zone::Battlefield, "Dragon Whelp");
    let at = target(driver.state(), whelp);
    let mut events = driver.cast_at("Lightning Bolt", &[at])?;
    events.extend(pass_while_resolving(&mut driver)?);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::AbilityTriggered(_))),
        "{events:?}"
    );
    assert!(
        names(driver.state(), Zone::Graveyard(pid(owner))).contains(&"Dragon Whelp".to_owned()),
        "the Dragon died"
    );
    Ok(())
}

#[test]
fn a_trigger_that_triggered_from_the_graveyard_resolves_from_its_snapshot_when_the_card_is_gone()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Untap)
            .graveyard(0, [echo_wraith().unwrap()])
            .hand(1, [banish().unwrap()]),
    )?;
    // The wraith's trigger is on the stack in the upkeep.
    assert_eq!(driver.state().turn().step(), Step::Upkeep);
    assert_eq!(driver.state().stack().len(), 1);
    let wraith = in_zone(driver.state(), Zone::Graveyard(pid(0)), "Echo Wraith");
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    let at = target(driver.state(), wraith);
    driver.cast_at("Banish", &[at])?;
    pass_while_resolving(&mut driver)?;
    assert_eq!(driver.state().object(wraith).unwrap().zone(), Zone::Exile);
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        23,
        "its power as it was in the graveyard"
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::in_the_hand(Place::Hand, 1)]
#[case::on_the_battlefield(Place::Battlefield, 0)]
#[case::in_the_graveyard(Place::Graveyard, 0)]
fn a_trigger_of_a_hand_triggers_only_while_the_card_is_in_the_hand(
    #[case] place: Place,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let hound = wary_hound().unwrap();
    let scenario = Scenario::new(2).at(2, 0, Step::Untap);
    let scenario = match place {
        Place::Hand => scenario.hand(0, [hound]),
        Place::Graveyard => scenario.graveyard(0, [hound]),
        Place::Battlefield => scenario.battlefield(0, [sick(hound)]),
    };
    let mut driver = Driver::from_scenario(scenario)?;
    pass_while_resolving(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20 + gained);
    assert_replays(&driver);
    Ok(())
}

#[test]
fn a_trigger_of_a_spell_goes_on_the_stack_above_it_and_resolves_first() -> Result<(), ScriptError> {
    let cast = mtg_testkit::zone_cards::thundering_cast().unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [cast])
            .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())]),
    )?;
    driver.cast("Thundering Cast")?;
    let stack: Vec<ObjectKind> = driver
        .state()
        .stack()
        .iter()
        .map(|id| driver.state().object(*id).unwrap().kind())
        .collect();
    assert_eq!(stack, [ObjectKind::Card, ObjectKind::Ability]);
    let events = pass_while_resolving(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 22);
    let order: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::LifeGained { .. } => Some("life"),
            Event::SpellResolved { .. } => Some("creature"),
            _ => None,
        })
        .collect();
    assert_eq!(order, ["life", "creature"]);
    assert!(on_battlefield(driver.state(), "Thundering Cast").is_some());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_mountain_and_anger_in_the_graveyard(Place::Graveyard, true, true)]
#[case::no_mountain(Place::Graveyard, false, false)]
#[case::anger_in_the_hand(Place::Hand, true, false)]
#[case::anger_on_the_battlefield(Place::Battlefield, true, false)]
fn a_static_ability_applies_from_the_graveyard_while_its_condition_holds(
    #[case] place: Place,
    #[case] mountain_in_play: bool,
    #[case] haste: bool,
) -> Result<(), ScriptError> {
    let anger = card("Anger").unwrap();
    let mut scenario = Scenario::new(2).battlefield(0, [sick(card("Grizzly Bears").unwrap())]);
    if mountain_in_play {
        scenario =
            scenario.battlefield(0, [sick(card("Grizzly Bears").unwrap()), ready(mountain())]);
    }
    scenario = match place {
        Place::Graveyard => scenario.graveyard(0, [anger]),
        Place::Hand => scenario.hand(0, [anger]),
        Place::Battlefield => scenario.battlefield(
            0,
            [
                sick(card("Grizzly Bears").unwrap()),
                ready(mountain()),
                ready(anger),
            ],
        ),
    };
    let driver = Driver::from_scenario(scenario)?;
    let bears = in_zone(driver.state(), Zone::Battlefield, "Grizzly Bears");
    assert_eq!(has_keyword(driver.state(), bears, &Keyword::Haste), haste);
    Ok(())
}

#[test]
fn anger_stops_giving_haste_when_it_is_exiled_from_the_graveyard() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [sick(card("Grizzly Bears").unwrap()), ready(mountain())])
            .graveyard(0, [card("Anger").unwrap()])
            .hand(1, [banish().unwrap()]),
    )?;
    let bears = in_zone(driver.state(), Zone::Battlefield, "Grizzly Bears");
    assert!(has_keyword(driver.state(), bears, &Keyword::Haste));
    driver.pass()?;
    let anger = in_zone(driver.state(), Zone::Graveyard(pid(0)), "Anger");
    let at = target(driver.state(), anger);
    driver.cast_at("Banish", &[at])?;
    pass_while_resolving(&mut driver)?;
    assert!(!has_keyword(driver.state(), bears, &Keyword::Haste));
    assert_replays(&driver);
    Ok(())
}
