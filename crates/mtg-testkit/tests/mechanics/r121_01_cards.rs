//! Goal: at 2 to 4 seats, The Madcap Jester lets its controller play the cards their own spells and abilities exile from
//! opponents' libraries (its attack trigger among them) with mana of any type, never a card exiled from their own
//! library or by an opponent's spell; a payment any number of times keeps its count under its name for any later step;
//! "put up to N counters" puts the number chosen, from none to N; the other cards of the pack (Borg Cube, The Best of
//! Both Worlds, The Guardian of Forever, Dominion Saboteur, T'Pring, Colossal Clone Master, Tresserhorn's Lord, Zero, Gift of
//! Perfection, Nog) play as printed; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::choose::NumberChoice;
use mtg_engine::choose::{CardsChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, cost, forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::with_rules;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Answers until the stack and the pending triggers are empty: `pick` answers the decisions it knows, the rest are
/// answered passively.
fn r121_resolve(
    driver: &mut Driver,
    pick: impl Fn(&Decision) -> Option<Answer>,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let busy =
            !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty();
        let decision = driver.decision()?.clone();
        if !matches!(decision.kind, DecisionKind::Priority(_))
            && let Some(answer) = pick(&decision)
        {
            driver.answer(answer)?;
            continue;
        }
        match &decision.kind {
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => return Ok(()),
            // The first legal target of each choice, rather than the cancel a passive answer may give.
            DecisionKind::ChooseTargets(options) if !options.legal.is_empty() => {
                let first = options.legal[..1].to_vec();
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(first)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the resolution".into()))
}

/// The log replays to the live state.
fn r121_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// A free card of the type with the one ability.
fn r121_card(name: &str, kind: CardType, rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

/// The table: each seat's library is red instants named after the seat ("Bolt 1").
fn r121_table(seats: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..5).map(|_| burn(&format!("Bolt {seat}"), 1)))
    })
}

/// The object of the named card, in any zone.
fn r121_id(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap()
}

/// The cards in exile, with their owners' seats.
fn r121_exiled(driver: &Driver) -> Vec<(ObjectId, usize)> {
    let state = driver.state();
    state
        .zone(Zone::Exile)
        .iter()
        .filter_map(|id| {
            let object = state.object(*id)?;
            Some((*id, object.owner().index()))
        })
        .collect()
}

/// The cards seat 0 may cast now.
fn r121_castable(driver: &Driver) -> Vec<ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) if driver.decision().unwrap().player == pid(0) => {
            options.castable.clone()
        }
        _ => Vec::new(),
    }
}

#[rstest]
#[case::each_opponent("Opponents", true)]
#[case::each_player("EachPlayer", true)]
#[case::yourself("You", false)]
fn r121_the_jester_plays_what_you_exile_from_opponents(
    #[values(2, 3, 4)] seats: usize,
    #[case] who: &str,
    #[case] plays: bool,
) -> Result<(), ScriptError> {
    let peek = r121_card(
        "Peek",
        CardType::Sorcery,
        &format!(r#"{{"Spell":{{"effects":[{{"ExileTop":{{"who":"{who}","as":"x"}}}}]}}}}"#),
    );
    let scenario = r121_table(seats)
        .hand(0, [peek])
        .battlefield(
            0,
            [ready(card("The Madcap Jester").unwrap()), ready(forest())],
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Peek")?;
    r121_resolve(&mut driver, |_| None)?;
    let exiled = r121_exiled(&driver);
    let castable = r121_castable(&driver);
    for (id, owner) in &exiled {
        assert_eq!(
            castable.contains(id),
            plays && *owner != 0,
            "seat {owner}'s card"
        );
    }
    if plays {
        // A red instant cast with the Forest's green mana: mana of any type can be spent.
        let (_, owner) = exiled.iter().find(|(_, owner)| *owner != 0).unwrap();
        driver.begin_cast(&format!("Bolt {owner}"))?;
        r121_resolve(&mut driver, |decision| match &decision.kind {
            DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
            _ => None,
        })?;
        assert_eq!(r121_exiled(&driver).len(), exiled.len() - 1);
    }
    r121_replays(&driver);
    Ok(())
}

#[rstest]
fn r121_an_opponents_spell_gives_the_jester_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let peek = Arc::new(
        CardDefinition::new("Quick Peek")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Spell":{"effects":[{"ExileTop":{"who":"EachPlayer","as":"x"}}]}}"#,
                )
                .unwrap(),
            ),
    );
    let scenario = r121_table(seats)
        .hand(1, [peek])
        .battlefield(
            0,
            [ready(card("The Madcap Jester").unwrap()), ready(forest())],
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    driver.cast("Quick Peek")?;
    r121_resolve(&mut driver, |_| None)?;
    assert_eq!(r121_exiled(&driver).len(), seats);
    assert!(driver.state().pending_triggers().is_empty());
    assert!(r121_castable(&driver).is_empty());
    r121_replays(&driver);
    Ok(())
}

#[rstest]
fn r121_the_jester_plays_what_its_attack_exiles(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = r121_table(seats)
        .battlefield(
            0,
            [ready(card("The Madcap Jester").unwrap()), ready(forest())],
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("The Madcap Jester", 1)])?;
    r121_resolve(&mut driver, |_| None)?;
    let exiled = r121_exiled(&driver);
    assert_eq!(exiled.len(), seats - 1);
    let castable = r121_castable(&driver);
    assert!(exiled.iter().all(|(id, _)| castable.contains(id)));
    r121_replays(&driver);
    Ok(())
}

#[rstest]
#[case::none(0)]
#[case::some(2)]
#[case::the_most(3)]
fn r121_a_payment_any_number_of_times_is_kept_by_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] times: u32,
) -> Result<(), ScriptError> {
    // Pay {1} any number of times, then gain that much life twice: two steps after it read the number by its name.
    let spell = r121_card(
        "Tithe",
        CardType::Sorcery,
        r#"{"Spell":{"effects":[{"PayAnyTimes":{"cost":"{1}","as":"paid"}},{"Draw":{"count":{"Bound":"paid"}}},{"GainLife":{"amount":{"Bound":"paid"}}}]}}"#,
    );
    let scenario = r121_table(seats)
        .hand(0, [spell])
        .battlefield(0, (0..3).map(|_| ready(forest())))
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.cast("Tithe")?;
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseNumber(options) => {
            assert_eq!(options.max, Some(3));
            Some(Answer::ChooseNumber(NumberChoice::Number(times)))
        }
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        _ => None,
    })?;
    let player = driver.state().player(pid(0)).unwrap();
    assert_eq!(player.life() - life, i32::try_from(times).unwrap());
    // The hand lost the spell and drew one card for each payment.
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len() + 1,
        hand + times as usize
    );
    r121_replays(&driver);
    Ok(())
}

#[rstest]
#[case::none(0)]
#[case::one(1)]
#[case::all_three(3)]
fn r121_up_to_n_counters_puts_the_number_chosen(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen: u32,
) -> Result<(), ScriptError> {
    let spell = r121_card(
        "Growth",
        CardType::Sorcery,
        r#"{"Spell":{"effects":[{"ChooseValue":{"kind":{"Number":[0,3]},"as":"up_to"}},{"PutCounters":{"on":{"Each":[{"Type":"Artifact"}]},"counter":"charge","count":{"ChosenAs":"up_to"}}}]}}"#,
    );
    let relic = Arc::new(
        CardDefinition::new("Relic")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(0, &[])),
    );
    let scenario = r121_table(seats)
        .hand(0, [spell])
        .battlefield(0, [ready(relic)])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Growth")?;
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseNumber(options) => {
            assert_eq!((options.min, options.max), (0, Some(3)));
            Some(Answer::ChooseNumber(NumberChoice::Number(chosen)))
        }
        _ => None,
    })?;
    let relic = driver.id_of("Relic")?;
    let charge = CounterKind::Charge;
    let held = driver
        .state()
        .object(relic)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&charge);
    assert_eq!(held, chosen);
    r121_replays(&driver);
    Ok(())
}

/// Whether a permanent named so is on the battlefield under the seat's control.
fn r121_controls(driver: &Driver, seat: usize, name: &str) -> bool {
    let state = driver.state();
    state.zone(Zone::Battlefield).iter().any(|id| {
        state
            .object(*id)
            .is_some_and(|object| object.card().name == name)
            && controller_of(state, *id) == Some(pid(seat))
    })
}

/// Aims a choice of targets at the first legal creature seat 0 does not control, else at nothing.
fn r121_aim_away(driver: &Driver, decision: &Decision) -> Option<Answer> {
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        return None;
    };
    let state = driver.state();
    let theirs: Vec<Target> = options
        .legal
        .iter()
        .filter(|target| {
            target
                .object()
                .is_some_and(|object| controller_of(state, object.id) != Some(pid(0)))
        })
        .take(1)
        .copied()
        .collect();
    Some(Answer::ChooseTargets(TargetChoice::Targets(theirs)))
}

#[rstest]
#[case::nothing_exiled(false)]
#[case::a_card_exiled(true)]
fn r121_borg_cube_assimilates_or_else_exiles_a_target(
    #[values(2, 3, 4)] seats: usize,
    #[case] exiled_first: bool,
) -> Result<(), ScriptError> {
    // The Cube is a creature at twelve charge counters, so it attacks; "{0}: Exile target creature card from a
    // graveyard" gives it a card exiled with it.
    let exile: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":[],"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    let cube = with_rules(&card("Borg Cube").unwrap(), vec![exile]);
    let scenario = r121_table(seats)
        .battlefield(0, [with_counters(ready(cube), CounterKind::Charge, 12)])
        .battlefield(1, [ready(bear())])
        .graveyard(1, [card("Llanowar Elves").unwrap()])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    if exiled_first {
        driver.begin_activation("Borg Cube")?;
        r121_resolve(&mut driver, |_| None)?;
    }
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Borg Cube", 1)])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if let Some(answer) = r121_aim_away(&driver, &decision) {
            driver.answer(answer)?;
            continue;
        }
        let busy =
            !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty();
        match decision.kind {
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    // Assimilated, the Elves are seat 0's and the Bears stay; else the Bears are exiled.
    assert_eq!(r121_controls(&driver, 0, "Llanowar Elves"), exiled_first);
    assert_eq!(r121_controls(&driver, 1, "Grizzly Bears"), exiled_first);
    r121_replays(&driver);
    Ok(())
}

#[rstest]
fn r121_the_best_of_both_worlds_returns_the_borg_it_assimilated(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let lands = [plains(), island(), forest(), forest(), forest(), forest()];
    let scenario = r121_table(seats)
        .hand(0, [card("The Best of Both Worlds").unwrap()])
        .battlefield(0, lands.into_iter().map(ready))
        .battlefield(1, [ready(bear())])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("The Best of Both Worlds")?;
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        _ => None,
    })?;
    assert!(r121_controls(&driver, 0, "Grizzly Bears"));
    let next = u32::try_from(2 + seats).unwrap();
    driver.idle_until(next, Step::Main1)?;
    r121_resolve(&mut driver, |_| None)?;
    assert!(r121_controls(&driver, 0, "Grizzly Bears"));
    driver.idle_until(next + u32::try_from(seats).unwrap(), Step::Main1)?;
    r121_resolve(&mut driver, |_| None)?;
    // Exiled and returned under its owner's control as a new object, no longer an artifact.
    assert!(r121_controls(&driver, 1, "Grizzly Bears"));
    let id = driver.id_of("Grizzly Bears")?;
    let found = characteristics(driver.state(), id).unwrap();
    assert!(!found.types.contains(CardType::Artifact));
    r121_replays(&driver);
    Ok(())
}

#[rstest]
fn r121_the_guardian_of_forever_returns_the_sacrificed_card_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = r121_table(seats)
        .battlefield(
            0,
            [
                ready(card("The Guardian of Forever").unwrap()),
                ready(bear()),
                ready(forest()),
                ready(forest()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("The Guardian of Forever")?;
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        _ => None,
    })?;
    assert!(!r121_controls(&driver, 0, "Grizzly Bears"));
    driver.pass_until(Step::End)?;
    r121_resolve(&mut driver, |_| None)?;
    assert!(r121_controls(&driver, 0, "Grizzly Bears"));
    r121_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_kinds(&[("charge", 3), ("+1/+1", 2)])]
#[case::none(&[])]
fn r121_dominion_saboteur_copies_with_the_copied_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: &[(&str, u32)],
) -> Result<(), ScriptError> {
    let relic = Arc::new(
        CardDefinition::new("Relic")
            .with_type(CardType::Artifact)
            .with_supertype(Supertype::Legendary)
            .with_mana_cost(cost(0, &[])),
    );
    let kinds: Vec<(CounterKind, u32)> = held
        .iter()
        .map(|(kind, count)| {
            let kind: CounterKind = serde_json::from_str(&format!("\"{kind}\"")).unwrap();
            (kind, *count)
        })
        .collect();
    let relic = kinds.iter().fold(ready(relic), |permanent, (kind, count)| {
        with_counters(permanent, kind.clone(), *count)
    });
    let lands = [island(), forest(), forest(), forest(), forest()];
    let scenario = r121_table(seats)
        .hand(0, [card("Dominion Saboteur").unwrap()])
        .battlefield(0, lands.into_iter().map(ready))
        .battlefield(1, [relic])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let original = driver.id_of("Relic")?;
    driver.cast("Dominion Saboteur")?;
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::ChooseCards(options) if options.from.contains(&original) => {
            Some(Answer::ChooseCards(CardsChoice(vec![original])))
        }
        _ => None,
    })?;
    let state = driver.state();
    let copy = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            *id != original
                && controller_of(state, *id) == Some(pid(0))
                && characteristics(state, *id).is_some_and(|c| c.types.contains(CardType::Artifact))
        })
        .unwrap();
    let found = characteristics(state, copy).unwrap();
    // A copy of the Relic: an artifact, no longer a creature.
    assert!(found.types.contains(CardType::Artifact) && !found.types.contains(CardType::Creature));
    assert!(!mtg_engine::characteristics::has_supertype(
        state,
        copy,
        Supertype::Legendary
    ));
    let permanent = state.object(copy).unwrap().permanent().unwrap();
    for (kind, count) in &kinds {
        assert_eq!(permanent.counters_of(kind), *count);
    }
    assert_eq!(permanent.counters().count(), kinds.len());
    r121_replays(&driver);
    Ok(())
}

#[rstest]
#[case::they_fight(true, 2)]
#[case::no_fight(false, 0)]
fn r121_tpring_draws_for_each_creature_that_fought_and_died(
    #[values(2, 3, 4)] seats: usize,
    #[case] fight: bool,
    #[case] draws: usize,
) -> Result<(), ScriptError> {
    let lands = [island(), forest(), forest(), forest(), forest()];
    let scenario = r121_table(seats)
        .hand(0, [card("T'Pring, Never Parted").unwrap()])
        .battlefield(0, lands.into_iter().map(ready).chain([ready(bear())]))
        .battlefield(1, [ready(bear())])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("T'Pring, Never Parted")?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(mtg_engine::choose::YesNoChoice(fight))),
        _ => None,
    })?;
    // Both 2/2 Bears die of the fight, each a draw; without the fight both live.
    assert_eq!(r121_controls(&driver, 0, "Grizzly Bears"), !fight);
    assert_eq!(r121_controls(&driver, 1, "Grizzly Bears"), !fight);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + draws);
    r121_replays(&driver);
    Ok(())
}

#[rstest]
fn r121_colossal_clone_master_copies_with_the_count_of_land_names(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Five differently named lands (two Forests are one name).
    let lands = [island(), plains(), swamp(), mountain(), forest(), forest()];
    let scenario = r121_table(seats)
        .hand(0, [card("Colossal Clone Master").unwrap()])
        .battlefield(0, lands.into_iter().map(ready))
        .battlefield(1, [ready(bear())])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let original = driver.id_of("Grizzly Bears")?;
    driver.cast("Colossal Clone Master")?;
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::ChooseCards(options) if options.from.contains(&original) => {
            Some(Answer::ChooseCards(CardsChoice(vec![original])))
        }
        _ => None,
    })?;
    let state = driver.state();
    let copy = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            controller_of(state, *id) == Some(pid(0))
                && characteristics(state, *id).is_some_and(|c| c.types.contains(CardType::Creature))
        })
        .unwrap();
    let found = characteristics(driver.state(), copy).unwrap();
    assert_eq!((found.power, found.toughness), (Some(5), Some(5)));
    r121_replays(&driver);
    Ok(())
}

#[rstest]
#[case::paid(3, true)]
#[case::declined(3, false)]
#[case::too_few(1, true)]
fn r121_tresserhorns_lord_stays_only_when_the_whole_price_is_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] creatures: usize,
    #[case] pay: bool,
) -> Result<(), ScriptError> {
    let lands = [island(), swamp(), mountain(), forest(), forest()];
    let scenario = r121_table(seats)
        .hand(0, [card("Tresserhorn's Lord, Returned").unwrap()])
        .battlefield(
            0,
            lands
                .into_iter()
                .map(ready)
                .chain((0..creatures).map(|_| ready(bear()))),
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    let theirs = driver.state().zone(Zone::Hand(pid(1))).len();
    let paid = pay && creatures >= 2;
    driver.cast("Tresserhorn's Lord, Returned")?;
    let mine = driver.state().zone(Zone::Hand(pid(0))).len();
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(mtg_engine::choose::YesNoChoice(paid))),
        _ => None,
    })?;
    assert_eq!(
        r121_controls(&driver, 0, "Tresserhorn's Lord, Returned"),
        paid
    );
    let state = driver.state();
    let lost = life - state.player(pid(0)).unwrap().life();
    assert_eq!(lost, if paid { 3 } else { 0 });
    assert_eq!(
        state.zone(Zone::Hand(pid(1))).len(),
        theirs + if paid { 3 } else { 0 }
    );
    // Each creature sacrificed draws a card, the Lord itself among them (it looks back as it leaves).
    assert_eq!(
        state.zone(Zone::Hand(pid(0))).len(),
        mine + if paid { 3 } else { 1 }
    );
    r121_replays(&driver);
    Ok(())
}

/// Seat 1's castable cards when it next has priority in seat 0's main phase.
fn r121_castable_by_seat_1(driver: &mut Driver) -> Result<Vec<ObjectId>, ScriptError> {
    driver.pass()?;
    let decision = driver.decision()?;
    assert_eq!(decision.player, pid(1));
    Ok(match &decision.kind {
        DecisionKind::Priority(options) => options.castable.clone(),
        _ => Vec::new(),
    })
}

#[rstest]
#[case::with_the_two_more(3, true)]
#[case::without_the_two_more(2, false)]
fn r121_zero_lets_the_owner_cast_the_exiled_card_for_two_more(
    #[values(2, 3, 4)] seats: usize,
    #[case] mountains: usize,
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let scenario = r121_table(seats)
        .hand(0, [card("Zero, Incorporeal Sensate").unwrap()])
        .battlefield(0, [ready(plains()), ready(plains()), ready(forest())])
        .hand(1, [burn("Shock", 2)])
        .battlefield(1, (0..mountains).map(|_| ready(mountain())))
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Zero, Incorporeal Sensate")?;
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::ChooseTargets(options) => Some(Answer::ChooseTargets(TargetChoice::Targets(
            vec![Target::Player(pid(1))]
                .into_iter()
                .filter(|t| options.legal.contains(t))
                .collect(),
        ))),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(mtg_engine::choose::YesNoChoice(true))),
        _ => None,
    })?;
    let shock = r121_id(&driver, "Shock");
    assert!(r121_exiled(&driver).iter().any(|(id, _)| *id == shock));
    assert_eq!(
        r121_castable_by_seat_1(&mut driver)?.contains(&shock),
        castable
    );
    r121_replays(&driver);
    Ok(())
}

/// The characteristics of the named permanent.
fn r121_chars(driver: &Driver, name: &str) -> mtg_engine::Characteristics {
    characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap()
}

#[rstest]
#[case::scout_then_officer(&["{1}", "{3}"], true)]
#[case::officer_first(&["{3}"], false)]
fn r121_nog_becomes_a_scout_then_an_officer(
    #[values(2, 3, 4)] seats: usize,
    #[case] costs: &[&str],
    #[case] scout: bool,
) -> Result<(), ScriptError> {
    let scenario = r121_table(seats)
        .battlefield(
            0,
            std::iter::once(ready(card("Nog, Ambitious Cadet").unwrap()))
                .chain((0..4).map(|_| ready(forest()))),
        )
        .graveyard(0, [island()])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    for activated in costs {
        let index = if *activated == "{1}" { 1 } else { 2 };
        driver.begin_ability("Nog, Ambitious Cadet", index)?;
        r121_resolve(&mut driver, |decision| match &decision.kind {
            DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
            _ => None,
        })?;
    }
    let found = r121_chars(&driver, "Nog, Ambitious Cadet");
    let rogue = Subtype("Rogue".to_owned());
    let officer = Subtype("Officer".to_owned());
    // The Scout became an Officer and plays lands from the graveyard; a Rogue stays one and gets nothing.
    let printed = &driver
        .state()
        .object(driver.id_of("Nog, Ambitious Cadet")?)
        .unwrap()
        .card()
        .subtypes
        .clone();
    assert_eq!(found.has_subtype(printed, &officer), scout);
    assert_eq!(found.has_subtype(printed, &rogue), !scout);
    assert_eq!(
        (found.power, found.toughness),
        if scout {
            (Some(3), Some(4))
        } else {
            (Some(2), Some(2))
        }
    );
    let island = driver.state().zone(Zone::Graveyard(pid(0)))[0];
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("seat 0 has priority");
    };
    assert_eq!(options.playable_lands.contains(&island), scout);
    r121_replays(&driver);
    Ok(())
}

#[rstest]
fn r121_gift_of_perfection_assimilates_one_creature_revealed_and_bottoms_the_rest(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Each opponent's library holds two Forests and a Grizzly Bears, whichever end is the top.
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Gift of Perfection").unwrap()])
        .battlefield(
            0,
            [island(), forest(), forest(), forest(), forest()]
                .into_iter()
                .map(ready),
        )
        .library(0, (0..5).map(|_| forest()))
        .at(2, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario.library(seat, [forest(), bear(), forest()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Gift of Perfection")?;
    r121_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        _ => None,
    })?;
    let state = driver.state();
    let mine = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|o| o.card().name == "Grizzly Bears")
                && controller_of(state, **id) == Some(pid(0))
        })
        .count();
    assert_eq!(mine, 1);
    let left: usize = (1..seats)
        .map(|seat| state.zone(Zone::Library(pid(seat))).len())
        .sum();
    assert_eq!(left, 3 * (seats - 1) - 1);
    r121_replays(&driver);
    Ok(())
}
