//! Static abilities under a condition the renderers do not word: "as long as you control another blue creature", "as
//! long as you control a white or black permanent", "as long as no opponent controls a white or blue creature", "as
//! long as ~ is equipped" and "as long as you've drawn two or more cards this turn". The ability is written under
//! "it's your turn" and the condition's words are put in its place.

use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::article;
use super::w4b_stand::{json, stand, swapped, word};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition};
use serde_json::json;

/// Adds the wordings of a static ability whose condition one of the phrases words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        condition: Some(condition),
        ..
    } = ability
    else {
        return;
    };
    let Some((trailing, leading)) = phrases(condition) else {
        return;
    };
    let Some(stand) = stand(ability, |value| {
        *value.pointer_mut("/Static/condition")? = json!("YourTurn");
        Some(())
    }) else {
        return;
    };
    texts.extend(swapped(
        &stand,
        &[(
            "as long as it's your turn",
            &format!("as long as {trailing}"),
        )],
    ));
    let opening = format!("As long as {leading}, ");
    let led = swapped(&stand, &[("As long as it's your turn, ", &opening)]);
    // After a condition about the card itself, "it" stands for the card.
    if leading.starts_with("~ ") {
        let named = format!("{opening}~ ");
        texts.extend(led.iter().filter_map(|text| {
            let rest = text.strip_prefix(&named)?;
            Some(format!("{opening}it {rest}"))
        }));
    }
    texts.extend(led);
}

/// The words of the condition after "as long as" at the end of the sentence and at its head: the card is "it" after
/// its own name and "~" before it.
fn phrases(condition: &Condition) -> Option<(String, String)> {
    match condition {
        Condition::Compare {
            left: Amount::Count(selector),
            op,
            right: Amount::Fixed(number),
        } => {
            let phrase = controlled(selector, *op, *number)?;
            Some((phrase.clone(), phrase))
        }
        Condition::Compare {
            left,
            op: Cmp::Ge,
            right: Amount::Fixed(number),
        } => {
            let count = word(&json!(number))?;
            (json(left) == json!({"Tallied": {"who": "You", "what": "Drawn"}}))
                .then(|| format!("you've drawn {count} or more cards this turn"))
                .map(|phrase| (phrase.clone(), phrase))
        }
        Condition::Matches {
            subject: Ref::This,
            selector,
        } => {
            let kind = attached_kind(selector)?;
            Some((format!("it's {kind}"), format!("~ is {kind}")))
        }
        _ => None,
    }
}

/// "equipped" or "enchanted" for a card that has an Equipment or an Aura attached.
fn attached_kind(selector: &Selector) -> Option<&'static str> {
    match selector.0.as_slice() {
        [Pred::HasAttached(held)] => match held.0.as_slice() {
            [Pred::Subtype(subtype)] if subtype.0 == "Equipment" => Some("equipped"),
            [Pred::Subtype(subtype)] if subtype.0 == "Aura" => Some("enchanted"),
            _ => None,
        },
        _ => None,
    }
}

/// What a player controls: "you control another blue creature", "you control a white or black permanent", "no opponent
/// controls a white or blue creature".
fn controlled(selector: &Selector, op: Cmp, number: i32) -> Option<String> {
    let mut preds = selector.0.clone();
    let at = preds
        .iter()
        .position(|pred| matches!(pred, Pred::ControlledBy(_)))?;
    let who = preds.remove(at);
    let other = preds.iter().position(|pred| *pred == Pred::Other);
    if let Some(at) = other {
        preds.remove(at);
    }
    let kind = kind_words(&preds)?;
    match (who, op, number, other.is_some()) {
        (Pred::ControlledBy(Ref::You), Cmp::Ge, 1, true) => {
            Some(format!("you control another {kind}"))
        }
        (Pred::ControlledBy(Ref::You), Cmp::Ge, 1, false) => {
            Some(format!("you control {} {kind}", article(&kind)))
        }
        (Pred::ControlledBy(Ref::Opponents), Cmp::Eq, 0, false) => {
            Some(format!("no opponent controls {} {kind}", article(&kind)))
        }
        _ => None,
    }
}

/// The words of one kind of permanent, with a choice of colors in front: "white or black permanent", "blue creature".
fn kind_words(preds: &[Pred]) -> Option<String> {
    let mut rest = preds.to_vec();
    let Some(at) = rest.iter().position(|pred| matches!(pred, Pred::AnyOf(_))) else {
        return phrase_text(&Selector(rest), Form::Subject);
    };
    let Pred::AnyOf(choices) = rest.remove(at) else {
        return None;
    };
    let colors: Vec<&str> = choices
        .iter()
        .map(|choice| match choice {
            Pred::Color(color) => Some(color_word(*color)),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let (last, others) = colors
        .split_last()
        .filter(|(_, others)| !others.is_empty())?;
    let named = if others.len() == 1 {
        format!("{} or {last}", others[0])
    } else {
        format!("{}, or {last}", others.join(", "))
    };
    let kind = phrase_text(&Selector(rest), Form::Subject)?;
    Some(format!("{named} {kind}"))
}

/// A color in lower case.
fn color_word(color: mtg_core::color::Color) -> &'static str {
    use mtg_core::color::Color;
    match color {
        Color::White => "white",
        Color::Blue => "blue",
        Color::Black => "black",
        Color::Red => "red",
        Color::Green => "green",
    }
}
