//! Goal: the rules of the game that cards change play whole games at two, three and four seats: a creature a rule requires to attack or block is named by the decision and must be declared, whether the rule is a static ability or an effect that ends with the turn, and one that cannot attack or block is not required; a spell that can't be countered resolves under Counterspell while another is countered; a player who can't gain life gains none from an effect or from lifelink, one who can't draw neither draws nor loses on an empty library, and one who can't search libraries is asked nothing; the top card of a revealed library is known to every seat; and each rule ends with its source or its turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::removal::{corpus, pass_to, resolve_the_stack};
use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::effects::Duration;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{forest, free_creature, island, keyworded, mountain, plains, salve};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;

fn refused(answer: IllegalAnswer) -> ScriptError {
    ScriptError::Engine(EngineError::Illegal(answer))
}

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A 2/2 whose only ability is `json`.
fn bearing(name: &str, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(ability(json)),
    )
}

/// What a refused answer must leave alone.
fn fingerprint(driver: &Driver) -> (u64, usize) {
    (driver.engine().state_hash(), driver.engine().events().len())
}

fn attackers(driver: &Driver) -> (Vec<mtg_core::ids::ObjectId>, Vec<mtg_core::ids::ObjectId>) {
    let DecisionKind::DeclareAttackers(options) = &driver.decision().unwrap().kind else {
        panic!("attackers are declared");
    };
    (
        options.attackers.iter().map(|a| a.object).collect(),
        options.required.clone(),
    )
}

fn blockers(driver: &Driver) -> (Vec<mtg_core::ids::ObjectId>, Vec<mtg_core::ids::ObjectId>) {
    let DecisionKind::DeclareBlockers(options) = &driver.decision().unwrap().kind else {
        panic!("blockers are declared");
    };
    (
        options.blockers.iter().map(|b| b.object).collect(),
        options.required.clone(),
    )
}

// Attack and block requirements

/// Seat 0 attacks with a Darksteel Juggernaut and a Wolf; every other seat has a 1/1.
fn raid(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(corpus("Darksteel Juggernaut")),
                ready(free_creature("Wolf", 2, 2)),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature(&format!("D{seat}"), 1, 1))]);
    }
    scenario
}

#[rstest]
fn a_creature_that_attacks_each_combat_if_able_is_declared_at_any_defender(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] defender: usize,
) -> Result<(), ScriptError> {
    let defender = defender.min(seats - 1);
    let mut driver = Driver::from_scenario(raid(seats))?;
    let juggernaut = driver.id_of("Darksteel Juggernaut")?;
    let (able, required) = attackers(&driver);
    assert_eq!(able.len(), 2);
    assert_eq!(
        required,
        [juggernaut],
        "only the one a rule names is required"
    );
    let before = fingerprint(&driver);
    for attack in [vec![], vec![("Wolf", defender)]] {
        assert_eq!(
            driver.attack(&attack).unwrap_err(),
            refused(IllegalAnswer::Required("attacker")),
            "{attack:?}"
        );
        assert_eq!(
            fingerprint(&driver),
            before,
            "a refused answer changes nothing"
        );
    }
    let declared = driver.attack(&[("Darksteel Juggernaut", defender)])?;
    assert!(declared.iter().any(|event| matches!(
        event,
        Event::AttackersDeclared { attackers, .. } if attackers.len() == 1 && attackers[0].0 == juggernaut
    )));
    Ok(())
}

#[rstest]
#[case::tapped(tapped(corpus("Darksteel Juggernaut")))]
#[case::summoning_sick(sick(corpus("Darksteel Juggernaut")))]
fn a_creature_that_cannot_attack_is_not_required_to(
    #[case] juggernaut: mtg_engine::setup::PresetPermanent,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [juggernaut, ready(free_creature("Wolf", 2, 2))])
            .battlefield(1, [ready(free_creature("D1", 1, 1))]),
    )?;
    let (able, required) = attackers(&driver);
    assert_eq!(able.len(), 1, "only the Wolf can attack");
    assert!(required.is_empty());
    driver.attack(&[])?;
    Ok(())
}

#[rstest]
fn all_creatures_attack_each_combat_if_able_whoever_controls_them(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] active: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(2, active, Step::DeclareAttackers);
    for seat in 0..seats {
        let mut creatures = vec![ready(free_creature(&format!("C{seat}"), 1, 1))];
        if seat == 0 {
            creatures.push(ready(corpus("Warmonger Hellkite")));
        }
        scenario = scenario.battlefield(seat, creatures);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let (able, required) = attackers(&driver);
    assert_eq!(able.len(), usize::from(active == 0) + 1);
    assert_eq!(required, able, "every creature that can attack must");
    let mine = format!("C{active}");
    assert_eq!(
        driver.attack(&[(&mine, (active + 1) % seats)]).is_err(),
        active == 0,
        "the Hellkite itself is required on its controller's turn"
    );
    Ok(())
}

#[test]
fn a_creature_that_blocks_each_combat_if_able_is_required_to_block_what_it_can()
-> Result<(), ScriptError> {
    let grudge = bearing("Grudge", r#"{"Rule":{"rule":{"MustBlock":"This"}}}"#);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("Wolf", 2, 2))])
            .battlefield(1, [ready(grudge), ready(free_creature("Wall", 0, 4))]),
    )?;
    driver.attack(&[("Wolf", 1)])?;
    driver.pass()?;
    driver.pass()?;
    driver.expect_decision(1, "declare-blockers")?;
    let (able, required) = blockers(&driver);
    assert_eq!(able.len(), 2);
    assert_eq!(required, [driver.id_of("Grudge")?]);
    let before = fingerprint(&driver);
    for blocks in [vec![], vec![("Wall", "Wolf")]] {
        assert_eq!(
            driver.block(&blocks).unwrap_err(),
            refused(IllegalAnswer::Required("blocker")),
            "{blocks:?}"
        );
        assert_eq!(fingerprint(&driver), before);
    }
    driver.block(&[("Grudge", "Wolf")])?;
    Ok(())
}

#[test]
fn a_creature_that_cannot_block_what_attacks_is_not_required_to_block_it() -> Result<(), ScriptError>
{
    let grudge = bearing("Grudge", r#"{"Rule":{"rule":{"MustBlock":"This"}}}"#);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(keyworded("Flyer", 2, 2, &[Keyword::Flying]))])
            .battlefield(1, [ready(grudge)]),
    )?;
    driver.attack(&[("Flyer", 1)])?;
    driver.pass()?;
    driver.pass()?;
    assert!(
        !matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)),
        "nothing can block: no question"
    );
    Ok(())
}

/// Seat 1 controls a Courtly Provocateur and Raider, seat 0 a Wall; it is seat 0's turn when the game begins.
fn provoked() -> Scenario {
    Scenario::new(2)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Wolf", 2, 2)),
                ready(corpus("Courtly Provocateur")),
            ],
        )
        .battlefield(1, [ready(free_creature("Wall", 0, 4))])
}

/// Activates the `index`th ability of the named permanent and aims it at the named creature.
fn provoke(driver: &mut Driver, index: usize, at: &str) -> Result<(), ScriptError> {
    let source = driver.id_of("Courtly Provocateur")?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority");
    };
    let ability = options
        .abilities
        .iter()
        .copied()
        .find(|ability| ability.object == source && ability.ability == index)
        .expect("the ability is offered");
    driver.answer(Answer::Priority(PriorityAction::Activate(ability)))?;
    let target = driver.target_named(at)?;
    driver.choose_targets(&[target])?;
    resolve_the_stack(driver)
}

#[test]
fn a_creature_told_to_attack_this_turn_must_and_the_rule_ends_with_the_turn()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(provoked())?;
    provoke(&mut driver, 0, "Wolf")?;
    assert_eq!(driver.state().rule_effects().len(), 1);
    driver.pass_until(Step::DeclareAttackers)?;
    let (able, required) = attackers(&driver);
    assert_eq!(able, [driver.id_of("Wolf")?]);
    assert_eq!(required, able);
    assert_eq!(
        driver.attack(&[]).unwrap_err(),
        refused(IllegalAnswer::Required("attacker"))
    );
    let begun = driver.engine().events().len();
    driver.attack(&[("Wolf", 1)])?;
    driver.idle_until(3, Step::Upkeep)?;
    assert!(
        driver.state().rule_effects().is_empty(),
        "it ended in the cleanup step"
    );
    assert!(
        driver.engine().events()[begun..]
            .iter()
            .any(|event| matches!(event, Event::RuleExpired { .. }))
    );
    Ok(())
}

#[test]
fn a_creature_told_to_block_this_turn_must() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(provoked())?;
    provoke(&mut driver, 1, "Wall")?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Wolf", 1)])?;
    driver.pass()?;
    driver.pass()?;
    driver.expect_decision(1, "declare-blockers")?;
    let (_, required) = blockers(&driver);
    assert_eq!(required, [driver.id_of("Wall")?]);
    assert_eq!(
        driver.block(&[]).unwrap_err(),
        refused(IllegalAnswer::Required("blocker"))
    );
    driver.block(&[("Wall", "Wolf")])?;
    Ok(())
}

// Countering

/// Seat 0 holds the spell and the lands to cast it; seat 1 holds a Counterspell and two Islands.
fn counter_duel(spell: &str, lands: &[Arc<CardDefinition>]) -> Scenario {
    Scenario::new(2)
        .at(2, 0, Step::Main1)
        .hand(0, [corpus(spell)])
        .battlefield(0, lands.iter().map(|land| ready(Arc::clone(land))))
        .battlefield(1, [ready(island()), ready(island())])
        .hand(1, [corpus("Counterspell")])
}

#[rstest]
#[case::a_spell_that_cant_be_countered("Supreme Verdict", &[plains(), plains(), island(), island()], false)]
#[case::a_creature_that_cant_be_countered("Carnage Tyrant", &[forest(), forest(), forest(), forest(), forest(), forest()], false)]
#[case::an_ordinary_creature("Grizzly Bears", &[forest(), forest()], true)]
fn counterspell_does_nothing_to_a_spell_that_cant_be_countered(
    #[case] spell: &str,
    #[case] lands: &[Arc<CardDefinition>],
    #[case] countered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(counter_duel(spell, lands))?;
    driver.cast(spell)?;
    let cast = *driver.state().stack().last().unwrap();
    pass_to(&mut driver, 1)?;
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(cast).unwrap().reference());
    let begun = driver.engine().events().len();
    driver.cast_at("Counterspell", &[target])?;
    resolve_the_stack(&mut driver)?;
    let played = &driver.engine().events()[begun..];
    let outcome = played.iter().find_map(|event| match event {
        Event::ResolutionEnded { object, outcome } if *object == cast => Some(*outcome),
        _ => None,
    });
    assert_eq!(
        outcome.is_some_and(|outcome| outcome == ResolutionOutcome::Countered),
        countered
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(1))),
        ["Counterspell"],
        "Counterspell resolved either way"
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))).contains(&spell.to_owned()),
        countered || spell == "Supreme Verdict"
    );
    Ok(())
}

#[rstest]
#[case::five_power("Titan", 5, false)]
#[case::six_power("Colossus", 6, false)]
#[case::four_power("Ogre", 4, true)]
fn spells_a_permanent_protects_are_the_ones_it_names(
    #[case] name: &str,
    #[case] power: i32,
    #[case] countered: bool,
) -> Result<(), ScriptError> {
    let big = free_creature(name, power, power);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .hand(0, [big])
            .battlefield(0, [ready(corpus("Spellbreaker Behemoth"))])
            .battlefield(1, [ready(island()), ready(island())])
            .hand(1, [corpus("Counterspell")]),
    )?;
    driver.cast(name)?;
    let cast = *driver.state().stack().last().unwrap();
    pass_to(&mut driver, 1)?;
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(cast).unwrap().reference());
    driver.cast_at("Counterspell", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))).contains(&name.to_owned()),
        countered
    );
    assert_eq!(
        names(driver.state(), Zone::Battlefield).contains(&name.to_owned()),
        !countered
    );
    Ok(())
}

// Gaining life, drawing and searching

/// Seat 0 casts Salve at seat 1: 2 damage, then its controller gains 2 life and draws a card.
fn salve_duel(extra: Vec<mtg_engine::setup::PresetPermanent>) -> Scenario {
    let mut battlefield = vec![ready(forest()), ready(forest())];
    battlefield.extend(extra);
    Scenario::new(2)
        .at(2, 0, Step::Main1)
        .hand(0, [salve("Salve")])
        .library(
            0,
            [free_creature("Top", 1, 1), free_creature("Below", 1, 1)],
        )
        .battlefield(0, battlefield)
}

fn cast_salve(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let begun = driver.engine().events().len();
    driver.cast_at("Salve", &[driver.target_player(1)])?;
    resolve_the_stack(driver)?;
    Ok(driver.engine().events()[begun..].to_vec())
}

#[rstest]
#[case::without_the_rule(Vec::new(), 22, 1)]
#[case::with_the_ferocidon(vec![ready(corpus("Rampaging Ferocidon"))], 20, 1)]
#[case::with_a_rule_about_something_else(vec![ready(corpus("Mindlock Orb"))], 22, 1)]
fn a_player_who_cant_gain_life_gains_none_and_the_rest_of_the_spell_happens(
    #[case] extra: Vec<mtg_engine::setup::PresetPermanent>,
    #[case] life: i32,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(salve_duel(extra))?;
    let events = cast_salve(&mut driver)?;
    driver.expect_life(0, life)?;
    driver.expect_life(1, 18)?;
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::LifeGained { .. }))
            .count(),
        usize::from(life == 22)
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::CardDrawn { .. }))
            .count(),
        drawn
    );
    Ok(())
}

#[rstest]
#[case::until_end_of_turn(Duration::UntilEndOfTurn)]
#[case::forever(Duration::Forever)]
fn an_effect_that_stops_life_gain_stops_it_for_its_duration(
    #[case] duration: Duration,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(salve_duel(Vec::new()))?;
    driver.add_rule(Rule::CantGainLife(vec![pid(0)]), duration)?;
    let events = cast_salve(&mut driver)?;
    assert!(!events.iter().any(|e| matches!(e, Event::LifeGained { .. })));
    driver.expect_life(0, 20)?;
    driver.idle_until(3, Step::Upkeep)?;
    assert_eq!(
        driver.state().rule_effects().len(),
        usize::from(duration == Duration::Forever),
        "a rule for the turn is gone, one for ever is not"
    );
    Ok(())
}

#[test]
fn lifelink_gains_no_life_for_a_player_who_cant() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(keyworded("Leech", 3, 3, &[Keyword::Lifelink])),
                    ready(corpus("Rampaging Ferocidon")),
                ],
            )
            .battlefield(1, [ready(free_creature("Wall", 0, 5))]),
    )?;
    let begun = driver.engine().events().len();
    driver.attack(&[("Leech", 1)])?;
    driver.idle_until(2, Step::EndCombat)?;
    let played = &driver.engine().events()[begun..];
    assert!(
        played
            .iter()
            .any(|e| matches!(e, Event::DamageDealt { .. }))
    );
    assert!(!played.iter().any(|e| matches!(e, Event::LifeGained { .. })));
    driver.expect_life(0, 20)?;
    Ok(())
}

#[test]
fn a_creature_entering_hurts_its_controller_under_the_ferocidon() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .hand(0, [free_creature("Bear", 2, 2)])
            .battlefield(0, [ready(corpus("Rampaging Ferocidon"))]),
    )?;
    driver.cast("Bear")?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(driver.state().stack(), []);
    driver.expect_life(0, 19)?;
    driver.expect_life(1, 20)
}

#[rstest]
#[case::without_the_rule(Vec::new(), 1)]
#[case::with_maralen(vec![ready(corpus("Maralen of the Mornsong"))], 0)]
fn a_player_who_cant_draw_draws_nothing(
    #[case] extra: Vec<mtg_engine::setup::PresetPermanent>,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(salve_duel(extra))?;
    let events = cast_salve(&mut driver)?;
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::CardDrawn { .. }))
            .count(),
        drawn
    );
    Ok(())
}

#[test]
fn a_player_who_cant_draw_does_not_lose_for_an_empty_library() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .hand(0, [salve("Salve")])
            .library(0, Vec::<Arc<CardDefinition>>::new())
            .battlefield(
                0,
                [
                    ready(forest()),
                    ready(forest()),
                    ready(corpus("Maralen of the Mornsong")),
                ],
            ),
    )?;
    let events = cast_salve(&mut driver)?;
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::DrewFromEmptyLibrary { .. }))
    );
    assert!(driver.state().player(pid(0)).unwrap().is_in_game());
    Ok(())
}

#[test]
fn maralen_takes_the_draw_step_and_gives_a_search_instead() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 1, Step::Upkeep)
            .battlefield(0, [ready(corpus("Maralen of the Mornsong"))])
            .library(
                1,
                [
                    free_creature("Card A", 1, 1),
                    free_creature("Card B", 2, 2),
                    free_creature("Card C", 3, 3),
                ],
            ),
    )?;
    let begun = driver.engine().events().len();
    let mut asked = false;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            assert_eq!(
                decision.player,
                pid(1),
                "the player whose draw step it is searches"
            );
            assert_eq!(options.from.len(), 3);
            let pick = options
                .from
                .iter()
                .copied()
                .find(|id| driver.state().object(*id).unwrap().card().name == "Card B")
                .unwrap();
            driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
            asked = true;
            continue;
        }
        if driver.state().turn().step() == Step::Main1 && driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(asked, "the search was asked");
    let played = &driver.engine().events()[begun..];
    assert!(
        !played.iter().any(|e| matches!(e, Event::CardDrawn { .. })),
        "nothing was drawn"
    );
    assert!(
        played
            .iter()
            .any(|e| matches!(e, Event::LibraryShuffled { .. }))
    );
    driver.expect_life(1, 17)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(1))), ["Card B"]);
    assert_eq!(driver.state().zone(Zone::Library(pid(1))).len(), 2);
    Ok(())
}

#[rstest]
#[case::no_orb(Vec::new(), 1)]
#[case::with_the_orb(vec![ready(mindlock_orb())], 0)]
fn a_player_who_cant_search_libraries_is_not_asked_and_the_rest_of_the_spell_happens(
    #[case] extra: Vec<mtg_engine::setup::PresetPermanent>,
    #[case] lands: usize,
) -> Result<(), ScriptError> {
    let mut battlefield = vec![ready(forest()), ready(forest())];
    battlefield.extend(extra);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .hand(0, [corpus("Rampant Growth")])
            .library(0, [forest(), free_creature("Top", 1, 1)])
            .battlefield(0, battlefield),
    )?;
    let begun = driver.engine().events().len();
    driver.cast("Rampant Growth")?;
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseCards(options) => {
                driver.answer(Answer::ChooseCards(CardsChoice(options.from.clone())))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let played = &driver.engine().events()[begun..];
    let found = played
        .iter()
        .filter(|e| matches!(e, Event::CardsFound(_)))
        .count();
    assert_eq!(found, lands, "a search is logged only when it happens");
    assert!(
        played
            .iter()
            .any(|e| matches!(e, Event::LibraryShuffled { .. })),
        "the shuffle is its own instruction"
    );
    let forests = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Forest")
        .count();
    assert_eq!(forests, 2 + lands);
    Ok(())
}

fn mindlock_orb() -> Arc<CardDefinition> {
    corpus("Mindlock Orb")
}

// The top card of a library

#[test]
fn the_top_of_a_revealed_library_is_known_to_every_seat_until_the_rule_ends()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::Main1)
            .library(
                0,
                [
                    free_creature("First", 1, 1),
                    free_creature("Second", 1, 1),
                    free_creature("Third", 1, 1),
                ],
            )
            .battlefield(0, [ready(corpus("Goblin Spy")), ready(mountain())])
            .battlefield(1, [ready(plains())]),
    )?;
    let state = driver.state();
    let top = state
        .revealed_top(pid(0))
        .expect("the top card is revealed");
    assert_eq!(state.object(top).unwrap().card().name, "First");
    assert_eq!(state.revealed_top(pid(1)), None);
    for viewer in 0..3 {
        assert!(state.is_shown_to(pid(viewer), top));
    }
    let spy = driver.id_of("Goblin Spy")?;
    driver.add_rule_from(spy, Rule::RevealTop(vec![pid(1)]), Duration::UntilEndOfTurn)?;
    assert!(driver.state().revealed_top(pid(1)).is_some());
    // Seat 0's rule goes with the Spy.
    driver.idle_until(3, Step::Upkeep)?;
    assert!(
        driver.state().revealed_top(pid(1)).is_none(),
        "the rule of the turn ended"
    );
    assert!(driver.state().revealed_top(pid(0)).is_some());
    Ok(())
}

// Triggers that trigger again

/// A zero-cost permanent of the type that gains its controller 1 life whenever a permanent of `on` enters.
fn watcher(name: &str, on: &str) -> Arc<CardDefinition> {
    let trigger = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"{on}"}}],"to":"Battlefield"}}}},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability(&trigger)),
    )
}

/// A zero-cost card of this type with no abilities, which enters from the hand.
fn newcomer(ty: CardType) -> Arc<CardDefinition> {
    let card = CardDefinition::new("Newcomer")
        .with_type(ty)
        .with_mana_cost(mtg_testkit::cards::cost(0, &[]));
    Arc::new(if ty == CardType::Creature {
        card.with_power_toughness(1, 1)
    } else {
        card
    })
}

/// How many times the watcher triggered when the newcomer entered.
fn triggers_on_entering(
    mine: &[&str],
    theirs: &[&str],
    on: &str,
    entering: Arc<CardDefinition>,
) -> Result<usize, ScriptError> {
    let land = entering.types.contains(&CardType::Land);
    let mut battlefield = vec![ready(watcher("Watcher", on))];
    battlefield.extend(mine.iter().map(|name| ready(corpus(name))));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .hand(0, [entering])
            .battlefield(0, battlefield)
            .battlefield(1, theirs.iter().map(|name| ready(corpus(name)))),
    )?;
    let begun = driver.engine().events().len();
    if land {
        driver.play_land("Newcomer")?;
    } else {
        driver.cast("Newcomer")?;
    }
    resolve_the_stack(&mut driver)?;
    let triggered = driver.engine().events()[begun..]
        .iter()
        .filter(|event| matches!(event, Event::AbilityTriggered(_)))
        .count();
    // Each time it triggers it gains one life.
    driver.expect_life(0, 20 + i32::try_from(triggered).unwrap())?;
    Ok(triggered)
}

#[rstest]
#[case::nothing_doubles(&[], &[], "Creature", CardType::Creature, 1)]
#[case::a_creature_entering(&["Panharmonicon"], &[], "Creature", CardType::Creature, 2)]
#[case::an_artifact_entering(&["Panharmonicon"], &[], "Artifact", CardType::Artifact, 2)]
#[case::a_land_entering(&["Panharmonicon"], &[], "Land", CardType::Land, 1)]
#[case::two_of_them(&["Panharmonicon", "Panharmonicon"], &[], "Creature", CardType::Creature, 3)]
#[case::three_of_them(&["Panharmonicon", "Panharmonicon", "Panharmonicon"], &[], "Creature", CardType::Creature, 4)]
#[case::another_players(&[], &["Panharmonicon"], "Creature", CardType::Creature, 1)]
#[case::a_death_is_not_an_entering(&["Teysa Karlov"], &[], "Creature", CardType::Creature, 1)]
#[case::a_wizard_that_is_not_one(&["Naban, Dean of Iteration"], &[], "Creature", CardType::Creature, 1)]
fn a_permanent_triggers_again_for_each_rule_whose_event_this_is(
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] on: &str,
    #[case] entering: CardType,
    #[case] expected: usize,
) -> Result<(), ScriptError> {
    assert_eq!(
        triggers_on_entering(mine, theirs, on, newcomer(entering))?,
        expected
    );
    Ok(())
}

#[rstest]
#[case::with_naban(&["Naban, Dean of Iteration"], 2)]
#[case::with_panharmonicon(&["Panharmonicon"], 2)]
#[case::with_both(&["Naban, Dean of Iteration", "Panharmonicon"], 3)]
#[case::with_neither(&[], 1)]
fn a_wizard_entering_triggers_again_under_naban(
    #[case] mine: &[&str],
    #[case] expected: usize,
) -> Result<(), ScriptError> {
    let wizard = Arc::new(
        CardDefinition::new("Newcomer")
            .with_type(CardType::Creature)
            .with_subtype("Wizard")
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_power_toughness(1, 1),
    );
    assert_eq!(
        triggers_on_entering(mine, &[], "Creature", wizard)?,
        expected
    );
    Ok(())
}

#[test]
fn an_ability_that_triggers_only_once_each_turn_is_not_doubled() -> Result<(), ScriptError> {
    let once = Arc::new(
        CardDefinition::new("Once")
            .with_type(CardType::Artifact)
            .with_rules(ability(
                r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"to":"Battlefield"}},"limit":"OncePerTurn","effects":[{"GainLife":{"amount":1}}]}}"#,
            )),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .hand(0, [newcomer(CardType::Creature)])
            .battlefield(0, [ready(once), ready(corpus("Panharmonicon"))]),
    )?;
    driver.cast("Newcomer")?;
    resolve_the_stack(&mut driver)?;
    driver.expect_life(0, 21)
}

#[test]
fn a_creature_entering_triggers_its_own_enters_ability_again() -> Result<(), ScriptError> {
    let gainer = Arc::new(
        CardDefinition::new("Gainer")
            .with_type(CardType::Creature)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_power_toughness(1, 1)
            .with_rules(ability(
                r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":3}}]}}"#,
            )),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .hand(0, [gainer])
            .battlefield(0, [ready(corpus("Panharmonicon"))]),
    )?;
    driver.cast("Gainer")?;
    resolve_the_stack(&mut driver)?;
    driver.expect_life(0, 26)
}

/// An artifact that gains its controller 1 life whenever a creature dies.
fn reaper() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Reaper").with_type(CardType::Artifact).with_rules(ability(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    )))
}

#[rstest]
#[case::no_teysa(&[], 2)]
#[case::teysa_among_the_dead(&["Teysa Karlov"], 6)]
#[case::two_teysas(&["Teysa Karlov", "Teysa Karlov"], 12)]
fn a_death_triggers_again_under_teysa_even_as_she_dies(
    #[case] teysas: &[&str],
    #[case] expected: i32,
) -> Result<(), ScriptError> {
    // Wrath of God destroys every creature at once: the Bears and each Teysa die together, and what leaves the
    // battlefield is judged as it was, so the rule of a Teysa that dies applies to every death in that step.
    let mut battlefield = vec![ready(reaper())];
    battlefield.extend(teysas.iter().map(|name| ready(corpus(name))));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .hand(0, [corpus("Wrath of God")])
            .battlefield(
                0,
                battlefield.into_iter().chain([
                    ready(plains()),
                    ready(plains()),
                    ready(plains()),
                    ready(plains()),
                ]),
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Bear", 2, 2)),
                    ready(free_creature("Ox", 3, 3)),
                ],
            ),
    )?;
    driver.cast("Wrath of God")?;
    resolve_the_stack(&mut driver)?;
    let dead = driver
        .engine()
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    from: Zone::Battlefield,
                    to: Zone::Graveyard(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(dead, 2 + teysas.len());
    // The Reaper gains 1 life for each death, once more for each Teysa's rule.
    driver.expect_life(0, 20 + expected)
}
