//! The land types a static ability gives a group as a whole: "Nonbasic lands are Mountains." (`SetLandTypes`, the
//! group has no other land type) and "Lands you control are every basic land type in addition to their other types."
//! (one `AddSubtype` for each basic land type). Read from the wording and written back the same way.

use super::selectors::plural_subtype;
use super::subtypes::plural_subtype as plural_name;
use super::tokens::Reader;
use crate::modify::Modification;
use mtg_core::card::Subtype;

/// The basic land types, in the order the rules list them.
const BASIC: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// "are Mountains" (the group's land types become that one) and "are every basic land type in addition to their other
/// types", the part of a sentence after its subject.
pub(super) fn mods(r: &mut Reader<'_>, plural: bool) -> Option<Vec<Modification>> {
    let (is, its) = verbs(plural);
    r.word(is).then_some(())?;
    if r.words(&format!(
        "every basic land type in addition to {its} other types"
    )) {
        return Some(BASIC.map(added).to_vec());
    }
    let kind = plural_subtype(r).filter(|kind| plural && BASIC.contains(&kind.as_str()))?;
    Some(vec![Modification::SetLandTypes(vec![Subtype(kind)])])
}

/// How many modifications from the first one a part of a predicate says, and its wording: "are Mountains" for the land
/// types set to one basic type, "are every basic land type in addition to their other types" for all five added.
pub(super) fn part(mods: &[Modification], plural: bool) -> Option<(usize, String)> {
    let (is, its) = verbs(plural);
    if let [Modification::SetLandTypes(types), ..] = mods {
        return match types.as_slice() {
            [Subtype(kind)] if plural && BASIC.contains(&kind.as_str()) => {
                Some((1, format!("are {}", plural_name(kind))))
            }
            _ => None,
        };
    }
    let every = mods.len() >= BASIC.len()
        && mods
            .iter()
            .zip(BASIC)
            .all(|(given, name)| *given == added(name));
    every.then(|| {
        (
            BASIC.len(),
            format!("{is} every basic land type in addition to {its} other types"),
        )
    })
}

fn added(name: &str) -> Modification {
    Modification::AddSubtype(Subtype(name.to_owned()))
}

fn verbs(plural: bool) -> (&'static str, &'static str) {
    if plural {
        ("are", "their")
    } else {
        ("is", "its")
    }
}
