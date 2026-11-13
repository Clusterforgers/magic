//! A cost change that holds during the controller's turn, said as the oracle does: "During your turn, this spell costs
//! {2}{U}{U} less to cast." The ability is written with the condition that it is the controller's turn.

use crate::ability::Ability;
use crate::value::Condition;

/// Adds "During your turn, ..." for each "If it's your turn, ..." wording of a cost change under that condition alone.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::CostModifier {
        condition: Some(Condition::YourTurn),
        ..
    } = ability
    else {
        return;
    };
    let during: Vec<String> = texts
        .iter()
        .filter_map(|text| text.strip_prefix("If it's your turn, "))
        .map(|rest| format!("During your turn, {rest}"))
        .collect();
    for text in during {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a cost change during your turn gets the oracle's "During your turn" wording, and one under another
    //! condition (or none) does not.
    use super::add;
    use crate::ability::Ability;
    use rstest::rstest;

    #[rstest]
    #[case::your_turn(r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}{U}{U}","condition":"YourTurn"}}"#, true)]
    #[case::other_turns(r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}{U}{U}","condition":{"Not":"YourTurn"}}}"#, false)]
    #[case::always(
        r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}{U}{U}"}}"#,
        false
    )]
    fn r408_during_your_turn(#[case] json: &str, #[case] added: bool) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let mut texts = vec!["If it's your turn, ~ costs {2}{U}{U} less to cast.".to_owned()];
        add(&ability, &mut texts);
        assert_eq!(
            texts.contains(&"During your turn, ~ costs {2}{U}{U} less to cast.".to_owned()),
            added
        );
    }
}
