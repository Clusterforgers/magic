//! A word chosen as a permanent enters ("As this enchantment enters, choose Khans or Dragons."): the controller picks one
//! of the words, the permanent keeps it while it stays, and `Condition::EnteredAs` reads it.

use mtg_cards::trigger::ReplaceBy;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Asked, Pass};
use crate::choose::{Choice, ChoiceOption, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::object::Chosen;

impl Pass<'_> {
    /// What `controller` chooses as the card enters for a choice of a kind (`ReplaceBy::Choose`) or of words
    /// (`ReplaceBy::ChooseWord`); nothing for any other effect.
    pub(super) fn e15f_chosen(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        by: &ReplaceBy,
    ) -> Result<Asked<Option<Chosen>>, EngineError> {
        match by {
            ReplaceBy::Choose(kind) => self.chooses(source, controller, *kind),
            ReplaceBy::ChooseWord(words) => self.e15f_chooses_word(source, controller, words),
            _ => Ok(Asked::Got(None)),
        }
    }

    /// The word `controller` chooses among `words` as the card enters; asked unless there is one word or none.
    pub(super) fn e15f_chooses_word(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        words: &[String],
    ) -> Result<Asked<Option<Chosen>>, EngineError> {
        let picked = match words {
            [] => None,
            [_] => Some(0),
            _ => match self.answer() {
                None => {
                    return Ok(Asked::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            can_cancel: false,
                            source,
                            options: words.iter().cloned().map(ChoiceOption::Label).collect(),
                            text: format!("Choose {}.", words.join(" or ")),
                        }),
                    }));
                }
                Some((who, Choice::Option(index)))
                    if *who == controller && *index < words.len() =>
                {
                    Some(*index)
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the choice of a word of {controller:?}"
                    )));
                }
            },
        };
        Ok(Asked::Got(picked.and_then(|index| {
            words.get(index).cloned().map(Chosen::Word)
        })))
    }
}
