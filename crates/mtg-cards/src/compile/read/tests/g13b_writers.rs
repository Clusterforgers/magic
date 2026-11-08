//! Goal: the creatures that left the battlefield under your control this turn ("for each creature that left the
//! battlefield under your control this turn", "if a creature left the battlefield under your control this turn") read
//! into their own tally and write back only from it; a rule that counts the creatures that died, or another player's,
//! reads `Differs`.
use super::*;

/// The clause the ability writes back, among its wordings.
fn writes(json: &str, clause: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, clause))
}

const FOR_EACH: &str = "Put a +1/+1 counter on ~ for each creature that left the battlefield under your control this turn.";
const IF_ONE: &str = "At the beginning of your end step, if a creature left the battlefield under your control this turn, draw a card.";

fn counters(who: &str, what: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"PutCounters":{{"on":"This","counter":"+1/+1","count":{{"Tallied":{{"who":"{who}","what":"{what}"}}}}}}}}]}}}}"#
    )
}

fn end_step(who: &str, what: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"if":{{"Compare":{{"left":{{"Tallied":{{"who":"{who}","what":"{what}"}}}},"op":"Ge","right":1}}}},"effects":[{{"Draw":{{"who":"You","count":1}}}}]}}}}"#
    )
}

#[test]
fn the_creatures_that_left_under_you_are_read_as_their_tally() {
    let read = golden(FOR_EACH, ClauseKind::Spell, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains("CreaturesLeftUnder"), "{json}");
}

#[rstest]
#[case::for_each(counters("You", "CreaturesLeftUnder"), FOR_EACH, true)]
#[case::if_one(end_step("You", "CreaturesLeftUnder"), IF_ONE, true)]
// The creatures that died are fewer: one bounced or exiled left the battlefield too.
#[case::died_instead(counters("You", "CreaturesDiedUnder"), FOR_EACH, false)]
#[case::any_permanent(end_step("You", "PermanentsLeftUnder"), IF_ONE, false)]
#[case::another_player(counters("Opponents", "CreaturesLeftUnder"), FOR_EACH, false)]
fn the_tally_writes_back_only_as_itself(
    #[case] json: String,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(writes(&json, clause), same, "{json}");
}
