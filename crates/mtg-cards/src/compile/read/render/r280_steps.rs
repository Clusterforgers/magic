//! Steps the card pack r28-0 writes as one run of sentences: a card a player reveals at random from their hand and the
//! steps that read its mana value ("Target opponent reveals a card at random from their hand. You gain life equal to
//! that card's mana value."), and the skip of the controller's next turn ("You skip your next turn.").

use super::{Effect, Ref, Writer};
use crate::modify::{Duration, GameRule, Prohibited};
use crate::refs::{Pred, ZoneKind};
use crate::value::{Amount, StatKind};

/// The words of an amount equal to the revealed card's mana value.
const EQUAL: &str = "equal to that card's mana value";

impl Writer<'_> {
    /// A card revealed at random from a player's hand and the steps after it that read only its mana value: the
    /// sentences and how many steps they cover.
    pub(super) fn r280_random_reveal(
        &mut self,
        effects: &[Effect],
    ) -> Option<(Vec<String>, usize)> {
        let [
            Effect::ChooseAtRandom {
                from: Ref::Each(selector),
                count: Amount::Fixed(1),
                bind,
            },
            Effect::Reveal {
                what: Ref::Bound(revealed),
            },
            rest @ ..,
        ] = effects
        else {
            return None;
        };
        (revealed == bind).then_some(())?;
        let [Pred::OwnedBy(who), Pred::In(ZoneKind::Hand)] = selector.0.as_slice() else {
            return None;
        };
        let valued = valued_json(bind)?;
        let player = self.player(who)?;
        let mut sentences = vec![format!("{player} reveals a card at random from their hand")];
        for effect in rest {
            let text = serde_json::to_string(effect).ok()?;
            let named = serde_json::to_string(&Ref::Bound(bind.clone())).ok()?;
            if text.matches(&named).count() != text.matches(&valued).count() {
                return None;
            }
            if !text.contains(&valued) {
                sentences.push(self.effect(effect)?);
                continue;
            }
            if let Effect::DealDamage {
                source: None,
                to: Ref::Target(0),
                ..
            } = effect
                && self.named
            {
                sentences.push(format!("~ deals damage {EQUAL} to that player"));
                continue;
            }
            let as_x: Effect = serde_json::from_str(&text.replace(&valued, "\"X\"")).ok()?;
            let plain = self.effect(&as_x)?;
            sentences.push(said_of_the_revealed(&plain)?);
        }
        Some((sentences, effects.len()))
    }
}

/// The JSON of the mana value of the bound card.
fn valued_json(bind: &str) -> Option<String> {
    serde_json::to_string(&Amount::Stat {
        of: Ref::Bound(bind.to_owned()),
        stat: StatKind::ManaValue,
    })
    .ok()
}

/// A sentence of the plain writer with X for the revealed card's mana value, said of that card: "gain life equal to that
/// card's mana value", "deals damage equal to that card's mana value to that player", "gets +X/+X until end of turn, where
/// X is the revealed card's mana value".
fn said_of_the_revealed(plain: &str) -> Option<String> {
    for verb in [" gain X life", " gains X life"] {
        if let Some(who) = plain.strip_suffix(verb) {
            return Some(format!("{who}{} life {EQUAL}", &verb[..verb.len() - 7]));
        }
    }
    if let Some((source, to)) = plain.split_once(" deals X damage to ") {
        let to = if to.starts_with("target ") {
            "that player"
        } else {
            to
        };
        return Some(format!("{source} deals damage {EQUAL} to {to}"));
    }
    plain
        .contains('X')
        .then(|| format!("{plain}, where X is the revealed card's mana value"))
}

impl Writer<'_> {
    /// "You skip your next turn.": a rule that makes the controller skip the next turn they would take.
    pub(super) fn r280_skip_turn(effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule:
                GameRule::Cant {
                    who: Ref::You,
                    action: Prohibited::SkipTurn,
                },
            duration: Duration::UntilTurnSkipped,
        } = effect
        else {
            return None;
        };
        Some("you skip your next turn".to_owned())
    }
}
