//! The alternative an activated ability's cost is paid with, chosen first (CR 602.2b, 601.2b): "Sacrifice an artifact or
//! discard a nonland card". The variants that can be paid now are offered with `ChooseOption` (one is taken without a
//! question); the one chosen is kept in `CostTaken::variant`, and every later step reads the ability with it in place of
//! the choices (`rules_of`), as the face of a spell is settled before the steps that read the card.

use mtg_core::ids::PlayerId;

use super::{Done, Next, Phase};
use crate::abilities::AbilityRef;
use crate::choose::{ChoiceOption, Choices, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::price::Price;
use crate::priority::{Activation, activation};
use crate::state::GameState;

/// Settles the variant of the cost of the ability at `address`, or returns the question that does or the end of the
/// activation; `None` at once for a cost with no choice.
pub(super) fn run(
    state: &GameState,
    player: PlayerId,
    address: AbilityRef,
    choices: &mut Choices,
    done: &mut Done,
) -> Option<Next> {
    if done.r115_cost {
        return None;
    }
    let rules = crate::ability_reader::ability(state, address)?;
    let mtg_cards::ability::Ability::Activated { cost, .. } = &*rules else {
        done.r115_cost = true;
        return None;
    };
    let Some(variants) = crate::r115_cost_choice::r115_ability_variants(&rules) else {
        done.r115_cost = true;
        return None;
    };
    let object = state.object(address.object)?;
    let usable: Vec<usize> = variants
        .iter()
        .enumerate()
        .filter(|(_, variant)| {
            activation(state, player, object, address, variant) == Activation::Ready
        })
        .map(|(index, _)| index)
        .collect();
    let picked = match usable.as_slice() {
        [] => return Some(Next::Drop),
        [only] => *only,
        _ => match done.r115_picked {
            Some(pick) if pick < usable.len() => usable[pick],
            _ => {
                let options = usable
                    .iter()
                    .map(|index| ChoiceOption::Label(label(cost, *index)))
                    .collect();
                done.asked = Some(Phase::Face);
                return Some(Next::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseOption(OptionOptions {
                        source: address.object,
                        options,
                        text: "Choose which cost to pay.".to_owned(),
                        can_cancel: true,
                    }),
                }));
            }
        },
    };
    choices.choose_cost(u8::try_from(picked).unwrap_or(0));
    done.r115_cost = true;
    None
}

/// The words of the alternatives the variant `index` of `cost` pays as the card prints them ("Sacrifice an artifact"),
/// else as the engine words their parts, with a capital.
fn label(cost: &mtg_cards::cost::Cost, index: usize) -> String {
    let parts = crate::r115_cost_choice::r115_chosen_parts(cost, index);
    if let Some(printed) = mtg_cards::compile::render_cost(&parts) {
        return printed;
    }
    let words = match Price::announced(&parts) {
        Ok(price) => super::required::words_of(
            price
                .mana
                .as_ref()
                .unwrap_or(&mtg_core::mana::ManaCost::ZERO),
            &price.parts,
        ),
        Err(_) => String::new(),
    };
    let mut chars = words.chars();
    chars.next().map_or_else(
        || format!("Cost {}", index + 1),
        |first| first.to_uppercase().chain(chars).collect(),
    )
}
