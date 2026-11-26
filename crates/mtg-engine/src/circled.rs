//! Colors circled as the deck is built: "As you create your deck, circle two of the colors below."
//! (`Ability::CircleColors`, Cryptic Spires).
//!
//! The opening asks the owner of each card with such an ability for its colors before any hand is dealt, one card at a
//! time in turn order from the first player; `ColorsCircled` records them by the card's id, which stays the card's in
//! every zone, and is public. The colors count as printed on the card: a permanent that is a copy of one reads the
//! colors of what it copies (`Produce::Circled`).

use mtg_cards::ability::Ability;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::choose::{ChoiceOption, OptionChoice, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::state::GameState;

/// The words of the question.
pub(crate) const ASKED: &str = "Circle the colors of this card for your deck.";

/// How many colors the card of `id` circles as its deck is built; none without the ability.
fn circles(state: &GameState, id: ObjectId) -> Option<u32> {
    state.object(id)?.whole_card().rules.iter().find_map(|rules| match &**rules {
        Ability::CircleColors(count) => Some(*count),
        _ => None,
    })
}

/// The colors circled for the card `id` before the game began, if they were.
fn circled_on(state: &GameState, id: ObjectId) -> Option<&[Color]> {
    state
        .circled
        .iter()
        .find(|(card, _)| *card == id)
        .map(|(_, colors)| colors.as_slice())
}

/// The colors circled for `id`: its own, or for a permanent that is a copy those of what it copies (the ruling of
/// Cryptic Spires: they are copiable values); none when no color was circled.
pub(crate) fn circled(state: &GameState, id: ObjectId) -> Vec<Color> {
    let mut at = id;
    // A copy of a copy reads through each; the bound keeps a cycle from looping.
    for _ in 0..16 {
        let copied = state
            .object(at)
            .and_then(crate::object::GameObject::copy_effect)
            .map(|effect| effect.of().id);
        match copied {
            Some(of) if of != at => at = of,
            _ => return circled_on(state, at).map(<[Color]>::to_vec).unwrap_or_default(),
        }
    }
    Vec::new()
}

/// Every way to circle `count` of the five colors, in WUBRG order.
fn ways(count: u32) -> Vec<Vec<Color>> {
    let count = usize::try_from(count).unwrap_or(0).min(Color::ALL.len());
    let mut out = Vec::new();
    for mask in 0u32..(1 << Color::ALL.len()) {
        if mask.count_ones() as usize == count {
            out.push(
                (0..Color::ALL.len())
                    .filter(|bit| mask & (1 << bit) != 0)
                    .map(|bit| Color::ALL[bit])
                    .collect::<Vec<_>>(),
            );
        }
    }
    out.sort_by_key(|colors| colors.iter().map(|c| *c as u8).collect::<Vec<_>>());
    out
}

/// The label of one way to circle colors: "White and blue".
fn label(colors: &[Color]) -> String {
    let names: Vec<String> = colors
        .iter()
        .map(|color| format!("{color:?}").to_lowercase())
        .collect();
    let joined = match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    };
    let mut chars = joined.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The next card the opening asks about: one with the ability and no colors yet, owned by the first seat in turn order
/// from the first player that has one, lowest id first.
pub(crate) fn question(state: &GameState) -> Option<Decision> {
    if !state.objects().any(|object| circles(state, object.id()).is_some()) {
        return None;
    }
    state
        .turn_order_from(state.first_player())
        .into_iter()
        .find_map(|player| {
            let (id, count) = state
                .objects()
                .filter(|object| object.owner() == player && !object.is_card_copy())
                .filter(|object| circled_on(state, object.id()).is_none())
                .find_map(|object| Some((object.id(), circles(state, object.id())?)))?;
            Some(Decision {
                player,
                kind: DecisionKind::ChooseOption(OptionOptions {
                    source: id,
                    options: ways(count)
                        .iter()
                        .map(|colors| ChoiceOption::Label(label(colors)))
                        .collect(),
                    text: ASKED.to_owned(),
                    can_cancel: false,
                }),
            })
        })
}

/// The event of the colors picked for the card the question names.
pub(crate) fn answer(
    state: &GameState,
    player: PlayerId,
    options: &OptionOptions,
    choice: OptionChoice,
) -> Result<Outcome, EngineError> {
    let count = circles(state, options.source).unwrap_or(0);
    let Some(colors) = ways(count).into_iter().nth(choice.0) else {
        return Err(IllegalAnswer::NotOffered("colors").into());
    };
    Ok(Outcome::Events(vec![Event::ColorsCircled {
        player,
        object: options.source,
        colors,
    }]))
}

#[cfg(test)]
mod tests {
    //! Goal: the ways to circle colors are every set of that many of the five, each once, labelled in WUBRG order.
    use super::*;

    #[rstest::rstest]
    #[case(1, 5, "White")]
    #[case(2, 10, "White and blue")]
    #[case(3, 10, "White, blue and black")]
    fn every_set_of_colors_is_offered_once(#[case] count: u32, #[case] sets: usize, #[case] first: &str) {
        let all = ways(count);
        assert_eq!(all.len(), sets);
        assert!(all.iter().all(|colors| colors.len() == count as usize));
        let mut unique = all.clone();
        unique.dedup();
        assert_eq!(unique.len(), sets);
        assert_eq!(label(&all[0]), first);
    }
}
