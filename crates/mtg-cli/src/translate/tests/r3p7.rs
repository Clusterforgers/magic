//! Goal: static parameters of round 3 piece P7 translate to the modifications the engine has, one table per rule, and a
//! translated rule changes a game the way the card prints.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::scenario::ready;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn static_of(line: &str) -> Vec<Value> {
    abilities(&format!("Name:Subject\nTypes:Artifact\n{line}\n"))
}

#[rstest]
#[case::panharmonicon(
    "S:Mode$ Panharmonicon | ValidMode$ ChangesZone,ChangesZoneAll | ValidCard$ Permanent.YouCtrl | ValidCause$ Artifact,Creature | Destination$ Battlefield",
    json!({"TriggerAgain": {"cause": {"ZoneChange": {"what": [{"AnyOf": [{"Type": "Artifact"}, {"Type": "Creature"}]}], "to": "Battlefield"}}}})
)]
#[case::teysa_karlov_dying(
    "S:Mode$ Panharmonicon | ValidMode$ ChangesZone,ChangesZoneAll | ValidCard$ Permanent.YouCtrl | ValidCause$ Creature | Origin$ Battlefield | Destination$ Graveyard",
    json!({"TriggerAgain": {"cause": {"ZoneChange": {"what": [{"Type": "Creature"}], "from": "Battlefield", "to": "Graveyard"}}}})
)]
#[case::isshin_attacking(
    "S:Mode$ Panharmonicon | ValidMode$ Attacks,AttackersDeclared,AttackersDeclaredOneTarget | ValidCard$ Permanent.YouCtrl | ValidCause$ Creature",
    json!({"TriggerAgain": {"cause": {"Attacks": {"what": [{"Type": "Creature"}]}}}})
)]
#[case::veyran_casting_or_copying(
    "S:Mode$ Panharmonicon | ValidMode$ SpellCast,SpellCopy,SpellCastOrCopy | ValidCard$ Permanent.YouCtrl | ValidCause$ Instant,Sorcery | ValidActivator$ You",
    json!({"TriggerAgain": {"cause": {"AnyOf": [
        {"SpellCast": {"what": [{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}], "by": "You"}},
        {"SpellCopied": {"what": [{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}], "by": "You"}}]}}})
)]
#[case::krang_drawing(
    "S:Mode$ Panharmonicon | ValidMode$ Drawn | ValidCard$ Permanent.YouCtrl",
    json!({"TriggerAgain": {"cause": {"Drew": {}}}})
)]
#[case::dr_beverly_crusher_life(
    "S:Mode$ Panharmonicon | ValidMode$ LifeGained | ValidPlayer$ You | ValidCard$ Permanent.YouCtrl",
    json!({"TriggerAgain": {"cause": {"LifeGained": {"who": "You"}}}})
)]
fn a_panharmonicon_line_names_the_event_that_triggers_again(
    #[case] line: &str,
    #[case] rule: Value,
) {
    assert_eq!(static_of(line), vec![json!({"Rule": {"rule": rule}})]);
}

#[rstest]
#[case::no_event(
    "S:Mode$ Panharmonicon | ValidCard$ Creature.YouCtrl+Legendary",
    "static:Panharmonicon-any-cause"
)]
#[case::unknown_event(
    "S:Mode$ Panharmonicon | ValidMode$ RoomEntered | ValidCard$ Dungeon.YouOwn",
    "static:Panharmonicon-mode=RoomEntered"
)]
#[case::unread_key(
    "S:Mode$ Panharmonicon | ValidMode$ ChangesZone | ValidCause$ Land | Destination$ Battlefield | ValidZone$ Command",
    "key:Static.Panharmonicon.ValidZone"
)]
fn a_panharmonicon_line_it_cannot_read_stays_a_gap_by_name(#[case] line: &str, #[case] gap: &str) {
    let text = format!("Name:Subject\nTypes:Artifact\n{line}\n");
    assert!(
        gaps(&text).iter().any(|found| found == gap),
        "{:?}",
        gaps(&text)
    );
    assert!(abilities(&text).is_empty());
}

fn card_with(types: &[CardType], name: &str, script: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_mana_cost(mtg_testkit::cards::cost(0, &[]));
    for kind in types {
        card = card.with_type(*kind);
    }
    if types.contains(&CardType::Creature) {
        card = card.with_power_toughness(1, 1);
    }
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

const ENTERS: &str = "Name:Imp\nTypes:Creature Imp\nPT:1/1\nT:Mode$ ChangesZone | ValidCard$ Card.Self | Origin$ Any | Destination$ Battlefield | Execute$ Gain | TriggerDescription$ When this enters, you gain 1 life.\nSVar:Gain:DB$ GainLife | LifeAmount$ 1\n";
const PANHARMONICON: &str = "Name:Echo\nTypes:Artifact\nS:Mode$ Panharmonicon | ValidMode$ ChangesZone,ChangesZoneAll | ValidCard$ Permanent.YouCtrl | ValidCause$ CAUSE | Destination$ Battlefield\n";

/// Life of seat 0 after it casts the Imp with the Echo in play and everything has resolved.
fn life_after_casting(cause: &str) -> i32 {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [ready(card_with(
                &[CardType::Artifact],
                "Echo",
                &PANHARMONICON.replace("CAUSE", cause),
            ))],
        )
        .hand(0, [card_with(&[CardType::Creature], "Imp", ENTERS)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let start = life(&driver);
    driver.cast("Imp").unwrap();
    for _ in 0..50 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            return life(&driver) - start;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

fn life(driver: &Driver) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(0))
        .unwrap()
        .life()
}

#[rstest]
#[case::an_entering_creature_triggers_twice("Artifact,Creature", 2)]
#[case::a_cause_the_rule_does_not_name_triggers_once("Artifact", 1)]
fn the_translated_rule_doubles_only_the_triggers_its_cause_names(
    #[case] cause: &str,
    #[case] gained: i32,
) {
    assert_eq!(life_after_casting(cause), gained);
}

fn stats_card(pt: &str, line: &str) -> String {
    format!(
        "Name:Subject\nTypes:Artifact Creature Construct\nPT:{pt}\n{line}\nSVar:X:Count$Valid Artifact.YouCtrl\n"
    )
}

#[rstest]
#[case::bronze_guardian_power("*/5", "SetPower$ X", json!({"power": {"Count": [{"Type": "Artifact"}, {"ControlledBy": "You"}]}, "toughness": 5}))]
#[case::aven_trailblazer_toughness("2/*", "SetToughness$ X", json!({"power": 2, "toughness": {"Count": [{"Type": "Artifact"}, {"ControlledBy": "You"}]}}))]
fn a_defining_line_that_sets_one_half_keeps_the_printed_other_half(
    #[case] pt: &str,
    #[case] set: &str,
    #[case] stats: Value,
) {
    let line = format!("S:Mode$ Continuous | CharacteristicDefining$ True | {set}");
    assert_eq!(
        abilities(&stats_card(pt, &line)),
        vec![json!({"Static": {"affected": "This", "mods": [{"SetPT": stats}], "defining": true}})]
    );
}

#[rstest]
#[case::plus_star("1+*/2", "CharacteristicDefining$ True | SetPower$ X")]
#[case::both_stars("*/*", "CharacteristicDefining$ True | SetPower$ X")]
#[case::wrong_half("*/3", "CharacteristicDefining$ True | SetToughness$ X")]
#[case::not_defining("6/5", "Affected$ Creature.OppCtrl | SetToughness$ 1")]
fn a_line_with_one_half_it_cannot_complete_stays_a_gap(#[case] pt: &str, #[case] set: &str) {
    let text = stats_card(pt, &format!("S:Mode$ Continuous | {set}"));
    assert!(
        gaps(&text).contains(&"key:Static.Continuous.half-SetPT".to_owned()),
        "{:?}",
        gaps(&text)
    );
}

#[rstest]
#[case::no_other_artifact(0, (1, 5))]
#[case::two_other_artifacts(2, (3, 5))]
fn a_half_set_defining_power_follows_the_count_in_a_game(
    #[case] others: usize,
    #[case] stats: (i32, i32),
) {
    let line = "S:Mode$ Continuous | CharacteristicDefining$ True | SetPower$ X";
    let mut card = CardDefinition::new("Guardian")
        .with_type(CardType::Artifact)
        .with_type(CardType::Creature)
        .with_power_toughness(0, 5);
    for value in abilities(&stats_card("*/5", line)) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    let rock = || card_with(&[CardType::Artifact], "Rock", "Name:Rock\nTypes:Artifact\n");
    let mut seat = vec![ready(Arc::new(card))];
    seat.extend((0..others).map(|_| ready(rock())));
    let driver = Driver::from_scenario(Scenario::new(2).at(1, 0, Step::Main1).battlefield(0, seat))
        .unwrap();
    let id = driver.id_of("Guardian").unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!((chars.power.unwrap(), chars.toughness.unwrap()), stats);
}
