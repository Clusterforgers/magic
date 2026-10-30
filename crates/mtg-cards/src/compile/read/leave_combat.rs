//! "Remove enchanted creature from combat.", "Remove target attacking creature from combat.", "Remove ~ from
//! combat.": `Effect::RemoveFromCombat` of the creatures the sentence names.

use super::effects::object;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;

/// The sentence "Remove <creatures> from combat."; "its controller" in the next sentence is the controller of the one
/// creature it names.
pub(super) fn remove_from_combat(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("remove")?;
    let what = if r.source() { Ref::This } else { object(r)? };
    r.need("from combat")?;
    let single = match &what {
        Ref::Target(index) => r
            .targets
            .get(usize::from(*index))
            .is_some_and(|spec| spec.bounds.is_one()),
        other => matches!(other, Ref::Attached | Ref::This),
    };
    if single {
        r.it = Some(what.clone());
    }
    Some(vec![Effect::RemoveFromCombat { what }])
}

/// "remove X from combat. Draw ..." is also worded "remove X from combat. Then draw ...": the sentence the creature's
/// removal makes room for.
pub(super) fn wordings(texts: &[String]) -> Vec<String> {
    const JOINT: &str = " from combat. Draw ";
    let mut more = Vec::new();
    for text in texts {
        for (at, _) in text.match_indices(JOINT) {
            let (head, tail) = (&text[..at], &text[at + JOINT.len()..]);
            more.push(format!("{head} from combat. Then draw {tail}"));
        }
    }
    more
}
