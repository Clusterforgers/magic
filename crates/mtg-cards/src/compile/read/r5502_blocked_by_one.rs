//! "~ must be blocked by a Wall if able." and "Equipped creature must be blocked by an Eldrazi if able." (Slayer's
//! Cleaver, beside its bonus): a requirement that some creature of a kind blocks the creature
//! (`CombatQuirk::MustBeBlockedByOne`), read and written back.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::modify::{CombatQuirk, GameRule};
use crate::refs::Ref;

/// "~ / equipped creature / enchanted creature must be blocked by a <kind> if able".
pub(super) fn r5502_read(r: &mut Reader<'_>) -> Option<GameRule> {
    let who = if r.source() {
        Ref::This
    } else if r.words("equipped creature") || r.words("enchanted creature") {
        Ref::Attached
    } else {
        return None;
    };
    r.need("must be blocked by")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let blockers = phrase(r, Form::Subject)?;
    r.need("if able")?;
    Some(GameRule::Combat {
        who,
        quirk: CombatQuirk::MustBeBlockedByOne(blockers),
    })
}

/// What `r5502_read` reads, written back without a period; nothing for another rule.
pub(super) fn r5502_texts(rule: &GameRule) -> Vec<String> {
    let GameRule::Combat {
        who,
        quirk: CombatQuirk::MustBeBlockedByOne(blockers),
    } = rule
    else {
        return Vec::new();
    };
    let subjects: &[&str] = match who {
        Ref::This => &["~"],
        Ref::Attached => &["Equipped creature", "Enchanted creature"],
        _ => return Vec::new(),
    };
    let Some(kind) = phrase_text(blockers, Form::Subject) else {
        return Vec::new();
    };
    subjects
        .iter()
        .map(|subject| {
            format!(
                "{subject} must be blocked by {} {kind} if able",
                article(&kind)
            )
        })
        .collect()
}
