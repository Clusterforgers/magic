//! Phrases the renderers write one way and the cards another, each with the shape of the rules it applies to: "creatures
//! can't block this turn" for a group, "legendary permanent that's an artifact, creature, or enchantment", the
//! controller of a permanent, a draw by two players, "it deals" for a divided damage, a play of the exiled card and the
//! "source" that deals the damage a sacrifice answers.

use super::text::json;
use crate::ability::Ability;

/// What the rules must hold for a swap to apply.
type Guard = fn(&str) -> bool;

/// The swaps: where they apply, the renderers' words and the card's.
const SWAPS: [(Guard, &str, &str); 8] = [
    (
        loses_life_of_target_controller,
        "That creature's controller loses ",
        "That player loses ",
    ),
    (
        blocks_for_all,
        ", all creatures can't block",
        ", creatures can't block",
    ),
    (
        always,
        "legendary artifact, creature, or enchantment",
        "legendary permanent that's an artifact, creature, or enchantment",
    ),
    (
        destroyed_target_pays,
        "That creature's controller creates",
        "That permanent's controller creates",
    ),
    (
        draws_for_both,
        "you draw that many cards. Then that player draws that many cards.",
        "you and that player each draw that many cards.",
    ),
    (divided_damage, "~ deals ", "it deals "),
    (
        plays_exiled,
        ". Then until the end of your next turn, you may play that card.",
        ". You may play that card until the end of your next turn.",
    ),
    (
        sacrifice_answers_damage,
        "Whenever a permanent an opponent controls deals damage to you, sacrifice ~. When you do,",
        "When a source an opponent controls deals damage to you, sacrifice ~. If you do,",
    ),
];

fn always(_: &str) -> bool {
    true
}

fn blocks_for_all(rules: &str) -> bool {
    rules
        .contains(r#""Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["CantBlock"]"#)
}

fn destroyed_target_pays(rules: &str) -> bool {
    rules.contains(r#""Destroy":{"what":{"Target":0}"#)
        && rules.contains(r#""who":{"ControllerOf":{"Target":0}}"#)
        && !rules.contains(r#"{"Type":"Creature"}"#)
}

fn loses_life_of_target_controller(rules: &str) -> bool {
    rules.contains(r#""LoseLife":{"who":{"ControllerOf":{"Target":0}}"#)
}

fn draws_for_both(rules: &str) -> bool {
    rules.contains(r#""Draw":{"count":"Triggering"}"#)
        && rules.contains(r#""Draw":{"who":"Triggering","count":"Triggering"}"#)
}

fn divided_damage(rules: &str) -> bool {
    rules.contains(r#""divide":"#) && rules.contains(r#""amount":"Divided""#)
}

fn plays_exiled(rules: &str) -> bool {
    rules.contains(r#""ExileTop""#) && rules.contains(r#""duration":"UntilEndOfYourNextTurn""#)
}

fn sacrifice_answers_damage(rules: &str) -> bool {
    rules.contains(r#""DealsDamage":{"source":[{"ControlledBy":"Opponents"}],"to":"You""#)
        && rules.contains(r#""Sacrifice":{"filter":["This"]}"#)
        && rules.contains(r#""condition":"Performed""#)
}

/// Adds the wordings with each swap that applies, one swap at a time.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let rules = json(ability);
    let mut found = Vec::new();
    for (applies, from, to) in SWAPS {
        if applies(&rules) {
            found.extend(
                texts
                    .iter()
                    .filter(|text| text.contains(from))
                    .map(|text| text.replacen(from, to, 1)),
            );
        }
    }
    texts.extend(found);
}
