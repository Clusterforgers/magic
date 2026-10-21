//! Goal: Auras and the state-based actions of attachments play as the rules say, at tables of two, three and four seats: an Aura spell targets through its enchant ability (it is offered what the ability allows and no hexproof creature of an opponent, and a spell with nothing to enchant is not cast), enters attached to what it targeted in the batch that brings it and takes the time it became attached as its timestamp, and goes to its owner's graveyard when its target is gone as it resolves; an Aura put onto the battlefield without being cast chooses its host among the permanents it may enchant (asked only when there is a choice) and stays where it was when there are none; an Aura attached to nothing or to what it may not enchant goes to its owner's graveyard, an Equipment or Fortification attached to what it may not be and any other attached permanent becomes unattached, and one attached rightly stays; an Aura whose host left goes to the graveyard in the next pass; the corpus Auras do what their cards say (Pacifism, Epic Proportions at instant speed, Control Magic, Squirrel Nest, Shielded by Faith moving to a creature that enters, Sram drawing for an Aura); and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::{ResolutionOutcome, ZoneCause};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::attach_cards::{call_back, hex_aura, mighty_aura, rune_aura};
use mtg_testkit::cards::{bear, forest, free_creature, island, keyworded, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

fn equipment(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Equipment"),
    )
}

fn fortification(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Fortification"),
    )
}

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn name_of(driver: &Driver, id: ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

/// The names in a zone, sorted.
fn in_zone(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut found = common::names(driver.state(), zone);
    found.sort();
    found
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn host_name(driver: &Driver, name: &str) -> Option<String> {
    let id = driver.id_of(name).ok()?;
    driver.state().host_of(id).map(|host| name_of(driver, host))
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// Seat 0 holds `hand` and has five lands; it has a Bear. Seat 1 has an Ogre and a hexproof Shroud.
fn table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    Scenario::new(seats)
        .hand(0, hand)
        .battlefield(
            0,
            [
                ready(bear()),
                ready(forest()),
                ready(forest()),
                ready(island()),
                ready(swamp()),
                ready(plains()),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Ogre", 3, 4)),
                ready(keyworded("Shroud", 1, 1, &[Keyword::Hexproof])),
            ],
        )
}

// Casting

#[rstest]
#[case::any_creature_but_a_hexproof_one_of_an_opponent("Mighty Aura", &["Grizzly Bears", "Ogre"])]
#[case::only_the_creatures_of_opponents("Hex Aura", &["Ogre"])]
#[case::lands("Rune Aura", &["Forest", "Forest", "Island", "Plains", "Swamp"])]
fn an_aura_is_offered_what_its_enchant_ability_allows_and_no_hexproof_creature_of_an_opponent(
    #[values(2, 3, 4)] seats: usize,
    #[case] aura: &str,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let hand = vec![
        mighty_aura().unwrap(),
        hex_aura().unwrap(),
        rune_aura().unwrap(),
    ];
    let mut driver = Driver::from_scenario(table(seats, hand))?;
    driver.begin_cast(aura)?;
    let DecisionKind::ChooseTargets(options) = driver.decision()?.kind.clone() else {
        panic!("an Aura spell asks for its target as it is cast");
    };
    let mut offered: Vec<String> = options
        .legal
        .iter()
        .map(|target| name_of(&driver, target.object().unwrap().id))
        .collect();
    offered.sort();
    let mut expected: Vec<String> = expected.iter().map(|n| (*n).to_owned()).collect();
    expected.sort();
    assert_eq!(offered, expected);
    assert_eq!((options.min, options.max), (1, 1));
    Ok(())
}

#[test]
fn an_aura_with_nothing_to_enchant_is_not_offered_as_castable() -> Result<(), ScriptError> {
    for (creatures, castable) in [(false, false), (true, true)] {
        let mut scenario = Scenario::new(2)
            .hand(0, [mighty_aura().unwrap()])
            .battlefield(0, [ready(forest()), ready(island())]);
        if creatures {
            scenario = scenario.battlefield(0, [ready(forest()), ready(island()), ready(bear())]);
        }
        let driver = Driver::from_scenario(scenario)?;
        let DecisionKind::Priority(options) = &driver.decision()?.kind else {
            panic!("priority");
        };
        let offered = options
            .castable
            .iter()
            .any(|id| name_of(&driver, *id) == "Mighty Aura");
        assert_eq!(offered, castable, "a creature on the table: {creatures}");
    }
    Ok(())
}

// Entering

#[rstest]
fn an_aura_enters_attached_to_what_it_targeted_in_the_batch_that_brings_it(
    #[values(2, 3, 4)] seats: usize,
    #[values("Grizzly Bears", "Ogre")] host: &str,
) -> Result<(), ScriptError> {
    let hand = || vec![mighty_aura().unwrap()];
    let setup = table(seats, hand()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, hand()))?;
    let target = driver.target_named(host)?;
    driver.cast_at("Mighty Aura", &[target])?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    let at = events
        .iter()
        .position(|event| matches!(event, Event::SpellResolved { .. }))
        .unwrap();
    let (
        Event::SpellResolved { object },
        Event::Attached {
            object: attached,
            to,
        },
    ) = (&events[at], &events[at + 1])
    else {
        panic!("the attachment follows the spell in the same batch: {events:?}");
    };
    assert_eq!(attached, object);
    assert_eq!(*to, target.object().unwrap().id);
    assert_eq!(host_name(&driver, "Mighty Aura").as_deref(), Some(host));
    let expected = if host == "Ogre" { (5, 6) } else { (4, 4) };
    assert_eq!(stats(&driver, host), expected);
    // The time it became attached is its timestamp, later than the host's and than every earlier one.
    let permanent = |name: &str| {
        driver
            .state()
            .object(driver.id_of(name).unwrap())
            .unwrap()
            .permanent()
            .unwrap()
            .timestamp()
    };
    assert!(permanent("Mighty Aura") > permanent(host));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn an_aura_whose_target_is_gone_as_it_resolves_is_put_into_its_owners_graveyard(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let setup_of = || {
        let mut scenario = Scenario::new(seats)
            .hand(0, [mighty_aura().unwrap()])
            .hand(1, [card("Doom Blade").unwrap()])
            .battlefield(0, [ready(forest()), ready(island()), ready(bear())])
            .battlefield(
                1,
                [
                    ready(swamp()),
                    ready(swamp()),
                    ready(free_creature("Ogre", 3, 4)),
                ],
            );
        for seat in 2..seats {
            scenario = scenario.battlefield(seat, [ready(plains())]);
        }
        scenario
    };
    let setup = setup_of().setup().unwrap();
    let mut driver = Driver::from_scenario(setup_of())?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Mighty Aura", &[ogre])?;
    // The other seat answers by destroying the creature the Aura is aimed at.
    while driver.decision()?.player != pid(1) {
        driver.pass()?;
    }
    driver.cast_at("Doom Blade", &[ogre])?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ResolutionEnded {
            outcome: ResolutionOutcome::Fizzled,
            ..
        }
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Attached { .. }))
    );
    assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0))), ["Mighty Aura"]);
    assert!(
        driver.id_of("Mighty Aura").is_err(),
        "it never reached the battlefield"
    );
    replays(&driver, &setup);
    Ok(())
}

// Entering without being cast

/// Seat 0 holds Call Back and has a Mighty Aura in its graveyard and `creatures` on the battlefield, with mana.
fn call_back_table(creatures: &[&str]) -> Scenario {
    let mut permanents = vec![ready(plains()), ready(plains()), ready(forest())];
    permanents.extend(
        creatures
            .iter()
            .map(|name| ready(free_creature(name, 2, 2))),
    );
    Scenario::new(2)
        .hand(0, [call_back().unwrap()])
        .graveyard(0, [mighty_aura().unwrap()])
        .battlefield(0, permanents)
}

fn return_the_aura(driver: &mut Driver) -> Result<(), ScriptError> {
    let aura = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .copied()
        .find(|id| name_of(driver, *id) == "Mighty Aura")
        .unwrap();
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(aura).unwrap().reference());
    driver.cast_at("Call Back", &[target])?;
    driver.pass()?;
    driver.pass()?;
    Ok(())
}

#[test]
fn an_aura_put_onto_the_battlefield_chooses_among_the_permanents_it_may_enchant()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(call_back_table(&["Wolf", "Boar"]))?;
    return_the_aura(&mut driver)?;
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!(
            "the controller chooses what the Aura enchants: {:?}",
            driver.decision()
        );
    };
    assert_eq!((options.min, options.max), (1, 1));
    assert_eq!(driver.decision()?.player, pid(0));
    let mut offered: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(&driver, *id))
        .collect();
    offered.sort();
    assert_eq!(offered, ["Boar", "Wolf"]);
    let boar = options
        .from
        .iter()
        .copied()
        .find(|id| name_of(&driver, *id) == "Boar")
        .unwrap();
    driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
        boar,
    ])))?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    assert_eq!(host_name(&driver, "Mighty Aura").as_deref(), Some("Boar"));
    assert_eq!(stats(&driver, "Boar"), (4, 4));
    let _ = begun;
    Ok(())
}

#[test]
fn an_aura_with_one_permanent_to_enchant_is_not_asked() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(call_back_table(&["Wolf"]))?;
    return_the_aura(&mut driver)?;
    assert_eq!(host_name(&driver, "Mighty Aura").as_deref(), Some("Wolf"));
    Ok(())
}

#[test]
fn an_aura_with_nothing_to_enchant_stays_where_it_was() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(call_back_table(&[]))?;
    return_the_aura(&mut driver)?;
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0))),
        ["Call Back", "Mighty Aura"]
    );
    assert!(driver.id_of("Mighty Aura").is_err());
    Ok(())
}

// The state-based actions of attachments

/// Seat 0 has a Giant (0) and a Forest (1) and the attachment (2), attached to `host` (a place on the battlefield).
fn attachments(card: Arc<CardDefinition>, host: Option<usize>) -> Result<Driver, ScriptError> {
    let attachment = match host {
        Some(place) => attached_to(ready(card), 0, place),
        None => ready(card),
    };
    Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [
                    ready(free_creature("Giant", 4, 4)),
                    ready(forest()),
                    attachment,
                ],
            )
            .battlefield(1, [ready(free_creature("Ogre", 3, 4))]),
    )
}

#[rstest]
#[case::an_aura_on_a_creature_stays(mighty_aura().unwrap(), Some(0), true, Some("Giant"))]
#[case::an_aura_on_what_it_may_not_enchant_goes(mighty_aura().unwrap(), Some(1), false, None)]
#[case::an_aura_on_nothing_goes(mighty_aura().unwrap(), None, false, None)]
#[case::an_aura_for_opponents_creatures_on_your_own_goes(hex_aura().unwrap(), Some(0), false, None)]
#[case::a_land_aura_on_a_land_stays(rune_aura().unwrap(), Some(1), true, Some("Forest"))]
#[case::an_equipment_on_a_creature_stays(equipment("Blade"), Some(0), true, Some("Giant"))]
#[case::an_equipment_on_a_land_comes_off(equipment("Blade"), Some(1), true, None)]
#[case::an_equipment_on_nothing_stays(equipment("Blade"), None, true, None)]
#[case::a_fortification_on_a_land_stays(fortification("Wall"), Some(1), true, Some("Forest"))]
#[case::a_fortification_on_a_creature_comes_off(fortification("Wall"), Some(0), true, None)]
fn the_state_based_actions_of_attachments_keep_each_one_where_it_may_be(
    #[case] card: Arc<CardDefinition>,
    #[case] host: Option<usize>,
    #[case] stays: bool,
    #[case] attached_after: Option<&str>,
) -> Result<(), ScriptError> {
    let name = card.name.clone();
    let driver = attachments(card, host)?;
    assert_eq!(driver.id_of(&name).is_ok(), stays);
    if stays {
        assert_eq!(host_name(&driver, &name).as_deref(), attached_after);
        let came_off = driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Unattached { .. }));
        assert_eq!(came_off, host.is_some() && attached_after.is_none());
    } else {
        assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0))), [name.as_str()]);
        let put = driver.engine().events().iter().any(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    cause: ZoneCause::StateBased,
                    from: Zone::Battlefield,
                    ..
                }
            )
        });
        assert!(put);
    }
    Ok(())
}

#[test]
fn a_permanent_that_is_no_attachment_becomes_unattached() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(bear()),
            attached_to(ready(free_creature("Wolf", 2, 2)), 0, 0),
        ],
    ))?;
    assert_eq!(host_name(&driver, "Wolf"), None);
    assert!(driver.id_of("Wolf").is_ok(), "it stays on the battlefield");
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|e| matches!(e, Event::Unattached { .. }))
    );
    Ok(())
}

#[test]
fn an_aura_whose_host_left_goes_to_the_graveyard_in_the_pass_after_the_host_does()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(1, [card("Doom Blade").unwrap()])
            .battlefield(
                0,
                [
                    ready(free_creature("Ogre", 3, 4)),
                    attached_to(ready(mighty_aura().unwrap()), 0, 0),
                ],
            )
            .battlefield(1, [ready(swamp()), ready(swamp())]),
    )?;
    while driver.decision()?.player != pid(1) {
        driver.pass()?;
    }
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Doom Blade", &[ogre])?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    let host_left = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::ZoneChanged {
                    cause: ZoneCause::Destroyed,
                    ..
                }
            )
        })
        .unwrap();
    let aura_left = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::ZoneChanged {
                    cause: ZoneCause::StateBased,
                    ..
                }
            )
        })
        .unwrap();
    assert!(host_left < aura_left);
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0))),
        ["Mighty Aura", "Ogre"]
    );
    Ok(())
}

// The corpus Auras

/// A free instant that destroys target enchantment.
fn disenchant() -> Arc<CardDefinition> {
    common::spell(
        "Disenchant",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Enchantment"}]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    )
}

#[test]
fn pacifism_keeps_a_creature_from_attacking() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Pacifism"])
            .battlefield(0, [ready(plains()), ready(plains())])
            .battlefield(1, [ready(free_creature("Ogre", 3, 4))]),
    )?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Pacifism", &[ogre])?;
    settle(&mut driver)?;
    assert_eq!(host_name(&driver, "Pacifism").as_deref(), Some("Ogre"));
    // On its own turn the only creature the other seat has cannot be declared as an attacker.
    driver.idle_until(2, Step::Main1)?;
    for _ in 0..30 {
        if driver.state().turn().step() == Step::End || driver.state().turn().number() > 2 {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(driver.engine().events().iter().any(|event| matches!(
        event,
        Event::NothingToDo { player, kind: mtg_engine::events::NothingToDo::Attack } if *player == pid(1)
    )));
    Ok(())
}

#[test]
fn control_magic_takes_a_creature_and_gives_it_back_when_it_leaves() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [disenchant()])
            .battlefield(
                0,
                [
                    ready(island()),
                    attached_to(ready(card("Control Magic").unwrap()), 1, 0),
                ],
            )
            .battlefield(1, [ready(free_creature("Ogre", 3, 4))]),
    )?;
    let ogre = driver.id_of("Ogre")?;
    let controller =
        |driver: &Driver| mtg_engine::characteristics::controller_of(driver.state(), ogre);
    assert_eq!(controller(&driver), Some(pid(0)));
    assert_eq!(driver.state().object(ogre).unwrap().owner(), pid(1));
    let aura = driver.target_named("Control Magic")?;
    driver.cast_at("Disenchant", &[aura])?;
    settle(&mut driver)?;
    assert_eq!(controller(&driver), Some(pid(1)));
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0))),
        ["Control Magic", "Disenchant"]
    );
    Ok(())
}

#[test]
fn epic_proportions_can_be_cast_at_instant_speed_and_pumps_a_creature() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 1, Step::Main1)
            .hand_named(0, ["Epic Proportions"])
            .battlefield(
                0,
                [
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                    ready(bear()),
                ],
            )
            .battlefield(1, [ready(free_creature("Ogre", 3, 4))]),
    )?;
    // The other seat holds priority first in its own turn; the Aura is cast when it passes.
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let bear = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Epic Proportions", &[bear])?;
    settle(&mut driver)?;
    assert_eq!(stats(&driver, "Grizzly Bears"), (7, 7));
    Ok(())
}

#[test]
fn squirrel_nest_gives_a_land_an_ability_that_makes_squirrels() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Squirrel Nest"])
            .battlefield(
                0,
                [
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                ],
            ),
    )?;
    // The fourth Forest wears the Aura, so it is not one of the three that pay for it.
    let fourth = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(&driver, *id) == "Forest")
        .nth(3)
        .unwrap();
    let land =
        mtg_engine::choose::Target::Object(driver.state().object(fourth).unwrap().reference());
    driver.begin_cast("Squirrel Nest")?;
    driver.choose_targets(&[land])?;
    driver.pay_tapping(&["Forest", "Forest", "Forest"], 0)?;
    settle(&mut driver)?;
    assert_eq!(
        host_name(&driver, "Squirrel Nest").as_deref(),
        Some("Forest")
    );
    driver.begin_activation("Forest")?;
    settle(&mut driver)?;
    assert_eq!(
        common::names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|n| *n == "Squirrel")
            .count(),
        1
    );
    Ok(())
}

#[test]
fn shielded_by_faith_moves_to_a_creature_that_enters_when_its_controller_says_so()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [bear()]).battlefield(
        0,
        [
            ready(forest()),
            ready(forest()),
            ready(free_creature("Wolf", 2, 2)),
            attached_to(ready(card("Shielded by Faith").unwrap()), 0, 2),
        ],
    ))?;
    assert_eq!(
        host_name(&driver, "Shielded by Faith").as_deref(),
        Some("Wolf")
    );
    let stamp = |driver: &Driver| {
        driver
            .state()
            .object(driver.id_of("Shielded by Faith").unwrap())
            .unwrap()
            .permanent()
            .unwrap()
            .timestamp()
    };
    let before = stamp(&driver);
    driver.cast("Grizzly Bears")?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(true)))?;
        } else if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        } else {
            driver.answer_passively()?;
        }
    }
    assert_eq!(
        host_name(&driver, "Shielded by Faith").as_deref(),
        Some("Grizzly Bears")
    );
    assert!(
        stamp(&driver) > before,
        "it took the time it became attached to the new host"
    );
    Ok(())
}

#[test]
fn sram_draws_a_card_for_an_aura_spell() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [mighty_aura().unwrap()])
            .battlefield(
                0,
                [
                    ready(forest()),
                    ready(island()),
                    ready(card("Sram, Senior Edificer").unwrap()),
                    ready(bear()),
                ],
            ),
    )?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Mighty Aura", &[bear])?;
    settle(&mut driver)?;
    let drawn = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::CardDrawn { player, .. } if *player == pid(0)))
        .count();
    assert_eq!(drawn, 1);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 1);
    Ok(())
}
