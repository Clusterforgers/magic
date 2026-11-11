//! Goal: the pieces of ep32 translate as the card prints them. A card's own discard caused by an opponent's spell or ability (`Discard$ True | ValidCause$ SpellAbility.OppCtrl`, from the hand) is replaced by its move to the battlefield, with the counters of the move; a discard by anyone, of another card, or a move elsewhere stays a gap. `K:Paradigm` is the keyword.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn smiter(line: &str, part: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elephant\nPT:4/4\nR:Event$ Moved | {line} | ReplaceWith$ SurpriseETB | Description$ If a spell or ability an opponent controls causes you to discard CARDNAME, put it onto the battlefield instead of putting it into your graveyard.\nSVar:SurpriseETB:DB$ ChangeZone | Defined$ ReplacedCard | Origin$ Hand | Destination$ Battlefield{part}\n"
    )
}

const LINE: &str = "ActiveZones$ Hand | ValidCard$ Card.Self | ValidCause$ SpellAbility.OppCtrl | Discard$ True | EffectOnly$ True";

#[rstest]
#[case::onto_the_battlefield("", json!({}))]
#[case::with_two_counters(
    " | WithCountersType$ P1P1 | WithCountersAmount$ 2",
    json!({"counters": [["+1/+1", 2]]})
)]
fn ep32_discard_by_an_opponent(#[case] part: &str, #[case] battlefield: serde_json::Value) {
    let script = smiter(LINE, part);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script),
        vec![json!({"Replacement": {
            "when": {"Discarded": {"what": ["This"], "by": "Opponents"}},
            "by": {"Instead": [{"Move": {"what": "This", "to": {"Battlefield": battlefield}}}]},
            "from": "Hand"
        }})]
    );
}

#[rstest]
#[case::any_cause(&LINE.replace(" | ValidCause$ SpellAbility.OppCtrl", ""), "")]
#[case::another_card(&LINE.replace("Card.Self", "Card.YouOwn"), "")]
#[case::from_the_battlefield(&LINE.replace("Hand", "Battlefield"), "")]
#[case::a_cost_too(&LINE.replace(" | EffectOnly$ True", ""), "")]
#[case::to_the_library(LINE, " | Hidden$ True")]
fn ep32_other_discards_stay_gaps(#[case] line: &str, #[case] part: &str) {
    let found = gaps(&smiter(line, part));
    assert!(!found.is_empty(), "{line}{part}");
}

#[test]
fn ep32_paradigm_is_the_keyword() {
    let script = "Name:T\nTypes:Sorcery Lesson\nA:SP$ Draw | NumCards$ 2 | SpellDescription$ Draw two cards.\nK:Paradigm\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    assert!(abilities(script).contains(&json!({"Keyword": "Paradigm"})));
}
