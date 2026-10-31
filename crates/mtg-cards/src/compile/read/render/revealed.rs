//! The top card of a library revealed and what it does: "Reveal the top card of your library. If it's a creature card,
//! put it onto the battlefield. Otherwise, you may put that card on the bottom of your library.", "... If it's a creature
//! card that shares a creature type with a creature you control, you may cast it without paying its mana cost. If you
//! don't cast it, put it on the bottom of your library."

use super::{Destination, Effect, Form, Place, Ref, Writer, phrase_text};
use crate::refs::{Pred, Selector};
use crate::value::{Amount, Condition};

/// Where the revealed card goes when nothing else happens to it.
fn bottom(to: &Destination) -> bool {
    matches!(
        to,
        Destination::Library {
            place: Place::Bottom,
            order: None
        }
    )
}

/// "put it onto the battlefield", "you may put that card on the bottom of your library", "you may cast it without paying
/// its mana cost. If you don't cast it, put it on the bottom of your library": the steps for the card `held`.
fn steps(effects: &[Effect], held: &Ref) -> Option<String> {
    match effects {
        [
            Effect::Move {
                what,
                to:
                    Destination::Battlefield {
                        tapped: false,
                        under: None,
                        transformed: false,
                        counters,
                    },
            },
        ] if what == held && counters.is_empty() => Some("put it onto the battlefield".to_owned()),
        [Effect::Move { what, to }] if what == held && bottom(to) => {
            Some("put it on the bottom of your library".to_owned())
        }
        [
            Effect::May {
                who: Ref::You,
                effects,
            },
        ] => match effects.as_slice() {
            [Effect::Move { what, to }] if what == held && bottom(to) => {
                Some("you may put that card on the bottom of your library".to_owned())
            }
            _ => None,
        },
        [
            Effect::CastFromZone {
                who: Ref::You,
                what,
                free: true,
                play: false,
                way: None,
                transformed: false,
                bind: None,
                otherwise,
            },
        ] if what == held => {
            let [Effect::Move { what, to }] = otherwise.as_slice() else {
                return None;
            };
            (what == held && bottom(to)).then(|| {
                "you may cast it without paying its mana cost. If you don't cast it, put it on the bottom of your library"
                    .to_owned()
            })
        }
        _ => None,
    }
}

/// "creature card", "creature card that shares a creature type with a creature you control": the kind of card the
/// condition asks for.
pub(super) fn kind_text(selector: &Selector) -> Option<String> {
    let (shared, rest): (Vec<&Pred>, Vec<&Pred>) = selector
        .0
        .iter()
        .partition(|pred| matches!(pred, Pred::SharesSubtype(_)));
    let kind = super::library::filter_text(&Selector(rest.into_iter().cloned().collect()), false)?;
    match shared.as_slice() {
        [] => Some(kind),
        [Pred::SharesSubtype(Ref::Each(group))] => {
            let group_text = phrase_text(group, Form::Count)?;
            let noun = if group_text.starts_with("another ") {
                group_text
            } else {
                format!("{} {group_text}", super::article(&group_text))
            };
            Some(format!("{kind} that shares a creature type with {noun}"))
        }
        _ => None,
    }
}

impl Writer<'_> {
    /// "The owner of target permanent shuffles it into their library, then reveals the top card of their library. If
    /// it's a permanent card, they put it onto the battlefield.": the target goes into its owner's library and the top
    /// card of that library comes out if it is the kind asked for; the number of effects it takes.
    pub(super) fn owner_shuffles(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let owner = || Ref::OwnerOf(Box::new(Ref::Target(0)));
        let [
            Effect::Move {
                what: Ref::Target(0),
                to:
                    Destination::Library {
                        place: Place::Shuffled,
                        order: None,
                    },
            },
            Effect::LookAtTop { who, count, bind },
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
        let entered = Destination::Battlefield {
            tapped: false,
            under: Some(owner()),
            transformed: false,
            counters: Vec::new(),
        };
        (*who == owner() && *count == Amount::one() && *what == held && *subject == held)
            .then_some(())?;
        (otherwise.is_empty()
            && *then
                == [Effect::Move {
                    what: held,
                    to: entered,
                }])
        .then_some(())?;
        self.is_a_permanent(&Ref::Target(0)).then_some(())?;
        let kind = kind_text(selector)?;
        let target = self.name_target()?;
        Some((
            format!(
                "the owner of {target} shuffles it into their library, then reveals the top card of their library. If it's {} {kind}, they put it onto the battlefield",
                super::article(&kind)
            ),
            4,
        ))
    }
}

/// The reveal of the top card, the test of the card and the steps for each outcome; the number of effects it takes.
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
    let kind = kind_text(selector)?;
    let mut sentences = vec![
        "reveal the top card of your library".to_owned(),
        format!(
            "If it's {} {kind}, {}",
            super::article(&kind),
            steps(then, &held)?
        ),
    ];
    // "If you don't cast it, put it on the bottom" also covers a card that fails the test.
    let covered = matches!(then.as_slice(), [Effect::CastFromZone { .. }])
        && matches!(otherwise.as_slice(), [Effect::Move { what, to }] if *what == held && bottom(to));
    if !otherwise.is_empty() && !covered {
        sentences.push(format!("Otherwise, {}", steps(otherwise, &held)?));
    }
    Some((sentences.join(". "), 3))
}
