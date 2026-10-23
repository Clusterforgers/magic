//! Manifest and cloak from the top of a library: "Manifest the top card of your library.", "Cloak the top card of your
//! library.", "Its controller manifests the top card of their library.", "you manifest the top card of that player's
//! library.", each read from its wording and written back the same way.

use super::library::{top_cards, top_text};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// The verb of each effect, as the imperative.
fn verb(cloak: bool) -> &'static str {
    if cloak { "cloak" } else { "manifest" }
}

/// The player who acts: "you" (also understood before an imperative), "its controller" of the object the sentence before
/// named, or "that player" of a trigger.
fn subject(r: &mut Reader<'_>) -> Option<Ref> {
    if r.word("you") {
        return Some(Ref::You);
    }
    if r.words("its controller") {
        return r.it.clone().map(|it| Ref::ControllerOf(Box::new(it)));
    }
    (r.that_player && r.words("that player")).then_some(Ref::Triggering)
}

/// "Manifest the top card of your library.", "Cloak the top two cards of your library.", "Its controller manifests the
/// top card of their library.", "you manifest the top card of that player's library.": the top cards of a library are
/// put onto the battlefield face down.
pub fn manifest(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let named = subject(r);
    let who = named.clone().unwrap_or(Ref::You);
    let third = !matches!(who, Ref::You);
    let word = r.take_word()?;
    let base = word.strip_suffix('s').unwrap_or(word);
    // A player other than you takes the verb with an "s".
    if (base != word) != third {
        return None;
    }
    let cloak = match base {
        "manifest" => false,
        "cloak" => true,
        _ => return None,
    };
    let count = top_cards(r)?;
    r.need("of")?;
    let from = if third {
        r.need("their library")?;
        None
    } else if r.words("your library") {
        None
    } else {
        r.need("that player's library")?;
        r.that_player.then_some(())?;
        Some(Ref::Triggering)
    };
    let bind = None;
    Some(vec![if cloak {
        Effect::Cloak {
            who,
            count,
            from,
            bind,
        }
    } else {
        Effect::Manifest {
            who,
            count,
            from,
            bind,
        }
    }])
}

/// The player an effect of manifest or cloak is about, for the renderer to word.
pub(super) fn who(effect: &Effect) -> Option<&Ref> {
    match effect {
        Effect::Manifest { who, .. } | Effect::Cloak { who, .. } => Some(who),
        _ => None,
    }
}

/// The sentence of a manifest or a cloak by `subject` without its first capital and period: "manifest the top card of
/// your library", "its controller manifests the top card of their library".
pub(super) fn text(effect: &Effect, subject: &str) -> Option<String> {
    let (cloak, who, count, from, bind): (bool, &Ref, &Amount, &Option<Ref>, &Option<String>) =
        match effect {
            Effect::Manifest {
                who,
                count,
                from,
                bind,
            } => (false, who, count, from, bind),
            Effect::Cloak {
                who,
                count,
                from,
                bind,
            } => (true, who, count, from, bind),
            _ => return None,
        };
    // A name is for the sentences that use the permanents, which are not read yet.
    if bind.is_some() {
        return None;
    }
    let cards = top_text(count)?;
    let verb = verb(cloak);
    Some(match (who, from) {
        (Ref::You, None) => format!("{verb} {cards} of your library"),
        (Ref::You, Some(Ref::Triggering)) => {
            format!("{subject} {verb} {cards} of that player's library")
        }
        (_, None) if *who != Ref::You => format!("{subject} {verb}s {cards} of their library"),
        _ => return None,
    })
}
