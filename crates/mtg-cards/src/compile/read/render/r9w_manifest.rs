//! A manifested or cloaked card that the next step names as "it": "Manifest the top card of your library, then put two
//! +1/+1 counters on it.", "cloak the top card of your library, then attach ~ to it" and the enchantment that becomes an
//! Aura on its manifest when the data manifests first.

use super::{Effect, Ref, Writer};
use crate::compile::read::c6_blk_aura::c6_blk_aura_effects;
use crate::compile::read::{counters, manifest};
use crate::value::Amount;

/// The name a manifest or a cloak binds its one card to, and the effect without the name.
fn named_one(effect: &Effect) -> Option<(&str, Effect)> {
    let mut plain = effect.clone();
    let (Effect::Manifest { bind, count, .. } | Effect::Cloak { bind, count, .. }) = &mut plain
    else {
        return None;
    };
    (*count == Amount::one()).then_some(())?;
    bind.take()?;
    let (Effect::Manifest {
        bind: Some(name), ..
    }
    | Effect::Cloak {
        bind: Some(name), ..
    }) = effect
    else {
        return None;
    };
    Some((name.as_str(), plain))
}

impl Writer<'_> {
    /// The manifest or cloak and the step on the card it put onto the battlefield, as sentences, and the number of
    /// effects they cover.
    pub(super) fn r9w_manifest_then(effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let [first, rest @ ..] = effects else {
            return None;
        };
        let (name, plain) = named_one(first)?;
        let it = Ref::Bound(name.to_owned());
        let put = manifest::text(&plain, "you")?;
        match rest {
            [Effect::PutCounters { on, counter, count }, ..] if *on == it => Some((
                vec![format!(
                    "{put}, then put {} on it",
                    counters::text(counter, count)?
                )],
                2,
            )),
            [
                Effect::Attach {
                    what: Ref::This,
                    to,
                },
                ..,
            ] if *to == it => Some((vec![format!("{put}, then attach ~ to it")], 2)),
            // The card becomes an Aura before or after the manifest: nothing sees it in between.
            [
                becomes,
                Effect::Attach {
                    what: Ref::This,
                    to,
                },
                ..,
            ] if *to == it => {
                let wanted = c6_blk_aura_effects();
                (Some(becomes) == wanted.first() && matches!(first, Effect::Manifest { .. }))
                    .then_some(())?;
                Some((
                    vec![
                        "it becomes an Aura with enchant creature".to_owned(),
                        format!("{put} and attach ~ to it"),
                    ],
                    3,
                ))
            }
            _ => None,
        }
    }
}
