//! Goal: the round 11 writers give the oracle wording of the players who control more, fewer or at least N more permanents of a type than the player who asks, and a rule of another meaning (other comparison, other type, other margin, the asker swapped for "you") never reads back as the clause.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// The permanents of a type a player controls, counted.
fn controlled(kind: &str, who: &str) -> String {
    format!(r#"{{"Count":[{{"Type":"{kind}"}},{{"ControlledBy":"{who}"}}]}}"#)
}

/// The players `who` names who control `kind` the way `op` and `more` compare with the asker, as a count.
fn than_you(who: &str, kind: &str, op: &str, more: u32, asker: &str) -> String {
    let theirs = controlled(kind, asker);
    let right = if more == 0 {
        theirs
    } else {
        format!(r#"{{"Sum":[{theirs},{more}]}}"#)
    };
    format!(
        r#"{{"Players":{{"PlayersWhere":{{"who":"{who}","test":{{"Compare":{{"left":{},"op":"{op}","right":{right}}}}}}}}}}}"#,
        controlled(kind, "You")
    )
}

fn at_end_step_tokens(count: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"effects":[{{"CreateToken":{{"count":{count},"token":{{"Custom":{{"abilities":[{{"Keyword":"Flying"}}],"colors":["White"],"name":"Spirit","power":1,"subtypes":["Spirit"],"toughness":1,"types":["Creature"]}}}}}}}}]}}}}"#
    )
}

fn entering_draw(count: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}},"effects":[{{"Draw":{{"count":{count}}}}}]}}}}"#
    )
}

const MORE_LANDS: &str = "At the beginning of your end step, create X 1/1 white Spirit creature tokens with flying, where X is the number of opponents who control more lands than you.";
const FEWER_CREATURES: &str = "When this creature enters, draw a card for each opponent who controls fewer creatures than you.";
const TWO_MORE_LANDS: &str = "When this creature enters, draw X cards, where X is the number of players who control at least two more lands than you.";

#[rstest]
#[case::more_lands(&at_end_step_tokens(&than_you("Opponents", "Land", "Gt", 0, "Asker")), MORE_LANDS)]
#[case::fewer_creatures(&entering_draw(&than_you("Opponents", "Creature", "Lt", 0, "Asker")), FEWER_CREATURES)]
#[case::two_more_lands(&entering_draw(&than_you("EachPlayer", "Land", "Ge", 2, "Asker")), TWO_MORE_LANDS)]
fn g11a_than_you_writes_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::fewer_for_more(&at_end_step_tokens(&than_you("Opponents", "Land", "Lt", 0, "Asker")), MORE_LANDS)]
#[case::at_least_as_many(&at_end_step_tokens(&than_you("Opponents", "Land", "Ge", 0, "Asker")), MORE_LANDS)]
#[case::creatures_for_lands(&at_end_step_tokens(&than_you("Opponents", "Creature", "Gt", 0, "Asker")), MORE_LANDS)]
#[case::players_for_opponents(&at_end_step_tokens(&than_you("EachPlayer", "Land", "Gt", 0, "Asker")), MORE_LANDS)]
#[case::themselves_for_you(&at_end_step_tokens(&than_you("Opponents", "Land", "Gt", 0, "You")), MORE_LANDS)]
#[case::more_for_fewer(&entering_draw(&than_you("Opponents", "Creature", "Gt", 0, "Asker")), FEWER_CREATURES)]
#[case::three_more(&entering_draw(&than_you("EachPlayer", "Land", "Ge", 3, "Asker")), TWO_MORE_LANDS)]
#[case::strictly_two_more(&entering_draw(&than_you("EachPlayer", "Land", "Gt", 2, "Asker")), TWO_MORE_LANDS)]
fn g11a_than_you_of_another_meaning_differs(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
