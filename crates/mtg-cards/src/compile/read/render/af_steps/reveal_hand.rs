//! The top card revealed, put onto the battlefield if it is a kind of card and put into the hand otherwise: "Reveal the
//! top card of your library. If it's a land card, put it onto the battlefield. Otherwise, put that card into your hand."

use super::super::revealed::kind_text;
use super::super::{Amount, Condition, Destination, Effect, Ref, article};

/// The sentences for the reveal, the test of the card and the two places it goes, and the three effects they cover.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::LookAtTop {
            who: Ref::You,
            count,
            bind,
        },
        Effect::Reveal { what },
        Effect::If {
            condition: Condition::Matches { subject, selector },
            then,
            otherwise,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let held = Ref::Bound(bind.clone());
    (*count == Amount::one() && *what == held && *subject == held).then_some(())?;
    let entered = Effect::Move {
        what: held.clone(),
        to: Destination::Battlefield {
            tapped: false,
            under: None,
            transformed: false,
            counters: Vec::new(),
        },
    };
    let kept = Effect::Move {
        what: held,
        to: Destination::Hand,
    };
    (*then == [entered] && *otherwise == [kept]).then_some(())?;
    let kind = kind_text(selector)?;
    Some((
        format!(
            "reveal the top card of your library. If it's {} {kind}, put it onto the battlefield. Otherwise, put that card into your hand",
            article(&kind)
        ),
        3,
    ))
}
