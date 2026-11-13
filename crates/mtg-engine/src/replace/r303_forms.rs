//! "As this creature enters, it becomes your choice of a 3/3 creature, a 2/2 creature with flying, or a 1/6 creature
//! with defender" (`ReplaceBy::BecomesChoice`): the controller picks a form and the form joins the permanent's copiable
//! values (CR 707.2) as a copy effect of the permanent's own values, so a copy of it is that form too.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::compile::form_text;
use mtg_cards::modify::Modification;
use mtg_cards::trigger::ReplaceBy;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Asked, Origin, Pass};
use crate::choose::{Choice, ChoiceOption, OptionOptions};
use crate::copies::{BecameCopy, copiable_values};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;

impl Pass<'_> {
    /// Asks `controller` which of `forms` the card `object` becomes as it enters (never asked for one form), then has
    /// it enter as a copy of its own copiable values with the form's changes; the index of this choice among the rules of
    /// those values, which must not apply again to the same entering.
    pub(super) fn r303_becomes_form(
        &mut self,
        at: usize,
        source: ObjectId,
        object: ObjectId,
        controller: PlayerId,
        forms: &[Vec<Modification>],
        applied: &[Origin],
    ) -> Result<Asked<Option<usize>>, EngineError> {
        let picked = match forms.len() {
            0 => return Ok(Asked::Got(None)),
            1 => 0,
            _ => match self.answer() {
                None => {
                    let labels: Vec<String> = forms
                        .iter()
                        .enumerate()
                        .map(|(at, form)| {
                            form_text(form).unwrap_or_else(|| format!("form {}", at + 1))
                        })
                        .collect();
                    return Ok(Asked::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            can_cancel: false,
                            source,
                            text: format!("Choose what this becomes: {}.", labels.join(", or ")),
                            options: labels.into_iter().map(ChoiceOption::Label).collect(),
                        }),
                    }));
                }
                Some((who, Choice::Option(index)))
                    if *who == controller && *index < forms.len() =>
                {
                    *index
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the choice of a form of {controller:?}"
                    )));
                }
            },
        };
        let Some(entering) = self
            .state
            .object(object)
            .map(crate::object::GameObject::reference)
        else {
            return Ok(Asked::Got(None));
        };
        let Some(mut values) = copiable_values(self.state, entering, None) else {
            return Ok(Asked::Got(None));
        };
        for change in forms.get(picked).into_iter().flatten() {
            if !values.except(change) {
                return Err(Self::invalid(format!("{change:?} is no change of a form")));
            }
        }
        let rule = crate::ability_reader::printed_rules(&values).position(|rule| {
            matches!(&**rule, Ability::Replacement { by: ReplaceBy::BecomesChoice(same), .. } if same == forms)
        });
        self.begin_copy(
            at,
            BecameCopy {
                object,
                of: entering,
                definition: Arc::new(values),
                until_end_of_turn: false,
                until_next_turn_of: None,
                attached_by: None,
            },
            applied,
        );
        Ok(Asked::Got(rule))
    }
}
