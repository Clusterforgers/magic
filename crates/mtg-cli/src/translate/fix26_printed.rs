//! Script words the printed text does not say: `Other` in what a permanent may enter as a copy of (the engine never
//! offers the entering permanent itself), a decider on a trigger whose printed text has no "may", and a cost count
//! `Relative$` to the caster that the engine would read for the modifier's controller.

use serde_json::Value;

use super::ctx::{Tr, unmapped};
use crate::forge::script::Params;

/// The selector of a copy with `Other` dropped when the printed words after "copy of" say neither "other" nor "another".
pub(super) fn copy_of(mut of: Vec<Value>, printed: Option<&str>) -> Vec<Value> {
    let says_other = printed.is_none_or(|text| {
        let Some((_, rest)) = text.split_once("copy of ") else {
            return true;
        };
        let rest = rest.split(", except").next().unwrap_or(rest);
        rest.split_whitespace()
            .any(|word| matches!(word, "other" | "another"))
    });
    if !says_other {
        of.retain(|pred| pred != "Other");
    }
    of
}

/// Whether a trigger's `OptionalDecider$ You` is a choice the printed text gives: it says "may", or nothing is printed.
pub(super) fn asks(p: &Params) -> bool {
    p.get("TriggerDescription").is_none_or(|text| {
        text.split(|c: char| !c.is_alphabetic())
            .any(|word| word.eq_ignore_ascii_case("may"))
    })
}

/// A count of what "you" control behind `Relative$ True` on a modifier of other players' spells counts for the caster
/// ("for each artifact its controller controls"), which the engine has no reference for: a gap.
pub(super) fn relative_count(p: &Params, count: Option<&str>) -> Tr<()> {
    let others = p.get("Activator").is_some_and(|who| who != "You");
    if p.get("Relative") == Some("True")
        && others
        && count.is_some_and(|text| text.contains("YouCtrl"))
    {
        return unmapped("key:Static.CostModifier.Relative-caster-count");
    }
    Ok(())
}
