//! Shields of the next damage this turn: "The next time a black source of your choice would deal damage to you this
//! turn, prevent that damage." (a source chosen, then a shield of its next damage) and "The next 1 damage that would be
//! dealt to this creature this turn is dealt to target creature you control instead." (a redirection).

use super::{Effect, Ref, Writer};
use crate::compile::read::selectors::{Form, color_name, phrase_text};
use crate::compile::read::triggers::article;
use crate::modify::Duration;
use crate::refs::{Pred, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The zones a source of damage can be in (CR 115.4 aside): the battlefield, the stack and the command zone.
fn source_zones() -> Pred {
    Pred::AnyOf(vec![
        Pred::In(ZoneKind::Battlefield),
        Pred::In(ZoneKind::Stack),
        Pred::In(ZoneKind::Command),
    ])
}

/// "a source of your choice", "a black source of your choice", "an artifact source of your choice", "a source of your
/// choice of the chosen color": the source chosen from the objects that can deal damage.
fn chosen_source(from: &Ref) -> Option<String> {
    let Ref::Each(Selector(preds)) = from else {
        return None;
    };
    let (zones, kinds) = preds.split_first()?;
    (*zones == source_zones()).then_some(())?;
    if kinds == [Pred::ChosenColor] {
        return Some("a source of your choice of the chosen color".to_owned());
    }
    // Only a color or a card type: what the card names before "source".
    let color = |pred: &Pred| match pred {
        Pred::Color(color) => Some(color_name(*color)),
        _ => None,
    };
    let kind = match kinds {
        [] => String::new(),
        [Pred::Color(c)] => format!("{} ", color_name(*c)),
        [Pred::Type(_)] => format!(
            "{} ",
            phrase_text(&Selector(kinds.to_vec()), Form::Subject)?
        ),
        [Pred::AnyOf(parts)] => match parts.as_slice() {
            [first, second] => format!("{} or {} ", color(first)?, color(second)?),
            _ => return None,
        },
        _ => return None,
    };
    let source = format!("{kind}source");
    let kind = kind.trim_end();
    // A color or a type is an adjective here: "a black source", not "a black permanent source".
    if kind.contains(' ') && !kind.contains(" or ") {
        return None;
    }
    Some(format!("{} {source} of your choice", article(&source)))
}

impl Writer<'_> {
    /// A shield of the next damage this turn, with the choice of its source before it, as one sentence, and the number of
    /// effects it covers.
    pub(super) fn r9w_shields(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        match effects {
            [
                Effect::Choose {
                    who: Ref::You,
                    from,
                    count,
                    bind,
                    rest_as: None,
                },
                Effect::Shield {
                    when:
                        ReplaceWhen::Damage {
                            to,
                            source,
                            combat: None,
                        },
                    by: ReplaceBy::PreventNext,
                    duration: Duration::UntilEndOfTurn,
                },
                ..,
            ] if count.is_one() && source.0 == [Pred::Is(Ref::Bound(bind.clone()))] => {
                let chosen = chosen_source(from)?;
                let to = match to {
                    None => String::new(),
                    Some(to) => format!(" to {}", self.recipient(to)?),
                };
                Some((
                    vec![format!(
                        "the next time {chosen} would deal damage{to} this turn, prevent that damage"
                    )],
                    2,
                ))
            }
            [
                Effect::Shield {
                    when:
                        ReplaceWhen::Damage {
                            to: Some(dealt),
                            source,
                            combat: None,
                        },
                    by: ReplaceBy::RedirectUpTo { most, to },
                    duration: Duration::UntilEndOfTurn,
                },
                ..,
            ] if source.0.is_empty() && *most > 0 => {
                let dealt = self.r9w_one(dealt)?;
                let to = self.r9w_one(to)?;
                Some((
                    vec![format!(
                        "the next {most} damage that would be dealt to {dealt} this turn is dealt to {to} instead"
                    )],
                    1,
                ))
            }
            _ => None,
        }
    }

    /// The card itself or the one target, as a redirection names it.
    fn r9w_one(&mut self, what: &Ref) -> Option<String> {
        match what {
            Ref::This => Some("~".to_owned()),
            Ref::You => Some("you".to_owned()),
            Ref::Target(0) if self.single(what) => self.name_target(),
            _ => None,
        }
    }
}
