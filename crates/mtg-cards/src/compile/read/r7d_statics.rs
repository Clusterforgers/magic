//! r7d: rules no writer wrote before, written back from the rule: a combat rule of the card itself ("You may choose not
//! to untap ~ during your untap step", "All creatures able to block ~ do so"), and the Oracle phrases of rules the
//! writers word otherwise.

use crate::modify::{Blocking, CombatQuirk, GameRule};
use crate::refs::Ref;

/// The sentences of a combat rule of the card itself, without their period.
pub(super) fn combat_texts(rule: &GameRule) -> Vec<String> {
    let GameRule::Combat {
        who: Ref::This,
        quirk,
    } = rule
    else {
        return Vec::new();
    };
    let text = match quirk {
        CombatQuirk::MayStayTapped => "You may choose not to untap ~ during your untap step",
        CombatQuirk::MustBeBlocked(Blocking::Every) => "All creatures able to block ~ do so",
        CombatQuirk::MustBeBlocked(Blocking::AtLeastOne) => "~ must be blocked if able",
        CombatQuirk::AssignsNoCombatDamage => "~ assigns no combat damage",
        _ => return Vec::new(),
    };
    vec![text.to_owned()]
}

/// Each row is a phrase the writers give and the phrase the Oracle text uses for the same rule, anywhere in a wording.
const PHRASES: &[(&str, &str)] = &[
    // A requirement for the turn holds in each combat of the turn (Anzrag).
    (
        "~ must be blocked this turn if able",
        "~ must be blocked each combat this turn if able",
    ),
    // The tapped target is "that creature" or "that permanent" as well as "it".
    (
        ". It doesn't untap during its controller's untap step for as long as ~",
        ". That creature doesn't untap during its controller's untap step for as long as ~",
    ),
    (
        ". It doesn't untap during its controller's untap step for as long as ~",
        ". That permanent doesn't untap during its controller's untap step for as long as ~",
    ),
    // A selector is a conjunction: "nontoken white permanent" is "white nontoken permanent".
    (" white nontoken ", " nontoken white "),
    (" blue nontoken ", " nontoken blue "),
    (" black nontoken ", " nontoken black "),
    (" red nontoken ", " nontoken red "),
    (" green nontoken ", " nontoken green "),
];

/// Adds the Oracle wordings of the rows, and "has" for "gains" in a change that lasts while the source stays tapped.
pub(super) fn wordings(texts: &mut Vec<String>) {
    let mut more = Vec::new();
    for text in texts.iter() {
        for (written, oracle) in PHRASES {
            if text.contains(written) {
                more.push(text.replace(written, oracle));
            }
        }
        if text.contains(super::render::WHILE_TAPPED) && text.contains(" gains ") {
            more.push(text.replace(" gains ", " has "));
        }
    }
    for text in more {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
