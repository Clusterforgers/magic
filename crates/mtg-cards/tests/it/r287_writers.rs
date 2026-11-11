//! Goal: the writers r28-7 adds write the card's words and nothing near them: "your life total becomes 4" (Captive
//! Audience), "each creature you control with modular" (a macro keyword by its own name), the turn facts "one or more cards
//! were put into exile this turn", "a permanent was put into your hand from the battlefield this turn" and "three or more cards were put into your graveyard from anywhere other than the battlefield this
//! turn", and the whole abilities of the r28-7 table, each changed in one place reading back as nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::{same_written_text, written_variants};
use rstest::rstest;

fn writes(json: &str, text: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).unwrap();
    written_variants(&ability)
        .iter()
        .any(|written| same_written_text(written, text))
}

fn upkeep(effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"effects":[{effect}]}}}}"#
    )
}

const LIFE_FOUR: &str = "At the beginning of your upkeep, your life total becomes 4.";

#[rstest]
#[case::four(r#"{"SetLife":{"amount":4}}"#, LIFE_FOUR, true)]
#[case::five(r#"{"SetLife":{"amount":5}}"#, LIFE_FOUR, false)]
#[case::opponents(r#"{"SetLife":{"who":"Opponents","amount":4}}"#, LIFE_FOUR, false)]
#[case::gain(r#"{"GainLife":{"amount":4}}"#, LIFE_FOUR, false)]
fn r287_a_life_total_becomes_a_number(
    #[case] effect: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(&upkeep(effect), text), reads);
}

const OVERSEER: &str = "At the beginning of your upkeep, put a +1/+1 counter on each creature you control with modular.";

#[rstest]
#[case::modular(r#"{"HasKeywordNamed":"Modular"}"#, true)]
#[case::bushido(r#"{"HasKeywordNamed":"Bushido"}"#, false)]
#[case::without(r#"{"Not":{"HasKeywordNamed":"Modular"}}"#, false)]
fn r287_a_creature_with_a_macro_keyword(#[case] pred: &str, #[case] reads: bool) {
    let effect = format!(
        r#"{{"PutCounters":{{"on":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},{pred}]}},"counter":"+1/+1"}}}}"#
    );
    assert_eq!(writes(&upkeep(&effect), OVERSEER), reads);
}

fn end_step_if(who: &str, what: &str, op: &str, at: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"if":{{"Compare":{{"left":{{"Tallied":{{"who":"{who}","what":"{what}"}}}},"op":"{op}","right":{at}}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

const EXILED: &str = "At the beginning of your end step, if one or more cards were put into exile this turn, draw a card.";
const TO_HAND: &str = "At the beginning of your end step, if a permanent was put into your hand from the battlefield this turn, draw a card.";
const GRAVED: &str = "At the beginning of your end step, if three or more cards were put into your graveyard from anywhere other than the battlefield this turn, draw a card.";

#[rstest]
#[case::exiled(end_step_if("EachPlayer", "CardsExiled", "Ge", 1), EXILED, true)]
#[case::exiled_yours_only(end_step_if("You", "CardsExiled", "Ge", 1), EXILED, false)]
#[case::exiled_two(end_step_if("EachPlayer", "CardsExiled", "Ge", 2), EXILED, false)]
#[case::to_hand(end_step_if("You", "PermanentsToHand", "Ge", 1), TO_HAND, true)]
#[case::to_any_hand(end_step_if("EachPlayer", "PermanentsToHand", "Ge", 1), TO_HAND, false)]
#[case::graved(
    end_step_if("You", "CardsGravedNotFromBattlefield", "Ge", 3),
    GRAVED,
    true
)]
#[case::graved_from_anywhere(end_step_if("You", "CardsGraved", "Ge", 3), GRAVED, false)]
fn r287_facts_of_the_turn(#[case] json: String, #[case] text: &str, #[case] reads: bool) {
    assert_eq!(writes(&json, text), reads);
}

const KROVIKAN: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"if":{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}],"directly":true}},"op":"Ge","right":1}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#;
const KROVIKAN_TEXT: &str = "At the beginning of the end step, if this card is in your graveyard with a creature card directly above it, you may return this card to your hand.";
const NETHER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}]}},"op":"Ge","right":3}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}]}}"#;
const NETHER_TEXT: &str = "At the beginning of your upkeep, if this card is in your graveyard with three or more creature cards above it, you may put this card onto the battlefield.";

#[rstest]
#[case::krovikan(KROVIKAN, KROVIKAN_TEXT, None, true)]
#[case::krovikan_not_directly(KROVIKAN, KROVIKAN_TEXT, Some((r#""directly":true"#, r#""directly":false"#)), false)]
#[case::krovikan_any_card(KROVIKAN, KROVIKAN_TEXT, Some((r#"[{"Type":"Creature"}]"#, "[]")), false)]
#[case::nether(NETHER, NETHER_TEXT, None, true)]
#[case::nether_two(NETHER, NETHER_TEXT, Some((r#""right":3"#, r#""right":2"#)), false)]
#[case::nether_from_the_battlefield(NETHER, NETHER_TEXT, Some((r#","from":"Graveyard""#, "")), false)]
#[case::nether_must(NETHER, NETHER_TEXT, Some((r#"{"May":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#, r#"{"Move":{"what":"This","to":{"Battlefield":{}}}}"#)), false)]
fn r287_whole_abilities_read_back_only_as_written(
    #[case] json: &str,
    #[case] text: &str,
    #[case] change: Option<(&str, &str)>,
    #[case] reads: bool,
) {
    let json = match change {
        Some((from, to)) => {
            assert!(json.contains(from), "the near miss changes the data");
            json.replacen(from, to, 1)
        }
        None => json.to_owned(),
    };
    assert_eq!(writes(&json, text), reads);
}
