//! Cards that change zones by a choice: "Return up to two target creature cards from your graveyard to the battlefield
//! tapped.", "Put target creature card from a graveyard onto the battlefield under your control.", "Exile target
//! player's graveyard." and "Put a land card from your hand onto the battlefield tapped.", each read from its wording
//! and written back the same way (`render.rs`).

use super::selectors::{Form, phrase, target};
use super::tokens::Reader;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};

/// The name the card chosen from the hand is bound to.
pub const CHOSEN: &str = "card";

/// The selector of the one target of the clause, when it names cards in a graveyard.
fn graveyard_target(r: &Reader<'_>) -> Option<Selector> {
    match r.targets.first().map(|spec| &spec.what) {
        Some(Targetable::Object(selector))
            if selector.0.contains(&Pred::In(ZoneKind::Graveyard)) =>
        {
            Some(selector.clone())
        }
        _ => None,
    }
}

/// "Return up to two target creature cards from your graveyard to the battlefield tapped.", "Return target permanent
/// card from your graveyard to your hand.", "Put target creature card from a graveyard onto the battlefield under your
/// control.": the cards of a graveyard go to their owner's hand or the battlefield, or under the control of the player
/// who takes them from a graveyard that is not theirs.
pub fn from_graveyard(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let put = if r.word("return") {
        false
    } else {
        r.need("put")?;
        true
    };
    let what = target(r)?;
    let selector = graveyard_target(r)?;
    let yours = selector.0.contains(&Pred::OwnedBy(Ref::You));
    let to = if put {
        // Only a card of another graveyard needs the words "under your control".
        (!yours).then_some(())?;
        r.need("onto the battlefield")?;
        let tapped = r.word("tapped");
        r.need("under your control")?;
        battlefield(tapped, Some(Ref::You))
    } else {
        // A card goes to its owner's hand and under its owner's control: your own graveyard.
        yours.then_some(())?;
        if r.words("to your hand") {
            Destination::Hand
        } else {
            r.need("to the battlefield")?;
            battlefield(r.word("tapped"), None)
        }
    };
    Some(vec![Effect::Move { what, to }])
}

/// A destination on the battlefield with nothing else said.
fn battlefield(tapped: bool, under: Option<Ref>) -> Destination {
    Destination::Battlefield {
        tapped,
        under,
        transformed: false,
        counters: Vec::new(),
    }
}

/// "Exile target player's graveyard.", "Exile target opponent's graveyard.", "Exile your graveyard.", "Exile each
/// opponent's graveyard.": every card in the graveyards named.
pub fn exile_graveyard(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("exile")?;
    let owner = if r.words("your graveyard") {
        Ref::You
    } else if r.words("each opponent's graveyard") {
        Ref::Opponents
    } else {
        r.need("target")?;
        let what = if r.word("player's") {
            Targetable::Player
        } else {
            r.need("opponent's")?;
            Targetable::Opponent
        };
        r.need("graveyard")?;
        r.targets.is_empty().then_some(())?;
        r.targets.push(TargetSpec {
            what,
            bounds: Bounds::default(),
            divide: None,
        });
        Ref::Target(0)
    };
    Some(vec![Effect::Exile {
        what: Ref::Each(Selector(vec![
            Pred::In(ZoneKind::Graveyard),
            Pred::OwnedBy(owner),
        ])),
        bind: None,
        duration: None,
    }])
}

/// "You may put a land card from your hand onto the battlefield tapped.": the player may choose a card of the hand, bound
/// as `card`, and it goes onto the battlefield.
pub fn may_put_from_hand(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may put")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let found = phrase(r, Form::Subject)?;
    // The files list the zone first.
    let zone = [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)];
    zone.iter()
        .all(|pred| found.0.contains(pred))
        .then_some(())?;
    r.need("onto the battlefield")?;
    let tapped = r.word("tapped");
    let mut preds: Vec<Pred> = zone.to_vec();
    preds.extend(found.0.into_iter().filter(|pred| !zone.contains(pred)));
    Some(vec![Effect::May {
        who: Ref::You,
        effects: vec![
            Effect::Choose {
                who: Ref::You,
                from: Ref::Each(Selector(preds)),
                count: Bounds::default(),
                bind: CHOSEN.to_owned(),
                rest_as: None,
            },
            Effect::Move {
                what: Ref::Bound(CHOSEN.to_owned()),
                to: battlefield(tapped, None),
            },
        ],
    }])
}
