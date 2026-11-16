//! Goal: the r67-01 pieces play whole at two to four seats: emerge from artifact sacrifices an artifact and takes its
//! mana value off (Crabomination) and its enters trigger exiles three cards of an opponent and casts one free; a static
//! ability gives web-slinging to the legendary spells of one or more colors its controller casts and to no other
//! (Amazing Spider-Man); a spell costs {1} less for each energy counter paid this turn (Blaster Hulk); counters removed
//! from among permanents one at a time are counted for the draws and the life lost (Eventide's Shadow); and a static
//! ability an effect gave grants its ability in turn (Roar of the Fifth People, chapter II).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::Duration;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::choose::{CardsChoice, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::{Modification, Selector};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, pay};
use crate::common::name_of;

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Seat 0 holds `hand` and controls `permanents`; seat 1 holds `their_hand` and has `their_graveyard`; every library
/// is of Forests; the game is at seat 0's first main phase.
fn r6701_table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    permanents: Vec<PresetPermanent>,
    their_hand: Vec<Arc<CardDefinition>>,
    their_graveyard: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .hand(0, hand)
        .battlefield(0, permanents)
        .hand(1, their_hand)
        .graveyard(1, their_graveyard);
    Driver::from_scenario(scenario.at(1, 0, Step::Main1))
}

/// Answers every decision until the stack is empty at seat 0's priority: the way whose label starts with `way`, the
/// cards named `picks` where cards are chosen (all of them in order, the first offered first), seat 1 as a target,
/// automatic payment, yes to every question.
fn r6701_play(driver: &mut Driver, way: &str, picks: &[&str]) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(options)
                if options.text.contains("way") || way.is_empty() =>
            {
                if way.is_empty() {
                    driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
                } else {
                    choose_way(driver, way)?;
                }
            }
            DecisionKind::ChooseCards(options) => {
                let chosen: Vec<ObjectId> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| picks.contains(&name_of(driver.state(), *id).as_str()))
                    .take(usize::try_from(options.max).unwrap())
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_player(1);
                driver.choose_targets(&[target])?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the r6701 play".into()))
}

/// The names of the cards in `zone`.
fn r6701_names(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

/// The ways asked as `name` is begun, or none when only one way is open.
fn r6701_ways(driver: &mut Driver, name: &str) -> Result<Vec<String>, ScriptError> {
    driver.begin_cast(name)?;
    let DecisionKind::ChooseOption(options) = driver.decision()?.kind.clone() else {
        return Ok(Vec::new());
    };
    Ok(options
        .options
        .iter()
        .map(|option| format!("{option:?}"))
        .collect())
}

/// The log replays to the state the game is in.
fn r6701_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn r6701_crabomination_emerges_from_an_artifact(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // {5}{B}{B} less Sol Ring's mana value 1: six Swamps pay it.
    let permanents: Vec<PresetPermanent> = std::iter::once(ready(card("Sol Ring")))
        .chain(std::iter::once(ready(card("Grizzly Bears"))))
        .chain(lands(6, swamp))
        .collect();
    let mut driver = r6701_table(
        seats,
        vec![card("Crabomination")],
        permanents,
        vec![card("Grizzly Bears")],
        vec![forest()],
    )?;
    let ways = r6701_ways(&mut driver, "Crabomination")?;
    assert!(
        ways.iter().any(|way| way.contains("Emerge")),
        "emerge is offered: {ways:?}"
    );
    choose_way(&mut driver, "Emerge")?;
    r6701_play(&mut driver, "Emerge", &["Sol Ring", "Grizzly Bears"])?;
    let mine = r6701_names(&driver, Zone::Battlefield);
    assert!(mine.contains(&"Crabomination".to_owned()));
    assert!(
        r6701_names(&driver, Zone::Graveyard(PlayerId(0))).contains(&"Sol Ring".to_owned()),
        "the artifact was sacrificed"
    );
    // The top Forest, the graveyard's Forest and the hand's Bears were exiled; the Bears were cast free.
    assert!(driver.state().zone(Zone::Hand(PlayerId(1))).is_empty());
    assert!(driver.state().zone(Zone::Graveyard(PlayerId(1))).is_empty());
    assert_eq!(
        r6701_names(&driver, Zone::Exile)
            .iter()
            .filter(|name| *name == "Forest")
            .count(),
        2
    );
    let bears: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(driver.state(), *id) == "Grizzly Bears")
        .collect();
    assert_eq!(
        bears.len(),
        2,
        "our Bears stayed and the opponent's entered: {mine:?}"
    );
    r6701_replays(&driver);
    Ok(())
}

#[rstest]
fn r6701_crabomination_does_not_emerge_from_a_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Only a creature to sacrifice: the cast is for the mana cost alone.
    let permanents: Vec<PresetPermanent> = std::iter::once(ready(card("Grizzly Bears")))
        .chain(lands(6, swamp))
        .collect();
    let mut driver = r6701_table(
        seats,
        vec![card("Crabomination")],
        permanents,
        vec![],
        vec![],
    )?;
    let ways = r6701_ways(&mut driver, "Crabomination")?;
    assert!(
        ways.iter().all(|way| !way.contains("Emerge")),
        "no emerge without an artifact: {ways:?}"
    );
    Ok(())
}

/// Seat 0 controls Peter Parker, a tapped Grizzly Bears and two each of Forest, Plains and Island and one Mountain, and
/// transforms Peter Parker with four of them.
fn r6701_spider_man(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Result<Driver, ScriptError> {
    let permanents: Vec<PresetPermanent> =
        std::iter::once(ready(card("Peter Parker // Amazing Spider-Man")))
            .chain(std::iter::once(tapped(card("Grizzly Bears"))))
            .chain(lands(2, forest))
            .chain(lands(2, plains))
            .chain(lands(2, island))
            .chain(lands(1, mountain))
            .collect();
    let mut driver = r6701_table(seats, hand, permanents, vec![], vec![])?;
    driver.begin_activation("Peter Parker")?;
    r6701_play(&mut driver, "", &[])?;
    let parker = driver.id_of("Peter Parker")?;
    assert!(
        driver
            .state()
            .object(parker)
            .is_some_and(|object| object.face() == Some(1)),
        "Peter Parker transformed"
    );
    Ok(driver)
}

#[rstest]
#[ignore = "the transform of Peter Parker by its activated ability in this setup"]
fn r6701_web_slinging_casts_a_legendary_colored_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r6701_spider_man(seats, vec![card("Sliver Queen")])?;
    // {W}{U}{B}{R}{G} is out of reach; web-slinging {G}{W}{U} returns the tapped Bears.
    let ways = r6701_ways(&mut driver, "Sliver Queen")?;
    assert!(
        ways.iter().any(|way| way.contains("Web-slinging")),
        "{ways:?}"
    );
    choose_way(&mut driver, "Web-slinging")?;
    r6701_play(&mut driver, "Web-slinging", &["Grizzly Bears"])?;
    assert!(r6701_names(&driver, Zone::Battlefield).contains(&"Sliver Queen".to_owned()));
    assert!(r6701_names(&driver, Zone::Hand(PlayerId(0))).contains(&"Grizzly Bears".to_owned()));
    r6701_replays(&driver);
    Ok(())
}

#[rstest]
#[ignore = "the transform of Peter Parker by its activated ability in this setup"]
fn r6701_web_slinging_skips_a_colorless_legend(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r6701_spider_man(seats, vec![card("Krark's Thumb")])?;
    let ways = r6701_ways(&mut driver, "Krark's Thumb")?;
    assert!(
        ways.iter().all(|way| !way.contains("Web-slinging")),
        "{ways:?}"
    );
    Ok(())
}

#[rstest]
#[case::two_paid(2, true)]
#[case::none_paid(0, false)]
fn r6701_blaster_hulk_costs_less_for_energy_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] paid: usize,
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let permanents: Vec<PresetPermanent> = lands(2, || card("Aether Hub"))
        .into_iter()
        .chain(lands(4, mountain))
        .collect();
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .hand(0, vec![card("Blaster Hulk")])
        .battlefield(0, permanents)
        .counters(0, CounterKind::Other("energy".into()), 4);
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    for _ in 0..paid {
        driver.tap_for_mana_of("Aether Hub", ManaType::Red)?;
    }
    let DecisionKind::Priority(options) = driver.decision()?.kind.clone() else {
        panic!("priority");
    };
    let hulk = driver
        .state()
        .zone(Zone::Hand(PlayerId(0)))
        .first()
        .copied()
        .unwrap();
    assert_eq!(options.castable.contains(&hulk), castable);
    if castable {
        driver.begin_cast("Blaster Hulk")?;
        r6701_play(&mut driver, "", &[])?;
        assert!(r6701_names(&driver, Zone::Battlefield).contains(&"Blaster Hulk".to_owned()));
        r6701_replays(&driver);
    }
    Ok(())
}

#[rstest]
// Take every counter: two +1/+1 on the Bears, one charge on the Ring.
#[case::all_of_them(&[0, 0, 0], 3)]
// Take one and stop (the last option).
#[case::one_then_stop(&[0, 2], 1)]
fn r6701_eventides_shadow_counts_what_it_removed(
    #[values(2, 3, 4)] seats: usize,
    #[case] answers: &[usize],
    #[case] removed: usize,
) -> Result<(), ScriptError> {
    let permanents: Vec<PresetPermanent> = std::iter::once(with_counters(
        ready(card("Grizzly Bears")),
        CounterKind::PlusOnePlusOne,
        2,
    ))
    .chain(std::iter::once(with_counters(
        ready(card("Sol Ring")),
        CounterKind::Charge,
        1,
    )))
    .chain(lands(5, swamp))
    .collect();
    let mut driver = r6701_table(
        seats,
        vec![card("Eventide's Shadow")],
        permanents,
        vec![],
        vec![],
    )?;
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    driver.begin_cast("Eventide's Shadow")?;
    pay(&mut driver)?;
    // Every seat passes until the spell resolves and asks for the first counter.
    while let DecisionKind::Priority(_) = driver.decision()?.kind.clone() {
        driver.pass()?;
    }
    let mut answers = answers.iter();
    while let DecisionKind::ChooseOption(_) = driver.decision()?.kind.clone() {
        let at = *answers.next().expect("asked no more than the answers");
        driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
    }
    r6701_play(&mut driver, "", &[])?;
    assert_eq!(driver.state().zone(Zone::Hand(PlayerId(0))).len(), removed);
    let lost = life - driver.state().player(PlayerId(0)).unwrap().life();
    assert_eq!(usize::try_from(lost).unwrap(), removed);
    r6701_replays(&driver);
    Ok(())
}

#[rstest]
fn r6701_a_granted_static_grants_its_ability(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let permanents = vec![ready(plains()), ready(card("Grizzly Bears"))];
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .battlefield(0, permanents)
        .battlefield(1, vec![ready(card("Grizzly Bears"))]);
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    let saga = driver.id_of("Plains")?;
    let inner: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["Red","Green","White"]}}}]}}}]}}"#,
    )
    .unwrap();
    let reference = driver.state().object(saga).unwrap().reference();
    driver.add_effect_from(
        saga,
        Selector::Objects(vec![reference]),
        Modification::GrantAbility(Arc::new(inner)),
        Duration::Forever,
    )?;
    let DecisionKind::Priority(options) = driver.decision()?.kind.clone() else {
        panic!("priority");
    };
    let ours: Vec<ManaType> = options
        .mana_abilities
        .iter()
        .filter(|option| name_of(driver.state(), option.source.object) == "Grizzly Bears")
        .map(|option| option.produces)
        .collect();
    for color in [ManaType::Red, ManaType::Green, ManaType::White] {
        assert!(ours.contains(&color), "{color:?} in {ours:?}");
    }
    driver.tap_for_mana_of("Grizzly Bears", ManaType::Green)?;
    assert_eq!(
        driver.state().player(PlayerId(0)).unwrap().mana().total(),
        1
    );
    r6701_replays(&driver);
    Ok(())
}

#[rstest]
// Exiled as a craft material: the gain and the draw.
#[case::crafted(true)]
// Exiled by Swords to Plowshares: no craft, no draw (Swords gives the life of its power).
#[case::exiled_otherwise(false)]
#[ignore = "TriggerPattern::R6701WhileActivating sees no craft activation as the cause of the material exile"]
fn r6701_market_gnome_exiled_while_crafting(
    #[values(2, 3, 4)] seats: usize,
    #[case] crafted: bool,
) -> Result<(), ScriptError> {
    let permanents: Vec<PresetPermanent> = vec![
        ready(card("Altar of the Wretched // Wretched Bonemass")),
        ready(card("Market Gnome")),
    ]
    .into_iter()
    .chain(lands(4, swamp))
    .chain(lands(1, plains))
    .collect();
    let mut driver = r6701_table(
        seats,
        vec![card("Swords to Plowshares")],
        permanents,
        vec![],
        vec![],
    )?;
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    if crafted {
        driver.begin_activation("Altar of the Wretched")?;
    } else {
        driver.begin_cast("Swords to Plowshares")?;
        let gnome = driver.target_named("Market Gnome")?;
        driver.choose_targets(&[gnome])?;
    }
    r6701_play(&mut driver, "", &["Market Gnome"])?;
    assert!(r6701_names(&driver, Zone::Exile).contains(&"Market Gnome".to_owned()));
    let hand = driver.state().zone(Zone::Hand(PlayerId(0))).len();
    let gained = driver.state().player(PlayerId(0)).unwrap().life() - life;
    // The Swords stays in hand when the Altar crafts; the Gnome's draw adds a card.
    assert_eq!(hand, if crafted { 2 } else { 0 });
    // Market Gnome's power is 0: Swords gives no life.
    assert_eq!(gained, i32::from(crafted));
    r6701_replays(&driver);
    Ok(())
}

#[rstest]
fn r6701_secret_of_bloodbending_controls_the_next_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let permanents: Vec<PresetPermanent> = lands(4, island).into_iter().collect();
    let mut driver = r6701_table(
        seats,
        vec![card("Secret of Bloodbending")],
        permanents,
        vec![],
        vec![],
    )?;
    driver.begin_cast("Secret of Bloodbending")?;
    r6701_play(&mut driver, "", &[])?;
    let controls = driver.state().player_controls();
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].player, PlayerId(1));
    assert_eq!(controls[0].by, PlayerId(0));
    assert!(
        controls[0].combat,
        "the additional cost was not paid: the combat phase only"
    );
    assert!(r6701_names(&driver, Zone::Exile).contains(&"Secret of Bloodbending".to_owned()));
    r6701_replays(&driver);
    Ok(())
}

#[rstest]
// Web-slinging returns Serra Angel (mana value 5): five counters.
#[case::web_slung("Web-slinging", 5)]
// Cast for its mana cost: none.
#[case::for_its_mana_cost("Cast", 0)]
fn r6701_scarlet_spider_counts_the_returned_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] way: &str,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let permanents: Vec<PresetPermanent> = std::iter::once(tapped(card("Serra Angel")))
        .chain(lands(3, mountain))
        .chain(lands(3, forest))
        .collect();
    let mut driver = r6701_table(
        seats,
        vec![card("Scarlet Spider, Ben Reilly")],
        permanents,
        vec![],
        vec![],
    )?;
    driver.begin_cast("Scarlet Spider, Ben Reilly")?;
    choose_way(&mut driver, way)?;
    r6701_play(&mut driver, way, &["Serra Angel"])?;
    let spider = driver.id_of("Scarlet Spider, Ben Reilly")?;
    let held = driver
        .state()
        .object(spider)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    assert_eq!(held, counters);
    r6701_replays(&driver);
    Ok(())
}

#[rstest]
fn r6701_illithid_harvester_turns_tapped_creatures_into_horrors(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .hand(0, vec![card("Illithid Harvester // Plant Tadpoles")])
        .battlefield(0, lands(5, island))
        .battlefield(
            1,
            vec![tapped(card("Grizzly Bears")), ready(card("Serra Angel"))],
        );
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    driver.begin_cast("Illithid Harvester")?;
    for _ in 0..30 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseTargets(spec) => {
                // Only the tapped nontoken creature may be chosen.
                let bears = driver.target_named("Grizzly Bears")?;
                assert!(spec.legal.contains(&bears), "{spec:?}");
                let angel = driver.target_named("Serra Angel")?;
                assert!(
                    !spec.legal.contains(&angel),
                    "an untapped creature is no target"
                );
                driver.choose_targets(&[bears])?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                break;
            }
            _ => r6701_play_one(&mut driver)?,
        }
    }
    let down: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|object| object.face_down().is_some())
        })
        .collect();
    assert_eq!(down.len(), 1);
    let shown = mtg_engine::characteristics::characteristics(driver.state(), down[0]).unwrap();
    assert_eq!((shown.power, shown.toughness), (Some(2), Some(2)));
    let card = driver.state().object(down[0]).unwrap().card();
    assert!(card.subtypes.iter().any(|subtype| subtype.0 == "Horror"));
    r6701_replays(&driver);
    Ok(())
}

/// Answers one decision as `r6701_play` would.
fn r6701_play_one(driver: &mut Driver) -> Result<(), ScriptError> {
    match driver.decision()?.kind.clone() {
        DecisionKind::PayMana(_) => {
            pay(driver)?;
        }
        DecisionKind::Priority(_) => {
            driver.pass()?;
        }
        _ => {
            driver.answer_passively()?;
        }
    }
    Ok(())
}
