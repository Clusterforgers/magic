//! Goal: the Drowned Spire holds together and its bosses do what their words say: each boss record names its precon,
//! a five-level innate, two phases and a satchel of real cards; a new run draws the Spire or the Warrens by its seed
//! only from the Spire's generation; a Spire ladder is three rooms ending in the Leviathan; and in real games Siren
//! Song, Borrowed Words and Grasping Deep act at levels I and V, a Tentacle stops an attacker, and each phase's
//! effect lands once.
use std::sync::Arc;

use mtg_campaign::content::Theme;
use mtg_campaign::innate::{LEVELS, innate_id};
use mtg_campaign::scale::PHASE_ROUNDS;
use mtg_campaign::{CardKind, Run, RunEvent, content};
use mtg_cards::CardDefinition;
use mtg_cards::support::Support;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature, free_creature, instant, island};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

/// The content generation the Drowned Spire joined in.
const SPIRE_FROM: u32 = 8;

const HERO: usize = 0;
const BOSS: usize = 1;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn spire() -> &'static Theme {
    content().unwrap().theme("spire").unwrap()
}

fn custom(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

/// The card of `id` written with levels, at `level`.
fn leveled(id: &str, level: u8) -> Arc<CardDefinition> {
    custom(&innate_id(id, level))
}

/// The hero (seat 0) with `heroes` on the battlefield against the boss (seat 1) with `boss` and the emblems; both draw
/// from libraries of Islands.
fn table(
    heroes: Vec<Arc<CardDefinition>>,
    boss: Vec<Arc<CardDefinition>>,
    emblems: Vec<Arc<CardDefinition>>,
) -> Scenario {
    Scenario::new(2)
        .life(HERO, 20)
        .life(BOSS, 40)
        .battlefield(HERO, heroes.into_iter().map(ready))
        .battlefield(BOSS, boss.into_iter().map(ready))
        .library(HERO, (0..40).map(|_| island()))
        .library(BOSS, (0..40).map(|_| island()))
        .emblems(BOSS, emblems)
}

/// Who controls the battlefield permanent named `name`.
fn controller(driver: &Driver, name: &str) -> Option<usize> {
    let id = driver.id_of(name).ok()?;
    controller_of(driver.state(), id).map(PlayerId::index)
}

/// How many cards named `name` are in `zone`.
fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| name.is_empty() || o.card().name == name)
        .count()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Answers decisions until `stop` holds: the first legal target of a trigger, yes to the first `yes` questions, mana
/// paid automatically, else passively.
fn act_until(
    driver: &mut Driver,
    mut yes: u32,
    stop: impl Fn(&Driver) -> bool,
) -> Result<(), ScriptError> {
    for _ in 0..2_000 {
        if stop(driver) {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(o) if !o.can_cancel && !o.legal.is_empty() => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![o.legal[0]]))
            }
            DecisionKind::YesNo(_) => {
                let said = yes > 0;
                yes = yes.saturating_sub(1);
                Answer::YesNo(YesNoChoice(said))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("act_until".into()))
}

fn at_step(turn: u32, step: Step) -> impl Fn(&Driver) -> bool {
    move |d: &Driver| d.state().turn().number() == turn && d.state().turn().step() == step
}

#[rstest]
#[case::siren_queen("siren_queen", "UB", "znc-sneak-attack.json", false)]
#[case::drowned_archivist("drowned_archivist", "UB", "mkc-revenant-recon.json", false)]
#[case::leviathan("leviathan", "UG", "cmr-reap-the-tides.json", true)]
fn a_spire_boss_holds_the_reworked_record(
    #[case] id: &str,
    #[case] colors: &str,
    #[case] deck: &str,
    #[case] last: bool,
) {
    let content = content().unwrap();
    let boss = spire().boss(id).unwrap();
    assert_eq!((boss.colors.as_str(), boss.final_boss), (colors, last));
    assert_eq!(boss.since, SPIRE_FROM);
    assert_eq!(boss.deck_file, deck);
    let path = format!("{}/content/decks/{deck}", env!("CARGO_MANIFEST_DIR"));
    let file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let cards: u64 = file["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c[1].as_u64().unwrap())
        .sum();
    assert_eq!(cards, 100);
    let innate = boss.innate.as_str();
    for level in 1..=LEVELS {
        let card = content.card(&innate_id(innate, level)).unwrap();
        assert_eq!((card.kind, card.level), (CardKind::Innate, level));
    }
    assert_eq!(boss.round_phases.len(), 2);
    for phase in &boss.round_phases {
        let kind = content
            .card(phase)
            .or_else(|_| content.card(&innate_id(phase, 1)))
            .unwrap()
            .kind;
        assert_eq!(kind, CardKind::Phase, "{phase}");
    }
    let reusable: Vec<bool> = boss.satchel.iter().map(|s| s.reusable).collect();
    assert_eq!(reusable, [true, true, true, false]);
    let corpus = corpus::corpus().unwrap();
    for card in &boss.satchel {
        let support = corpus.support(&card.card).unwrap().support;
        assert!(
            matches!(support, Support::Verified | Support::Read),
            "{}",
            card.card
        );
    }
}

#[test]
fn a_new_run_draws_its_theme_by_seed_from_the_spires_generation_only() {
    let content = content().unwrap();
    let draw = |seed, generation| content.theme_for(seed, generation).unwrap().id.clone();
    assert!((0..200).all(|seed| draw(seed, SPIRE_FROM - 1) == "warrens"));
    let drawn: Vec<String> = (0..200)
        .map(|seed| draw(seed, mtg_campaign::run::CONTENT))
        .collect();
    for theme in ["warrens", "spire"] {
        assert!(drawn.iter().any(|t| t == theme), "{theme} is drawn");
    }
    assert!((0..200).all(
        |seed| draw(seed, mtg_campaign::run::CONTENT) == drawn[usize::try_from(seed).unwrap()]
    ));
}

fn spire_run(seed: u64) -> Run {
    let mut events = vec![
        RunEvent::Content {
            version: mtg_campaign::run::CONTENT,
        },
        RunEvent::Joined {
            name: "H".into(),
            user: Some("u".into()),
            bot: None,
        },
        RunEvent::DeckChosen {
            hero: 0,
            deck: mtg_campaign::HeroDeck {
                id: "x".into(),
                name: "x".into(),
                colors: "U".into(),
                commanders: Vec::new(),
                makeup: None,
            },
        },
    ];
    events.push(RunEvent::Started);
    Run::replay(content().unwrap(), "spire", seed, 0, &events).unwrap()
}

#[rstest]
fn a_spire_ladder_is_three_rooms_ending_in_the_leviathan(#[values(1, 2, 3, 99)] seed: u64) {
    let run = spire_run(seed);
    let bosses: Vec<&str> = run
        .map
        .nodes
        .iter()
        .filter_map(|n| n.boss.as_deref())
        .collect();
    assert_eq!(bosses.len(), 3);
    assert_eq!(bosses.last(), Some(&"leviathan"));
    let plan = run.fight_plan(content().unwrap()).unwrap().unwrap();
    let rework = &plan.boss;
    let boss = spire().boss(&plan.boss.boss).unwrap();
    let innate = boss.innate.as_str();
    assert_eq!(rework.innate, innate_id(innate, 1));
    let rounds: Vec<u32> = rework.round_phases.iter().map(|(_, r)| *r).collect();
    assert_eq!(rounds, PHASE_ROUNDS);
    for ((card, _), written) in rework.round_phases.iter().zip(&boss.round_phases) {
        let at_level = innate_id(written, 1);
        let expected = if content().unwrap().cards.contains_key(&at_level) {
            at_level
        } else {
            written.clone()
        };
        assert_eq!(card, &expected);
    }
}

#[rstest]
#[case::level_one_takes_mana_value_two(1, 2, true)]
#[case::level_one_spares_mana_value_three(1, 3, false)]
#[case::level_five_takes_mana_value_six(5, 6, true)]
#[case::level_five_spares_mana_value_seven(5, 7, false)]
fn siren_song_takes_an_attacker_until_the_end_of_her_next_turn(
    #[case] level: u8,
    #[case] mana_value: u32,
    #[case] taken: bool,
) -> Result<(), ScriptError> {
    let diver = creature("Diver", mana_value, &[], 2, 2);
    let scenario =
        table(vec![diver], vec![], vec![leveled("siren_song", level)]).at(1, HERO, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Diver", BOSS)])?;
    act_until(&mut driver, 0, at_step(1, Step::End))?;
    let seat = if taken { BOSS } else { HERO };
    assert_eq!(controller(&driver, "Diver"), Some(seat));
    assert_eq!(life(&driver, BOSS), if taken { 40 } else { 38 });
    driver.idle_until(2, Step::End)?;
    assert_eq!(controller(&driver, "Diver"), Some(seat));
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(controller(&driver, "Diver"), Some(HERO));
    Ok(())
}

#[rstest]
#[case::level_one_casts_mana_value_two(1, 2, true)]
#[case::level_one_cannot_cast_three(1, 3, false)]
#[case::level_five_casts_mana_value_six(5, 6, true)]
#[case::level_five_cannot_cast_seven(5, 7, false)]
fn borrowed_words_casts_a_hero_spell_once_a_turn_and_exiles_it(
    #[case] level: u8,
    #[case] mana_value: u32,
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let lesson = || instant("Lesson", mana_value, &[]);
    let scenario = table(vec![], vec![], vec![leveled("borrowed_words", level)])
        .graveyard(HERO, [lesson(), lesson()])
        .at(2, BOSS, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(driver.cast("Lesson").is_ok(), castable);
    if castable {
        act_until(&mut driver, 0, |d| d.state().zone(Zone::Stack).is_empty())?;
        assert_eq!(count(&driver, Zone::Exile, "Lesson"), 1);
        assert_eq!(count(&driver, Zone::Graveyard(pid(HERO)), "Lesson"), 1);
        assert!(driver.cast("Lesson").is_err(), "once each turn");
    }
    Ok(())
}

#[rstest]
#[case::level_one(1, 2)]
#[case::level_five(5, 6)]
fn grasping_deep_tops_up_its_tentacles_each_upkeep(
    #[case] level: u8,
    #[case] tentacles: usize,
) -> Result<(), ScriptError> {
    let scenario =
        table(vec![], vec![], vec![leveled("grasping_deep", level)]).at(1, HERO, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Tentacle"), tentacles);
    driver.idle_until(4, Step::Main1)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Tentacle"), tentacles);
    Ok(())
}

#[rstest]
#[case::sacrificed(true)]
#[case::kept(false)]
fn a_tentacle_may_be_sacrificed_to_stop_an_attacker(
    #[case] sacrifice: bool,
) -> Result<(), ScriptError> {
    let scenario = table(
        vec![free_creature("Diver", 2, 2)],
        vec![],
        vec![leveled("grasping_deep", 1)],
    )
    .at(1, HERO, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::DeclareAttackers)?;
    driver.attack(&[("Diver", BOSS)])?;
    act_until(&mut driver, u32::from(sacrifice), at_step(3, Step::End))?;
    let tentacles = count(&driver, Zone::Battlefield, "Tentacle");
    assert_eq!(tentacles, if sacrifice { 1 } else { 2 });
    assert_eq!(life(&driver, BOSS), if sacrifice { 40 } else { 38 });
    driver.idle_until(5, Step::Main1)?;
    let diver = driver.id_of("Diver")?;
    let tapped = driver
        .state()
        .object(diver)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(tapped, sacrifice, "it skips its next untap");
    assert_eq!(count(&driver, Zone::Battlefield, "Tentacle"), 2);
    Ok(())
}

/// `id` as a phase that begins at the end of round 1.
fn phase(id: &str) -> BossPhase {
    BossPhase {
        begins: PhaseStart::AtRound(1),
        emblem: custom(id),
    }
}

#[test]
fn the_chorus_takes_each_heros_strongest_creature_once() -> Result<(), ScriptError> {
    let scenario = table(
        vec![free_creature("Big", 5, 5), free_creature("Small", 1, 1)],
        vec![],
        vec![],
    )
    .phases(BOSS, [phase("the_chorus")])
    .at(1, HERO, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(controller(&driver, "Big"), Some(BOSS));
    assert_eq!(controller(&driver, "Small"), Some(HERO));
    driver.idle_until(5, Step::Main1)?;
    assert_eq!(controller(&driver, "Big"), Some(BOSS));
    assert_eq!(controller(&driver, "Small"), Some(HERO));
    Ok(())
}

#[rstest]
#[case::level_one(1)]
#[case::level_five(5)]
fn the_stacks_collapse_mills_each_hero_ten_once(#[case] level: u8) -> Result<(), ScriptError> {
    let scenario = table(vec![], vec![], vec![])
        .phases(
            BOSS,
            [BossPhase {
                begins: PhaseStart::AtRound(1),
                emblem: leveled("the_stacks_collapse", level),
            }],
        )
        .at(1, HERO, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(count(&driver, Zone::Graveyard(pid(HERO)), ""), 10);
    assert_eq!(count(&driver, Zone::Graveyard(pid(BOSS)), ""), 0);
    driver.idle_until(5, Step::Main1)?;
    assert_eq!(count(&driver, Zone::Graveyard(pid(HERO)), ""), 10);
    Ok(())
}

#[test]
fn the_tower_floods_returns_all_but_the_deep_and_makes_lands_islands() -> Result<(), ScriptError> {
    let kraken =
        Arc::new(CardDefinition::clone(&free_creature("Kraken", 4, 4)).with_subtype("Kraken"));
    let scenario = table(
        vec![free_creature("Bear", 2, 2), mtg_testkit::cards::mountain()],
        vec![kraken],
        vec![],
    )
    .phases(BOSS, [phase("the_tower_floods")])
    .at(1, HERO, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(count(&driver, Zone::Hand(pid(HERO)), "Bear"), 1);
    assert_eq!(controller(&driver, "Kraken"), Some(BOSS));
    let mountain = driver.id_of("Mountain")?;
    let types = characteristics(driver.state(), mountain).unwrap();
    assert!(
        format!("{:?}", types.subtypes).contains("Island"),
        "{:?}",
        types.subtypes
    );
    Ok(())
}

#[test]
fn the_deep_takes_all_grows_the_bosss_creatures() -> Result<(), ScriptError> {
    let scenario = table(
        vec![free_creature("Bear", 2, 2)],
        vec![free_creature("Kraken", 4, 4)],
        vec![custom("the_deep_takes_all")],
    )
    .at(1, HERO, Step::Main1);
    let driver = Driver::from_scenario(scenario)?;
    let power = |name: &str| {
        let id = driver.id_of(name).unwrap();
        characteristics(driver.state(), id).and_then(|c| c.power)
    };
    assert_eq!((power("Kraken"), power("Bear")), (Some(6), Some(2)));
    Ok(())
}

#[test]
fn the_final_lesson_copies_a_spell_cast_from_a_heros_graveyard() -> Result<(), ScriptError> {
    let scenario = table(vec![], vec![], vec![leveled("the_final_lesson", 1)])
        .graveyard(HERO, [burn("Zap", 2)])
        .at(2, BOSS, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Zap")?;
    let hero = driver.target_player(HERO);
    for _ in 0..200 {
        if driver.state().zone(Zone::Stack).is_empty() && count(&driver, Zone::Exile, "Zap") == 1 {
            break;
        }
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![hero]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert_eq!(life(&driver, HERO), 16);
    assert_eq!(count(&driver, Zone::Exile, "Zap"), 1);
    Ok(())
}

#[rstest]
#[case::level_one_takes_mana_value_two(1, 2, true)]
#[case::level_one_spares_mana_value_three(1, 3, false)]
fn the_drowning_song_makes_heroes_attack_and_takes_each_attacker_it_can(
    #[case] level: u8,
    #[case] mana_value: u32,
    #[case] taken: bool,
) -> Result<(), ScriptError> {
    let scenario = table(
        vec![creature("Diver", mana_value, &[], 2, 2)],
        vec![],
        vec![leveled("the_drowning_song", level)],
    )
    .at(1, HERO, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    act_until(&mut driver, 0, at_step(1, Step::End))?;
    assert_eq!(
        controller(&driver, "Diver"),
        Some(if taken { BOSS } else { HERO })
    );
    assert_eq!(life(&driver, BOSS), if taken { 40 } else { 38 });
    Ok(())
}
