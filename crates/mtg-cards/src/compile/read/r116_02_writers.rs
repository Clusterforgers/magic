//! Wordings of the Star Trek Commander set for triggers the plain writers word another way: a trigger condition of
//! the turn said after the event ("When this creature dies during your turn", "Whenever an opponent draws a card during
//! your turn"; an intervening "if it's your turn", CR 603.4) and a life total ("if your life total is 0 or less").

use crate::ability::Ability;
use crate::value::Condition;

/// Adds the wordings of a trigger whose condition the set says after the event.
pub(super) fn r116_02_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    life_total(texts);
    let Ability::Triggered {
        intervening_if: Some(condition),
        ..
    } = ability
    else {
        return;
    };
    if *condition != Condition::YourTurn {
        return;
    }
    for text in during_your_turn(ability, texts, written) {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "if you have N or less life" also as "if your life total is N or less" (CR 119.1: a player's life total).
fn life_total(texts: &mut Vec<String>) {
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (before, rest) = text.split_once("if you have ")?;
            let (number, after) = rest.split_once(" or less life")?;
            number
                .chars()
                .all(|c| c.is_ascii_digit())
                .then(|| format!("{before}if your life total is {number} or less{after}"))
        })
        .collect();
    for text in said {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "<event>, if it's your turn, <effect>" as "<event> during your turn, <effect>".
fn during_your_turn(
    ability: &Ability,
    texts: &[String],
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    const IF: &str = ", if it's your turn, ";
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let mut out = Vec::new();
    for text in texts {
        let Some((head, rest)) = text.split_once(IF) else {
            continue;
        };
        out.push(format!("{head} during your turn, {rest}"));
    }
    // The plain wordings give the effect in each of its forms ("investigate" as well as the Clue it makes).
    for text in written(&plain) {
        let Some((head, rest)) = text.split_once(", ") else {
            continue;
        };
        if head.starts_with("When") && !head.contains(" if ") {
            out.push(format!("{head} during your turn, {rest}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    //! Goal: a trigger under "if it's your turn" is also worded "during your turn" after its event, an attack trigger
    //! under a total power checked as it triggers is worded "while creatures you control have total power" and one under
    //! an intervening "if" is not, and the plain ability or another condition gets neither wording.
    #![allow(clippy::unwrap_used)]
    use crate::ability::Ability;
    use crate::compile::read::render::written_variants;
    use rstest::rstest;

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    const DIES: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"if":"YourTurn","effects":[{"CreateToken":{"token":{"Named":"clue"}}},{"Performed":{"action":"Investigated"}}]}}"#;
    const DRAWS: &str = r#"{"Triggered":{"when":{"Drew":{"who":"Opponents"}},"if":"YourTurn","effects":[{"Draw":{}}]}}"#;
    const ATTACK: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"if":{"AtTrigger":{"Compare":{"left":{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}},"op":"Ge","right":8}}},"effects":[{"Draw":{}}]}}"#;

    #[rstest]
    #[case::dies(DIES, "When ~ dies during your turn, investigate.", true)]
    #[case::draws(
        DRAWS,
        "Whenever an opponent draws a card during your turn, draw a card.",
        true
    )]
    #[case::not_on_another_turn(
        &DIES.replace(r#""if":"YourTurn","#, r#""if":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":1}},"#),
        "When ~ dies during your turn, investigate.",
        false
    )]
    #[case::attack(
        ATTACK,
        "Whenever you attack while creatures you control have total power 8 or greater, draw a card.",
        true
    )]
    #[case::attack_nine(
        ATTACK,
        "Whenever you attack while creatures you control have total power 9 or greater, draw a card.",
        false
    )]
    #[case::attack_without_the_condition(
        &ATTACK.replace(r#""if":{"AtTrigger":{"Compare":{"left":{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}},"op":"Ge","right":8}}},"#, ""),
        "Whenever you attack while creatures you control have total power 8 or greater, draw a card.",
        false
    )]
    #[case::attack_under_an_intervening_if(
        &ATTACK.replace(r#"{"AtTrigger":{"Compare""#, r#"{"Compare""#).replace("}}},\"effects", "}},\"effects"),
        "Whenever you attack while creatures you control have total power 8 or greater, draw a card.",
        false
    )]
    #[case::life_total(
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":0}},"then":[{"Draw":{}}]}}]}}"#,
        "At the beginning of your end step, if your life total is 0 or less, draw a card.",
        true
    )]
    #[case::life_total_greater(
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":0}},"then":[{"Draw":{}}]}}]}}"#,
        "At the beginning of your end step, if your life total is 0 or less, draw a card.",
        false
    )]
    fn r116_02_the_set_wordings_of_trigger_conditions(
        #[case] json: &str,
        #[case] text: &str,
        #[case] written: bool,
    ) {
        let texts = written_variants(&ability(json));
        assert_eq!(texts.iter().any(|t| t == text), written, "{texts:#?}");
    }
}
