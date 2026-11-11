//! Secret numbers (Wheel of Misfortune): "Each player secretly chooses a number 0 or greater, then all players reveal
//! those numbers simultaneously and determine the highest and lowest numbers revealed this way. ~ deals damage equal to
//! the highest number to each player who chose that number. Each player who didn't choose the lowest number discards
//! their hand, then draws seven cards." is a `ChooseNumbers` and the three effects that read its names.

use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Condition};

/// The name the highest number is remembered under.
const NUMBER: &str = "number";
/// The name of the players who chose the highest number.
const HIGHEST: &str = "highest";
/// The name of the players who chose the lowest number.
const LOWEST: &str = "lowest";

/// Every card in every hand: what "discards their hand" takes from each player.
fn hands() -> Amount {
    Amount::Count(Selector(vec![
        Pred::In(ZoneKind::Hand),
        Pred::OwnedBy(Ref::EachPlayer),
    ]))
}

/// The players who did not choose the lowest number.
fn not_lowest() -> Ref {
    Ref::PlayersWhere {
        who: Box::new(Ref::EachPlayer),
        test: Box::new(Condition::Not(Box::new(Condition::Among(Ref::Bound(
            LOWEST.to_owned(),
        ))))),
    }
}

/// The four effects of the text.
fn effects() -> Vec<Effect> {
    vec![
        Effect::ChooseNumbers {
            who: Ref::EachPlayer,
            min: Amount::zero(),
            secret: true,
            bind: NUMBER.to_owned(),
            highest: HIGHEST.to_owned(),
            lowest: LOWEST.to_owned(),
        },
        Effect::DealDamage {
            source: None,
            to: Ref::Bound(HIGHEST.to_owned()),
            amount: Amount::ChosenAs(NUMBER.to_owned()),
            excess: None,
            unpreventable: false,
        },
        Effect::Discard {
            who: not_lowest(),
            count: hands(),
            random: false,
            filter: None,
            by: None,
            optional: false,
            bind: None,
        },
        Effect::Draw {
            who: not_lowest(),
            count: Amount::Fixed(7),
            bind: None,
        },
    ]
}

/// The sentences of the text, without the final period, as `dk27_numbers_text` writes them.
const SENTENCES: [&str; 3] = [
    "each player secretly chooses a number 0 or greater, then all players reveal those numbers simultaneously and determine the highest and lowest numbers revealed this way",
    "~ deals damage equal to the highest number to each player who chose that number",
    "each player who didn't choose the lowest number discards their hand, then draws seven cards",
];

/// Reads the three sentences, leaving the final period.
pub(super) fn dk27_numbers(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("each player secretly chooses a number")?;
    (r.number()? == 0).then_some(())?;
    r.need("or greater")?;
    r.punct(',').then_some(())?;
    r.need("then all players reveal those numbers simultaneously and determine the highest and lowest numbers revealed this way")?;
    r.punct('.').then_some(())?;
    r.source().then_some(())?;
    r.need("deals damage equal to the highest number to each player who chose that number")?;
    r.punct('.').then_some(())?;
    r.need("each player who didn't choose the lowest number discards their hand")?;
    r.punct(',').then_some(())?;
    r.need("then draws")?;
    (r.take_word()? == "seven").then_some(())?;
    r.need("cards")?;
    Some(effects())
}

/// The sentences of the four effects at the head of `effects` when they are the ones [`dk27_numbers`] reads, and how many
/// they cover.
pub(super) fn dk27_numbers_text(effects_written: &[Effect]) -> Option<(Vec<String>, usize)> {
    let wanted = effects();
    (effects_written.get(..wanted.len())? == wanted.as_slice()).then(|| {
        (
            SENTENCES
                .iter()
                .map(|sentence| (*sentence).to_owned())
                .collect(),
            wanted.len(),
        )
    })
}
