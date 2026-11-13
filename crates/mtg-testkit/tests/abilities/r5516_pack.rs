//! Goal: the r55-16 pieces play at tables of two to four: a reflexive trigger with an intervening "if" goes on the stack
//! only while its condition holds after the step (Ajani, Nacatl Avenger); "If you would copy a spell one or more times,
//! instead copy it that many times plus an additional time." (Twinning Staff) adds a copy for its controller only; and
//! Cemetery Desecrator's exiled card sets the X of the mode its reflexive trigger chooses, with no trigger when no card
//! was exiled; Ancient Adamantoise takes the damage dealt to its controller and their other permanents, and keeps its
//! damage through the cleanup step while every other permanent's wears off; Retriever Phoenix in its owner's graveyard
//! is offered in place of its owner's learn, and returns instead on a yes (no learn) or leaves the learn on a no.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::refs::ZoneKind;
use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ModeChoice, Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};

use crate::common::pid;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A corpus card for no mana.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

/// Answers until the stack is empty and nothing waits: `mode` for a choice of modes, the target named `target` (else the
/// first legal one) for a choice of targets, the passive answer otherwise.
fn play_out(
    driver: &mut Driver,
    mode: usize,
    target: Option<&str>,
) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(events);
            }
            DecisionKind::ChooseMode(_) => Answer::ChooseMode(ModeChoice::Modes(vec![mode])),
            DecisionKind::ChooseTargets(options) => {
                let wanted = target.and_then(|name| driver.target_named(name).ok());
                let pick = wanted
                    .filter(|wanted| options.legal.contains(wanted))
                    .or_else(|| options.legal.first().copied());
                Answer::ChooseTargets(TargetChoice::Targets(pick.into_iter().collect()))
            }
            _ => Driver::passive_answer(&decision),
        };
        events.extend(driver.answer(answer)?);
    }
    Err(ScriptError::NoSuchCard {
        name: "a finished resolution".to_owned(),
        place: "60 answers",
    })
}

/// "{T}: You gain 1 life. When you do, if you control another red permanent, it deals 2 damage to target opponent."
fn rig() -> Arc<CardDefinition> {
    let steps: Vec<Effect> = serde_json::from_str(
        r#"[{"GainLife":{"amount":1}},{"Reflexive":{"targets":[{"what":"Opponent"}],
            "effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}],
            "if":{"Compare":{"left":{"Count":[{"Color":"Red"},{"ControlledBy":"You"},"Other"]},"op":"Ge","right":1}}}}]"#,
    )
    .unwrap();
    artifact_with(
        "Rig",
        vec![Ability::Activated {
            cost: Cost(vec![CostItem::Tap]),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            by: mtg_cards::ability::Activators::Controller,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: steps,
        }],
    )
}

/// A red 1/1 for no mana.
fn red_creature() -> Arc<CardDefinition> {
    let creature = (*free_creature("Ember", 1, 1)).clone();
    Arc::new(creature.with_color_indicator([Color::Red].into_iter().collect::<ColorSet>()))
}

#[rstest]
fn a_reflexive_trigger_with_an_intervening_if_triggers_only_while_it_holds(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] red: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(rig())];
    if red {
        mine.push(ready(red_creature()));
    }
    let scenario = || Scenario::new(seats).battlefield(0, mine.clone());
    let mut driver = Driver::from_scenario(scenario())?;
    let mut events = driver.begin_activation("Rig")?;
    events.extend(play_out(&mut driver, 0, None)?);
    let start = 20;
    driver.expect_life(0, start + 1)?;
    driver.expect_life(1, if red { start - 2 } else { start })?;
    let put = events
        .iter()
        .filter(|e| matches!(e, Event::AbilityPutOnStack(_)))
        .count();
    assert_eq!(put, 1 + usize::from(red), "{events:?}");
    let replayed = replay(&scenario().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn twinning_staff_adds_a_copy_for_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(0), Some(1))] staff: Option<usize>,
) -> Result<(), ScriptError> {
    // Seat 0 bolts seat 1, then copies the Bolt with Reverberate.
    let mut scenario = Scenario::new(seats)
        .hand(0, [free("Lightning Bolt"), free("Reverberate")])
        .battlefield(0, [ready(mountain())]);
    if let Some(owner) = staff {
        scenario = scenario.battlefield(owner, [ready(card("Twinning Staff").unwrap())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let face = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[face])?;
    let top = *driver.state().stack().last().unwrap();
    let bolt = Target::Object(driver.state().object(top).unwrap().reference());
    driver.cast_at("Reverberate", &[bolt])?;
    let events = play_out(&mut driver, 0, None)?;
    let copies = events
        .iter()
        .filter(|e| matches!(e, Event::SpellCopied(_)))
        .count();
    let expected = if staff == Some(0) { 2 } else { 1 };
    assert_eq!(copies, expected, "{events:?}");
    driver.expect_life(1, 20 - 3 * (1 + i32::try_from(expected).unwrap()))
}

#[rstest]
fn cemetery_desecrator_shrinks_by_the_exiled_cards_mana_value(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] exiled: bool,
) -> Result<(), ScriptError> {
    // A Grizzly Bears (mana value 2) in seat 1's graveyard, another on seat 1's battlefield.
    let mut scenario = Scenario::new(seats)
        .hand(0, [free("Cemetery Desecrator")])
        .battlefield(seats - 1, [ready(card("Grizzly Bears").unwrap())]);
    if exiled {
        scenario = scenario.graveyard(seats - 1, [card("Grizzly Bears").unwrap()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cemetery Desecrator")?;
    let events = play_out(&mut driver, 1, Some("Grizzly Bears"))?;
    let modal = events
        .iter()
        .filter(|e| matches!(e, Event::AbilityPutOnStack(_)))
        .count();
    // The enters trigger, then the reflexive one only when a card was exiled.
    assert_eq!(modal, 1 + usize::from(exiled), "{events:?}");
    let theirs = pid(seats - 1);
    if exiled {
        driver.expect_zone(Zone::Exile, &["Grizzly Bears"])?;
        driver.expect_zone(Zone::Graveyard(theirs), &["Grizzly Bears"])?;
        driver.expect_zone(Zone::Battlefield, &["Cemetery Desecrator"])
    } else {
        driver.expect_zone(Zone::Battlefield, &["Grizzly Bears", "Cemetery Desecrator"])
    }
}

/// The damage marked on the permanent with this name.
fn damage_on(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage()
}

#[rstest]
fn ancient_adamantoise_takes_its_controllers_damage_and_keeps_it(
    #[values(2, 3, 4)] seats: usize,
    #[values("you", "yours", "theirs")] hit: &str,
) -> Result<(), ScriptError> {
    // Seat 0 controls the Adamantoise and a Wall; the last seat controls a Post; seat 0 bolts one of them.
    let theirs = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [free("Lightning Bolt")])
            .battlefield(
                0,
                [
                    ready(card("Ancient Adamantoise").unwrap()),
                    ready(free_creature("Wall", 0, 5)),
                ],
            )
            .battlefield(theirs, [ready(free_creature("Post", 0, 5))]),
    )?;
    let target = match hit {
        "you" => driver.target_player(0),
        "yours" => driver.target_named("Wall")?,
        _ => driver.target_named("Post")?,
    };
    driver.cast_at("Lightning Bolt", &[target])?;
    play_out(&mut driver, 0, None)?;
    let redirected = hit != "theirs";
    driver.expect_life(0, 20)?;
    assert_eq!(damage_on(&driver, "Wall"), 0);
    assert_eq!(
        damage_on(&driver, "Ancient Adamantoise"),
        if redirected { 3 } else { 0 }
    );
    assert_eq!(damage_on(&driver, "Post"), if redirected { 0 } else { 3 });
    // The cleanup step removes the Post's damage and leaves the Adamantoise's.
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(
        damage_on(&driver, "Ancient Adamantoise"),
        if redirected { 3 } else { 0 }
    );
    assert_eq!(damage_on(&driver, "Post"), 0);
    Ok(())
}

#[rstest]
fn retriever_phoenix_returns_in_place_of_its_owners_learn(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] yes: bool,
    #[values(0, 1)] owner: usize,
) -> Result<(), ScriptError> {
    // Seat 0 casts Cram Session (gain 4 life, learn) with a card in hand; the Phoenix is in `owner`'s graveyard.
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [free("Cram Session"), free_creature("Spare", 1, 1)])
            .graveyard(owner, [card("Retriever Phoenix").unwrap()])
            .library(0, (0..5).map(|_| mountain())),
    )?;
    driver.cast("Cram Session")?;
    let mut asked = false;
    for _ in 0..30 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::YesNo(_) => {
                asked = true;
                assert_eq!(decision.player, pid(0));
                Answer::YesNo(YesNoChoice(yes))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    let mine = owner == 0;
    assert_eq!(asked, mine, "only the learner's own Phoenix is offered");
    let returned = mine && yes;
    let on_battlefield = driver.state().zone(Zone::Battlefield).len();
    assert_eq!(on_battlefield, usize::from(returned));
    let mut graveyard = vec!["Cram Session"];
    if !returned {
        graveyard.push("Retriever Phoenix");
    }
    if !mine {
        driver.expect_zone(Zone::Graveyard(pid(owner)), &["Retriever Phoenix"])?;
        graveyard.pop();
    }
    driver.expect_zone(Zone::Graveyard(pid(0)), &graveyard)?;
    driver.expect_life(0, 24)
}
