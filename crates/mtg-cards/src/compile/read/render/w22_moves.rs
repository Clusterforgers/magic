//! w22: cards put into a graveyard from the battlefield this turn and returned ("return to your hand all creature
//! cards in your graveyard that were put there from the battlefield this turn", "choose target creature card in a
//! graveyard that was put there from the battlefield this turn. Put that card onto the battlefield under your
//! control"), and the card with the ability or the card it enchants put into a library or into a hand.

use super::Writer;
use crate::ability::Ability;
use crate::compile::read::library;
use crate::effect::{Destination, Effect, Place};
use crate::refs::{Pred, Ref, ZoneKind};

/// What the sentence says last about the cards: they were put there from the battlefield this turn.
const PUT_THERE: &str = " put there from the battlefield this turn";

/// The other wordings of a return of cards that were put into a graveyard from the battlefield this turn: the
/// destination before the cards, and the cards chosen in a sentence before the move.
pub fn w22_graved_wordings(texts: &mut Vec<String>) {
    let mut found = Vec::new();
    for text in texts.iter() {
        let Some(at) = text.find(PUT_THERE) else {
            continue;
        };
        let end = at + PUT_THERE.len();
        let (before, tail) = (&text[..end], &text[end..]);
        let lower = before[..at].to_lowercase();
        let Some((start, verb)) = ["return ", "put "]
            .iter()
            .filter_map(|verb| lower.rfind(verb).map(|found| (found, *verb)))
            .max_by_key(|(found, _)| *found)
        else {
            continue;
        };
        let head = &before[..start];
        let cards = &before[start + verb.len()..];
        if cards.contains(['.', ';', ':']) {
            continue;
        }
        let opens = head.is_empty() || head.ends_with(": ");
        let plural = cards.contains(" were put there");
        let stop = tail.find(['.', ',', ';']).unwrap_or(tail.len());
        let (motion, after) = (&tail[..stop], &tail[stop..]);
        let chosen = cards.starts_with("target ") || cards.starts_with("up to ");
        match (
            verb,
            motion
                .strip_prefix(" to ")
                .or_else(|| motion.strip_prefix(" onto ")),
        ) {
            ("return ", Some(dest)) => {
                found.push(format!(
                    "{head}{} to {dest} {cards}{after}",
                    lead("return", opens)
                ));
                // The cards of your own graveyard are said by where they were put.
                let own = " in your graveyard that were put there from the battlefield this turn";
                if let Some(kind) = cards.strip_suffix(own) {
                    let alt = format!(
                        "{kind} that were put into your graveyard from the battlefield this turn"
                    );
                    found.push(format!(
                        "{head}{} to {dest} {alt}{after}",
                        lead("return", opens)
                    ));
                    found.push(format!(
                        "{head}{} {alt} to {dest}{after}",
                        lead("return", opens)
                    ));
                }
                if chosen {
                    let them = if plural { "them" } else { "it" };
                    found.push(format!(
                        "{head}{} {cards}. Return {them} to {dest}{after}",
                        lead("choose", opens)
                    ));
                }
            }
            ("put ", Some(dest)) => {
                found.push(format!(
                    "{head}{} onto {dest} {cards}{after}",
                    lead("put", opens)
                ));
                if chosen {
                    let them = if plural { "them" } else { "that card" };
                    found.push(format!(
                        "{head}{} {cards}. Put {them} onto {dest}{after}",
                        lead("choose", opens)
                    ));
                }
            }
            _ => {}
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// A verb that opens a sentence of the ability has a capital.
fn lead(verb: &str, opens: bool) -> String {
    if !opens {
        return verb.to_owned();
    }
    let mut chars = verb.chars();
    chars
        .next()
        .map(|first| format!("{}{}", first.to_uppercase(), chars.as_str()))
        .unwrap_or_default()
}

/// The card of an ability that puts itself into the library from the graveyard says so: "put this card from your
/// graveyard into your library third from the top"; the plain wording is for the battlefield alone.
pub fn w22_library_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated {
        from: ZoneKind::Graveyard,
        effects,
        ..
    } = ability
    else {
        return;
    };
    if !matches!(
        effects.as_slice(),
        [Effect::Move {
            what: Ref::This,
            to: Destination::Library { .. },
            ..
        }]
    ) {
        return;
    }
    let moved: Vec<String> = texts
        .iter()
        .filter(|text| text.to_lowercase().contains("put ~ into "))
        .map(|text| {
            let mut text = text
                .replace("put ~ into", "put ~ from your graveyard into")
                .replace("Put ~ into", "Put ~ from your graveyard into");
            for owner in ["its", "her", "his"] {
                text = text.replace(&format!(" {owner} owner's library"), " your library");
            }
            text
        })
        .flat_map(|text| {
            // The zone says it: the clause does not add that the ability works only there.
            let plain = text.replace(", then activate only if it is in your graveyard", "");
            let card = plain
                .replace("put ~ from", "put this card from")
                .replace("Put ~ from", "Put this card from");
            [text, plain, card]
        })
        .collect();
    texts.retain(|text| !text.to_lowercase().contains("put ~ "));
    for text in moved {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

impl Writer<'_> {
    /// The card with the ability or the card it enchants, put into a library or into a hand: "put ~ on top of its
    /// owner's library", "put enchanted creature into its owner's library third from the top", "return enchanted
    /// creature to its owner's hand".
    pub(super) fn w22_move(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Move { what, to, .. } = effect else {
            return None;
        };
        if let (
            Ref::Each(group),
            Destination::Library {
                place: Place::Shuffled,
                order: None,
            },
        ) = (what, to)
        {
            // The cards of every hand and graveyard go to their owners' libraries, which are shuffled.
            let zones = match group.0.as_slice() {
                [
                    Pred::In(ZoneKind::Graveyard),
                    Pred::OwnedBy(Ref::EachPlayer),
                ]
                | [
                    Pred::OwnedBy(Ref::EachPlayer),
                    Pred::In(ZoneKind::Graveyard),
                ] => "graveyard",
                [Pred::AnyOf(parts)]
                    if parts.as_slice()
                        == [Pred::In(ZoneKind::Hand), Pred::In(ZoneKind::Graveyard)] =>
                {
                    "hand and graveyard"
                }
                _ => return None,
            };
            return Some(format!(
                "each player shuffles their {zones} into their library"
            ));
        }
        let object = match what {
            Ref::This => "~",
            Ref::Attached => "enchanted creature",
            _ => return None,
        };
        match to {
            Destination::Hand if *what == Ref::Attached => {
                Some(format!("return {object} to its owner's hand"))
            }
            Destination::Library {
                place: Place::Shuffled,
                order: None,
            } if *what == Ref::Attached => {
                Some(format!("shuffle {object} into its owner's library"))
            }
            Destination::Library {
                place: Place::Below(below),
                order: None,
            } => Some(format!(
                "put {object} into its owner's library {}",
                library::below_text(*below)?
            )),
            Destination::Library {
                place: place @ (Place::Top | Place::Bottom),
                order: None,
            } => Some(format!(
                "put {object} {} its owner's library",
                library::place_text(*place)?
            )),
            _ => None,
        }
    }
}
