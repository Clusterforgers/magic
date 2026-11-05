//! Goal: the slot frame reads who acts, what, how many, who decides, optional and destination of `ChangeZone` in one place: slots combine without a special case (a second way decided by any player, a library of another player searched by you), a value no slot can honour stays a named gap, a verb that is not `ChangeZone` reads the same slots, and the cards it frees play through the engine with the owner picking, a declined option taking nothing and nothing stuck.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use super::super::ctx::{Ctx, World};
use super::super::frame::{self, Slot, Verb};
use super::{abilities, gaps};
use crate::forge::script::Params;

fn effects_of(line: &str) -> Value {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | {line} | SpellDescription$ You may do it.\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new(), "{line}");
    abilities(&text)[0]["Spell"]["effects"].clone()
}

fn choice(words: [&str; 2], who: Option<Value>) -> Value {
    let mut choose = json!({"kind": {"Options": words}, "as": "way"});
    if let Some(who) = who {
        choose["who"] = who;
    }
    json!({ "ChooseValue": choose })
}

fn on_library(what: &Value, place: &Value) -> Value {
    json!({"Move": {"what": what, "to": {"Library": {"place": place}}}})
}

/// `AlternativeDecider$`, `DestinationAlternative$` and `LibraryPositionAlternative$` follow from the slots, each read alone.
#[rstest]
#[case::owner_picks_top_or_bottom(
    "ValidTgts$ Creature | AlternativeDecider$ TargetedOwner | Origin$ Battlefield | Destination$ Library | DestinationAlternative$ Library | LibraryPositionAlternative$ -1",
    json!({"Target": 0}), json!("Top"), json!("Bottom"), ["Top", "Bottom"], Some(json!({"OwnerOf": {"Target": 0}}))
)]
#[case::second_from_the_top_or_bottom(
    "ValidTgts$ Creature | AlternativeDecider$ TargetedOwner | Origin$ Battlefield | Destination$ Library | LibraryPosition$ 1 | DestinationAlternative$ Library | LibraryPositionAlternative$ -1",
    json!({"Target": 0}), json!({"Below": 1}), json!("Bottom"), ["Second from the top", "Bottom"], Some(json!({"OwnerOf": {"Target": 0}}))
)]
#[case::you_decide_without_a_decider(
    "ValidTgts$ Creature | Origin$ Battlefield | Destination$ Library | DestinationAlternative$ Library | LibraryPositionAlternative$ -1",
    json!({"Target": 0}), json!("Top"), json!("Bottom"), ["Top", "Bottom"], None
)]
#[case::the_source_named(
    "Defined$ Self | Origin$ Graveyard | Destination$ Library | DestinationAlternative$ Library | LibraryPositionAlternative$ -1",
    json!("This"), json!("Top"), json!("Bottom"), ["Top", "Bottom"], None
)]
fn two_ways_are_a_choice_of_a_word_then_the_move(
    #[case] line: &str,
    #[case] what: Value,
    #[case] first: Value,
    #[case] second: Value,
    #[case] words: [&str; 2],
    #[case] who: Option<Value>,
) {
    let found = effects_of(line);
    let name = found[0]["ChooseValue"]["as"].clone();
    let mut asked = found[0].clone();
    asked["ChooseValue"]["as"] = json!("way");
    assert_eq!(asked, choice(words, who));
    assert_eq!(
        found[1]["If"],
        json!({
            "condition": {"ChosenWord": {"name": name, "word": words[1]}},
            "then": [on_library(&what, &second)],
            "otherwise": [on_library(&what, &first)],
        })
    );
}

#[test]
fn two_ways_also_follow_a_pick_and_another_zone() {
    let found = effects_of(
        "Origin$ Graveyard | Destination$ Hand | DestinationAlternative$ Exile | ChangeType$ Creature.YouOwn | ChangeNum$ 1 | Hidden$ True",
    );
    assert!(found[0].get("Choose").is_some());
    assert_eq!(
        found[1]["ChooseValue"]["kind"]["Options"],
        json!(["Hand", "Exile"])
    );
    assert_eq!(
        found[2]["If"]["then"][0]["Exile"]["what"]["Bound"],
        found[0]["Choose"]["as"]
    );
}

/// Who acts and who decides are two slots: the actor's zone, the decider's pick.
#[rstest]
#[case::caster_searches_a_targets_library("ValidTgts$ Opponent", json!({"who": null, "of": {"Target": 0}}))]
#[case::caster_chooser_said("ValidTgts$ Player | Chooser$ You", json!({"who": null, "of": {"Target": 0}}))]
#[case::the_target_searches_its_own("ValidTgts$ Player | Chooser$ Targeted", json!({"who": {"Target": 0}, "of": null}))]
#[case::the_target_picks_from_yours("ValidTgts$ Opponent | DefinedPlayer$ You | Chooser$ Targeted", json!({"who": {"Target": 0}, "of": "You"}))]
fn the_library_is_the_actors_and_the_pick_the_deciders(
    #[case] slots: &str,
    #[case] expected: Value,
) {
    let found = effects_of(&format!(
        "Origin$ Library | Destination$ Exile | ChangeType$ Card | ChangeNum$ 3 | {slots}"
    ));
    let search = &found[0]["Search"];
    assert_eq!(
        search.get("who").cloned().unwrap_or(Value::Null),
        expected["who"]
    );
    assert_eq!(
        search.get("of").cloned().unwrap_or(Value::Null),
        expected["of"]
    );
    assert_eq!(found.as_array().map(Vec::len), Some(3));
    let owner = search
        .get("of")
        .or(search.get("who"))
        .filter(|who| **who != json!("You"))
        .cloned();
    assert_eq!(found[2]["Shuffle"].get("who").cloned(), owner);
}

/// A pick from the battlefield is among what a named player controls, or everyone's when no player is named.
#[rstest]
#[case::you_named("DefinedPlayer$ You", json!([{"Type": "Land"}, {"ControlledBy": "You"}]))]
#[case::the_target_named(
    "ValidTgts$ Opponent | DefinedPlayer$ Targeted",
    json!([{"Type": "Land"}, {"ControlledBy": {"Target": 0}}])
)]
#[case::nobody_named("Mandatory$ True", json!([{"Type": "Land"}]))]
fn a_pick_from_the_battlefield_says_whose(#[case] who: &str, #[case] filter: Value) {
    let found = effects_of(&format!(
        "Origin$ Battlefield | Destination$ Hand | Hidden$ True | ChangeType$ Land | {who}"
    ));
    assert_eq!(found[0]["Choose"]["from"]["Each"], filter);
}

/// `GainControl$` names who controls what enters.
#[rstest]
#[case::you("True", json!("You"))]
#[case::the_target("Targeted", json!({"Target": 0}))]
fn what_enters_is_under_the_player_said(#[case] word: &str, #[case] under: Value) {
    let found = effects_of(&format!(
        "ValidTgts$ Opponent | DefinedPlayer$ You | Chooser$ Targeted | Origin$ Graveyard | Destination$ Battlefield | ChangeType$ Creature.YouOwn | Hidden$ True | GainControl$ {word}"
    ));
    assert_eq!(found[1]["Move"]["to"]["Battlefield"]["under"], under);
}

/// What the destination adds reads the object the move made, never the reference it began with (CR 400.7).
#[rstest]
#[case::attacking(
    "ValidTgts$ Creature.YouOwn | Origin$ Graveyard | Destination$ Battlefield | Tapped$ True | Attacking$ True",
    "Move",
    json!({"JoinAttack": {"what": {"Bound": "moved"}}})
)]
#[case::exiled_with_counters(
    "Defined$ Self | Origin$ Stack | Destination$ Exile | WithCountersType$ TIME | WithCountersAmount$ 3",
    "Exile",
    json!({"PutCounters": {"on": {"Bound": "exiled"}, "counter": {"Other": "time"}, "count": 3}})
)]
fn what_follows_a_move_reads_what_it_made(
    #[case] line: &str,
    #[case] verb: &str,
    #[case] then: Value,
) {
    let found = effects_of(line);
    assert_eq!(found[1], then);
    assert_eq!(
        found[0][verb]["as"],
        json!(if verb == "Move" { "moved" } else { "exiled" })
    );
}

/// What a slot cannot honour stays a gap, and the card out.
#[rstest]
#[case::a_conditional_alternative(
    "ValidTgts$ Creature | Origin$ Battlefield | Destination$ Library | DestinationAlternative$ Library | DestAltSVar$ X",
    "key:ChangeZone.DestAltSVar"
)]
#[case::a_group_decides(
    "ValidTgts$ Creature | AlternativeDecider$ Player | Origin$ Battlefield | Destination$ Library | DestinationAlternative$ Library | LibraryPositionAlternative$ -1",
    "key:Frame.AlternativeDecider=Player"
)]
#[case::a_shuffle_with_two_ways(
    "ValidTgts$ Creature | Shuffle$ True | Origin$ Battlefield | Destination$ Library | DestinationAlternative$ Hand",
    "key:Frame.DestinationAlternative-with-Shuffle"
)]
#[case::two_ways_that_read_the_same(
    "ValidTgts$ Creature | Origin$ Battlefield | Destination$ Library | DestinationAlternative$ Library",
    "key:Frame.DestinationAlternative-same"
)]
#[case::two_ways_remembered(
    "ValidTgts$ Creature | RememberChanged$ True | Origin$ Battlefield | Destination$ Library | DestinationAlternative$ Library | LibraryPositionAlternative$ -1",
    "key:ChangeZone.DestinationAlternative-with-memory"
)]
#[case::a_pool_on_the_named_shape(
    "ValidTgts$ Creature | ChangeType$ Land | Origin$ Battlefield | Destination$ Hand",
    "key:ChangeZone.ChangeType"
)]
#[case::hidden_with_nothing_named(
    "ValidTgts$ Creature | Hidden$ True | Origin$ Battlefield | Destination$ Hand",
    "key:ChangeZone.Hidden"
)]
#[case::attacking_for_several(
    "ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ 2 | Origin$ Graveyard | Destination$ Battlefield | Attacking$ True",
    "key:Frame.Attacking-for-several"
)]
#[case::attacking_a_named_player(
    "ValidTgts$ Creature | Origin$ Graveyard | Destination$ Battlefield | Attacking$ Remembered",
    "key:Frame.Attacking"
)]
#[case::attacking_off_the_battlefield(
    "ValidTgts$ Creature | Origin$ Battlefield | Destination$ Hand | Attacking$ True",
    "key:ChangeZone.Attacking"
)]
#[case::a_group_gains_control(
    "ValidTgts$ Creature | Origin$ Graveyard | Destination$ Battlefield | GainControl$ Player",
    "key:Frame.GainControl=Player"
)]
#[case::a_target_named_again_and_remembered(
    "Defined$ Targeted | ValidTgts$ Creature | RememberLKI$ True | Origin$ Battlefield | Destination$ Hand",
    "key:ChangeZone.Defined-with-memory"
)]
#[case::a_group_chooses_a_search(
    "Origin$ Library | Destination$ Hand | ChangeType$ Card | Chooser$ Opponent",
    "key:ChangeZone.Search.Chooser"
)]
#[case::several_targets_with_another_decider(
    "ValidTgts$ Creature | TargetMax$ 2 | TargetMin$ 0 | Optional$ True | Chooser$ TargetedOwner | Origin$ Battlefield | Destination$ Hand",
    "key:ChangeZone.Optional-for-several"
)]
#[case::an_optional_remembered_for_another_decider(
    "ValidTgts$ Creature | RememberChanged$ True | Optional$ True | Chooser$ TargetedOwner | Origin$ Battlefield | Destination$ Hand",
    "key:ChangeZone.Optional-remembered"
)]
#[case::a_chooser_with_nothing_to_decide(
    "ValidTgts$ Creature | Chooser$ TargetedOwner | Origin$ Battlefield | Destination$ Hand",
    "key:ChangeZone.Chooser-unread"
)]
fn a_value_no_slot_honours_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | {line} | SpellDescription$ You may do it.\n"
    );
    assert!(
        gaps(&text).iter().any(|found| found == gap),
        "{gap} not in {:?}",
        gaps(&text)
    );
}

/// A verb that is not `ChangeZone`, declared here as a next verb would be, reads the same slots with its own spellings.
const OTHER: Verb = Verb {
    label: "Other",
    slots: &[
        Slot::Actor,
        Slot::Target,
        Slot::Count,
        Slot::Decider,
        Slot::Optional,
        Slot::Pool,
    ],
    pool: "SacValid",
    count: "Amount",
    own: &[],
};

#[rstest]
#[case::the_slots_of_the_other_verb(
    "Amount$ 2 | SacValid$ Creature | Mandatory$ True | DefinedPlayer$ You",
    None
)]
#[case::a_key_outside_them("Origin$ Hand", Some("key:Other.Origin"))]
#[case::the_spelling_of_another_verb("ChangeNum$ 2", Some("key:Other.ChangeNum"))]
fn another_verb_declares_its_own_slots(#[case] line: &str, #[case] gap: Option<&str>) {
    let params = Params::parse(&format!("DB$ Other | {line}"));
    assert_eq!(
        OTHER.admit(&params).err().map(|found| found.0),
        gap.map(str::to_owned)
    );
}

#[test]
fn another_verb_gets_the_slots_read_the_same() {
    let world = World::default();
    let face = crate::forge::script::Face::default();
    let mut ctx = Ctx::new(&face, &world);
    let params = Params::parse(
        "DB$ Other | Amount$ 2 | SacValid$ Creature | Mandatory$ True | ValidTgts$ Opponent | Chooser$ You",
    );
    let parties = frame::parties(&mut ctx, &params, OTHER.label).unwrap();
    assert_eq!(parties.acts, json!({"Target": 0}));
    assert_eq!(parties.decides, json!("You"));
    let count = frame::count(&ctx, &params, &OTHER).unwrap();
    assert_eq!((count.min, count.max), (json!(2), json!(2)));
}

fn spell(name: &str, line: &str) -> Arc<CardDefinition> {
    let text = format!(
        "Name:{name}\nTypes:Sorcery\nA:SP$ ChangeZone | {line} | SpellDescription$ You may.\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Sorcery)
        .with_mana_cost(cost(0, &[]));
    for value in abilities(&text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn creature(name: &str, power: i32, toughness: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, toughness),
    )
}

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

const REVENGE: &str = "ValidTgts$ Creature | AlternativeDecider$ TargetedOwner | Origin$ Battlefield | Destination$ Library | DestinationAlternative$ Library | LibraryPositionAlternative$ -1";

fn game(card: Arc<CardDefinition>) -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .library(0, vec![forest(); 6])
            .library(1, vec![forest(); 6])
            .hand(0, [card])
            .battlefield(1, vec![ready(creature("Bear", 2, 2))]),
    )
    .unwrap()
}

/// Passes priority until the spell has resolved or a question other than priority is asked.
fn resolve(driver: &mut Driver) -> Decision {
    for _ in 0..8 {
        let decision = driver.decision().unwrap().clone();
        if !matches!(decision.kind, DecisionKind::Priority(_)) {
            return decision;
        }
        if driver.state().stack().is_empty() {
            return decision;
        }
        driver.pass().unwrap();
    }
    panic!("the spell did not resolve");
}

#[rstest]
#[case::top(0, 6)]
#[case::bottom(1, 0)]
fn revenge_of_the_drowned_lets_the_owner_pick_top_or_bottom(
    #[case] word: usize,
    #[case] at: usize,
) {
    let mut driver = game(spell("Revenge", REVENGE));
    let target = driver.target_named("Bear").unwrap();
    let bear_id = driver.id_of("Bear").unwrap();
    driver.cast_at("Revenge", &[target]).unwrap();
    let asked = resolve(&mut driver);
    assert_eq!(asked.player, FOE, "the owner decides, not the caster");
    let DecisionKind::ChooseOption(options) = asked.kind else {
        panic!("a choice of words was expected, got {asked:?}");
    };
    assert_eq!(options.options.len(), 2);
    driver
        .answer(Answer::ChooseOption(OptionChoice(word)))
        .unwrap();
    let library = driver.state().zone(Zone::Library(FOE));
    assert_eq!(library.len(), 7, "the creature is in its owner's library");
    assert_eq!(library.iter().position(|id| *id == bear_id), Some(at));
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
}

#[rstest]
#[case::declined(false, 1)]
#[case::accepted(true, 0)]
fn a_declined_option_moves_nothing_and_goes_on(#[case] yes: bool, #[case] on_field: usize) {
    let mut driver = game(spell(
        "Maybe",
        "ValidTgts$ Creature | Optional$ True | DefinedPlayer$ TargetedOwner | Chooser$ TargetedOwner | Origin$ Battlefield | Destination$ Hand",
    ));
    let target = driver.target_named("Bear").unwrap();
    driver.cast_at("Maybe", &[target]).unwrap();
    let asked = resolve(&mut driver);
    assert_eq!(asked.player, FOE, "the chooser decides");
    assert!(matches!(asked.kind, DecisionKind::YesNo(_)), "{asked:?}");
    driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
    let state = driver.state();
    assert_eq!(state.zone(Zone::Battlefield).len(), on_field);
    assert_eq!(state.zone(Zone::Hand(FOE)).len(), 1 - on_field);
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
}

#[rstest]
#[case::finds_one(true)]
#[case::fails_to_find(false)]
fn a_search_of_another_library_is_the_casters_pick_and_shuffles_that_library(#[case] finds: bool) {
    let mut driver = game(spell(
        "Acquire",
        "Origin$ Library | Destination$ Battlefield | ValidTgts$ Opponent | ChangeType$ Land | GainControl$ True",
    ));
    let foe = driver.target_player(1);
    driver.cast_at("Acquire", &[foe]).unwrap();
    let asked = resolve(&mut driver);
    assert_eq!(asked.player, ME, "the caster searches");
    let DecisionKind::ChooseCards(options) = asked.kind else {
        panic!("a pick of cards was expected, got {asked:?}");
    };
    assert_eq!(options.from.len(), 6, "among the opponent's library");
    let picked = if finds {
        vec![options.from[0]]
    } else {
        Vec::new()
    };
    driver
        .answer(Answer::ChooseCards(CardsChoice(picked)))
        .unwrap();
    let state = driver.state();
    assert_eq!(
        state.zone(Zone::Library(FOE)).len(),
        if finds { 5 } else { 6 }
    );
    assert_eq!(state.zone(Zone::Library(ME)).len(), 6);
    let lands_mine = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.controller() == ME)
        .count();
    assert_eq!(lands_mine, usize::from(finds));
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
}

#[test]
fn a_creature_put_onto_the_battlefield_attacking_joins_the_attack_as_a_new_object() {
    let instant_rules = abilities(
        "Name:Rally\nTypes:Instant\nA:SP$ ChangeZone | ValidTgts$ Creature.YouOwn | Origin$ Graveyard | Destination$ Battlefield | Tapped$ True | Attacking$ True | SpellDescription$ d\n",
    );
    let mut rally = CardDefinition::new("Rally")
        .with_type(CardType::Instant)
        .with_mana_cost(cost(0, &[]));
    for value in instant_rules {
        rally = rally.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .library(0, vec![forest(); 6])
            .library(1, vec![forest(); 6])
            .hand(0, [Arc::new(rally)])
            .graveyard(0, vec![creature("Bear", 2, 2)])
            .battlefield(0, vec![ready(creature("Ogre", 3, 3))]),
    )
    .unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Ogre", 1)]).unwrap();
    let dead = driver.state().zone(Zone::Graveyard(ME))[0];
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(dead).unwrap().reference());
    driver.cast_at("Rally", &[target]).unwrap();
    resolve(&mut driver);
    let state = driver.state();
    let bear = driver.id_of("Bear").unwrap();
    assert_eq!(state.combat().attackers().len(), 2);
    assert!(
        state
            .combat()
            .attackers()
            .iter()
            .any(|a| a.object() == bear)
    );
}

#[test]
fn a_spell_exiled_with_counters_has_them_in_exile() {
    let text = "Name:Fade\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Stack | Destination$ Exile | WithCountersType$ TIME | WithCountersAmount$ 3 | SpellDescription$ d\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let mut card = CardDefinition::new("Fade")
        .with_type(CardType::Sorcery)
        .with_mana_cost(cost(0, &[]));
    for value in abilities(text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    let mut driver = game(Arc::new(card));
    driver.cast("Fade").unwrap();
    resolve(&mut driver);
    let state = driver.state();
    let exiled = state.zone(Zone::Exile);
    assert_eq!(exiled.len(), 1, "the spell is in exile");
    let object = state.object(exiled[0]).unwrap();
    assert_eq!(
        object.card_counters_of(&serde_json::from_value(json!({"Other": "time"})).unwrap()),
        3
    );
}

fn rally() -> Arc<CardDefinition> {
    let rules = abilities(
        "Name:Rally\nTypes:Instant\nA:SP$ ChangeZone | ValidTgts$ Creature.YouOwn | Origin$ Graveyard | Destination$ Battlefield | Attacking$ True | SpellDescription$ d\n",
    );
    let mut rally = CardDefinition::new("Rally")
        .with_type(CardType::Instant)
        .with_mana_cost(cost(0, &[]));
    for value in rules {
        rally = rally.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(rally)
}

fn rally_game(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 6]);
    }
    Driver::from_scenario(
        scenario
            .hand(0, [rally()])
            .graveyard(0, vec![creature("Bear", 2, 2)])
            .battlefield(0, vec![ready(creature("Ogre", 3, 3))]),
    )
    .unwrap()
}

fn cast_rally(driver: &mut Driver) -> Decision {
    let dead = driver.state().zone(Zone::Graveyard(ME))[0];
    let reference = driver.state().object(dead).unwrap().reference();
    driver
        .cast_at("Rally", &[mtg_engine::choose::Target::Object(reference)])
        .unwrap();
    resolve(driver)
}

#[test]
fn with_several_defenders_the_controller_chooses_what_it_attacks() {
    let mut driver = rally_game(3);
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Ogre", 1)]).unwrap();
    let asked = cast_rally(&mut driver);
    assert_eq!(asked.player, ME);
    assert!(
        matches!(asked.kind, DecisionKind::ChooseOption(_)),
        "{asked:?}"
    );
    driver
        .answer(Answer::ChooseOption(OptionChoice(1)))
        .unwrap();
    assert_eq!(driver.state().combat().attackers().len(), 2);
}

#[test]
fn outside_an_attack_it_enters_and_nothing_is_asked() {
    let mut driver = rally_game(2);
    let asked = cast_rally(&mut driver);
    assert!(matches!(asked.kind, DecisionKind::Priority(_)), "{asked:?}");
    assert!(
        driver.id_of("Bear").is_ok(),
        "the creature is on the battlefield"
    );
    assert_eq!(driver.state().combat().attackers().len(), 0);
}
