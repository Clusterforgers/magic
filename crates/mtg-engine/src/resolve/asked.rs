//! Where in its rules a spell or ability asks a decision: the effect its resolution stands at, or the modes it chooses.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, branches};
use mtg_core::ids::ObjectId;

use super::{rules_of, tree};
use crate::decision::DecisionKind;
use crate::state::GameState;

/// The part of a spell's or ability's rules a decision asks about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskedAt {
    /// The rules the spell or ability runs, which the path reads.
    pub rules: Arc<Ability>,
    /// What the decision asks about in them.
    pub part: AskedPart,
}

/// A place in the effects of a rules text, as a path in the card's own terms (`mtg_cards::effect::branches`): an
/// effect index, then a branch and an effect index for each level down, a branch of a `Modal` being its mode on the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskedPart {
    /// The modes of the modal effect at the path, chosen as the spell is cast or the ability put on the stack.
    Modes(Vec<usize>),
    /// The effect at the path, which the resolution runs as it asks.
    Effect(Vec<usize>),
}

/// Where the spell or ability `stack` asks the decision `kind`: the modes of its modal effect for a choice of modes, else
/// the effect its resolution stands at. `None` while it does not resolve (a target, a cost or a payment as it is cast)
/// and for an object that runs no effects.
pub fn asked_at(state: &GameState, stack: ObjectId, kind: &DecisionKind) -> Option<AskedAt> {
    let object = state.object(stack)?;
    let rules = rules_of(state, object)?;
    let chosen = &object.choices().modes;
    let at = state
        .resolving()
        .filter(|resolving| resolving.object() == stack && !resolving.r115_continuing())
        .and_then(|resolving| tree::card_path(rules.effects(), chosen, resolving.position()));
    let part = match (kind, at) {
        (DecisionKind::ChooseMode(_), Some(path))
            if matches!(
                effect_on_card(rules.effects(), &path),
                Some(Effect::Modal { .. })
            ) =>
        {
            AskedPart::Modes(path)
        }
        (DecisionKind::ChooseMode(_), _) => AskedPart::Modes(first_modal(rules.effects())?),
        (_, Some(path)) => AskedPart::Effect(path),
        (_, None) => return None,
    };
    Some(AskedAt { rules, part })
}

/// The rules the spell or ability `stack` runs: those of the ability, or the spell ability of the card as cast.
pub fn stack_rules(state: &GameState, stack: ObjectId) -> Option<Arc<Ability>> {
    rules_of(state, state.object(stack)?)
}

/// The effect at a path in the card's own terms.
pub fn effect_on_card<'a>(root: &'a [Effect], path: &[usize]) -> Option<&'a Effect> {
    let (&index, mut rest) = path.split_first()?;
    let mut effect = root.get(index)?;
    while let [branch, index, deeper @ ..] = rest {
        effect = branches(effect).get(*branch)?.get(*index)?;
        rest = deeper;
    }
    rest.is_empty().then_some(effect)
}

/// The path of the first modal effect, the lists inside each effect searched before the next effect.
fn first_modal(list: &[Effect]) -> Option<Vec<usize>> {
    list.iter().enumerate().find_map(|(index, effect)| {
        if matches!(effect, Effect::Modal { .. }) {
            return Some(vec![index]);
        }
        branches(effect)
            .iter()
            .enumerate()
            .find_map(|(branch, inner)| {
                first_modal(inner).map(|deeper| [vec![index, branch], deeper].concat())
            })
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a path in the card's own terms names the effect it leads to, through every kind of branch, and the first
    //! modal effect is found at any depth.
    use super::*;
    use rstest::rstest;

    fn effects(json: &str) -> Vec<Effect> {
        serde_json::from_str(json).unwrap_or_default()
    }

    fn tree() -> Vec<Effect> {
        effects(
            r#"[
                {"Draw":{}},
                {"If":{"condition":"YourTurn","then":[{"GainLife":{"amount":1}}],"otherwise":[{"Modal":{"modes":[
                    {"effects":[{"Draw":{}}]},
                    {"effects":[{"Shuffle":{}},{"LoseLife":{"amount":1}}]}
                ]}}]}}
            ]"#,
        )
    }

    fn kind(effect: Option<&Effect>) -> String {
        effect
            .and_then(|effect| serde_json::to_value(effect).ok())
            .and_then(|value| value.as_object().and_then(|map| map.keys().next().cloned()))
            .unwrap_or_default()
    }

    #[rstest]
    #[case::top(&[0], "Draw")]
    #[case::then(&[1, 0, 0], "GainLife")]
    #[case::the_modal(&[1, 1, 0], "Modal")]
    #[case::a_mode_by_its_card_index(&[1, 1, 0, 1, 1], "LoseLife")]
    #[case::a_branch_without_index(&[1, 0], "")]
    #[case::nowhere(&[5], "")]
    fn a_card_path_names_its_effect(#[case] path: &[usize], #[case] expected: &str) {
        assert_eq!(kind(effect_on_card(&tree(), path)), expected);
    }

    #[test]
    fn the_first_modal_is_found_inside_a_branch() {
        assert_eq!(first_modal(&tree()), Some(vec![1, 1, 0]));
        assert_eq!(first_modal(&effects(r#"[{"Draw":{}}]"#)), None);
    }
}
