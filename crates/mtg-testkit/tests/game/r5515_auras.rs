//! Goal: Auras put onto the battlefield attached to a player or restricted to some hosts play as their cards say at tables
//! of two to four seats: Retether returns only the Aura cards that can enchant a creature and attaches each to a creature
//! (an Aura that enchants players or lands stays in the graveyard, CR 303.4g and the card's reminder text), Bitterheart
//! Witch's dies trigger puts a Curse from the library onto the battlefield attached to the target player, a Curse put onto
//! the battlefield without being cast and with no player named (Open the Vaults) asks which player it enchants
//! (CR 303.4f), and Radiant
//! Grace returns transformed under its owner's control attached to the target opponent.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until the stack is empty: yes to every "may", the target `seat` for a trigger's player target, the first
/// card of every search, and the option `option` of every choice of options.
fn r5515_settle(driver: &mut Driver, seat: usize, option: usize) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        let answer = r5515_answer(driver, seat, option)?;
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The answer `r5515_settle` gives to the pending decision.
fn r5515_answer(driver: &Driver, seat: usize, option: usize) -> Result<Answer, ScriptError> {
    let decision = driver.decision()?.clone();
    Ok(match &decision.kind {
        DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
        DecisionKind::ChooseTargets(options)
            if !options.can_cancel && options.legal.contains(&driver.target_player(seat)) =>
        {
            Answer::ChooseTargets(TargetChoice::Targets(vec![driver.target_player(seat)]))
        }
        DecisionKind::ChooseCards(options) => Answer::ChooseCards(CardsChoice(
            options
                .from
                .iter()
                .copied()
                .take((options.min as usize).max(1).min(options.max as usize))
                .collect(),
        )),
        DecisionKind::ChooseOption(options) => {
            Answer::ChooseOption(OptionChoice(option.min(options.options.len() - 1)))
        }
        _ => Driver::passive_answer(&decision),
    })
}

/// The card named `name` in seat `seat`'s graveyard, if it is there.
fn r5515_in_graveyard(driver: &Driver, seat: usize, name: &str) -> Option<ObjectId> {
    driver
        .state()
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == name)
}

/// Seat `seat`'s creature dies: it gets -9/-9 and the state-based actions put it into the graveyard.
fn r5515_kill(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let id = driver.id_of(name)?;
    driver.add_effect(
        driver.on(id),
        Modification::ModifyPowerToughness {
            power: -9,
            toughness: -9,
        },
        Duration::UntilEndOfTurn,
    )?;
    Ok(())
}

/// Seat 0 with four Plains and four Swamps ready, `hand` and `graveyard`, and a Bear on every seat.
fn r5515_table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    graveyard: Vec<Arc<CardDefinition>>,
    library: Vec<Arc<CardDefinition>>,
    mine: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut lands: Vec<_> = (0..4).map(|_| ready(plains())).collect();
    lands.extend((0..4).map(|_| ready(swamp())));
    lands.extend(mine.into_iter().map(ready));
    let mut scenario = Scenario::new(seats)
        .battlefield(0, lands)
        .hand(0, hand)
        .graveyard(0, graveyard)
        .library(0, library);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    Driver::from_scenario(scenario.at(1, 0, Step::Main1))
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5515_retether_returns_only_auras_that_can_enchant_a_creature(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_table(
        seats,
        vec![card("Retether").unwrap()],
        vec![
            card("Pacifism").unwrap(),
            card("Curse of Predation").unwrap(),
            card("Wild Growth").unwrap(),
        ],
        Vec::new(),
        Vec::new(),
    )?;
    driver.cast("Retether")?;
    r5515_settle(&mut driver, 1, 0)?;
    let pacifism = driver.id_of("Pacifism")?;
    let host = driver.state().host_of(pacifism).expect("Pacifism enchants");
    assert!(
        driver
            .state()
            .object(host)
            .is_some_and(|object| object.card().name == "Grizzly Bears"),
        "Pacifism enchants a creature"
    );
    assert!(r5515_in_graveyard(&driver, 0, "Curse of Predation").is_some());
    assert!(r5515_in_graveyard(&driver, 0, "Wild Growth").is_some());
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 3)]
#[case::its_own_controller(3, 0)]
fn r5515_bitterheart_witch_puts_a_curse_onto_the_target_player(
    #[case] seats: usize,
    #[case] victim: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_table(
        seats,
        Vec::new(),
        Vec::new(),
        vec![card("Curse of Predation").unwrap(), plains()],
        vec![card("Bitterheart Witch").unwrap()],
    )?;
    r5515_kill(&mut driver, "Bitterheart Witch")?;
    r5515_drive(&mut driver, victim, 0, |driver| {
        driver.id_of("Curse of Predation").is_ok() && driver.state().stack().is_empty()
    })?;
    let curse = driver.id_of("Curse of Predation")?;
    assert_eq!(driver.state().player_host_of(curse), Some(pid(victim)));
    assert_eq!(driver.state().object(curse).unwrap().controller(), pid(0));
    Ok(())
}

#[rstest]
#[case::two_seats(2, 0, 0)]
#[case::three_seats_first(3, 0, 0)]
#[case::three_seats_second(3, 1, 1)]
#[case::four_seats_last(4, 3, 3)]
fn r5515_a_curse_returned_without_a_named_player_asks_which_player(
    #[case] seats: usize,
    #[case] option: usize,
    #[case] enchanted: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_table(
        seats,
        vec![card("Open the Vaults").unwrap()],
        vec![card("Curse of Predation").unwrap()],
        Vec::new(),
        Vec::new(),
    )?;
    driver.cast("Open the Vaults")?;
    r5515_settle(&mut driver, 1, option)?;
    let curse = driver.id_of("Curse of Predation")?;
    assert_eq!(driver.state().player_host_of(curse), Some(pid(enchanted)));
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 1)]
fn r5515_radiant_grace_returns_transformed_on_the_target_opponent(
    #[case] seats: usize,
    #[case] victim: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_table(
        seats,
        vec![card("Radiant Grace // Radiant Restraints").unwrap()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )?;
    driver.begin_cast("Radiant Grace")?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bear])?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    r5515_settle(&mut driver, 1, 0)?;
    let grace = driver.id_of("Radiant Grace")?;
    let host = driver.state().host_of(grace).expect("Radiant Grace enchants");
    let name = driver.state().object(host).unwrap().card().name.clone();
    r5515_kill(&mut driver, &name)?;
    r5515_drive(&mut driver, victim, 0, |driver| {
        driver.id_of("Radiant Restraints").is_ok() && driver.state().stack().is_empty()
    })?;
    let restraints = driver.id_of("Radiant Restraints")?;
    assert_eq!(driver.state().player_host_of(restraints), Some(pid(victim)));
    assert_eq!(driver.state().object(restraints).unwrap().controller(), pid(0));
    Ok(())
}

/// Answers as `r5515_settle` does, passing priority too, until `done` holds with a decision pending.
fn r5515_drive(
    driver: &mut Driver,
    seat: usize,
    option: usize,
    done: impl Fn(&Driver) -> bool,
) -> Result<(), ScriptError> {
    for _ in 0..400 {
        if done(driver) && driver.decision().is_ok() {
            return Ok(());
        }
        let answer = r5515_answer(driver, seat, option)?;
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the drive".into()))
}

/// Seat `active` is on turn 1 in its first main phase; seat 0 has `mine` (each enchanting the player in `on`, when
/// given) and `library`, every other seat a Bear and four Plains.
fn r5515_board(
    seats: usize,
    active: usize,
    mine: Vec<(Arc<CardDefinition>, Option<usize>)>,
    library: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mine: Vec<_> = mine
        .into_iter()
        .map(|(card, on)| match on {
            Some(seat) => mtg_testkit::enchanting(ready(card), seat),
            None => ready(card),
        })
        .collect();
    let mut scenario = Scenario::new(seats)
        .battlefield(0, mine)
        .library(0, library);
    for seat in 1..seats {
        let mut theirs = vec![ready(bear())];
        theirs.extend((0..4).map(|_| ready(plains())));
        scenario = scenario
            .battlefield(seat, theirs)
            .hand(seat, [mtg_testkit::cards::instant("Hush", 0, &[])])
            .library(seat, (0..5).map(|_| plains()).collect::<Vec<_>>());
    }
    Driver::from_scenario(scenario.at(1, active, Step::Main1))
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 3)]
fn r5515_soul_seizer_transforms_and_takes_a_creature_of_the_player_it_hit(
    #[case] seats: usize,
    #[case] victim: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_board(
        seats,
        0,
        vec![(card("Soul Seizer // Ghastly Haunting").unwrap(), None)],
        vec![plains()],
    )?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Soul Seizer", victim)])?;
    r5515_drive(&mut driver, victim, 0, |driver| {
        driver.state().turn().step() == Step::Main2
    })?;
    let haunting = driver.id_of("Ghastly Haunting")?;
    let bear = driver.state().host_of(haunting).expect("attached");
    assert_eq!(
        mtg_engine::characteristics::controller_of(driver.state(), bear),
        Some(pid(0))
    );
    assert_eq!(driver.state().object(bear).unwrap().owner(), pid(victim));
    Ok(())
}

#[rstest]
#[case::attacked_two_seats(2, 1, 0, true)]
#[case::attacked_three_seats(3, 2, 0, true)]
#[case::another_player_attacked(3, 1, 2, false)]
#[case::another_player_attacked_four_seats(4, 3, 1, false)]
fn r5515_kazuul_makes_an_ogre_unless_the_attacker_pays(
    #[case] seats: usize,
    #[case] attacker: usize,
    #[case] defender: usize,
    #[case] ogre: bool,
) -> Result<(), ScriptError> {
    let mut driver = r5515_board(
        seats,
        attacker,
        vec![(card("Kazuul, Tyrant of the Cliffs").unwrap(), None)],
        vec![plains()],
    )?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", defender)])?;
    // The passive attacker declines to pay.
    for _ in 0..50 {
        if driver.state().turn().step() == Step::DeclareBlockers {
            break;
        }
        driver.answer_passively()?;
    }
    let ogres = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Ogre")
        .count();
    assert_eq!(ogres, usize::from(ogre));
    Ok(())
}

#[rstest]
#[case::two_seats(2, 0, 1)]
#[case::three_seats_first(3, 0, 1)]
#[case::three_seats_second(3, 1, 2)]
#[case::four_seats(4, 2, 3)]
fn r5515_lynde_moves_a_curse_from_you_to_an_opponent_and_draws_two(
    #[case] seats: usize,
    #[case] option: usize,
    #[case] enchanted: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_board(
        seats,
        seats - 1,
        vec![
            (card("Lynde, Cheerful Tormentor").unwrap(), None),
            (card("Curse of Predation").unwrap(), Some(0)),
        ],
        (0..5).map(|_| plains()).collect(),
    )?;
    r5515_drive(&mut driver, 0, option, |driver| {
        driver.state().turn().active() == pid(0) && driver.state().turn().step() == Step::Main1
    })?;
    let curse = driver.id_of("Curse of Predation")?;
    assert_eq!(driver.state().player_host_of(curse), Some(pid(enchanted)));
    // Two cards from Lynde and the draw of the turn.
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 3);
    Ok(())
}

#[rstest]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5515_lynde_returns_a_curse_put_into_your_graveyard_attached_to_you(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let leaving = seats - 1;
    let mut driver = r5515_board(
        seats,
        0,
        vec![
            (card("Lynde, Cheerful Tormentor").unwrap(), None),
            (card("Curse of Predation").unwrap(), Some(leaving)),
        ],
        vec![plains()],
    )?;
    driver.concede(pid(leaving))?;
    r5515_drive(&mut driver, 0, 0, |driver| driver.state().turn().number() == 2)?;
    let curse = driver.id_of("Curse of Predation")?;
    assert_eq!(driver.state().player_host_of(curse), Some(pid(0)));
    Ok(())
}

#[rstest]
#[case::two_seats_stays(2)]
#[case::three_seats_moves(3)]
#[case::four_seats_moves(4)]
fn r5515_maddening_hex_hurts_the_caster_and_moves_to_another_opponent(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_board(
        seats,
        1,
        vec![(card("Maddening Hex").unwrap(), Some(1))],
        vec![plains()],
    )?;
    let life = driver.state().player(pid(1)).unwrap().life();
    driver.cast("Hush")?;
    r5515_settle(&mut driver, 1, 0)?;
    let lost = life - driver.state().player(pid(1)).unwrap().life();
    assert!((1..=6).contains(&lost), "a d6 of damage: {lost}");
    let hex = driver.id_of("Maddening Hex")?;
    let now = driver.state().player_host_of(hex).expect("attached");
    if seats == 2 {
        assert_eq!(now, pid(1), "no other opponent to move to");
    } else {
        assert!(now != pid(1) && now != pid(0), "another opponent: {now:?}");
    }
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5515_curse_of_misfortunes_finds_a_curse_of_another_name(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5515_board(
        seats,
        seats - 1,
        vec![
            (card("Curse of Misfortunes").unwrap(), Some(1)),
            (card("Curse of Predation").unwrap(), Some(1)),
        ],
        vec![
            card("Curse of Predation").unwrap(),
            card("Curse of the Forsaken").unwrap(),
            plains(),
        ],
    )?;
    r5515_drive(&mut driver, 1, 0, |driver| {
        driver.state().turn().active() == pid(0) && driver.state().turn().step() == Step::Main1
    })?;
    let forsaken = driver.id_of("Curse of the Forsaken")?;
    assert_eq!(driver.state().player_host_of(forsaken), Some(pid(1)));
    let predations = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Curse of Predation")
        .count();
    assert_eq!(predations, 1, "a Curse of the same name stays in the library");
    Ok(())
}

#[rstest]
#[case::nobody_pays(3, None, false)]
#[case::the_ice_caves_controller_pays(2, Some(0), true)]
#[case::a_third_player_pays(3, Some(2), true)]
#[case::the_caster_may_not_pay(4, Some(1), false)]
fn r5515_ice_cave_counters_a_spell_another_player_pays_for(
    #[case] seats: usize,
    #[case] payer: Option<usize>,
    #[case] countered: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        vec![
            ready(card("Ice Cave").unwrap()),
            ready(plains()),
            ready(plains()),
        ],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(plains()), ready(plains())]);
    }
    scenario = scenario.hand(1, [mtg_testkit::cards::instant("Zap", 1, &[])]);
    let mut driver = Driver::from_scenario(scenario.at(1, 1, Step::Main1))?;
    driver.cast("Zap")?;
    for _ in 0..100 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => {
                Answer::YesNo(YesNoChoice(payer == Some(decision.player.index())))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(mtg_engine::decision::PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert!(r5515_in_graveyard(&driver, 1, "Zap").is_some(), "Zap resolved or was countered");
    let tapped = |seat: usize| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                let object = driver.state().object(**id).unwrap();
                object.controller() == pid(seat) && object.permanent().unwrap().tapped()
            })
            .count()
    };
    assert_eq!(tapped(1), 1, "the caster paid for Zap only");
    if let Some(seat) = payer.filter(|seat| *seat != 1) {
        assert_eq!(tapped(seat) == 1, countered, "another player's payment counters");
    }
    Ok(())
}
