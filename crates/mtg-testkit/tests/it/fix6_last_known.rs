//! Goal: what an ability reads of an object that left the battlefield is its last known information (CR 608.2h, 400.7): the power of the creature a reflexive trigger's source sacrificed, the counters of a permanent sacrificed as a cost, the controller of a creature that died to the damage dealt, and a card that moved to hand from among the cards looked at is not moved again; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::scenario::with_counters;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact with one activated ability: `cost` and `effects` as the card DSL writes them.
fn rig(name: &str, cost: &str, effects: &str, targets: &str) -> Arc<CardDefinition> {
    artifact_with(
        name,
        vec![ability(&format!(
            r#"{{"Activated":{{"cost":{cost},"effects":{effects},"targets":{targets}}}}}"#
        ))],
    )
}

/// Activates `name` and answers until the stack is empty: cards are chosen up to the most, targets are `aim` when
/// legal and the first legal one else.
fn run(driver: &mut Driver, name: &str, aim: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        let done = matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty();
        if done {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(CardsChoice(
                o.from.iter().copied().take(o.max as usize).collect(),
            )),
            DecisionKind::ChooseTargets(o) => {
                let wanted = aim.filter(|target| o.legal.contains(target));
                Answer::ChooseTargets(TargetChoice::Targets(
                    wanted
                        .into_iter()
                        .chain(o.legal.iter().copied())
                        .take(o.min.max(1) as usize)
                        .collect(),
                ))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::NoSuchCard {
        name: "a finished resolution".to_owned(),
        place: "40 answers",
    })
}

fn name_of(driver: &Driver, id: mtg_core::ids::ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| name_of(&driver, *id).to_owned())
        .collect()
}

/// Minsc & Boo, minus two: the damage is the power the sacrificed creature had on the battlefield, counters included.
const MINSC: &str = r#"[{"PutCounters":{"count":3,"counter":"+1/+1","on":{"Each":[{"Type":"Creature"}]}}},{"Sacrifice":{"as":"sacrificed","filter":[{"Type":"Creature"}]}},{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[{"Type":"Creature"}],"subject":{"Bound":"sacrificed"}}},"then":[{"Bind":{"as":"when2","what":{"Bound":"sacrificed"}}},{"Reflexive":{"effects":[{"DealDamage":{"amount":{"Stat":{"of":{"Bound":"when2"},"stat":"Power"}},"to":{"Target":0}}}],"targets":[{"what":"AnyTarget"}]}}]}}]"#;

#[rstest]
fn the_damage_is_the_last_known_power_of_the_sacrificed_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        vec![
            ready(rig("Walker", r#"["Tap"]"#, MINSC, "[]")),
            ready(free_creature("Fodder", 1, 1)),
        ],
    ))?;
    let aim = driver.target_player(seats - 1);
    run(&mut driver, "Walker", Some(aim))?;
    assert_eq!(life(&driver, seats - 1), 20 - 4, "1 power and 3 counters");
    Ok(())
}

/// Rakdos Riteknife: "Sacrifice this: target player sacrifices a permanent for each blood counter on this."
const KNIFE: &str = r#"[{"GainLife":{"who":"You","amount":{"Counters":{"counter":{"Other":"blood"},"on":"This"}}}}]"#;

#[rstest]
fn the_counters_of_a_permanent_sacrificed_as_a_cost_are_counted_as_it_had_them(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] counters: u32,
) -> Result<(), ScriptError> {
    let knife = with_counters(
        ready(rig("Knife", r#"["SacrificeSelf"]"#, KNIFE, "[]")),
        CounterKind::Other("blood".into()),
        counters,
    );
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, vec![knife]))?;
    run(&mut driver, "Knife", None)?;
    assert_eq!(life(&driver, 0), 20 + i32::try_from(counters).unwrap());
    Ok(())
}

/// Acorn Catapult: one damage to any target, then the Squirrel goes to that player or that permanent's controller.
const CATAPULT: &str = r#"[{"DealDamage":{"amount":1,"to":{"Target":0}}},{"CreateToken":{"token":{"Custom":{"colors":["Green"],"name":"Squirrel","power":1,"subtypes":["Squirrel"],"toughness":1,"types":["Creature"]}},"who":{"OrController":{"Target":0}}}}]"#;

fn squirrels(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(&driver, **id) == "Squirrel")
        .filter(|id| driver.state().object(**id).unwrap().controller() == pid(seat))
        .count()
}

#[rstest]
fn the_squirrel_goes_to_the_player_or_to_the_controller_of_the_permanent_even_when_it_died(
    #[values(2, 3, 4)] seats: usize,
    #[values("player", "dying creature", "planeswalker")] at: &str,
) -> Result<(), ScriptError> {
    let victim = seats - 1;
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        vec![ready(rig(
            "Catapult",
            r#"["Tap"]"#,
            CATAPULT,
            r#"[{"what":"AnyTarget"}]"#,
        ))],
    );
    scenario = scenario.battlefield(
        victim,
        vec![
            ready(free_creature("Victim", 1, 1)),
            ready(mtg_testkit::cards::walker("Jace", 3)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let aim = match at {
        "player" => driver.target_player(victim),
        "dying creature" => driver.target_named("Victim")?,
        _ => driver.target_named("Jace")?,
    };
    run(&mut driver, "Catapult", Some(aim))?;
    for seat in 0..seats {
        assert_eq!(
            squirrels(&driver, seat),
            usize::from(seat == victim),
            "{at}"
        );
    }
    Ok(())
}

/// Garruk's Harbinger: look at three, put the one creature into the hand, the rest on the bottom.
const HARBINGER: &str = r#"[{"LookAtTop":{"as":"top","count":3}},{"Choose":{"as":"kept2","count":{"max":1,"min":0},"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Creature"}]}}},{"Move":{"to":"Hand","what":{"Bound":"kept2"}}},{"Move":{"to":{"Library":{"order":"Random","place":"Bottom"}},"what":{"Bound":"top"}}}]"#;

#[rstest]
fn the_card_taken_into_the_hand_is_not_put_on_the_bottom_with_the_rest(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, vec![ready(rig("Rig", r#"["Tap"]"#, HARBINGER, "[]"))])
            .library(
                0,
                vec![
                    mountain(),
                    free_creature("Pick", 2, 2),
                    mountain(),
                    free_creature("Deep", 3, 3),
                ],
            ),
    )?;
    run(&mut driver, "Rig", None)?;
    let hand = names_in(&driver, Zone::Hand(pid(0)));
    let library = names_in(&driver, Zone::Library(pid(0)));
    assert_eq!(hand.len(), 1, "{hand:?}");
    assert!(hand[0] == "Pick" || hand[0] == "Deep", "{hand:?}");
    assert_eq!(library.len(), 3, "the other three stay: {library:?}");
    assert!(!library.contains(&hand[0]));
    Ok(())
}

/// "Search your library for up to three creature cards with different names, put them into your hand."
const SEARCH_NAMES: &str = r#"[{"Search":{"filter":[{"Type":"Creature"}],"as":"found","count":{"min":0,"max":3},"distinct":true}},{"Move":{"to":"Hand","what":{"Bound":"found"}}}]"#;

#[rstest]
fn a_search_with_different_names_offers_one_card_per_name_and_refuses_a_second_of_a_name(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, vec![ready(rig("Rig", r#"["Tap"]"#, SEARCH_NAMES, "[]"))])
            .library(
                0,
                vec![
                    free_creature("Wolf", 2, 2),
                    free_creature("Wolf", 2, 2),
                    free_creature("Wolf", 2, 2),
                    free_creature("Bear", 2, 2),
                ],
            ),
    )?;
    driver.begin_activation("Rig")?;
    for _ in 0..10 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        panic!("the search asks for cards");
    };
    assert!(options.one_per_name, "the client is told");
    assert_eq!(options.max, 2, "two names, so two cards at most");
    let wolves: Vec<_> = options
        .from
        .iter()
        .copied()
        .filter(|id| name_of(&driver, *id) == "Wolf")
        .collect();
    let refused = driver.answer(Answer::ChooseCards(CardsChoice(wolves[..2].to_vec())));
    assert!(refused.is_err(), "two Wolves are refused");
    let bear = options
        .from
        .iter()
        .copied()
        .find(|id| name_of(&driver, *id) == "Bear")
        .unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![wolves[0], bear])))?;
    let mut hand = names_in(&driver, Zone::Hand(pid(0)));
    hand.sort();
    assert_eq!(hand, ["Bear", "Wolf"]);
    Ok(())
}
