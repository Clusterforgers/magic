//! A draw replaced while the library is empty: "If you would draw a card while your library has no cards in it, you win
//! the game instead." and "..., instead put five +1/+1 counters on ~."

use super::super::super::amounts::count_word;
use crate::ability::Ability;

/// The replacement of a draw while the library is empty, with the steps `instead` (compact JSON) in place of the draw.
fn empty_draw(instead: &str) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Draw":{{}}}},"by":{{"Instead":{instead}}},"condition":{{"Compare":{{"left":{{"Count":[{{"In":"Library"}},{{"OwnedBy":"You"}}]}},"op":"Eq","right":0}}}}}}}}"#
    )
}

/// The wording of a draw replaced while the library is empty: a win, or +1/+1 counters on the source.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement { .. } = ability else {
        return;
    };
    let Ok(json) = serde_json::to_string(ability) else {
        return;
    };
    let lead = "If you would draw a card while your library has no cards in it,";
    if json == empty_draw(r#"[{"WinGame":{}}]"#) {
        texts.push(format!("{lead} you win the game instead."));
    }
    for count in 2..=10u32 {
        let counters =
            format!(r#"[{{"PutCounters":{{"on":"This","counter":"+1/+1","count":{count}}}}}]"#);
        if json == empty_draw(&counters)
            && let Some(word) = count_word(count)
        {
            texts.push(format!("{lead} instead put {word} +1/+1 counters on ~."));
        }
    }
}
