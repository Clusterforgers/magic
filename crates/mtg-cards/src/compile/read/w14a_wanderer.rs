//! The loyalty abilities of The Eternal Wanderer, written: an exile whose card returns "at the beginning of that
//! player's next end step" (a delayed trigger at the end step of the exiled card's owner) and "For each player, choose a
//! creature that player controls. Each player sacrifices all creatures they control not chosen this way." (a pass for
//! each player that chooses one of their creatures and sacrifices the rest).

use crate::effect::Effect;
use mtg_core::card::CardType;

use super::tokens::Reader;
use crate::refs::{Bounds, Pred, Ref, Selector};
use crate::value::Amount;

/// The name each pass binds to its player.
pub const W14A_PLAYER: &str = "player";

/// The end step of a delayed return: "the next end step" of anyone, or "that player's next end step" of the owner of the
/// exiled card `exiled`.
pub fn w14a_end_step(whose: &Ref, exiled: &str) -> Option<&'static str> {
    match whose {
        Ref::EachPlayer => Some("the next end step"),
        Ref::OwnerOf(card) if **card == Ref::Bound(exiled.to_owned()) => {
            Some("that player's next end step")
        }
        _ => None,
    }
}

/// "for each player, choose a creature that player controls. Each player sacrifices all creatures they control not
/// chosen this way": a pass for each player in which you choose one of their creatures and they sacrifice the rest.
pub fn w14a_keep_one_text(effect: &Effect) -> Option<String> {
    let Effect::RepeatEach {
        over: Ref::EachPlayer,
        bind,
        effects,
    } = effect
    else {
        return None;
    };
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(Selector(from)),
            count,
            rest_as: Some(rest),
            ..
        },
        Effect::Sacrifice {
            who: Ref::Bound(sacrificer),
            filter: Selector(filter),
            count: Amount::Count(Selector(counted)),
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    let player = Ref::Bound(bind.clone());
    let theirs = [
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(player.clone()),
    ];
    let the_rest = [Pred::Is(Ref::Bound(rest.clone()))];
    (count.is_one()
        && *sacrificer == *bind
        && from.as_slice() == theirs
        && filter.as_slice() == the_rest
        && counted.as_slice() == the_rest)
        .then(|| {
            "for each player, choose a creature that player controls. Each player sacrifices all creatures they \
             control not chosen this way"
                .to_owned()
        })
}

/// "For each player, choose a creature that player controls. Each player sacrifices all creatures they control not
/// chosen this way", read into the pass `w14a_keep_one_text` writes.
pub fn w14a_keep_one(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("for each player")?;
    r.punct(',').then_some(())?;
    r.need("choose a creature that player controls")?;
    r.punct('.').then_some(())?;
    r.need("each player sacrifices all creatures they control not chosen this way")?;
    let player = Ref::Bound(W14A_PLAYER.to_owned());
    let rest = [Pred::Is(Ref::Bound("rest".to_owned()))];
    Some(vec![Effect::RepeatEach {
        over: Ref::EachPlayer,
        bind: W14A_PLAYER.to_owned(),
        effects: vec![
            Effect::Choose {
                who: Ref::You,
                from: Ref::Each(Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::ControlledBy(player.clone()),
                ])),
                count: Bounds::default(),
                bind: "kept".to_owned(),
                rest_as: Some("rest".to_owned()),
            },
            Effect::Sacrifice {
                who: player,
                filter: Selector(rest.to_vec()),
                count: Amount::Count(Selector(rest.to_vec())),
            },
        ],
    }])
}
