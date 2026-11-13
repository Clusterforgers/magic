//! r47-10 steps written back: "you can't become the monarch this turn" (Jared Carthalion, True Heir), and another
//! player's sacrifice followed by what happens if they do ("defending player sacrifices a creature of their choice. If
//! they do, you create a Walker token.", Gisa's Favorite Shovel), and the top card revealed and put onto the
//! battlefield if the player wishes and it is of a kind, else into the hand (Matter Reshaper), and cards cloaked or
//! manifested from the hand (Vannifar, Evolved Enigma).

use super::Writer;
use crate::effect::{Destination, Effect};
use crate::modify::{Duration, GameRule, Prohibited};
use crate::refs::{Pred, Ref, ZoneKind};
use crate::value::{Amount, Cmp, Condition};

impl Writer<'_> {
    /// "you can't become the monarch this turn": a prohibition of the player for the turn.
    pub(super) fn r4710_no_monarch(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule:
                GameRule::Cant {
                    who,
                    action: Prohibited::BecomeMonarch,
                },
            duration: Duration::UntilEndOfTurn,
        } = effect
        else {
            return None;
        };
        let player = self.player(who)?;
        Some(format!("{player} can't become the monarch this turn"))
    }

    /// "defending player sacrifices a creature of their choice. If they do, you create a Walker token": another player's
    /// sacrifice bound by name, then steps done only when it took something; the two steps it takes.
    pub(super) fn r4710_if_they_do(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            sacrifice @ Effect::Sacrifice {
                who,
                bind: Some(name),
                ..
            },
            Effect::If {
                condition:
                    Condition::Compare {
                        left: Amount::Bound(moved),
                        op: Cmp::Ge,
                        right: Amount::Fixed(1),
                    },
                then,
                otherwise,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        if *who == Ref::You
            || *moved != format!("{name}~moved")
            || !otherwise.is_empty()
            || then.is_empty()
        {
            return None;
        }
        let sacrificed = self.effect(sacrifice)?;
        let mut done = Vec::new();
        for effect in then {
            let text = self.effect(effect)?;
            // A step of yours after the other player's needs its subject.
            done.push(
                if text.starts_with("create ") || text.starts_with("draw ") {
                    format!("you {text}")
                } else {
                    text
                },
            );
        }
        Some((
            format!("{sacrificed}. If they do, {}", done.join(", then ")),
            2,
        ))
    }
}

/// "reveal the top card of your library. You may put that card onto the battlefield if it's a permanent card with mana
/// value 3 or less. Otherwise, put that card into your hand": a card that may enter if it is of the kind and goes to the
/// hand when it did not; the four steps it takes.
pub(super) fn r4710_may_put_revealed(effects: &[Effect]) -> Option<(String, usize)> {
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
        Effect::Move {
            what: last,
            to: Destination::Hand,
            bind: None,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let held = Ref::Bound(bind.clone());
    (*count == Amount::one()
        && *what == held
        && *subject == held
        && *last == held
        && otherwise.is_empty())
    .then_some(())?;
    let [
        Effect::May {
            who: Ref::You,
            effects,
        },
    ] = then.as_slice()
    else {
        return None;
    };
    let [
        Effect::Move {
            what: entering,
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters,
                },
            bind: None,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    (*entering == held && counters.is_empty()).then_some(())?;
    let kind = super::revealed::kind_text(selector)?;
    Some((
        format!(
            "reveal the top card of your library. You may put that card onto the battlefield if it's {} {kind}. Otherwise, put that card into your hand",
            super::article(&kind)
        ),
        4,
    ))
}

/// "cloak a card from your hand", "manifest two cards from your hand": the cards chosen from the hand of the player who
/// puts them down, then put down by name; the two steps it takes.
pub(super) fn r4710_from_hand(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(selector),
            count,
            bind,
            rest_as: None,
        },
        put @ (Effect::Manifest { .. } | Effect::Cloak { .. }),
        ..,
    ] = effects
    else {
        return None;
    };
    let (Effect::Manifest {
        who: Ref::You,
        cards: Some(Ref::Bound(name)),
        bind: None,
        ..
    }
    | Effect::Cloak {
        who: Ref::You,
        cards: Some(Ref::Bound(name)),
        bind: None,
        ..
    }) = put
    else {
        return None;
    };
    let hand = [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)];
    if name != bind || selector.0.as_slice() != hand || count.max.as_ref() != Some(&count.min) {
        return None;
    }
    let verb = if matches!(put, Effect::Cloak { .. }) {
        "cloak"
    } else {
        "manifest"
    };
    let cards = match count.min {
        Amount::Fixed(1) => "a card".to_owned(),
        Amount::Fixed(n) => format!("{} cards", super::count_word(u32::try_from(n).ok()?)?),
        _ => return None,
    };
    Some((format!("{verb} {cards} from your hand"), 2))
}
