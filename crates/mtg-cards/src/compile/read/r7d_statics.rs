//! r7d: rules no writer wrote before, written back from the rule: a combat rule of the card itself ("You may choose not
//! to untap ~ during your untap step", "All creatures able to block ~ do so"), a static of the enchanted permanent while
//! it is a creature, and the Oracle phrases of rules the writers word otherwise.

use mtg_core::card::CardType;

use crate::ability::Ability;
use crate::modify::{Blocking, CombatQuirk, GameRule};
use crate::refs::{Pred, Ref, Selector};
use crate::value::Condition;

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
        CombatQuirk::BlockAsThoughShadow => {
            "~ can block creatures with shadow as though it had shadow"
        }
        CombatQuirk::BlockIgnoringShadow => {
            "~ can block creatures with shadow as though they didn't have shadow"
        }
        CombatQuirk::AssignsAsUnblocked => {
            "You may have ~ assign its combat damage as though it weren't blocked"
        }
        _ => return Vec::new(),
    };
    vec![text.to_owned()]
}

/// "As long as enchanted permanent is a creature, it gets -1/-1 and can't block": a static of the attached permanent that
/// holds only while it is a creature, written as the sentences of the static without the condition after it (r6 audit
/// F1). Empty for any other ability.
pub(super) fn creature_attached(
    ability: &Ability,
    written: impl Fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let Ability::Static {
        affected: Ref::Attached,
        mods,
        condition: Some(condition),
        defining: false,
        from,
    } = ability
    else {
        return Vec::new();
    };
    let creature = Condition::Matches {
        subject: Ref::Attached,
        selector: Selector(vec![Pred::Type(CardType::Creature)]),
    };
    if *condition != creature {
        return Vec::new();
    }
    let unconditional = Ability::Static {
        affected: Ref::Attached,
        mods: mods.clone(),
        condition: None,
        defining: false,
        from: *from,
    };
    written(&unconditional)
        .iter()
        .filter_map(|text| {
            let text = text.strip_suffix('.').unwrap_or(text);
            // The writers name the attached permanent with one noun; the condition names it "enchanted permanent".
            [
                "Equipped creature ",
                "Enchanted creature ",
                "Enchanted permanent ",
            ]
            .iter()
            .find_map(|noun| text.strip_prefix(noun))
        })
        .map(|rest| format!("As long as enchanted permanent is a creature, it {rest}."))
        .collect()
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
