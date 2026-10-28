//! The sentence of a villainous choice: "S faces a villainous choice — A, or B", the options written with "that player"
//! for the player who faces it.

use super::super::w2d_villainous::{W2D_FACING, w2d_swap};
use super::{Effect, Writer};
use crate::refs::Ref;
use crate::value::Condition;

/// "that player loses 2 life and ..." as "they lose 2 life and ...": the subject and its verb in the plural.
fn w2d_they(text: &str) -> String {
    let Some(rest) = text.strip_prefix("that player ") else {
        return text.to_owned();
    };
    let (verb, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    let plural = match verb {
        "has" => "have",
        other => other.strip_suffix('s').unwrap_or(other),
    };
    format!("they {plural} {tail}").trim_end().to_owned()
}

impl Writer<'_> {
    /// Steps of one subject: the controller's say "you", as a bare verb would be the chooser's.
    fn w2d_step(&mut self, steps: &[Effect], they: bool) -> Option<String> {
        let text = self.consequence(&Condition::All(Vec::new()), steps)?;
        let yours = steps.first().and_then(super::player_of) == Some(&Ref::You);
        Some(if yours && !text.starts_with("you ") {
            format!("you {text}")
        } else if they {
            w2d_they(&text)
        } else {
            text
        })
    }

    /// The villainous choice that opens the effects, alone or as the only step of a pass over the opponents, and the
    /// number of effects it covers.
    pub(super) fn w2d_villainous(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let (subject, choice) = match effects.first()? {
            Effect::RepeatEach {
                over: Ref::Opponents,
                bind,
                effects: steps,
            } if bind == W2D_FACING => match steps.as_slice() {
                [choice @ Effect::VillainousChoice { who, .. }]
                    if *who == Ref::Bound(W2D_FACING.to_owned()) =>
                {
                    ("each opponent".to_owned(), choice)
                }
                _ => return None,
            },
            choice @ Effect::VillainousChoice { who, .. } => (self.player(who)?, choice),
            _ => return None,
        };
        let Effect::VillainousChoice {
            who,
            first,
            second,
            they,
        } = choice
        else {
            return None;
        };
        let mut options = Vec::new();
        for option in [first, second] {
            let named = w2d_swap(option, who, &Ref::Triggering)?;
            let players: Vec<Option<&Ref>> = named.iter().map(super::player_of).collect();
            // Steps of the chooser and of the controller each say their subject: "they lose 2 life and you draw".
            let text = if players.len() > 1 && players.windows(2).any(|pair| pair[0] != pair[1]) {
                let steps: Option<Vec<String>> = named
                    .iter()
                    .map(|step| self.w2d_step(std::slice::from_ref(step), *they))
                    .collect();
                steps?.join(" and ")
            } else {
                self.w2d_step(&named, *they)?
            };
            options.push(text);
        }
        Some((
            vec![format!(
                "{subject} faces a villainous choice \u{2014} {}, or {}",
                super::capitalize(&options[0]),
                options[1]
            )],
            1,
        ))
    }
}
