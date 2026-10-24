//! The sentence after tokens were created that acts on them: counters put on them ("Put X +1/+1 counters on it", "Put a
//! reach counter on each of them"), an Equipment attached ("Attach ~ to it") and a grant until end of turn ("That token
//! gains haste until end of turn and attacks this combat if able").

use super::{Amount, Effect, Ref, Writer, create};
use crate::compile::read::token_binds::{TOKENS, tokens};
use crate::modify::{Duration, GameRule};

impl Writer<'_> {
    /// Tokens made and the sentence after them, as sentences, and the number of effects they cover: what is done to the
    /// tokens, or the delayed exile or sacrifice of a copy.
    pub(super) fn made_then(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        if let Some(found) = self.bound_tokens(effects) {
            return Some(found);
        }
        if let [created @ Effect::CreateToken { .. }, delayed, ..] = effects
            && let Some(delay) = super::copies::delay_text(created, delayed)
        {
            return Some((vec![self.copy_token(created, true)?, delay], 2));
        }
        self.exile_and_return_later(effects)
            .or_else(|| Writer::return_itself_later(effects))
            .or_else(|| self.exile_chosen_from_graveyard(effects))
    }

    /// The creation and the sentence that acts on its tokens, and the number of effects they cover.
    fn bound_tokens(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let [
            created @ Effect::CreateToken {
                bind: Some(name),
                count,
                ..
            },
            follow,
            rest @ ..,
        ] = effects
        else {
            return None;
        };
        (name == TOKENS).then_some(())?;
        let first = create::text_bound(created, self.per)?;
        let one = *count == Amount::one();
        self.made = Some(one);
        let written = self.follow_up(follow, rest, one);
        self.made = None;
        let (second, used) = written?;
        Some((vec![first, second], 1 + used))
    }

    /// The sentence for the effect after the creation, and how many effects it covers.
    fn follow_up(
        &mut self,
        follow: &Effect,
        rest: &[Effect],
        one: bool,
    ) -> Option<(String, usize)> {
        match follow {
            Effect::PutCounters { on, .. } if *on == tokens() => Some((self.counters(follow)?, 1)),
            Effect::Attach {
                what: Ref::This,
                to,
            } if *to == tokens() && one => Some(("attach ~ to it".to_owned(), 1)),
            Effect::Continuous {
                affected,
                mods,
                duration,
            } if *affected == tokens() => {
                let text = self.pump(affected, mods, *duration)?;
                match rest.first() {
                    Some(Effect::Rule {
                        rule: GameRule::MustAttack(who),
                        duration: Duration::UntilEndOfTurn,
                    }) if *who == tokens() => {
                        let verb = if one { "attacks" } else { "attack" };
                        Some((format!("{text} and {verb} this combat if able"), 2))
                    }
                    _ => Some((text, 1)),
                }
            }
            _ => None,
        }
    }
}
