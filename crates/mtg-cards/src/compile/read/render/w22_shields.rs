//! w22: a shield for the turn that prevents all damage, or all combat damage, dealt to players and permanents, by a
//! kind of source, or both ("prevent all damage that would be dealt to creatures this turn", "prevent all combat
//! damage that would be dealt to you and planeswalkers you control this turn", "prevent all damage that creatures
//! would deal to players this turn"), one shield per recipient joined in one sentence.

use mtg_core::card::CardType;

use super::Writer;
use crate::compile::read::selectors::{Form, color_name, phrase_text};
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// The parts of a shield that prevents all damage this turn: who is protected, the sources and which damage.
struct Prevent<'a> {
    to: Option<&'a Ref>,
    source: &'a Selector,
    combat: Option<bool>,
}

/// A shield of all damage for the turn, or none for any other effect.
fn prevent(effect: &Effect) -> Option<Prevent<'_>> {
    let Effect::Shield {
        when: ReplaceWhen::Damage { to, source, combat },
        by: ReplaceBy::Prevent,
        duration: Duration::UntilEndOfTurn,
    } = effect
    else {
        return None;
    };
    Some(Prevent {
        to: to.as_ref(),
        source,
        combat: *combat,
    })
}

/// The selector without the battlefield a creature is on anyway.
fn on_battlefield(selector: &Selector) -> Selector {
    if selector.0.contains(&Pred::Type(CardType::Creature)) {
        Selector(
            selector
                .0
                .iter()
                .filter(|pred| **pred != Pred::In(ZoneKind::Battlefield))
                .cloned()
                .collect(),
        )
    } else {
        selector.clone()
    }
}

/// "black sources and red sources", "blue creatures and black creatures": a color list split into one phrase per color.
fn colors_apart(selector: &Selector) -> Option<String> {
    let at = selector
        .0
        .iter()
        .position(|pred| matches!(pred, Pred::AnyOf(parts) if parts.iter().all(|part| matches!(part, Pred::Color(_)))))?;
    let Pred::AnyOf(parts) = &selector.0[at] else {
        return None;
    };
    let mut rest = selector.0.clone();
    rest.remove(at);
    let noun = if rest.is_empty() {
        "sources".to_owned()
    } else {
        phrase_text(&Selector(rest.clone()), Form::All)?
    };
    let mut phrases = Vec::new();
    for part in parts {
        let Pred::Color(color) = part else {
            return None;
        };
        phrases.push(format!("{} {noun}", color_name(*color)));
    }
    (phrases.len() >= 2).then(|| phrases.join(" and "))
}

/// The damage in words: "damage", "combat damage", "noncombat damage".
const fn damage(combat: Option<bool>) -> &'static str {
    match combat {
        None => "damage",
        Some(true) => "combat damage",
        Some(false) => "noncombat damage",
    }
}

impl Writer<'_> {
    /// Who is protected, in words: "you", "players", "creatures you control", "target creature", "~".
    fn w22_protected(&mut self, to: &Ref) -> Option<String> {
        match to {
            Ref::You => Some("you".to_owned()),
            Ref::EachPlayer => Some("players".to_owned()),
            Ref::This => Some("~".to_owned()),
            Ref::Each(selector) if selector.0 == [Pred::This] => Some("~".to_owned()),
            Ref::Each(selector) if !selector.0.is_empty() => {
                phrase_text(&on_battlefield(selector), Form::All)
            }
            Ref::Target(0) => self.g8e_object(to),
            _ => None,
        }
    }

    /// The sources of the damage in words: "creatures your opponents control", "black sources and red sources",
    /// "target creature".
    fn w22_sources(&mut self, source: &Selector) -> Option<String> {
        if let [Pred::Is(named)] = source.0.as_slice() {
            return self.g8e_object(named);
        }
        // A source of one color, or none, in any zone: "colorless sources", "black sources".
        match source.0.as_slice() {
            [Pred::Colorless] => return Some("colorless sources".to_owned()),
            [Pred::Color(color)] => return Some(format!("{} sources", color_name(*color))),
            _ => {}
        }
        let source = on_battlefield(source);
        colors_apart(&source).or_else(|| phrase_text(&source, Form::All))
    }

    /// The shields at the start of the steps, as one sentence, and the number of steps it covers: one shield, or one
    /// per recipient of the same damage and sources.
    pub(super) fn w22_shields(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        // A target that could not be named must stay unnamed for the writers tried next.
        let named = self.named;
        let found = self.w22_shields_named(effects);
        if found.is_none() {
            self.named = named;
        }
        found
    }

    /// [`Self::w22_shields`] without the restoring of the target's name.
    fn w22_shields_named(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let first = prevent(effects.first()?)?;
        let mut recipients = Vec::new();
        let mut used = 0;
        for effect in effects {
            let Some(next) = prevent(effect) else {
                break;
            };
            if next.combat != first.combat || next.source != first.source {
                break;
            }
            let Some(to) = next.to else {
                break;
            };
            recipients.push(self.w22_protected(to)?);
            used += 1;
        }
        let kind = damage(first.combat);
        if used == 0 {
            // No recipient: the sources alone, which the older writer words.
            return None;
        }
        let to = recipients.join(" and ");
        let text = if first.source.0.is_empty() {
            format!("prevent all {kind} that would be dealt to {to} this turn")
        } else {
            let by = self.w22_sources(first.source)?;
            format!("prevent all {kind} that would be dealt to {to} this turn by {by}")
        };
        Some((text, used))
    }

    /// "prevent the next 1 damage that would be dealt to any target this turn": a number of damage to the one target,
    /// whatever it is, and "the next time a creature of the chosen type would deal damage to you this turn, prevent that
    /// damage".
    pub(super) fn w22_next(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Shield {
            when:
                ReplaceWhen::Damage {
                    to,
                    source,
                    combat: None,
                },
            by,
            duration: Duration::UntilEndOfTurn,
        } = effect
        else {
            return None;
        };
        match (by, to) {
            (ReplaceBy::PreventUpTo(most), Some(Ref::Target(0)))
                if source.0.is_empty() && *most > 0 =>
            {
                let named = self.named;
                let object = self.name_target();
                if object.is_none() {
                    self.named = named;
                }
                Some(format!(
                    "prevent the next {most} damage that would be dealt to {} this turn",
                    object?
                ))
            }
            (ReplaceBy::PreventNext, Some(Ref::You)) => {
                let who = match source.0.as_slice() {
                    [Pred::ChosenSubtype, Pred::In(ZoneKind::Battlefield)] => {
                        "a creature of the chosen type"
                    }
                    [Pred::Is(Ref::This)] => "~",
                    _ => return None,
                };
                Some(format!(
                    "the next time {who} would deal damage to you this turn, prevent that damage"
                ))
            }
            _ => None,
        }
    }

    /// The shield of the damage of some sources to nobody in particular: "prevent all damage that would be dealt this
    /// turn by creatures your opponents control", with the battlefield a creature is on left out.
    pub(super) fn w22_source_shield(&mut self, effect: &Effect) -> Option<String> {
        if let Effect::Shield {
            when: when @ ReplaceWhen::Damage { to: None, .. },
            by: ReplaceBy::PreventAnd(and),
            duration,
        } = effect
            && let [
                Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Triggering,
                },
            ] = and.as_slice()
        {
            let plain = Effect::Shield {
                when: when.clone(),
                by: ReplaceBy::Prevent,
                duration: duration.clone(),
            };
            let text = self.w22_source_shield(&plain)?;
            return Some(format!(
                "{text}. You gain life equal to the damage prevented this way"
            ));
        }
        let first = prevent(effect)?;
        if first.to.is_some() || first.source.0.is_empty() {
            return None;
        }
        if matches!(first.source.0.as_slice(), [Pred::Is(_)]) {
            return None;
        }
        let by = self.w22_sources(first.source)?;
        Some(format!(
            "prevent all {} that would be dealt this turn by {by}",
            damage(first.combat)
        ))
    }
}

/// "untap all attacking creatures and goad them. After this phase, ..." for the steps the writers join with a comma.
pub fn w22_goad_wordings(texts: &mut Vec<String>) {
    let from = ", then goad all attacking creatures and after this phase, there is an additional combat phase";
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("ntap all attacking creatures") && text.contains(from))
        .map(|text| {
            text.replace(
                from,
                " and goad them. After this phase, there is an additional combat phase",
            )
        })
        .collect();
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The other wordings of the shields written above: the recipients before or after the sources, "would deal" for
/// "would be dealt by", and the turn at either place.
pub fn w22_wordings(texts: &mut Vec<String>) {
    let mut found = Vec::new();
    for text in texts.iter() {
        // "damage that would be dealt to R this turn by S" and the same with R and S the other way round.
        for (at, _) in text.match_indices(" that would be dealt to ") {
            let head = &text[..at];
            if !head.to_lowercase().ends_with("damage") {
                continue;
            }
            let rest = &text[at + " that would be dealt to ".len()..];
            let Some(end) = rest.find(" this turn") else {
                continue;
            };
            let (to, tail) = (&rest[..end], &rest[end + " this turn".len()..]);
            if to.contains([',', '.', ';', '"']) {
                continue;
            }
            found.push(format!(
                "{head} that would be dealt this turn to {to}{tail}"
            ));
            let Some(by) = tail.strip_prefix(" by ") else {
                continue;
            };
            let stop = by.find(['.', ',', ';', '"']).unwrap_or(by.len());
            let (by, after) = (&by[..stop], &by[stop..]);
            found.push(format!(
                "{head} that would be dealt to {to} by {by} this turn{after}"
            ));
            found.push(format!(
                "{head} that would be dealt this turn to {to} by {by}{after}"
            ));
            found.push(format!(
                "{head} that {by} would deal to {to} this turn{after}"
            ));
            found.push(format!("{head} {by} would deal to {to} this turn{after}"));
        }
        // "damage that would be dealt this turn by S".
        for (at, _) in text.match_indices(" that would be dealt this turn by ") {
            let head = &text[..at];
            if !head.to_lowercase().ends_with("damage") {
                continue;
            }
            let rest = &text[at + " that would be dealt this turn by ".len()..];
            let stop = rest.find(['.', ',', ';', '"']).unwrap_or(rest.len());
            let (by, after) = (&rest[..stop], &rest[stop..]);
            if by.contains(" would ") || by.is_empty() || by.starts_with("target ") {
                continue;
            }
            found.push(format!(
                "{head} that would be dealt by {by} this turn{after}"
            ));
            found.push(format!("{head} that {by} would deal this turn{after}"));
            found.push(format!("{head} {by} would deal this turn{after}"));
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
