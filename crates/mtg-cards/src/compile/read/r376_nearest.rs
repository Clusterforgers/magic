//! Attacks limited to the nearest opponent on a side (Pramikon, Sky Rampart): "Each player may attack only the nearest
//! opponent in the chosen direction and planeswalkers controlled by that opponent.", and the side chosen as it enters
//! ("As ~ enters, choose left or right."), and "Room abilities of dungeons you own trigger an additional time."

use super::tokens::Reader;
use crate::ability::Ability;
use crate::trigger::ReplaceBy;

/// The words of the static ability, without the final period.
const NEAREST: &str = "each player may attack only the nearest opponent in the chosen direction and planeswalkers controlled by that opponent";

/// The static ability the words read as.
fn r376_nearest_rule() -> Option<Ability> {
    serde_json::from_str(r#"{"Rule":{"rule":{"AttackOnlyNearest":"Entered"}}}"#).ok()
}

/// Reads "each player may attack only the nearest opponent in the chosen direction and planeswalkers controlled by that
/// opponent", leaving the period.
pub(super) fn r376_nearest(r: &mut Reader<'_>) -> Option<Ability> {
    r.r376_text(NEAREST).then_some(())?;
    r376_nearest_rule()
}

/// The words of Hama Pashar's static ability, without the final period.
const ROOMS_AGAIN: &str = "room abilities of dungeons you own trigger an additional time";

/// The rule "room abilities of dungeons you own trigger an additional time" makes.
fn r376_rooms_rule() -> Option<Ability> {
    serde_json::from_str(
        r#"{"Rule":{"rule":{"TriggerAgain":{"source":[{"In":"Command"},"Dungeon",{"OwnedBy":"You"}]}}}}"#,
    )
    .ok()
}

/// Reads "room abilities of dungeons you own trigger an additional time", leaving the period.
pub(super) fn r376_rooms_again(r: &mut Reader<'_>) -> Option<Ability> {
    r.r376_text(ROOMS_AGAIN).then_some(())?;
    r376_rooms_rule()
}

/// The choice of a side as the permanent enters.
fn r376_side_choice() -> Ability {
    super::statics::replacement(
        ReplaceBy::ChooseWord(vec!["left".to_owned(), "right".to_owned()]),
        None,
    )
}

/// Reads "as ~ enters, choose left or right", leaving the period.
pub(super) fn r376_side_entering(r: &mut Reader<'_>) -> Option<Ability> {
    r.r376_text("as ~ enters, choose left or right")
        .then_some(())?;
    Some(r376_side_choice())
}

/// The wordings of the abilities [`r376_nearest`] and [`r376_side_entering`] read.
pub(super) fn r376_nearest_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let text = if Some(ability) == r376_nearest_rule().as_ref() {
        let mut text = NEAREST.to_owned();
        text.replace_range(..1, "E");
        text
    } else if Some(ability) == r376_rooms_rule().as_ref() {
        let mut text = ROOMS_AGAIN.to_owned();
        text.replace_range(..1, "R");
        text
    } else if *ability == r376_side_choice() {
        "As ~ enters, choose left or right".to_owned()
    } else {
        return;
    };
    if !texts.contains(&text) {
        texts.push(text);
    }
}
