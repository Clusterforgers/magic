//! Goal: the costs of an activated ability beyond `{T}`, mana and the sacrifice of its source (life, counters removed, a permanent sacrificed, tapped or returned, a card discarded or exiled from a graveyard) and cycling are announced in the order the cost prints them (the picks with `ChooseCards`, asked only when there is something to choose, then the payment), paid with the events the engine already logs for them before the ability goes on the stack, offered only when the whole cost can be paid and refused when it cannot, never taken from what the mana needs when the player has another way, backed out of with no trace at any question, and kept in the log that replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, names, pid};
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::abilities::AbilityRef;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::cards::{forest, free_creature, island, mountain, swamp};
use mtg_testkit::corpus;
use mtg_testkit::part_cards::{
    bleeder, charger, cycler, drummer, gauntlet, gravedigger, levy, life_cycler, mana_elf,
    returner, rummager, sacrificer,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    options
        .abilities
        .iter()
        .map(|ability| name_of(driver.state(), ability.object))
        .collect()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The id of the card called `name` in `zone`.
fn id_in(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    *driver
        .state()
        .zone(zone)
        .iter()
        .find(|id| name_of(driver.state(), **id) == name)
        .unwrap_or_else(|| panic!("no {name} in {zone:?}"))
}

/// The cards the pending question offers, by name, and what it asks.
fn asked(driver: &Driver) -> (Vec<String>, String, bool) {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!(
            "a choice of cards was expected, not {:?}",
            driver.decision()
        );
    };
    let mut from: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    from.sort();
    assert_eq!((options.min, options.max), (options.min, options.min));
    (from, options.text.clone(), options.can_cancel)
}

fn choose(driver: &mut Driver, zone: Zone, picks: &[&str]) -> Result<Vec<Event>, ScriptError> {
    let ids = picks.iter().map(|name| id_in(driver, zone, name)).collect();
    driver.answer(Answer::ChooseCards(CardsChoice(ids)))
}

fn activate(
    driver: &mut Driver,
    object: ObjectId,
    ability: usize,
) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::Priority(PriorityAction::Activate(
        AbilityRef::printed(object, ability),
    )))
}

/// What happened to the cost, in the words of the events.
fn story(driver: &Driver, events: &[Event]) -> Vec<String> {
    let name = |id: &ObjectId| name_of(driver.state(), *id);
    events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentTapped { object } => Some(format!("tap {}", name(object))),
            Event::LifePaid { amount, .. } => Some(format!("pay {amount} life")),
            Event::CardDiscarded { object, .. } => Some(format!("discard {}", name(object))),
            Event::Cycled { object, .. } => Some(format!("cycle {}", name(object))),
            Event::CountersChanged(changed) => Some(format!("counters {:+}", changed.delta)),
            Event::ZoneChanged {
                object,
                from,
                to,
                cause,
            } => Some(format!("{} {from:?} to {to:?} ({cause:?})", name(object))),
            Event::AbilityActivated { .. } => Some("activated".to_owned()),
            Event::AbilityPutOnStack(_) => Some("on the stack".to_owned()),
            Event::ManaSpent { .. } => Some("mana spent".to_owned()),
            _ => None,
        })
        .collect()
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn table(scenario: Scenario) -> Result<Driver, ScriptError> {
    Driver::from_scenario(scenario)
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn a_sacrifice_is_picked_when_the_ability_is_announced_and_paid_before_it_goes_on_the_stack(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(sacrificer("Seer")),
            ready(free_creature("Wolf", 2, 2)),
            ready(free_creature("Ogre", 3, 3)),
        ],
    );
    let setup = scenario.clone().setup()?;
    let mut driver = table(scenario)?;
    assert_eq!(offered(&driver), ["Seer"]);
    driver.begin_activation("Seer")?;
    let (from, text, can_cancel) = asked(&driver);
    assert_eq!(from, ["Ogre", "Seer", "Wolf"]);
    assert_eq!(text, "Sacrifice a creature.");
    assert!(can_cancel, "an ability can be backed out of");
    let events = choose(&mut driver, Zone::Battlefield, &["Wolf"])?;
    assert_eq!(
        story(&driver, &events),
        [
            "Wolf Battlefield to Graveyard(PlayerId(0)) (Sacrificed)",
            "activated",
            "on the stack"
        ]
    );
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Wolf"]);
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    settle(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + 1);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn the_ability_can_be_backed_out_of_at_the_question_and_leaves_no_trace() -> Result<(), ScriptError>
{
    let mut driver = table(Scenario::new(2).battlefield(
        0,
        [
            ready(sacrificer("Seer")),
            ready(free_creature("Wolf", 2, 2)),
        ],
    ))?;
    let (events, hash) = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_activation("Seer")?;
    driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
    assert_eq!(driver.engine().events().len(), events, "nothing was logged");
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(offered(&driver), ["Seer"], "it can be activated again");
    Ok(())
}

#[test]
fn nothing_is_asked_when_there_is_nothing_to_choose() -> Result<(), ScriptError> {
    // The Seer is the only creature, so it is the one sacrificed.
    let mut driver = table(Scenario::new(2).battlefield(0, [ready(sacrificer("Seer"))]))?;
    let events = driver.begin_activation("Seer")?;
    assert_eq!(
        story(&driver, &events),
        [
            "Seer Battlefield to Graveyard(PlayerId(0)) (Sacrificed)",
            "activated",
            "on the stack"
        ]
    );
    Ok(())
}

#[rstest]
#[case::a_discard_needs_a_card(
    Scenario::new(2).battlefield(0, [ready(rummager("Goblin"))]),
    false
)]
#[case::a_discard_with_a_card(
    Scenario::new(2).hand(0, [free_creature("Spare", 1, 1)]).battlefield(0, [ready(rummager("Goblin"))]),
    true
)]
#[case::paying_more_life_than_there_is(
    Scenario::new(2).life(0, 2).battlefield(0, [ready(bleeder("Bleeder"))]),
    false
)]
#[case::paying_all_the_life(
    Scenario::new(2).life(0, 3).battlefield(0, [ready(bleeder("Bleeder"))]),
    true
)]
#[case::no_counters_to_remove(
    Scenario::new(2).battlefield(0, [ready(charger("Charger"))]),
    false
)]
#[case::a_counter_to_remove(
    Scenario::new(2).battlefield(0, [with_counters(ready(charger("Charger")), CounterKind::Charge, 1)]),
    true
)]
#[case::counters_of_another_kind(
    Scenario::new(2).battlefield(0, [with_counters(ready(charger("Charger")), CounterKind::Poison, 2)]),
    false
)]
#[case::no_card_to_exile(
    Scenario::new(2).battlefield(0, [ready(gravedigger("Digger"))]),
    false
)]
#[case::a_card_of_another_kind_to_exile(
    Scenario::new(2).graveyard(0, [forest()]).battlefield(0, [ready(gravedigger("Digger"))]),
    false
)]
#[case::a_creature_card_to_exile(
    Scenario::new(2).graveyard(0, [free_creature("Dead", 1, 1)]).battlefield(0, [ready(gravedigger("Digger"))]),
    true
)]
#[case::the_graveyard_of_another_player_does_not_count(
    Scenario::new(2).graveyard(1, [free_creature("Dead", 1, 1)]).battlefield(0, [ready(gravedigger("Digger"))]),
    false
)]
#[case::two_creatures_to_tap_with_one(
    Scenario::new(2).battlefield(0, [ready(drummer("Drum")), ready(free_creature("Wolf", 2, 2))]),
    false
)]
#[case::two_untapped_creatures_to_tap_with_one_tapped(
    Scenario::new(2).battlefield(0, [ready(drummer("Drum")), ready(free_creature("Wolf", 2, 2)), tapped(free_creature("Ogre", 3, 3))]),
    false
)]
#[case::two_untapped_creatures(
    Scenario::new(2).battlefield(0, [ready(drummer("Drum")), ready(free_creature("Wolf", 2, 2)), ready(free_creature("Ogre", 3, 3))]),
    true
)]
#[case::summoning_sickness_does_not_hold_back_the_tapping_of_another(
    Scenario::new(2).battlefield(0, [ready(drummer("Drum")), mtg_testkit::sick(free_creature("Wolf", 2, 2)), mtg_testkit::sick(free_creature("Ogre", 3, 3))]),
    true
)]
#[case::the_creatures_of_another_player_do_not_count(
    Scenario::new(2).battlefield(0, [ready(drummer("Drum")), ready(free_creature("Wolf", 2, 2))]).battlefield(1, [ready(free_creature("Ogre", 3, 3))]),
    false
)]
#[case::the_only_land_pays_the_mana_so_it_cannot_also_be_returned(
    Scenario::new(2).battlefield(0, [ready(returner("Returner")), ready(forest())]),
    false
)]
#[case::a_land_to_return_and_another_for_the_mana(
    Scenario::new(2).battlefield(0, [ready(returner("Returner")), ready(forest()), ready(island())]),
    true
)]
#[case::no_land_to_return(
    Scenario::new(2).battlefield(0, [ready(returner("Returner"))]),
    false
)]
fn an_ability_is_offered_only_when_its_whole_cost_can_be_paid(
    #[case] scenario: Scenario,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let driver = table(scenario)?;
    assert_eq!(
        !offered(&driver).is_empty(),
        expected,
        "{:?}",
        offered(&driver)
    );
    Ok(())
}

#[test]
fn an_attempt_to_activate_what_cannot_be_paid_is_refused_without_a_change()
-> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).battlefield(0, [ready(rummager("Goblin"))]))?;
    let (events, hash) = (driver.engine().events().len(), driver.engine().state_hash());
    let goblin = driver.id_of("Goblin")?;
    assert!(
        activate(&mut driver, goblin, 0).is_err(),
        "no card to discard"
    );
    assert_eq!(driver.engine().events().len(), events);
    assert_eq!(driver.engine().state_hash(), hash);
    Ok(())
}

#[test]
fn a_discard_is_a_cost_paid_after_the_tap_and_names_the_card_to_every_seat()
-> Result<(), ScriptError> {
    let mut driver = table(
        Scenario::new(3)
            .hand(
                0,
                [free_creature("Spare", 1, 1), free_creature("Other", 2, 2)],
            )
            .battlefield(0, [ready(rummager("Goblin"))]),
    )?;
    driver.begin_activation("Goblin")?;
    let (from, text, _) = asked(&driver);
    assert_eq!(
        (from, text.as_str()),
        (
            vec!["Other".to_owned(), "Spare".to_owned()],
            "Discard a card."
        )
    );
    let events = choose(&mut driver, Zone::Hand(pid(0)), &["Spare"])?;
    assert_eq!(
        story(&driver, &events),
        ["tap Goblin", "discard Spare", "activated", "on the stack"]
    );
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Spare"]);
    Ok(())
}

#[test]
fn life_is_paid_with_no_question_and_before_the_ability_goes_on_the_stack()
-> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).battlefield(0, [ready(bleeder("Bleeder"))]))?;
    let events = driver.begin_activation("Bleeder")?;
    assert_eq!(
        story(&driver, &events),
        ["pay 3 life", "activated", "on the stack"]
    );
    assert_eq!(life(&driver, 0), 17);
    Ok(())
}

#[test]
fn counters_are_removed_from_the_source_and_the_ability_remembers_it_as_it_then_stands()
-> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).battlefield(
        0,
        [with_counters(
            ready(charger("Charger")),
            CounterKind::Charge,
            2,
        )],
    ))?;
    let events = driver.begin_activation("Charger")?;
    assert_eq!(
        story(&driver, &events),
        ["counters -1", "activated", "on the stack"]
    );
    let put = events
        .iter()
        .find_map(|event| match event {
            Event::AbilityPutOnStack(put) => Some(put.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(put.snapshot.counters, [(CounterKind::Charge, 1)]);
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 21);
    // The last counter goes, and then the ability is no longer offered.
    driver.begin_activation("Charger")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 22);
    assert_eq!(offered(&driver), Vec::<String>::new());
    Ok(())
}

#[test]
fn a_card_is_exiled_from_the_graveyard_and_the_exile_is_public() -> Result<(), ScriptError> {
    let mut driver = table(
        Scenario::new(2)
            .graveyard(
                0,
                [
                    free_creature("Dead One", 1, 1),
                    free_creature("Dead Two", 2, 2),
                    forest(),
                ],
            )
            .battlefield(0, [ready(gravedigger("Digger"))]),
    )?;
    driver.begin_activation("Digger")?;
    let (from, text, _) = asked(&driver);
    assert_eq!(from, ["Dead One", "Dead Two"], "only creature cards");
    assert_eq!(text, "Exile a creature card from your graveyard.");
    let events = choose(&mut driver, Zone::Graveyard(pid(0)), &["Dead Two"])?;
    assert_eq!(
        story(&driver, &events),
        [
            "Dead Two Graveyard(PlayerId(0)) to Exile (Exiled)",
            "activated",
            "on the stack"
        ]
    );
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 22);
    assert_eq!(names(driver.state(), Zone::Exile), ["Dead Two"]);
    Ok(())
}

#[test]
fn untapped_creatures_are_tapped_and_the_question_is_left_out_when_exactly_enough_are_there()
-> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).battlefield(
        0,
        [
            ready(drummer("Drum")),
            ready(free_creature("Wolf", 2, 2)),
            ready(free_creature("Ogre", 3, 3)),
        ],
    ))?;
    let events = driver.begin_activation("Drum")?;
    assert_eq!(
        story(&driver, &events),
        ["tap Wolf", "tap Ogre", "activated", "on the stack"]
    );
    // With a third, the player picks which two.
    let mut driver = table(Scenario::new(2).battlefield(
        0,
        [
            ready(drummer("Drum")),
            ready(free_creature("Wolf", 2, 2)),
            ready(free_creature("Ogre", 3, 3)),
            ready(free_creature("Bear", 2, 2)),
        ],
    ))?;
    driver.begin_activation("Drum")?;
    let (from, text, _) = asked(&driver);
    assert_eq!(from, ["Bear", "Ogre", "Wolf"]);
    assert_eq!(text, "Tap 2 untapped creatures you control.");
    let events = choose(&mut driver, Zone::Battlefield, &["Wolf", "Bear"])?;
    assert_eq!(
        story(&driver, &events),
        ["tap Wolf", "tap Bear", "activated", "on the stack"]
    );
    Ok(())
}

#[test]
fn a_land_is_returned_to_its_owners_hand_and_does_not_pay_the_mana() -> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).battlefield(
        0,
        [
            ready(returner("Returner")),
            ready(forest()),
            ready(island()),
        ],
    ))?;
    driver.begin_activation("Returner")?;
    let (from, text, _) = asked(&driver);
    assert_eq!(from, ["Forest", "Island"]);
    assert_eq!(text, "Return a land you control to its owner's hand.");
    choose(&mut driver, Zone::Battlefield, &["Forest"])?;
    // The payment is of the other land alone.
    let options = driver.pay_options()?.clone();
    assert_eq!(options.cost.to_string(), "{1}");
    assert_eq!(options.sources.len(), 1);
    let events = driver.pay_tapping(&["Island"], 0)?;
    assert_eq!(
        story(&driver, &events),
        [
            "Forest Battlefield to Hand(PlayerId(0)) (Moved)",
            "activated",
            "on the stack",
            "tap Island",
            "mana spent"
        ]
    );
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0)))
            .last()
            .map(String::as_str),
        Some("Forest")
    );
    Ok(())
}

#[test]
fn a_creature_that_makes_the_mana_is_left_for_it_when_the_cost_can_take_another()
-> Result<(), ScriptError> {
    let scenario = |extra: bool| {
        Scenario::new(2).battlefield(
            0,
            [
                vec![ready(levy("Levy")), ready(mana_elf("Elf"))],
                if extra {
                    vec![ready(free_creature("Wolf", 2, 2))]
                } else {
                    vec![]
                },
            ]
            .concat(),
        )
    };
    // The only creature is the one that pays the mana, so it cannot also be tapped for the cost.
    assert_eq!(offered(&table(scenario(false))?), Vec::<String>::new());
    // With another creature the cost taps that one, which is the only choice, and the Elf pays.
    let mut driver = table(scenario(true))?;
    assert_eq!(offered(&driver), ["Levy"]);
    driver.begin_activation("Levy")?;
    let (from, _, _) = asked(&driver);
    assert_eq!(from, ["Elf", "Wolf"]);
    choose(&mut driver, Zone::Battlefield, &["Wolf"])?;
    let options = driver.pay_options()?.clone();
    assert_eq!(
        options.sources.len(),
        1,
        "the Elf is the source of the mana"
    );
    let events = driver.pay_tapping(&["Elf"], 0)?;
    assert_eq!(
        story(&driver, &events),
        [
            "tap Wolf",
            "activated",
            "on the stack",
            "tap Elf",
            "mana spent"
        ]
    );
    Ok(())
}

#[test]
fn picking_the_creature_the_mana_needs_rewinds_the_announcement() -> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).battlefield(
        0,
        [
            ready(levy("Levy")),
            ready(mana_elf("Elf")),
            ready(free_creature("Wolf", 2, 2)),
        ],
    ))?;
    let (events, hash) = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_activation("Levy")?;
    choose(&mut driver, Zone::Battlefield, &["Elf"])?;
    assert_eq!(driver.engine().events().len(), events, "nothing was logged");
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(offered(&driver), ["Levy"]);
    Ok(())
}

#[test]
fn every_kind_of_part_is_announced_in_the_order_the_cost_prints_them_and_paid_in_that_order()
-> Result<(), ScriptError> {
    let scenario = Scenario::new(3)
        .hand(
            0,
            [free_creature("Spare", 1, 1), free_creature("Other", 2, 2)],
        )
        .battlefield(
            0,
            [
                ready(gauntlet("Gauntlet")),
                ready(free_creature("Wolf", 2, 2)),
                ready(mountain()),
                ready(swamp()),
            ],
        );
    let setup = scenario.clone().setup()?;
    let mut driver = table(scenario)?;
    driver.begin_activation("Gauntlet")?;
    // The sacrifice comes first, then the discard, then the payment.
    assert_eq!(asked(&driver).1, "Sacrifice a creature.");
    let events = choose(&mut driver, Zone::Battlefield, &["Wolf"])?;
    assert!(
        events.is_empty(),
        "the announcement goes on without a trace"
    );
    assert_eq!(asked(&driver).1, "Discard a card.");
    choose(&mut driver, Zone::Hand(pid(0)), &["Other"])?;
    let options = driver.pay_options()?.clone();
    assert_eq!(options.cost.to_string(), "{1}");
    assert_eq!(options.sources.len(), 2);
    let events = driver.pay_tapping(&["Mountain"], 0)?;
    assert_eq!(
        story(&driver, &events),
        [
            "tap Gauntlet",
            "pay 1 life",
            "Wolf Battlefield to Graveyard(PlayerId(0)) (Sacrificed)",
            "discard Other",
            "activated",
            "on the stack",
            "tap Mountain",
            "mana spent"
        ]
    );
    assert_eq!(life(&driver, 0), 19);
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 22);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn backing_out_at_the_payment_after_the_picks_leaves_no_trace() -> Result<(), ScriptError> {
    let mut driver = table(
        Scenario::new(2)
            .hand(0, [free_creature("Spare", 1, 1)])
            .battlefield(
                0,
                [
                    ready(gauntlet("Gauntlet")),
                    ready(free_creature("Wolf", 2, 2)),
                    ready(mountain()),
                ],
            ),
    )?;
    let (events, hash) = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_activation("Gauntlet")?;
    choose(&mut driver, Zone::Battlefield, &["Wolf"])?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.engine().events().len(), events);
    assert_eq!(driver.engine().state_hash(), hash);
    Ok(())
}

#[test]
fn cycling_discards_the_card_for_its_cost_and_draws_at_instant_speed() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .hand(1, [cycler("Cycler")])
        .battlefield(1, [ready(mountain())])
        .battlefield(0, []);
    let setup = scenario.clone().setup()?;
    let mut driver = table(scenario)?;
    // It is the other seat's turn: the card is cycled whenever its owner has priority.
    while driver.decision()?.player != pid(1)
        || !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    assert_eq!(offered(&driver), ["Cycler"], "from the hand");
    driver.begin_activation("Cycler")?;
    let options = driver.pay_options()?.clone();
    assert_eq!(options.cost.to_string(), "{1}");
    let events = driver.pay_tapping(&["Mountain"], 0)?;
    assert_eq!(
        story(&driver, &events),
        [
            "discard Cycler",
            "cycle Cycler",
            "activated",
            "on the stack",
            "tap Mountain",
            "mana spent"
        ]
    );
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))), ["Cycler"]);
    let hand = driver.state().zone(Zone::Hand(pid(1))).len();
    settle(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(1))).len(), hand + 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::without_the_mana(Scenario::new(2).hand(0, [cycler("Cycler")]), false)]
#[case::with_the_mana(Scenario::new(2).hand(0, [cycler("Cycler")]).battlefield(0, [ready(mountain())]), true)]
#[case::from_the_battlefield_it_is_not_cycled(Scenario::new(2).battlefield(0, [ready(cycler("Cycler")), ready(mountain())]), false)]
#[case::from_the_graveyard_it_is_not_cycled(Scenario::new(2).graveyard(0, [cycler("Cycler")]).battlefield(0, [ready(mountain())]), false)]
#[case::cycling_for_life(Scenario::new(2).hand(0, [life_cycler("Cycler")]), true)]
#[case::cycling_for_too_much_life(Scenario::new(2).life(0, 1).hand(0, [life_cycler("Cycler")]), false)]
fn a_card_is_cycled_only_from_a_hand_when_its_cost_can_be_paid(
    #[case] scenario: Scenario,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let driver = table(scenario)?;
    assert_eq!(!offered(&driver).is_empty(), expected);
    Ok(())
}

#[test]
fn cycling_for_life_pays_the_life_and_then_discards() -> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).hand(0, [life_cycler("Cycler")]))?;
    let events = driver.begin_activation("Cycler")?;
    assert_eq!(
        story(&driver, &events),
        [
            "pay 2 life",
            "discard Cycler",
            "cycle Cycler",
            "activated",
            "on the stack"
        ]
    );
    assert_eq!(life(&driver, 0), 18);
    Ok(())
}

fn real(name: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    corpus::card(name).unwrap()
}

#[test]
fn flooded_strand_pays_life_taps_and_sacrifices_itself_then_fetches_a_land()
-> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .library(0, [forest(), island(), mountain()])
        .battlefield(0, [ready(real("Flooded Strand"))]);
    let setup = scenario.clone().setup()?;
    let mut driver = table(scenario)?;
    let events = driver.begin_activation("Flooded Strand")?;
    assert_eq!(
        story(&driver, &events),
        [
            "tap Flooded Strand",
            "pay 1 life",
            "Flooded Strand Battlefield to Graveyard(PlayerId(0)) (Sacrificed)",
            "activated",
            "on the stack"
        ]
    );
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                assert_eq!(
                    options.from.len(),
                    1,
                    "only the Island is a Plains or an Island"
                );
                let found = options.from.clone();
                driver.answer(Answer::ChooseCards(CardsChoice(found)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(life(&driver, 0), 19);
    assert_eq!(names(driver.state(), Zone::Battlefield), ["Island"]);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Flooded Strand"]
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::nantuko_husk("Nantuko Husk", "Sacrifice a creature.", 4)]
#[case::llanowar_behemoth("Llanowar Behemoth", "Tap an untapped creature you control.", 5)]
fn a_real_card_asks_for_the_permanent_its_cost_takes(
    #[case] name: &str,
    #[case] words: &str,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).battlefield(
        0,
        [
            ready(real(name)),
            ready(free_creature("Wolf", 2, 2)),
            ready(free_creature("Ogre", 3, 3)),
        ],
    ))?;
    driver.begin_activation(name)?;
    assert_eq!(asked(&driver).1, words);
    choose(&mut driver, Zone::Battlefield, &["Wolf"])?;
    settle(&mut driver)?;
    let id = driver.id_of(name)?;
    let shown = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!(shown.power, Some(power));
    Ok(())
}

#[test]
fn rummaging_goblin_discards_a_card_and_draws_one() -> Result<(), ScriptError> {
    let mut driver = table(
        Scenario::new(2)
            .hand(0, [free_creature("Spare", 1, 1)])
            .battlefield(0, [ready(real("Rummaging Goblin"))]),
    )?;
    let hand = |driver: &Driver| names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(hand(&driver), ["Spare"]);
    driver.begin_activation("Rummaging Goblin")?;
    settle(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Spare"]);
    assert_eq!(hand(&driver).len(), 1, "a card was drawn");
    assert_ne!(hand(&driver), ["Spare"]);
    Ok(())
}

#[test]
fn soul_shepherd_exiles_a_creature_card_of_the_graveyard_and_gains_a_life()
-> Result<(), ScriptError> {
    let mut driver = table(
        Scenario::new(2)
            .graveyard(0, [free_creature("Dead", 1, 1)])
            .battlefield(
                0,
                [
                    ready(real("Soul Shepherd")),
                    ready(mtg_testkit::cards::plains()),
                ],
            ),
    )?;
    driver.begin_activation("Soul Shepherd")?;
    driver.pay_tapping(&["Plains"], 0)?;
    settle(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Dead"]);
    assert_eq!(life(&driver, 0), 21);
    Ok(())
}

#[test]
fn tumble_magnet_taps_a_target_and_loses_its_last_counter() -> Result<(), ScriptError> {
    let mut driver = table(
        Scenario::new(2)
            .battlefield(
                0,
                [with_counters(
                    ready(real("Tumble Magnet")),
                    CounterKind::Charge,
                    1,
                )],
            )
            .battlefield(1, [ready(free_creature("Wolf", 2, 2))]),
    )?;
    driver.begin_activation("Tumble Magnet")?;
    let wolf = driver.target_named("Wolf")?;
    driver.choose_targets(&[wolf])?;
    settle(&mut driver)?;
    let wolf = driver.id_of("Wolf")?;
    assert!(
        driver
            .state()
            .object(wolf)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    let magnet = driver.id_of("Tumble Magnet")?;
    let held = driver
        .state()
        .object(magnet)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Charge);
    assert_eq!(held, 0);
    Ok(())
}

#[test]
fn lurching_rotbeast_is_cycled_for_black_mana_from_the_hand() -> Result<(), ScriptError> {
    let mut driver = table(
        Scenario::new(2)
            .hand(0, [real("Lurching Rotbeast")])
            .battlefield(0, [ready(swamp())]),
    )?;
    assert_eq!(offered(&driver), ["Lurching Rotbeast"]);
    driver.begin_activation("Lurching Rotbeast")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{B}");
    driver.pay_tapping(&["Swamp"], 0)?;
    settle(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Lurching Rotbeast"]
    );
    Ok(())
}

#[test]
fn souldrinker_pays_three_life_for_a_counter() -> Result<(), ScriptError> {
    let mut driver = table(Scenario::new(2).battlefield(0, [ready(real("Souldrinker"))]))?;
    driver.begin_activation("Souldrinker")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 17);
    let id = driver.id_of("Souldrinker")?;
    let held = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(held, 1);
    Ok(())
}

#[test]
fn a_new_engine_rebuilt_from_the_log_asks_the_question_of_a_pick_again() -> Result<(), ScriptError>
{
    let scenario = Scenario::new(2).battlefield(
        0,
        [
            ready(sacrificer("Seer")),
            ready(free_creature("Wolf", 2, 2)),
        ],
    );
    let setup = scenario.clone().setup()?;
    let mut driver = table(scenario)?;
    let before = driver.decision()?.clone();
    driver.begin_activation("Seer")?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseCards(_)
    ));
    // The activation is outside the state until it is paid for: a rebuilt engine stands at the priority before it.
    let resumed = Engine::resume(setup, driver.engine().events().to_vec())?;
    assert_eq!(resumed.decision(), Some(&before));
    Ok(())
}
