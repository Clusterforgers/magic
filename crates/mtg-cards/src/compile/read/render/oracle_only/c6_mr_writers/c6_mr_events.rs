//! Events of the sixth sample of M to R: "Whenever a creature ... attacks you, its controller ..." and "Whenever you
//! cast your first spell during each opponent's turn".

use super::{c6_mr_swapped, c6_mr_twin_all};

/// The attack of you, and the attack of anyone the renderers write.
const C6_MR_ATTACKS_YOU: [(&str, &str); 2] = [
    (r#"{"AttacksPlayer":{"what":"#, r#"{"Attacks":{"what":"#),
    (r#"],"at":"You"}}"#, "]}}"),
];

/// Your first spell in a turn that is not yours, and your second spell in any turn.
const C6_MR_FIRST_SPELL: [(&str, &str); 1] = [(
    r#"{"Nth":{"event":{"SpellCast":{"by":"You"}},"nth":1}},"if":{"Not":"YourTurn"}"#,
    r#"{"Nth":{"event":{"SpellCast":{"by":"You"}},"nth":2}}"#,
)];

/// Adds the wordings of the events.
pub(super) fn add(json: &str, texts: &mut Vec<String>) {
    let attacked = c6_mr_swapped(
        &c6_mr_twin_all(json, &C6_MR_ATTACKS_YOU),
        " attacks, ",
        " attacks you, ",
    );
    for owner in ["that permanent's controller", "that creature's controller"] {
        texts.extend(c6_mr_swapped(&attacked, owner, "its controller"));
    }
    texts.extend(attacked);
    texts.extend(c6_mr_swapped(
        &c6_mr_twin_all(json, &C6_MR_FIRST_SPELL),
        "your second spell each turn",
        "your first spell during each opponent's turn",
    ));
}
