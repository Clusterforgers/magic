//! Tokens created beside other steps of "you": a list of tokens ("create a 1/1 Snake token, a 2/2 Wolf token, and a 3/3
//! Elephant token"), a draw or a mill and a token ("draw two cards and create a Treasure token", "create three Elf
//! tokens, then mill three cards").

use super::{Effect, Ref, Writer, create, join_list};

/// Whether the effect creates tokens for you with nothing bound to them.
fn made(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::CreateToken {
            who: Ref::You,
            bind: None,
            ..
        }
    )
}

/// Whether the effect is a step of "you" that begins with its verb in `chained`'s reading.
fn plain_step(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::Draw { who: Ref::You, .. }
            | Effect::Mill { who: Ref::You, .. }
            | Effect::Scry { who: Ref::You, .. }
            | Effect::Surveil { who: Ref::You, .. }
    )
}

/// Whether the mask closes the joints inside the `used` effects, and not the one after them.
fn joined(joints: u32, used: usize) -> bool {
    let inside = (1u32 << (used - 1)) - 1;
    joints & inside == inside && joints & (1 << (used - 1)) == 0
}

impl Writer<'_> {
    /// Tokens and the steps beside them as one sentence whose joints the mask closes, and the number of effects it
    /// covers. Separate sentences are the wording of a mask that leaves them open.
    pub(super) fn token_steps(
        &mut self,
        effects: &[Effect],
        joints: u32,
    ) -> Option<(String, usize)> {
        if self.per {
            return None;
        }
        let run = effects.iter().take_while(|effect| made(effect)).count();
        if run >= 2 {
            joined(joints, run).then_some(())?;
            let mut items = Vec::with_capacity(run);
            for effect in &effects[..run] {
                items.push(
                    create::text(effect, false)?
                        .strip_prefix("create ")?
                        .to_owned(),
                );
            }
            return Some((format!("create {}", join_list(&items)), run));
        }
        if !joined(joints, 2) {
            return None;
        }
        match effects {
            [first, token, ..] if plain_step(first) && made(token) => {
                let step = self.predicate(first, false)?;
                Some((format!("{step} and {}", create::text(token, false)?), 2))
            }
            [token, second, ..] if made(token) && plain_step(second) => {
                let step = self.predicate(second, false)?;
                Some((format!("{}, then {step}", create::text(token, false)?), 2))
            }
            _ => None,
        }
    }
}
