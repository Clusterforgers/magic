//! Goal: the r47-04 pieces play as printed at tables of two, three and four seats: Overclocked Electromancer gets energy
//! equal to the excess damage its combat damage dealt a blocker this turn and none when the damage was not excess;
//! Ghastly Conscription manifests every creature card of the target player's graveyard under its caster; Become Anonymous
//! cloaks its target and the top two cards of the library tapped; Jeskai Infiltrator manifests itself and the top card of
//! the library after its combat damage to a player; Triumph of Saint Katherine goes back into the top seven cards of the
//! library with six cards under it only when the library holds six; Slaughter the Strong and Destined Confrontation keep
//! what each player chooses within a total power of 4 (a negative power subtracting) and sacrifice the rest; and every
//! log replays to the state it was played to. Darksteel Garrison fortifies a land its controller controls at sorcery
//! speed for {3}, the land has indestructible, and tapping it gives the target creature +1/+1 until end of turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::casting::lands;
use common::{name_of, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, NumberChoice, OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::FaceDown;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// How the table answers what the casts, triggers and combat ask.
#[derive(Debug, Clone, Default)]
struct R4704Answers {
    /// Say yes to a question and pay optional costs.
    yes: bool,
    /// The option picked from a list.
    option: usize,
    /// The cards picked from a choice, by name.
    picks: Vec<&'static str>,
    /// The number chosen when one is asked.
    number: u32,
    /// The blocks declared: (blocker, attacker).
    blocks: Vec<(&'static str, &'static str)>,
}

/// Answers until `done` holds at a priority with an empty stack and nothing pending.
fn r4704_run(
    driver: &mut Driver,
    how: &R4704Answers,
    done: impl Fn(&Driver) -> bool,
) -> Result<(), ScriptError> {
    for _ in 0..400 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
            && done(driver)
        {
            return Ok(());
        }
        if matches!(decision.kind, DecisionKind::DeclareBlockers(_)) && !how.blocks.is_empty() {
            let blocks = how.blocks.clone();
            let ours = decision.player != pid(0);
            if ours {
                driver.block(&blocks)?;
                continue;
            }
        }
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(how.yes)),
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(how.option)),
            DecisionKind::ChooseCards(options) => {
                let mut chosen: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| how.picks.contains(&name_of(driver.state(), *id).as_str()))
                    .take(usize::try_from(options.max).unwrap())
                    .collect();
                if chosen.len() < usize::try_from(options.min).unwrap() {
                    chosen = options
                        .from
                        .iter()
                        .copied()
                        .take(usize::try_from(options.min).unwrap())
                        .collect();
                }
                Answer::ChooseCards(CardsChoice(chosen))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(how.number)),
            DecisionKind::ChooseTargets(_) => Driver::passive_answer(&decision),
            _ => Driver::passive_answer(&decision),
        };
        let _ = TargetChoice::Targets(Vec::new());
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the table did not settle".to_owned()))
}

fn r4704_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn r4704_energy(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .player(pid(seat))
        .unwrap()
        .counters_of(&CounterKind::Other("energy".into()))
}

/// Overclocked Electromancer (2/2, doubled to 4 power as it attacks) is blocked by a creature of toughness 1 or 4: the
/// toughness-1 blocker takes 3 excess damage and its controller's opponent gets 3 energy; the toughness-4 one takes no
/// excess and no energy is got.
#[rstest]
fn r4704_electromancer_gets_energy_for_the_excess(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 4)] toughness: i32,
) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Overclocked Electromancer").unwrap())])
        .battlefield(foe, [ready(free_creature("Wall", 0, toughness))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Overclocked Electromancer", foe)])?;
    r4704_run(
        &mut driver,
        &R4704Answers {
            blocks: vec![("Wall", "Overclocked Electromancer")],
            ..R4704Answers::default()
        },
        |d| d.state().turn().step() == Step::EndCombat || d.state().turn().step() == Step::Main2,
    )?;
    assert_eq!(
        r4704_energy(&driver, 0),
        u32::try_from((4 - toughness).max(0)).unwrap()
    );
    assert_eq!(
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .any(|id| name_of(driver.state(), *id) == "Wall"),
        toughness > 4,
        "the wall died to lethal damage"
    );
    r4704_replays(&driver);
    Ok(())
}

/// The objects on the battlefield controlled by the seat that are face down in the given way.
fn r4704_face_down(driver: &Driver, seat: usize, kind: FaceDown) -> Vec<mtg_core::ids::ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            let object = driver.state().object(*id).unwrap();
            object.face_down() == Some(kind) && object.controller() == pid(seat)
        })
        .collect()
}

fn r4704_names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

/// Ghastly Conscription at the last seat, whose graveyard holds two creature cards and a land: both creatures come face
/// down under the caster, the land stays.
#[rstest]
fn r4704_ghastly_conscription_manifests_the_creature_cards(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Ghastly Conscription").unwrap()])
        .battlefield(0, lands(7, swamp))
        .graveyard(
            foe,
            [
                free_creature("Bear", 2, 2),
                free_creature("Elk", 3, 3),
                island(),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_player(foe);
    driver.cast_at("Ghastly Conscription", &[target])?;
    r4704_run(&mut driver, &R4704Answers::default(), |_| true)?;
    let made = r4704_face_down(&driver, 0, FaceDown::Manifest);
    assert_eq!(made.len(), 2);
    let mut cards: Vec<String> = made
        .iter()
        .map(|id| {
            driver
                .state()
                .object(*id)
                .unwrap()
                .whole_card()
                .name
                .clone()
        })
        .collect();
    cards.sort();
    assert_eq!(cards, ["Bear", "Elk"]);
    assert_eq!(
        r4704_names_in(&driver, Zone::Graveyard(pid(foe))),
        ["Island"]
    );
    r4704_replays(&driver);
    Ok(())
}

/// Become Anonymous on the caster's own creature cloaks it and the top two cards of the library, all tapped.
#[rstest]
fn r4704_become_anonymous_cloaks_three_tapped(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Become Anonymous").unwrap()])
        .battlefield(0, {
            let mut permanents = lands(4, island);
            permanents.push(ready(free_creature("Bear", 2, 2)));
            permanents
        });
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| plains()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Bear")?;
    driver.cast_at("Become Anonymous", &[target])?;
    r4704_run(&mut driver, &R4704Answers::default(), |_| true)?;
    let made = r4704_face_down(&driver, 0, FaceDown::Cloak);
    assert_eq!(made.len(), 3);
    for id in &made {
        assert!(
            driver
                .state()
                .object(*id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        );
    }
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 3);
    r4704_replays(&driver);
    Ok(())
}

/// Jeskai Infiltrator unblocked deals combat damage to the last seat: it and the top card of its controller's library
/// come back as two manifested 2/2s.
#[rstest]
fn r4704_jeskai_infiltrator_manifests_itself_and_the_top_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Jeskai Infiltrator").unwrap())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| plains()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Jeskai Infiltrator", foe)])?;
    r4704_run(&mut driver, &R4704Answers::default(), |d| {
        d.state().turn().step() == Step::EndCombat || d.state().turn().step() == Step::Main2
    })?;
    let made = r4704_face_down(&driver, 0, FaceDown::Manifest);
    assert_eq!(made.len(), 2);
    let mut cards: Vec<String> = made
        .iter()
        .map(|id| {
            driver
                .state()
                .object(*id)
                .unwrap()
                .whole_card()
                .name
                .clone()
        })
        .collect();
    cards.sort();
    assert_eq!(cards, ["Jeskai Infiltrator", "Plains"]);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 4);
    r4704_replays(&driver);
    Ok(())
}

/// Triumph of Saint Katherine dies to a burn: with six or more cards in the library it and six cards go on top in a
/// random order (seven cards more on top, none in the graveyard); with five it stays in the graveyard and the library is
/// untouched.
#[rstest]
fn r4704_triumph_goes_back_on_top_with_six_cards(
    #[values(2, 3, 4)] seats: usize,
    #[values(5, 8)] library: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [mtg_testkit::cards::burn("Blast", 9)])
        .battlefield(
            0,
            [
                ready(card("Triumph of Saint Katherine").unwrap()),
                ready(mountain()),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..library).map(|_| plains()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Triumph of Saint Katherine")?;
    driver.cast_at("Blast", &[target])?;
    r4704_run(&mut driver, &R4704Answers::default(), |_| true)?;
    let in_library = r4704_names_in(&driver, Zone::Library(pid(0)));
    let in_graveyard = r4704_names_in(&driver, Zone::Graveyard(pid(0)));
    if library >= 6 {
        assert_eq!(in_library.len(), library + 1);
        let top: Vec<&String> = in_library.iter().rev().take(7).collect();
        assert!(top.iter().any(|name| *name == "Triumph of Saint Katherine"));
        assert!(!in_graveyard.contains(&"Triumph of Saint Katherine".to_owned()));
    } else {
        assert_eq!(in_library.len(), library);
        assert!(in_graveyard.contains(&"Triumph of Saint Katherine".to_owned()));
    }
    r4704_replays(&driver);
    Ok(())
}

/// Every seat holds a 3/3, a 2/2, a 1/1 and a 5/5 and is asked for one creature at a time, never the 5/5; seat 0 keeps
/// the 3/3 and the 1/1 (total 4, after which nothing more fits and it is not asked again), every other seat keeps the
/// 2/2 and then stops; the rest are sacrificed together.
#[rstest]
fn r4704_keep_total_power(
    #[values(2, 3, 4)] seats: usize,
    #[values("Slaughter the Strong", "Destined Confrontation")] spell: &str,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(spell).unwrap()]);
    for seat in 0..seats {
        let mut permanents = vec![
            ready(free_creature(&format!("Three{seat}"), 3, 3)),
            ready(free_creature(&format!("Two{seat}"), 2, 2)),
            ready(free_creature(&format!("One{seat}"), 1, 1)),
            ready(free_creature(&format!("Five{seat}"), 5, 5)),
        ];
        if seat == 0 {
            permanents.extend(lands(4, plains));
        }
        scenario = scenario.battlefield(seat, permanents);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(spell)?;
    let mut picks: Vec<String> = vec!["Three0".into(), "One0".into()];
    picks.extend((1..seats).map(|seat| format!("Two{seat}")));
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) => {
                asked += 1;
                assert_eq!(options.max, 1, "one at a time");
                assert_eq!(options.min, 0, "every kept total is within 4");
                for id in &options.from {
                    let name = name_of(driver.state(), *id);
                    assert!(!name.starts_with("Five"), "a 5/5 never fits within 4");
                }
                let chosen: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| picks.contains(&name_of(driver.state(), *id)))
                    .take(1)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty()
                    && asked > 0 =>
            {
                break;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    let mut left = r4704_names_in(&driver, Zone::Battlefield);
    left.retain(|name| name != "Plains");
    left.sort();
    let mut wanted: Vec<String> = vec!["One0".into(), "Three0".into()];
    wanted.extend((1..seats).map(|seat| format!("Two{seat}")));
    wanted.sort();
    assert_eq!(left, wanted);
    r4704_replays(&driver);
    Ok(())
}

/// Darksteel Garrison is fortified onto a Forest with three Mountains; the Forest then has indestructible, and tapping
/// it for mana pumps the targeted Bear.
#[rstest]
fn r4704_darksteel_garrison_fortifies_a_land(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(0, {
        let mut permanents = vec![
            ready(card("Darksteel Garrison").unwrap()),
            ready(forest()),
            ready(free_creature("Bear", 2, 2)),
        ];
        permanents.extend(lands(3, mountain));
        permanents
    });
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Darksteel Garrison")?;
    let land = driver.target_named("Forest")?;
    driver.choose_targets(&[land])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r4704_run(&mut driver, &R4704Answers::default(), |_| true)?;
    let garrison = driver.id_of("Darksteel Garrison")?;
    let forest_id = driver.id_of("Forest")?;
    assert_eq!(
        driver
            .state()
            .object(garrison)
            .unwrap()
            .permanent()
            .unwrap()
            .attached_to()
            .map(|r| r.id),
        Some(forest_id)
    );
    assert!(
        mtg_engine::keywords::keywords_of(driver.state(), forest_id)
            .iter()
            .any(|k| matches!(k, mtg_cards::keyword::Keyword::Indestructible))
    );
    driver.tap_for_mana("Forest")?;
    let bear = driver.target_named("Bear")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.choose_targets(&[bear])?;
    }
    r4704_run(&mut driver, &R4704Answers::default(), |_| true)?;
    let bear_id = driver.id_of("Bear")?;
    let chars = mtg_engine::characteristics::characteristics(driver.state(), bear_id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    r4704_replays(&driver);
    Ok(())
}
