//! A token created and the Equipment of an earlier sentence attached to it: "Gain control of target Equipment, then create
//! a 0/0 black Phyrexian Germ creature token and attach that Equipment to it."

use super::Writer;
use crate::compile::read::create;
use crate::compile::read::selectors::{Form, phrase_text};
use crate::compile::read::token_binds::{TOKENS, tokens};
use crate::effect::Effect;
use crate::refs::{Ref, Targetable};
use crate::value::Amount;

impl Writer<'_> {
    /// The sentence of one token and the attach of the target a sentence before named, and the effects it covers.
    pub(super) fn token_and_attach(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let [
            created @ Effect::CreateToken {
                bind: Some(name),
                count,
                ..
            },
            Effect::Attach {
                what: Ref::Target(0),
                to,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        (name == TOKENS && *to == tokens() && *count == Amount::one() && self.named)
            .then_some(())?;
        let Targetable::Object(selector) = &self.targets.first()?.what else {
            return None;
        };
        let attached = phrase_text(selector, Form::Target)?;
        let made = create::text_bound(created, self.per)?;
        Some((vec![format!("{made} and attach that {attached} to it")], 2))
    }
}
