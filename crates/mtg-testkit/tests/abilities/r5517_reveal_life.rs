//! Goal: the r55-17 pieces play at tables of two to four: Thought Dissector reveals until an artifact or X cards,
//! whichever comes first, takes the artifact and sacrifices itself only when one was revealed, and mills the rest; Old
//! Stickfingers' cast trigger reveals until X creature cards, mills those and puts the rest on the bottom; Rain of Gore
//! turns life a spell or ability gives its own controller into a loss, and leaves life another player's spell gives and
//! the life of lifelink combat damage (its ruling); Invasion of Arcavios finds an instant or sorcery card in the
//! graveyard, outside the game finding nothing in Commander (the wish rulings).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ChoiceOption, NumberChoice, OptionChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, healer, island, keyworded_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A corpus card for no mana.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

fn zone_names(driver: &Driver, zone: Zone) -> Vec<String> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|object| object.card().name.clone())
        .collect()
}

fn count_named(driver: &Driver, zone: Zone, name: &str) -> usize {
    zone_names(driver, zone)
        .iter()
        .filter(|n| n.as_str() == name)
        .count()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(PlayerId(seat as u64)).unwrap().life()
}

/// Answers until the stack is empty: X is `x`, targets are `target` (or the first legal ones), mana is paid
/// automatically, anything else passively.
fn settle(driver: &mut Driver, x: u32, target: Option<Target>) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && state.resolving().is_none()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => {
                let chosen = match target {
                    Some(target) => vec![target],
                    None => options.legal.iter().take(1).copied().collect(),
                };
                Answer::ChooseTargets(TargetChoice::Targets(chosen))
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack did not empty".into()))
}

/// A creature with "{T}: You gain 1 life."
fn healer_card() -> Arc<CardDefinition> {
    healer("Mender", 0)
}

/// An artifact with no abilities.
fn trinket() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Trinket")
            .with_type(mtg_core::card::CardType::Artifact)
            .with_mana_cost(ManaCost::parse("{1}").unwrap()),
    )
}

#[rstest]
#[case::fewer_cards_than_the_artifact(2, 3, false)]
#[case::the_artifact_last(3, 3, true)]
#[case::the_artifact_first(5, 1, true)]
#[case::nothing(0, 1, false)]
fn r5517_thought_dissector_takes_an_artifact_before_x_cards(
    #[case] x: u32,
    #[case] depth: usize,
    #[case] taken: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let mut library: Vec<Arc<CardDefinition>> = (0..8).map(|_| forest()).collect();
    library[depth - 1] = trinket();
    let lands: Vec<_> = (0..5).map(|_| ready(island())).collect();
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            std::iter::once(ready(card("Thought Dissector").unwrap())).chain(lands),
        )
        .library(foe, library);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_player(foe);
    driver.begin_activation("Thought Dissector")?;
    settle(&mut driver, x, Some(target))?;
    let revealed = if taken { depth } else { x as usize };
    let ours = count_named(&driver, Zone::Battlefield, "Trinket");
    assert_eq!(ours, usize::from(taken));
    if taken {
        let id = driver.id_of("Trinket")?;
        assert_eq!(driver.state().object(id).unwrap().controller(), PlayerId(0));
    }
    assert_eq!(
        count_named(&driver, Zone::Battlefield, "Thought Dissector"),
        usize::from(!taken),
        "sacrificed only when it took an artifact"
    );
    let milled = zone_names(&driver, Zone::Graveyard(PlayerId(foe as u64)));
    assert_eq!(milled.len(), revealed - usize::from(taken));
    assert!(milled.iter().all(|name| name == "Forest"));
    Ok(())
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::none(0)]
fn r5517_old_stickfingers_mills_x_creature_cards(
    #[case] x: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Top first: Forest, Bear, Forest, Bear, Bear, Forest, Forest.
    let library = vec![
        forest(),
        bear(),
        forest(),
        bear(),
        bear(),
        forest(),
        forest(),
    ];
    let lands: Vec<_> = (0..8)
        .map(|_| ready(mtg_testkit::cards::land(Color::Black)))
        .collect();
    let more: Vec<_> = (0..2).map(|_| ready(forest())).collect();
    let scenario = Scenario::new(seats)
        .hand(0, [card("Old Stickfingers").unwrap()])
        .battlefield(0, lands.into_iter().chain(more))
        .library(0, library);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Old Stickfingers")?;
    settle(&mut driver, x, None)?;
    // With X 0 nothing is milled and the 0/0 Old Stickfingers dies.
    let graveyard: Vec<String> = zone_names(&driver, Zone::Graveyard(PlayerId(0)))
        .into_iter()
        .filter(|name| name != "Old Stickfingers")
        .collect();
    assert_eq!(graveyard.len(), x as usize);
    assert!(graveyard.iter().all(|name| name == "Grizzly Bears"));
    // The revealed noncreature cards went under the rest: the top of the library is the first card not revealed.
    let revealed = [0, 2, 4, 5][x as usize];
    let library = zone_names(&driver, Zone::Library(PlayerId(0)));
    let top: Vec<&str> = library.iter().rev().map(String::as_str).collect();
    let expected_top = [
        "Forest",
        "Grizzly Bears",
        "Forest",
        "Grizzly Bears",
        "Grizzly Bears",
        "Forest",
        "Forest",
    ];
    assert_eq!(top.first().copied(), expected_top.get(revealed).copied());
    assert_eq!(library.len() + graveyard.len(), 7);
    Ok(())
}

#[rstest]
fn r5517_rain_of_gore_turns_an_abilitys_own_gain_into_a_loss(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
) -> Result<(), ScriptError> {
    let healer = seats - 1;
    let mut scenario = Scenario::new(seats);
    scenario = if owner == healer {
        scenario.battlefield(owner, [ready(free("Rain of Gore")), ready(healer_card())])
    } else {
        scenario
            .battlefield(owner, [ready(free("Rain of Gore"))])
            .battlefield(healer, [ready(healer_card())])
    };
    let mut driver = Driver::from_scenario(scenario.at(2, seats - 1, Step::Main1))?;
    driver.begin_activation("Mender")?;
    settle(&mut driver, 0, None)?;
    assert_eq!(life(&driver, seats - 1), 19);
    Ok(())
}

#[rstest]
fn r5517_rain_of_gore_leaves_life_another_players_spell_gives(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Swords to Plowshares: the exiled creature's controller gains life equal to its power.
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(free("Rain of Gore")),
                ready(mtg_testkit::cards::plains()),
            ],
        )
        .hand(0, [card("Swords to Plowshares").unwrap()])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.begin_cast("Swords to Plowshares")?;
    settle(&mut driver, 0, Some(bear))?;
    assert_eq!(life(&driver, 1), 22);
    assert_eq!(life(&driver, 0), 20);
    Ok(())
}

#[rstest]
fn r5517_rain_of_gore_leaves_lifelink_combat_damage(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let leech = keyworded_creature("Leech", 0, &[], 2, 2, &[Keyword::Lifelink]);
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free("Rain of Gore")), ready(leech)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Leech", 1)])?;
    driver.pass_until(Step::EndCombat)?;
    assert_eq!((life(&driver, 0), life(&driver, 1)), (22, 18));
    Ok(())
}

#[rstest]
fn r5517_invasion_of_arcavios_finds_a_spell_in_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(
            0,
            [free("Invasion of Arcavios // Invocation of the Founders")],
        )
        .graveyard(0, [card("Opt").unwrap()])
        .library(0, (0..5).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario)?;
    let events = driver.cast("Invasion of Arcavios")?;
    assert!(!events.is_empty(), "{:?}", driver.decision()?);
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && state.resolving().is_none()
        {
            break;
        }
        match &decision.kind {
            // Outside the game is no choice: only the library and the graveyard are offered.
            DecisionKind::ChooseOption(options)
                if options
                    .options
                    .iter()
                    .all(|option| matches!(option, ChoiceOption::Label(_))) =>
            {
                let labels: Vec<String> = options
                    .options
                    .iter()
                    .filter_map(|option| match option {
                        ChoiceOption::Label(label) => Some(label.to_lowercase()),
                        _ => None,
                    })
                    .collect();
                assert!(
                    labels.iter().all(|label| !label.contains("outside")),
                    "{labels:?}"
                );
                let at = labels
                    .iter()
                    .position(|label| label == "graveyard")
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    assert_eq!(
        count_named(&driver, Zone::Hand(PlayerId(0)), "Opt"),
        1,
        "{:?} {:?} {:?}",
        zone_names(&driver, Zone::Hand(PlayerId(0))),
        zone_names(&driver, Zone::Battlefield),
        driver
            .engine()
            .events()
            .iter()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// Seat 0 casts `first` (when given) and then each spell of `then`, all for no mana, answering everything with the first
/// legal choice; every other seat has a library of Forests.
fn r5517_cast_in_turn(
    seats: usize,
    first: Option<&str>,
    then: &[&str],
) -> Result<Driver, ScriptError> {
    let hand: Vec<_> = first
        .into_iter()
        .chain(then.iter().copied())
        .map(free)
        .collect();
    let mut scenario = Scenario::new(seats).hand(0, hand);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for name in first.into_iter().chain(then.iter().copied()) {
        driver.begin_cast(name)?;
        settle(&mut driver, 0, None)?;
    }
    Ok(driver)
}

fn r5517_controllers(driver: &Driver, name: &str) -> Vec<PlayerId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name)
        .map(|object| object.controller())
        .collect()
}

#[rstest]
#[case::crafty_cutpurse(Some("Crafty Cutpurse"), true)]
#[case::gather_specimens(Some("Gather Specimens"), true)]
#[case::neither(None, false)]
fn r5517_tokens_an_opponent_would_get_come_to_you(
    #[case] first: Option<&str>,
    #[case] taken: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Hunted Horror: "When this creature enters, target opponent creates two 3/3 green Centaur creature tokens".
    let driver = r5517_cast_in_turn(seats, first, &["Hunted Horror"])?;
    let centaurs = r5517_controllers(&driver, "Centaur");
    assert_eq!(centaurs.len(), 2);
    assert!(
        centaurs
            .iter()
            .all(|controller| (*controller == PlayerId(0)) == taken),
        "{centaurs:?}"
    );
    Ok(())
}

#[rstest]
#[case::gather_specimens(Some("Gather Specimens"), true)]
#[case::crafty_cutpurse_takes_only_tokens(Some("Crafty Cutpurse"), false)]
#[case::neither(None, false)]
fn r5517_a_creature_entering_under_an_opponent_enters_under_you(
    #[case] first: Option<&str>,
    #[case] taken: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Xantcha enters under an opponent's control (its own replacement first, CR 616.1a), then Gather Specimens moves it
    // to its caster (CR 616.1b).
    let driver = r5517_cast_in_turn(seats, first, &["Xantcha, Sleeper Agent"])?;
    let xantcha = r5517_controllers(&driver, "Xantcha, Sleeper Agent");
    assert_eq!(xantcha.len(), 1);
    assert_eq!(xantcha[0] == PlayerId(0), taken, "{xantcha:?}");
    Ok(())
}
