//! g8e: a shield for the turn that prevents the damage dealt to one object or by it ("prevent all combat damage that
//! would be dealt to it this turn", "prevent all damage that would be dealt by target creature this turn"), and the two
//! joined ("prevent all combat damage that would be dealt to and dealt by ~ this turn").

use super::Writer;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, Ref, Targetable};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// Which damage of an object a shield prevents.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The damage dealt to it.
    To,
    /// The damage it deals.
    By,
}

/// The object a shield for the turn of all damage, or of all combat damage, is about, and which of its damage it
/// prevents; none for any other effect.
fn shield_side(effect: &Effect) -> Option<(&Ref, Side, Option<bool>)> {
    let Effect::Shield {
        when: ReplaceWhen::Damage { to, source, combat },
        by: ReplaceBy::Prevent,
        duration: Duration::UntilEndOfTurn,
    } = effect
    else {
        return None;
    };
    if matches!(combat, Some(false)) {
        return None;
    }
    match (to, source.0.as_slice()) {
        (Some(named), []) => Some((named, Side::To, *combat)),
        (None, [Pred::Is(named)]) => Some((named, Side::By, *combat)),
        _ => None,
    }
}

/// "damage" or "combat damage".
const fn damage(combat: Option<bool>) -> &'static str {
    match combat {
        Some(true) => "combat damage",
        _ => "damage",
    }
}

impl Writer<'_> {
    /// The one object a shield names: "~", "enchanted creature", "it" for the target named before, "target creature".
    fn g8e_object(&mut self, named: &Ref) -> Option<String> {
        match named {
            Ref::This => Some("~".to_owned()),
            Ref::Attached => Some("enchanted creature".to_owned()),
            Ref::Target(0) if self.target_is(|what| matches!(what, Targetable::Object(_))) => {
                if self.named {
                    self.single(named).then(|| "it".to_owned())
                } else {
                    self.name_target()
                }
            }
            _ => None,
        }
    }

    /// "prevent all combat damage that would be dealt to it this turn", "prevent all combat damage that would be dealt
    /// by target creature this turn". All damage a named object would deal is the older writer's ("prevent all damage
    /// target creature would deal this turn").
    pub(super) fn g8e_shield(&mut self, effect: &Effect) -> Option<String> {
        let (named, side, combat) = shield_side(effect)?;
        if side == Side::By && combat.is_none() && !matches!(named, Ref::This | Ref::Attached) {
            return None;
        }
        let object = self.g8e_object(named)?;
        let side = match side {
            Side::To => "to",
            Side::By => "by",
        };
        Some(format!(
            "prevent all {} that would be dealt {side} {object} this turn",
            damage(combat)
        ))
    }

    /// The shields of the damage dealt to an object and by it, of the same kind, as one sentence: "prevent all combat
    /// damage that would be dealt to and dealt by ~ this turn".
    pub(super) fn g8e_shield_pair(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [first, second, ..] = effects else {
            return None;
        };
        let (to, Side::To, combat) = shield_side(first)? else {
            return None;
        };
        let (by, Side::By, also) = shield_side(second)? else {
            return None;
        };
        (to == by && combat == also).then_some(())?;
        let object = self.g8e_object(to)?;
        Some((
            format!(
                "prevent all {} that would be dealt to and dealt by {object} this turn",
                damage(combat)
            ),
            2,
        ))
    }
}

/// The Oracle wordings of a shield of the damage an object deals that the writers give in other words: "that would be
/// dealt this turn by target creature" and "target creature would deal this turn" for "that would be dealt by target
/// creature this turn".
pub fn g8e_wordings(texts: &mut Vec<String>) {
    let mut found = Vec::new();
    for text in texts.iter() {
        for (start, _) in text.match_indices(" that would be dealt by ") {
            let head = &text[..start];
            let rest = &text[start + " that would be dealt by ".len()..];
            let Some(end) = rest.find(" this turn") else {
                continue;
            };
            let object = &rest[..end];
            let tail = &rest[end + " this turn".len()..];
            if object.contains([',', '.', ';', '"'])
                || object.contains(" would ")
                || !head.to_lowercase().ends_with("damage")
            {
                continue;
            }
            found.push(format!(
                "{head} that would be dealt this turn by {object}{tail}"
            ));
            found.push(format!("{head} {object} would deal this turn{tail}"));
        }
        for (start, _) in text.match_indices(" would deal this turn") {
            let head = &text[..start];
            let tail = &text[start + " would deal this turn".len()..];
            let Some(at) = head.rfind("damage ") else {
                continue;
            };
            let object = &head[at + "damage ".len()..];
            if object.is_empty() || object.contains([',', '.', ';', '"']) {
                continue;
            }
            found.push(format!(
                "{} that would be dealt this turn by {object}{tail}",
                &head[..at + "damage".len()]
            ));
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
