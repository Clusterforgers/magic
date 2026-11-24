//! `Effect::R120ChooseEachColor`: one optional choice per color among some permanents, each among the objects of that
//! color no earlier color took ("For each of those colors, you may exile a card of that color from among the revealed
//! cards", Sanar, Innovative First-Year).

use mtg_cards::effect::Effect;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;

use super::wrong_effect;
use crate::characteristics::characteristics;
use crate::choose::{CardsOptions, Choice, Target, check_cards};
use crate::decision::{Decision, DecisionKind};
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The colors among the objects `colors_of` picks are read once; for each, in the order of the color wheel, the player
/// may choose one object of that color among what `from` names and no earlier color took. A color with nothing to choose
/// asks nothing. The answers are read back from the journal in order.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R120ChooseEachColor {
        who,
        colors_of,
        from,
        bind,
    } = effect
    else {
        return Err(wrong_effect("R120ChooseEachColor"));
    };
    let Some(player) = cx.scope.players(who)?.into_iter().next() else {
        return Err(ResolveError::Invalid(
            "no player chooses a card per color".into(),
        ));
    };
    let state = cx.state();
    let colors: Vec<Color> = crate::mana_kinds::colors_among(&cx.scope, colors_of)
        .into_iter()
        .filter_map(|mana| {
            Color::ALL
                .into_iter()
                .find(|color| crate::mana_kinds::mana_of(*color) == mana)
        })
        .collect();
    let objects: Vec<_> = cx
        .scope
        .refs(from)?
        .into_iter()
        .filter_map(crate::choose::Target::object)
        .collect();
    let mut answers = cx.journal().iter();
    let mut chosen: Vec<ObjectId> = Vec::new();
    for color in colors {
        let offered: Vec<ObjectId> = objects
            .iter()
            .map(|object| object.id)
            .filter(|id| !chosen.contains(id))
            .filter(|id| {
                characteristics(state, *id).is_some_and(|chars| chars.colors.contains(color))
            })
            .collect();
        if offered.is_empty() {
            continue;
        }
        let options = CardsOptions {
            source: cx.scope.source.id,
            from: offered,
            min: 0,
            max: 1,
            text: format!("Choose a {} card", color_word(color)),
            can_cancel: false,
            one_per_name: false,
            total: None,
            groups: Vec::new(),
            targeted: Box::default(),
        };
        match answers.next() {
            None => {
                return Ok(Run::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseCards(options),
                }));
            }
            Some((answerer, Choice::Cards(cards)))
                if *answerer == player && check_cards(&options, cards).is_ok() =>
            {
                chosen.extend(cards.iter().copied());
            }
            Some(other) => {
                return Err(ResolveError::Invalid(format!(
                    "{other:?} is no answer to a choice of a card of a color"
                )));
            }
        }
    }
    let bound = objects
        .into_iter()
        .filter(|object| chosen.contains(&object.id))
        .map(Target::Object)
        .collect();
    Ok(Run::Done {
        events: Vec::new(),
        bound: vec![(bind.clone(), bound)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The color as a word of a prompt.
const fn color_word(color: Color) -> &'static str {
    match color {
        Color::White => "white",
        Color::Blue => "blue",
        Color::Black => "black",
        Color::Red => "red",
        Color::Green => "green",
    }
}
