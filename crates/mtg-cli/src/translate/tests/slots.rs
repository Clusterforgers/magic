//! Goal: the slot frame reads the actor of a player verb from `Defined$`, writes one "may" however many parts of a line ask, reads a player word by its kind, and gives a chooser who is the line's target; a name bound inside a "may" and read outside stays a gap, and an asked player is asked once when the card is played through the engine.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;
use serde_json::{Value, json};

use super::super::frame::{self, PlayerWord};
use super::{abilities, gaps};

fn sacrifice(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigSac | TriggerDescription$ When T enters, sacrifice it.\nSVar:TrigSac:DB$ Sacrifice | {line}\n"
    )
}

fn trigger_effects(text: &str) -> Value {
    let found = abilities(text);
    let triggered = found
        .iter()
        .find(|ability| ability.get("Triggered").is_some())
        .expect("a trigger");
    triggered["Triggered"]["effects"].clone()
}

/// `Defined$` is the sacrificing player; a card named as the sacrificer is its controller, never a permanent in the `who` slot.
#[rstest]
#[case::the_card_itself("Defined$ Self", json!([{"Sacrifice": {"filter": ["This"]}}]))]
#[case::the_card_named_as_a_card("Defined$ Card.Self", json!([{"Sacrifice": {"filter": ["This"]}}]))]
#[case::you("Defined$ You | SacValid$ Creature", json!([{"Sacrifice": {"filter": [{"Type": "Creature"}]}}]))]
#[case::each_player(
    "Defined$ Player | SacValid$ Creature",
    json!([{"Sacrifice": {"who": "EachPlayer", "filter": [{"Type": "Creature"}]}}])
)]
fn the_sacrificing_player_is_defined(#[case] line: &str, #[case] expected: Value) {
    let text = sacrifice(line);
    let found = trigger_effects(&text);
    assert_eq!(gaps(&text), Vec::<String>::new(), "{line}");
    assert_eq!(found, expected);
}

#[rstest]
#[case::objects_are_no_player(
    "Defined$ Valid Creature.YouCtrl | SacValid$ Creature",
    "key:Sacrifice.Defined"
)]
#[case::another_player_needs_a_filter("Defined$ Player", "key:Sacrifice.SacValid")]
#[case::the_actor_spelling_of_another_verb("DefinedPlayer$ You", "key:Sacrifice.DefinedPlayer")]
fn a_sacrifice_no_slot_honours_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&sacrifice(line)), vec![gap.to_owned()]);
}

#[rstest]
#[case::event("TriggeredPlayer", Some(PlayerWord::Event))]
#[case::remembered("Remembered", Some(PlayerWord::Earlier))]
#[case::target_owner("TargetedOwner", Some(PlayerWord::OfTarget))]
#[case::enchanted("Player.EnchantedBy", Some(PlayerWord::Enchanted))]
#[case::defending("DefendingPlayer", Some(PlayerWord::Defending))]
#[case::a_card("Card.Self", None)]
#[case::a_group("Valid Creature", None)]
fn a_player_word_is_read_by_its_kind(#[case] word: &str, #[case] kind: Option<PlayerWord>) {
    assert_eq!(PlayerWord::parse(word), kind);
}

fn may(effects: &[Value], who: Option<&str>) -> Value {
    let mut node = json!({"May": {"effects": effects}});
    if let Some(who) = who {
        node["May"]["who"] = json!(who);
    }
    node
}

/// A "may" is written once: the same player asked again takes the rest into the first question, another player is asked apart.
#[rstest]
#[case::plain(vec![json!("a")], "You", vec![may(&[json!("a")], None)])]
#[case::another_player(vec![json!("a")], "Opponents", vec![may(&[json!("a")], Some("Opponents"))])]
#[case::asked_again(
    vec![may(&[json!("a")], None)], "You", vec![may(&[json!("a")], None)]
)]
#[case::asked_again_with_more_after(
    vec![may(&[json!("a")], None), json!("b")], "You", vec![may(&[json!("a"), json!("b")], None)]
)]
#[case::another_player_asked_first(
    vec![may(&[json!("a")], Some("Opponents"))], "You",
    vec![may(&[may(&[json!("a")], Some("Opponents"))], None)]
)]
fn a_may_is_asked_once(
    #[case] effects: Vec<Value>,
    #[case] who: &str,
    #[case] expected: Vec<Value>,
) {
    assert_eq!(frame::ask(effects, &json!(who)), expected);
}

fn search_trigger(decider: &str, optional: &str) -> String {
    format!(
        "Name:Finder\nTypes:Creature Elf\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self{decider} | Execute$ TrigSearch | TriggerDescription$ When Finder enters, you may search your library for a land card, put it into your hand.\nSVar:TrigSearch:DB$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Land{optional} | ChangeNum$ 1\n"
    )
}

/// A trigger that says "you may" asks once, however the script spreads the question over the trigger and its step.
#[rstest]
#[case::both(" | OptionalDecider$ You", " | Optional$ True")]
#[case::the_trigger_only(" | OptionalDecider$ You", "")]
#[case::the_step_only("", " | Optional$ True")]
fn a_trigger_that_says_may_has_one_may(#[case] decider: &str, #[case] optional: &str) {
    let text = search_trigger(decider, optional);
    let found = trigger_effects(&text);
    let steps = found.as_array().unwrap();
    assert_eq!(steps.len(), 1, "{found}");
    let inner = steps[0]["May"]["effects"].as_array().unwrap();
    assert!(
        inner.iter().all(|step| step.get("May").is_none()),
        "{found}"
    );
}

const ME: PlayerId = PlayerId(0);

fn finder(text: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Finder")
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_mana_cost(cost(0, &[]));
    for value in abilities(text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

/// Passes priority until a question other than priority is asked.
fn until_asked(driver: &mut Driver) -> Decision {
    for _ in 0..8 {
        let decision = driver.decision().unwrap().clone();
        if !matches!(decision.kind, DecisionKind::Priority(_)) {
            return decision;
        }
        driver.pass().unwrap();
    }
    panic!("nothing was asked");
}

/// Played through the engine, the owner of the card is asked once whether to search, then which land; declining takes nothing.
#[rstest]
#[case::accepts(true, 1)]
#[case::declines(false, 0)]
fn the_player_is_asked_once_whether_to_search(#[case] yes: bool, #[case] drawn: usize) {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .library(0, vec![forest(); 6])
            .library(1, vec![forest(); 6])
            .hand(
                0,
                [finder(&search_trigger(
                    " | OptionalDecider$ You",
                    " | Optional$ True",
                ))],
            ),
    )
    .unwrap();
    driver.cast("Finder").unwrap();
    let asked = until_asked(&mut driver);
    assert_eq!(asked.player, ME);
    assert!(matches!(asked.kind, DecisionKind::YesNo(_)), "{asked:?}");
    driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
    if yes {
        let next = until_asked(&mut driver);
        let DecisionKind::ChooseCards(options) = next.kind else {
            panic!("the pick of the land was expected, got {next:?}");
        };
        let first = options.from[0];
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![first])))
            .unwrap();
    }
    let state = driver.state();
    assert_eq!(state.zone(Zone::Hand(ME)).len(), drawn);
    assert!(
        matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)),
        "nothing else is asked"
    );
}

const CLASH: &str = "Name:Clash\nTypes:Instant\nA:SP$ ChangeZone | ValidTgts$ Permanent.nonLand | Optional$ True | DefinedPlayer$ TargetedOwner | Chooser$ TargetedOwner | Origin$ Battlefield | Destination$ Library | LibraryPosition$ 0 | RememberChanged$ True | SubAbility$ DBDamage | SpellDescription$ Its owner may put it on top. If they do, Clash deals 2 damage to them.\nSVar:DBDamage:DB$ DealDamage | Defined$ TargetedOwner | NumDmg$ 2 | ConditionDefined$ Remembered | ConditionPresent$ Card\n";

/// A name bound inside a "may" that another player answers is not readable outside it: the line stays a gap.
#[test]
fn a_name_bound_inside_a_may_and_read_outside_stays_a_gap() {
    assert!(
        gaps(CLASH)
            .iter()
            .any(|gap| gap.contains("Optional-remembered"))
    );
}

/// "Target opponent chooses a creature they control": the target is the chooser and the pool reads that target.
#[test]
fn the_target_player_chooses_from_what_they_control() {
    let text = "Name:Pick\nTypes:Sorcery\nA:SP$ ChooseCard | ValidTgts$ Opponent | Mandatory$ True | Choices$ Creature.TargetedPlayerCtrl | SubAbility$ DBBounce | SpellDescription$ Target opponent chooses a creature they control. Return it to its owner's hand.\nSVar:DBBounce:DB$ ChangeZone | Defined$ ChosenCard | Origin$ Battlefield | Destination$ Hand\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let spell = &abilities(text)[0]["Spell"];
    let choose = &spell["effects"][0]["Choose"];
    assert_eq!(choose["who"], json!({"Target": 0}));
    assert_eq!(spell["targets"][0]["what"], json!("Opponent"));
}

/// A keyword whose cost the line reduces is a gap, never the printed cost alone.
#[rstest]
#[case::plain("K:Flashback:8 B B", true)]
#[case::reduced(
    "K:Flashback:8 B B:ReduceCost$ X:This spell costs {X} less to cast this way.\nSVar:X:Count$xPaid",
    false
)]
fn a_reduced_keyword_cost_is_not_dropped(#[case] line: &str, #[case] whole: bool) {
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 1\n{line}\n");
    assert_eq!(gaps(&text).is_empty(), whole, "{:?}", gaps(&text));
}

#[test]
fn a_chooser_that_is_an_object_target_stays_a_gap() {
    let text = "Name:Pick\nTypes:Sorcery\nA:SP$ ChooseCard | ValidTgts$ Creature.YouCtrl | SpellDescription$ Choose target creature you control.\n";
    assert_eq!(gaps(text), vec!["key:ChooseCard.ValidTgts".to_owned()]);
}
