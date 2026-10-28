//! More causes of a rule that triggers abilities again: "If a player drawing a card causes a triggered ability of a
//! permanent you control to trigger, that ability triggers an additional time." and "a creature dying".

use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;

/// The causes as JSON, each with its words.
const C6_GL_CAUSES: [(&str, &str); 2] = [
    (r#"{"Drew":{}}"#, "a player drawing a card"),
    (
        r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}}"#,
        "a creature dying",
    ),
];

/// Adds the wording of a rule that makes the triggered abilities of the controller's permanents trigger again.
pub(super) fn c6_gl_add_causes(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::TriggerAgain { cause: event },
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    for (pattern, words) in C6_GL_CAUSES {
        if serde_json::from_str::<TriggerPattern>(pattern).is_ok_and(|cause| cause == *event) {
            texts.push(format!(
                "If {words} causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
            ));
        }
    }
}
