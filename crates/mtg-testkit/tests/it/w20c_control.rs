//! Goal: the control and payment sentences the compiler reads in the t12c sweep (Word of Seizing, Homeward Path after
//! a permanent theft, Diffusion Sliver) play through the engine at tables of two, three and four seats: an untapped
//! permanent taken for the turn goes home with the turn and its haste, base size (layer 7b under an Aura's 7c), keywords
//! (annihilator for the new controller) and added creature type end at cleanup; a player who owns what another controls
//! takes it back for good, over a control Aura that stays attached; a targeting spell or ability is countered unless its
//! controller, the one asked, pays; every answer keeps the invariants and the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype};
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, mountain};
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::on_battlefield;
use crate::w7d_control_scenarios::w7d_oracle;

type W20cResult = Result<(), ScriptError>;

/// Word of Seizing as printed.
fn w20c_seizing() -> Arc<CardDefinition> {
    w7d_oracle(
        "Word of Seizing",
        "{3}{R}",
        "Instant",
        None,
        "Split second (As long as this spell is on the stack, players can't cast spells or activate abilities that aren't mana abilities.)\nUntap target permanent and gain control of it until end of turn. It gains haste until end of turn.",
    )
}

/// Homeward Path as printed.
fn w20c_path() -> Arc<CardDefinition> {
    w7d_oracle(
        "Homeward Path",
        "",
        "Land",
        None,
        "{T}: Add {C}.\n{T}: Each player gains control of all creatures they own.",
    )
}

/// A permanent theft, so a creature has an owner other than its controller.
fn w20c_theft() -> Arc<CardDefinition> {
    w7d_oracle(
        "Blatant Thievery",
        "{U}",
        "Sorcery",
        None,
        "For each opponent, gain control of target permanent that player controls.",
    )
}

/// Diffusion Sliver as printed.
fn w20c_sliver() -> Arc<CardDefinition> {
    w7d_oracle(
        "Diffusion Sliver",
        "{2}{U}",
        "Creature \u{2014} Sliver",
        Some((2, 2)),
        "Whenever a Sliver creature you control becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {2}.",
    )
}

/// A burn spell with no other effect.
fn w20c_bolt() -> Arc<CardDefinition> {
    w7d_oracle(
        "Bolt",
        "{R}",
        "Instant",
        None,
        "Bolt deals 3 damage to target creature.",
    )
}

/// Answers until the stack is empty, nothing resolves and no trigger waits: each target decision takes the next of
/// `picks`, mana is paid automatically, anything else is answered passively.
fn w20c_answer(driver: &mut Driver, picks: &mut VecDeque<Target>) -> W20cResult {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        let idle = driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty();
        if matches!(decision.kind, DecisionKind::Priority(_)) && idle {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(options) if !picks.is_empty() => {
                let take = (options.max as usize).min(picks.len());
                Answer::ChooseTargets(TargetChoice::Targets(picks.drain(..take).collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The deciding player casts `name` at `picks` and every seat answers until the stack is empty.
fn w20c_cast(driver: &mut Driver, name: &str, picks: Vec<Target>) -> W20cResult {
    driver.begin_cast(name)?;
    w20c_answer(driver, &mut picks.into())
}

/// The seat that controls the permanent called `name`.
fn w20c_holds(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    mtg_engine::characteristics::controller_of(driver.state(), id)
        .unwrap()
        .index()
}

#[rstest]
fn w20c_a_tapped_permanent_is_taken_untapped_for_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let lands: Vec<_> = (0..4).map(|_| ready(mountain())).collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20c_seizing()])
        .battlefield(0, lands)
        .battlefield(1, [tapped(free_creature("Ogre", 3, 3))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    w20c_cast(&mut driver, "Word of Seizing", vec![ogre])?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 0, "taken for the turn");
    let id = driver.id_of("Ogre")?;
    let held = driver.state().object(id).unwrap().permanent().unwrap();
    assert!(!held.tapped(), "untapped by the spell");
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 1, "home with the turn");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w20c_an_owner_takes_back_what_another_player_controls(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20c_theft()])
        .battlefield(0, [ready(island()), ready(w20c_path())])
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    w20c_cast(&mut driver, "Blatant Thievery", vec![ogre])?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 0, "stolen for good");
    driver.begin_ability("Homeward Path", 0)?;
    w20c_answer(&mut driver, &mut VecDeque::new())?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 1, "back with its owner");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w20c_a_targeting_spell_is_countered_when_it_is_not_paid_for(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20c_bolt()])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(w20c_sliver())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let sliver = driver.target_named("Diffusion Sliver")?;
    w20c_cast(&mut driver, "Bolt", vec![sliver])?;
    let id = driver.id_of("Diffusion Sliver")?;
    let sliver = driver.state().object(id).unwrap().permanent().unwrap();
    assert_eq!(sliver.damage(), 0, "the spell was countered, not resolved");
    replays(&driver, &setup);
    Ok(())
}

/// A 3/3 Ogre creature of seat 1 with its creature type.
fn w20c_ogre() -> Arc<CardDefinition> {
    Arc::new(
        free_creature("Ogre", 3, 3)
            .as_ref()
            .clone()
            .with_subtype("Ogre"),
    )
}

/// The number of battlefield permanents seat `seat` controls.
fn w20c_count(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            mtg_engine::characteristics::controller_of(state, **id)
                .map(mtg_core::ids::PlayerId::index)
                == Some(seat)
        })
        .count()
}

/// Whether the permanent called `name` has `keyword` now.
fn w20c_has(driver: &Driver, name: &str, keyword: &Keyword) -> bool {
    has_keyword(driver.state(), driver.id_of(name).unwrap(), keyword)
}

#[rstest]
fn w20c_word_of_seizing_grants_haste_for_the_turn_only(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let lands: Vec<_> = (0..4).map(|_| ready(mountain())).collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20c_seizing()])
        .battlefield(0, lands)
        .battlefield(1, [tapped(free_creature("Ogre", 3, 3))]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(!w20c_has(&driver, "Ogre", &Keyword::Haste));
    let ogre = driver.target_named("Ogre")?;
    w20c_cast(&mut driver, "Word of Seizing", vec![ogre])?;
    assert!(
        w20c_has(&driver, "Ogre", &Keyword::Haste),
        "haste this turn"
    );
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 1);
    assert!(
        !w20c_has(&driver, "Ogre", &Keyword::Haste),
        "haste ends at cleanup"
    );
    Ok(())
}

#[rstest]
fn w20c_flayer_sets_a_base_size_under_the_aura_and_annihilates_for_its_new_controller(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let flayer = w7d_oracle(
        "Flayer",
        "{R}",
        "Creature \u{2014} Eldrazi",
        Some((5, 5)),
        "When you cast this spell, gain control of target creature until end of turn. Untap that creature. Until end of turn, it has base power and toughness 10/10 and gains trample, annihilator 2, and haste.",
    );
    let boon = w7d_oracle(
        "Boon",
        "{W}",
        "Enchantment \u{2014} Aura",
        None,
        "Enchant creature\nEnchanted creature gets +2/+2.",
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [flayer])
        .battlefield(0, [ready(mountain())])
        .battlefield(
            1,
            [
                tapped(w20c_ogre()),
                attached_to(ready(boon), 1, 0),
                ready(island()),
                ready(island()),
            ],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    w20c_cast(&mut driver, "Flayer", vec![ogre])?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 0);
    let id = driver.id_of("Ogre")?;
    assert!(
        !driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    let now = characteristics(driver.state(), id).unwrap();
    assert_eq!(
        (now.power, now.toughness),
        (Some(12), Some(12)),
        "7b then the Aura in 7c"
    );
    for keyword in [Keyword::Trample, Keyword::Annihilator(2), Keyword::Haste] {
        assert!(w20c_has(&driver, "Ogre", &keyword), "{keyword:?}");
    }
    let before = w20c_count(&driver, 1);
    while driver.decision()?.kind.name() != "declare-attackers" {
        let decision = driver.decision()?.clone();
        driver.answer(Driver::passive_answer(&decision))?;
    }
    driver.attack(&[("Ogre", 1)])?;
    w20c_answer(&mut driver, &mut VecDeque::new())?;
    assert_eq!(
        w20c_count(&driver, 1),
        before - 2,
        "annihilator 2 for seat 0"
    );
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 1, "home with the turn");
    let id = driver.id_of("Ogre")?;
    let home = characteristics(driver.state(), id).unwrap();
    assert!(
        home.power.unwrap_or_default() <= 5,
        "the base size ends with the turn"
    );
    assert!(!w20c_has(&driver, "Ogre", &Keyword::Annihilator(2)));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w20c_recruiter_adds_pirate_and_keeps_the_other_types(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let recruiter = w7d_oracle(
        "Recruiter",
        "{R}",
        "Creature \u{2014} Human Pirate",
        Some((2, 2)),
        "Whenever this creature or another Pirate you control enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it gains haste and becomes a Pirate in addition to its other types.",
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [recruiter])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [tapped(w20c_ogre())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let is = |driver: &Driver, kind: &str| {
        let id = driver.id_of("Ogre").unwrap();
        let printed = &driver.state().object(id).unwrap().card().subtypes;
        characteristics(driver.state(), id)
            .unwrap()
            .has_subtype(printed, &Subtype(kind.into()))
    };
    assert!(is(&driver, "Ogre") && !is(&driver, "Pirate"));
    let ogre = driver.target_named("Ogre")?;
    w20c_cast(&mut driver, "Recruiter", vec![ogre])?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 0);
    assert!(is(&driver, "Ogre") && is(&driver, "Pirate"), "layer 4 adds");
    let now = characteristics(driver.state(), driver.id_of("Ogre")?).unwrap();
    assert!(now.types.contains(CardType::Creature));
    assert!(w20c_has(&driver, "Ogre", &Keyword::Haste));
    driver.idle_until(2, Step::Main1)?;
    assert!(
        is(&driver, "Ogre") && !is(&driver, "Pirate"),
        "ends at cleanup"
    );
    assert_eq!(w20c_holds(&driver, "Ogre"), 1);
    Ok(())
}

#[rstest]
fn w20c_homeward_path_overrides_a_control_aura_that_stays_attached(
    #[values(2, 3, 4)] seats: usize,
) -> W20cResult {
    let mut mine: Vec<_> = (0..4).map(|_| ready(island())).collect();
    mine.push(ready(w20c_path()));
    mine.push(ready(free_creature("Scout", 1, 1)));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Control Magic"])
        .battlefield(0, mine)
        .battlefield(1, [ready(w20c_ogre())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    w20c_cast(&mut driver, "Control Magic", vec![ogre])?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 0, "the Aura takes it");
    driver.begin_ability("Homeward Path", 0)?;
    w20c_answer(&mut driver, &mut VecDeque::new())?;
    assert_eq!(w20c_holds(&driver, "Ogre"), 1, "the later timestamp wins");
    assert_eq!(
        w20c_holds(&driver, "Scout"),
        0,
        "its owner already controls it"
    );
    assert_eq!(
        w20c_holds(&driver, "Control Magic"),
        0,
        "the Aura keeps its controller"
    );
    let host = driver
        .state()
        .object(driver.id_of("Control Magic")?)
        .unwrap();
    assert_eq!(
        host.permanent().unwrap().attached_to().map(|r| r.id),
        Some(driver.id_of("Ogre")?)
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::spell_paid(false, true)]
#[case::ability_unpaid(true, false)]
#[case::ability_paid(true, true)]
fn w20c_the_controller_of_the_targeting_object_is_asked_to_pay(
    #[case] ability: bool,
    #[case] pays: bool,
    #[values(2, 3)] seats: usize,
) -> W20cResult {
    let pinger = w7d_oracle(
        "Pinger",
        "{0}",
        "Creature \u{2014} Shaman",
        Some((1, 1)),
        "{T}: This creature deals 3 damage to target creature.",
    );
    let mut mine: Vec<_> = (0..3).map(|_| ready(mountain())).collect();
    mine.push(ready(pinger));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [w20c_bolt()])
        .battlefield(0, mine)
        .battlefield(1, [ready(w20c_sliver())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let sliver = driver.target_named("Diffusion Sliver")?;
    if ability {
        driver.begin_ability("Pinger", 0)?;
    } else {
        driver.begin_cast("Bolt")?;
    }
    let mut picks = VecDeque::from([sliver]);
    let mut asked = Vec::new();
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        let idle = driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty();
        if matches!(decision.kind, DecisionKind::Priority(_)) && idle {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) if !picks.is_empty() => {
                Answer::ChooseTargets(TargetChoice::Targets(picks.drain(..).collect()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::YesNo(_) => {
                asked.push(decision.player.index());
                Answer::YesNo(YesNoChoice(pays))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert_eq!(
        asked,
        vec![0],
        "the controller of the spell or ability decides"
    );
    assert_eq!(
        on_battlefield(driver.state(), "Diffusion Sliver").is_none(),
        pays
    );
    Ok(())
}
