//! Goal: the helpers a spell offers (convoke creatures, improvise artifacts, delve cards) never include an object its own additional cost takes, whatever the cost does with it (sacrifice, tap, return to hand, exile from the graveyard) and whether the keyword is printed or given by a permanent (Inspiring Statuary), at two to four seats: a spell whose cost would take the only helper is not offered, the pick for the cost leaves the other helpers and no more, a payment that names a taken object is refused by the payment check with the log unchanged, the bots pay with what is offered, and the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::{IllegalAnswer, PaymentError};
use mtg_engine::help::Convoked;
use mtg_engine::mana::Payment;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, PresetPermanent, replay};
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, improvise_card, ready};
use rstest::rstest;

use crate::common::pid;

/// What the additional cost of the spell does with the object it takes.
#[derive(Debug, Clone, Copy)]
enum Part {
    Sacrifice,
    Tap,
    Return,
    Exile,
}

/// The helper a keyword uses.
#[derive(Debug, Clone, Copy)]
enum Help {
    Improvise,
    Convoke,
    Delve,
}

impl Help {
    fn keyword(self) -> Keyword {
        match self {
            Help::Improvise => Keyword::Improvise,
            Help::Convoke => Keyword::Convoke,
            Help::Delve => Keyword::Delve,
        }
    }

    /// The objects a cost takes when it takes what the keyword uses.
    fn filter(self) -> &'static str {
        match self {
            Help::Improvise => r#"[{"Type":"Artifact"}]"#,
            Help::Convoke => r#"[{"Type":"Creature"}]"#,
            Help::Delve => "[]",
        }
    }
}

/// "As an additional cost to cast this spell, <part>" on a sorcery for `{1}` with the keyword; the cost takes the
/// objects that match `filter`.
fn spell_with(keyword: Keyword, part: Part, filter: &str) -> Arc<CardDefinition> {
    let item = match part {
        Part::Sacrifice => format!(r#"{{"Sacrifice":{{"filter":{filter}}}}}"#),
        Part::Tap => format!(r#"{{"TapPermanents":{{"filter":{filter}}}}}"#),
        Part::Return => format!(r#"{{"ReturnToHand":{{"filter":{filter}}}}}"#),
        Part::Exile => format!(r#"{{"ExileFromGraveyard":{{"filter":{filter}}}}}"#),
    };
    let cost = format!(r#"{{"AdditionalCost":{{"cost":[{item}],"required":true}}}}"#);
    Arc::new(
        CardDefinition::new("Helped Spell")
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(Ability::Keyword(keyword))
            .with_rules(serde_json::from_str::<Ability>(&cost).unwrap())
            .with_rules(
                serde_json::from_str::<Ability>(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap(),
            ),
    )
}

/// The spell of a keyword whose cost takes the objects that keyword uses.
fn spell(help: Help, part: Part) -> Arc<CardDefinition> {
    spell_with(help.keyword(), part, help.filter())
}

/// A card by name: two artifacts and the Statuary, two creatures, a Mountain, or a card of the corpus.
fn card(name: &str) -> Arc<CardDefinition> {
    match name {
        "Bear" | "Cub" => creature(name, 1, &[Color::Green], 2, 2),
        "Mountain" => mountain(),
        "Trinket" | "Gadget" | "Pricey Relic" | "Inspiring Statuary" => {
            improvise_card(name).unwrap()
        }
        other => corpus::card(other).unwrap(),
    }
}

fn permanents(names: &[&str]) -> Vec<PresetPermanent> {
    names.iter().map(|name| ready(card(name))).collect()
}

/// Seat 0 holds `spell` with `mine` in play and `grave` in its graveyard.
fn table(
    seats: usize,
    spell: Arc<CardDefinition>,
    mine: &[&str],
    grave: &[&str],
) -> (Driver, Scenario) {
    let scenario = Scenario::new(seats)
        .hand(0, [spell])
        .battlefield(0, permanents(mine))
        .graveyard(0, grave.iter().map(|name| card(name)));
    (Driver::from_scenario(scenario.clone()).unwrap(), scenario)
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, not {}", other.name()),
    }
}

/// The object called `name` on the battlefield or in the graveyard of seat 0.
fn find(driver: &Driver, name: &str) -> ObjectId {
    let state = driver.state();
    [Zone::Battlefield, Zone::Graveyard(pid(0))]
        .into_iter()
        .flat_map(|zone| state.zone(zone).iter().copied())
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .unwrap()
}

/// The helpers of a kind offered now, by name.
fn offered(driver: &Driver, help: Help) -> Vec<String> {
    let options = driver.pay_options().unwrap();
    let ids: Vec<ObjectId> = match help {
        Help::Improvise => options.help.improvise.clone(),
        Help::Convoke => options.help.convoke.iter().map(|o| o.creature).collect(),
        Help::Delve => options.help.delve.clone(),
    };
    let mut names: Vec<String> = ids
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    names.sort();
    names
}

/// A payment that names `object` as the helper and nothing else.
fn naming(help: Help, object: ObjectId) -> Payment {
    match help {
        Help::Improvise => Payment {
            improvise: vec![object],
            ..Payment::default()
        },
        Help::Convoke => Payment {
            convoke: vec![Convoked {
                creature: object,
                pays: None,
            }],
            ..Payment::default()
        },
        Help::Delve => Payment {
            delve: vec![object],
            ..Payment::default()
        },
    }
}

fn replays(driver: &Driver, scenario: &Scenario) {
    let replayed = replay(&scenario.clone().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Every keyword with every part of a cost that takes what the keyword uses.
const PARTS: [(Help, Part); 7] = [
    (Help::Improvise, Part::Sacrifice),
    (Help::Improvise, Part::Tap),
    (Help::Improvise, Part::Return),
    (Help::Convoke, Part::Sacrifice),
    (Help::Convoke, Part::Tap),
    (Help::Convoke, Part::Return),
    (Help::Delve, Part::Exile),
];

/// The two objects a keyword can use.
fn pair(help: Help) -> [&'static str; 2] {
    match help {
        Help::Improvise | Help::Delve => ["Trinket", "Gadget"],
        Help::Convoke => ["Bear", "Cub"],
    }
}

/// Where the objects of a keyword are: the battlefield, or the graveyard for delve.
fn placed<'a>(help: Help, names: &[&'a str]) -> (Vec<&'a str>, Vec<&'a str>) {
    match help {
        Help::Delve => (Vec::new(), names.to_vec()),
        _ => (names.to_vec(), Vec::new()),
    }
}

#[rstest]
#[case::the_only_object_is_taken_by_the_cost(1, false)]
#[case::an_object_beside_the_one_taken(2, true)]
fn a_spell_whose_cost_takes_the_only_helper_is_not_offered(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6)] at: usize,
    #[case] objects: usize,
    #[case] expected: bool,
) {
    let (help, part) = PARTS[at];
    let (mine, grave) = placed(help, &pair(help)[..objects]);
    let (driver, _) = table(seats, spell(help, part), &mine, &grave);
    assert_eq!(castable(&driver), expected, "{help:?} with {part:?}");
}

#[rstest]
fn a_pick_for_the_cost_leaves_the_other_helper_and_the_taken_one_is_refused(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6)] at: usize,
    #[values(0, 1)] picked: usize,
) -> Result<(), ScriptError> {
    let (help, part) = PARTS[at];
    let names = pair(help);
    let (mine, grave) = placed(help, &names);
    let (mut driver, scenario) = table(seats, spell(help, part), &mine, &grave);
    driver.begin_cast("Helped Spell")?;
    let (taken, left) = (names[picked], names[1 - picked]);
    let taken_id = find(&driver, taken);
    driver.expect_decision(0, "choose-cards")?;
    driver.answer(Answer::ChooseCards(CardsChoice(vec![taken_id])))?;
    assert_eq!(offered(&driver, help), vec![left.to_owned()], "{help:?}");
    // The check every payment goes through refuses the object the cost took, and nothing changes.
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    let result = driver.answer(Answer::PayMana(PayChoice::Manual(naming(help, taken_id))));
    let Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Payment(error)))) = result
    else {
        panic!("a payment error was expected, not {result:?}");
    };
    let expected = match help {
        Help::Delve => PaymentError::NotInGraveyard(taken_id),
        _ => PaymentError::NotOffered(taken_id),
    };
    assert_eq!(error, expected);
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), log);
    // The other one pays, and the log replays.
    let left_id = find(&driver, left);
    driver.answer(Answer::PayMana(PayChoice::Manual(naming(help, left_id))))?;
    assert_eq!(driver.state().stack().len(), 1, "the spell is on the stack");
    replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::an_artifact_beside_a_sacrificed_creature(Keyword::Improvise, Help::Convoke, "Trinket")]
#[case::a_creature_beside_a_sacrificed_artifact(Keyword::Convoke, Help::Improvise, "Bear")]
fn an_object_the_cost_does_not_touch_still_helps(
    #[values(2, 3, 4)] seats: usize,
    #[values(Part::Sacrifice, Part::Tap, Part::Return)] part: Part,
    #[case] keyword: Keyword,
    #[case] taking: Help,
    #[case] spare: &str,
) -> Result<(), ScriptError> {
    let taken = if spare == "Trinket" {
        "Bear"
    } else {
        "Trinket"
    };
    let spell = spell_with(keyword.clone(), part, taking.filter());
    let (mut driver, _) = table(seats, spell, &[taken, spare], &[]);
    assert!(castable(&driver), "{keyword:?} with {part:?}");
    driver.begin_cast("Helped Spell")?;
    let help = if spare == "Trinket" {
        Help::Improvise
    } else {
        Help::Convoke
    };
    assert_eq!(offered(&driver, help), vec![spare.to_owned()]);
    Ok(())
}

// Improvise given by Inspiring Statuary to a spell with an additional cost: Trash for Treasure.

/// The artifact card Trash for Treasure returns.
const RELIC: &str = "Pricey Relic";

#[rstest]
#[case::the_statuary_is_the_only_artifact(&["Mountain", "Mountain", "Inspiring Statuary"], false)]
#[case::two_artifacts_are_one_short(&["Mountain", "Inspiring Statuary", "Trinket"], false)]
#[case::three_artifacts_pay_after_one_is_sacrificed(&["Mountain", "Inspiring Statuary", "Trinket", "Gadget"], true)]
fn a_sacrificed_statuary_does_not_improvise_the_spell_it_was_sacrificed_for(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: &[&str],
    #[case] expected: bool,
) {
    let spell = corpus::card("Trash for Treasure").unwrap();
    let (driver, _) = table(seats, spell, mine, &[RELIC]);
    assert_eq!(castable(&driver), expected);
}

#[rstest]
fn the_artifact_sacrificed_for_the_cost_is_not_among_the_artifacts_offered(
    #[values(2, 3, 4)] seats: usize,
    #[values("Inspiring Statuary", "Trinket", "Gadget")] sacrificed: &str,
) -> Result<(), ScriptError> {
    let spell = corpus::card("Trash for Treasure").unwrap();
    let mine = ["Mountain", "Inspiring Statuary", "Trinket", "Gadget"];
    let (mut driver, scenario) = table(seats, spell, &mine, &[RELIC]);
    driver.begin_cast("Trash for Treasure")?;
    let id = find(&driver, sacrificed);
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let relic = find(&driver, RELIC);
        let target = Target::Object(driver.state().object(relic).unwrap().reference());
        driver.choose_targets(&[target])?;
    }
    let mut expected: Vec<String> = ["Inspiring Statuary", "Trinket", "Gadget"]
        .iter()
        .filter(|name| **name != sacrificed)
        .map(|name| (*name).to_owned())
        .collect();
    expected.sort();
    assert_eq!(offered(&driver, Help::Improvise), expected);
    let helpers: Vec<&str> = expected.iter().map(String::as_str).collect();
    driver.pay_helped_by(&["Mountain"], &[], &helpers, &[], 0)?;
    assert_eq!(driver.state().stack().len(), 1);
    replays(&driver, &scenario);
    Ok(())
}

// The bots

#[rstest]
fn a_bot_pays_the_spell_with_the_helpers_offered_and_the_log_replays(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6)] at: usize,
) {
    let (help, part) = PARTS[at];
    let (mine, grave) = placed(help, &pair(help));
    let mut cast = 0;
    for seed in 0..24 {
        let (mut driver, scenario) = table(seats, spell(help, part), &mine, &grave);
        driver.begin_cast("Helped Spell").unwrap();
        let mut bot = RandomBot::new(seed, 0).with_partial_payments();
        for _ in 0..8 {
            let decision = driver.decision().unwrap().clone();
            if !matches!(
                decision.kind,
                DecisionKind::ChooseCards(_) | DecisionKind::PayMana(_)
            ) {
                break;
            }
            let answer = bot.choose(&decision);
            if let Err(error) = driver.answer(answer.clone()) {
                panic!("seed {seed}: {error} for {answer:?}");
            }
        }
        cast += driver.state().stack().len();
        replays(&driver, &scenario);
    }
    assert!(cast > 0, "the bot cast the spell in some game");
}
