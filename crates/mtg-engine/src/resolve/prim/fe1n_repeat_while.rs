//! `Effect::RepeatWhile`: the body is done again and again while a condition holds, at most `most` times.

use mtg_cards::effect::{Effect, MOST_PASSES};
use mtg_core::ids::PlayerId;

use super::repeat_each::{clear_pass, repeat_at};
use super::wrong_effect;
use crate::choose::Target;
use crate::resolve::tree::is_next_pass;
use crate::resolve::{CLEAR, Cx, Next, ResolveError, Run};

/// The name under which the passes made so far are counted, one name for each repeat; no card can write it, since a name
/// has no `~`.
fn passes_name(cx: &Cx<'_>) -> String {
    let at: Vec<String> = repeat_at(cx).iter().map(ToString::to_string).collect();
    format!("~passes:{}", at.join("."))
}

/// The passes made so far: the count is the length of the list bound under the counter's name.
fn made(cx: &Cx<'_>, name: &str) -> usize {
    if is_next_pass(cx.resolving.position()) {
        cx.resolving.bindings().get(name).map_or(0, Vec::len)
    } else {
        0
    }
}

/// Begins a pass, at the effect itself and at the start of each next pass: the pass begins when fewer than `most` (and
/// never more than [`MOST_PASSES`]) have been made and the condition holds in the state the passes before left; the
/// counter is bound anew at the first entry, so a repeat inside another starts from nought each time. Otherwise the
/// repeat is done and the resolution goes on.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RepeatWhile {
        condition,
        effects,
        most,
        times,
    } = effect
    else {
        return Err(wrong_effect("RepeatWhile"));
    };
    let name = passes_name(cx);
    let count = made(cx, &name);
    let mut bound_on_passes = usize::try_from((*most).min(MOST_PASSES)).unwrap_or(0);
    if let Some(times) = times {
        let wanted = usize::try_from(cx.scope.amount(times)?.max(0)).unwrap_or(usize::MAX);
        bound_on_passes = bound_on_passes.min(wanted);
    }
    let mut bound = Vec::new();
    if count >= bound_on_passes || !cx.scope.holds(condition)? {
        if cx.resolving.bindings().contains_key(&name) {
            bound.push((format!("{CLEAR}{name}"), Vec::new()));
        }
        return Ok(Run::Done {
            events: Vec::new(),
            bound,
            snapshots: Vec::new(),
            next: Next::Sibling,
        });
    }
    bound.extend(clear_pass(cx, effects, std::slice::from_ref(&name)));
    // Only the length counts: one stand-in entry for each pass made, this one included.
    bound.push((name, vec![Target::Player(PlayerId(0)); count + 1]));
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Enter(0),
    })
}
