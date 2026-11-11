//! Bidding life (Pain's Reward): "Each player may bid life. You start the bidding with a bid of any number. In turn
//! order, each player may top the high bid. The bidding ends if the high bid stands. The high bidder loses life equal to
//! the high bid and draws four cards." is a `BidLife` and the loss and the draw of the high bidder.

use super::amounts::{count_word, number_word};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// The name the high bid is remembered under.
const BID: &str = "bid";
/// The name of the high bidder.
const BIDDER: &str = "bidder";

/// The three effects, the high bidder drawing `cards`.
fn effects(cards: u32) -> Vec<Effect> {
    let bidder = Ref::Bound(BIDDER.to_owned());
    vec![
        Effect::BidLife {
            who: Ref::EachPlayer,
            start: None,
            bind: BID.to_owned(),
            winner: BIDDER.to_owned(),
        },
        Effect::LoseLife {
            who: bidder.clone(),
            amount: Amount::ChosenAs(BID.to_owned()),
        },
        Effect::Draw {
            who: bidder,
            count: Amount::Fixed(i32::try_from(cards).unwrap_or(i32::MAX)),
            bind: None,
        },
    ]
}

/// The sentences of the bidding, without the final period.
const BIDDING: [&str; 4] = [
    "each player may bid life",
    "you start the bidding with a bid of any number",
    "in turn order, each player may top the high bid",
    "the bidding ends if the high bid stands",
];

/// Reads the five sentences, leaving the final period.
pub(super) fn dk27_bid(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("each player may bid life")?;
    r.punct('.').then_some(())?;
    r.need("you start the bidding with a bid of any number")?;
    r.punct('.').then_some(())?;
    r.need("in turn order")?;
    r.punct(',').then_some(())?;
    r.need("each player may top the high bid")?;
    r.punct('.').then_some(())?;
    r.need("the bidding ends if the high bid stands")?;
    r.punct('.').then_some(())?;
    r.need("the high bidder loses life equal to the high bid and draws")?;
    let cards = number_word(r.take_word()?).filter(|n| *n > 1)?;
    r.need("cards")?;
    Some(effects(cards))
}

/// The sentences of the three effects at the head of `written` when they are the ones [`dk27_bid`] reads, and how many
/// they cover.
pub(super) fn dk27_bid_text(written: &[Effect]) -> Option<(Vec<String>, usize)> {
    let [_, _, Effect::Draw { count, .. }, ..] = written else {
        return None;
    };
    let cards = count.fixed().filter(|n| *n > 1)?;
    let wanted = effects(cards);
    (written.get(..wanted.len())? == wanted.as_slice()).then_some(())?;
    let mut sentences: Vec<String> = BIDDING.iter().map(|s| (*s).to_owned()).collect();
    sentences.push(format!(
        "the high bidder loses life equal to the high bid and draws {} cards",
        count_word(cards)?
    ));
    Some((sentences, wanted.len()))
}
