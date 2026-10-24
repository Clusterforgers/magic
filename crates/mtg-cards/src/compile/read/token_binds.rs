//! A result bound to the tokens a sentence created: "Create a 0/0 green and blue Fractal creature token. Put X +1/+1
//! counters on it, where X is ...", "Create a 1/1 white Soldier creature token, then attach this Equipment to it.",
//! "Create two 1/1 red Goblin creature tokens. Those tokens gain haste until end of turn." The tokens are named
//! `tokens`, and the sentence after the creation reads them as `Bound`.

use super::tokens::Reader;
use super::{changes, counters, where_x};
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};
use crate::refs::Ref;

/// The name the created tokens are bound to.
pub const TOKENS: &str = "tokens";

/// The tokens a sentence created, as a later sentence refers to them.
pub fn tokens() -> Ref {
    Ref::Bound(TOKENS.to_owned())
}

/// Whether the effect creates described tokens that a later sentence may name.
fn nameable(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::CreateToken { token, bind, .. }
            if bind.as_deref().is_none_or(|name| name == TOKENS)
                && !matches!(token, crate::effect::TokenRef::CopyOf { .. })
    )
}

/// Whether the last effect creates tokens that the next step may name, with no other kind of tokens before them
/// ("those tokens" would name only the last).
fn nameable_last(effects: &[Effect]) -> bool {
    effects.split_last().is_some_and(|(last, before)| {
        nameable(last) && !matches!(before.last(), Some(Effect::CreateToken { .. }))
    })
}

/// A step that acts on the tokens the effects before it created, read as `Bound`: a sentence of its own or a step joined
/// to the creation by ", then" or "and".
pub fn joined(r: &mut Reader<'_>, effects: &[Effect]) -> Option<Vec<Effect>> {
    nameable_last(effects).then_some(())?;
    let outer = r.it.replace(tokens());
    let found = r.attempt(follow_up);
    r.it = outer;
    found
}

/// The sentence after tokens were created that acts on them: it binds the tokens.
pub fn after_tokens(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let found = r.attempt(|r| {
        let mut found = joined(r, effects)?;
        if r.where_x {
            found = where_x::bind(r, &found)?;
        }
        r.sentence_end().then_some(found)
    })?;
    effects.extend(found);
    bind(effects);
    Some(())
}

/// Binds the last tokens created when a later effect names them.
pub fn bind(effects: &mut [Effect]) {
    let Some(at) = effects
        .iter()
        .rposition(|effect| matches!(effect, Effect::CreateToken { .. }))
    else {
        return;
    };
    let named = serde_json::to_string(&effects[at + 1..])
        .is_ok_and(|json| json.contains(r#"{"Bound":"tokens"}"#));
    if named && let Effect::CreateToken { bind, .. } = &mut effects[at] {
        *bind = Some(TOKENS.to_owned());
    }
}

/// What a sentence may do to the tokens just created.
fn follow_up(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if let Some(found) = r.attempt(counters::put) {
        return Some(found);
    }
    if let Some(found) = r.attempt(attach) {
        return Some(found);
    }
    r.attempt(gains)
}

/// "Attach this Equipment to it."
fn attach(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("attach")?;
    r.source().then_some(())?;
    r.need("to")?;
    (r.word("it") || r.words("that token")).then_some(())?;
    Some(vec![Effect::Attach {
        what: Ref::This,
        to: tokens(),
    }])
}

/// "That token gains haste until end of turn.", "Those tokens gain haste until end of turn and attack this combat if
/// able."
fn gains(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let mut found = changes::pump(r)?;
    let [Effect::Continuous { affected, .. }] = found.as_slice() else {
        return None;
    };
    (*affected == tokens()).then_some(())?;
    if r.attempt(|r| {
        r.need("and")?;
        (r.word("attacks") || r.word("attack")).then_some(())?;
        r.need("this combat if able")
    })
    .is_some()
    {
        found.push(Effect::Rule {
            rule: GameRule::MustAttack(tokens()),
            duration: Duration::UntilEndOfTurn,
        });
    }
    Some(found)
}
