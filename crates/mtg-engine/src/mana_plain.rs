//! The automatic payment the game makes first: plain mana from the pool and from mana abilities that add mana and do
//! nothing else, and why a payment is asked when no such payment exists.

use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaSymbol};
use serde::{Deserialize, Serialize};

use crate::help::Help;
use crate::mana::{ManaOption, ManaPool, ManaSource, Payment, auto_payment_with};
use crate::spend::Spend;

/// Why the automatic payment needs more than plain mana, so "Always pay for me" asks the player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum AskedWhy {
    /// No automatic payment exists at all.
    NoPayment,
    /// The cost needs 2 life for this Phyrexian symbol (CR 107.4f).
    Life(ManaSymbol),
    /// The rest needs creatures tapped for convoke.
    Convoke,
    /// The rest needs artifacts tapped for improvise.
    Improvise,
    /// The rest needs graveyard cards exiled for delve.
    Delve,
    /// The rest needs these sources sacrificed or exiled (a Treasure, a Spirit Guide).
    GivesUp(Vec<ObjectId>),
    /// The rest needs these sources' abilities that pay life.
    PaysLife(Vec<ObjectId>),
    /// The rest needs these sources' abilities that pay energy.
    Energy(Vec<ObjectId>),
    /// The rest needs these sources' abilities that cost mana of their own (a Signet).
    PaidAbility(Vec<ObjectId>),
    /// The rest needs these sources' abilities that tap or sacrifice other permanents.
    Takes(Vec<ObjectId>),
    /// The rest needs these sources' abilities that do more than add mana (a painland's damage).
    DoesMore(Vec<ObjectId>),
}

/// The game's payment for a cost and why it is not plain: `asked` is left out exactly when `payment` uses plain mana only.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Proposal {
    /// The payment the game makes; none when no automatic payment exists.
    pub payment: Option<Payment>,
    /// Why the payment is not plain mana only.
    pub asked: Option<AskedWhy>,
}

/// Whether `option` adds mana and does nothing else: no sacrifice, exile, life, energy, own mana cost or other
/// permanents taken, and nothing done beside the mana.
pub fn is_plain(option: &ManaOption) -> bool {
    !(option.extra || option.gives_up())
        && option.life == 0
        && option.energy == 0
        && !option.is_costly()
        && option.takes.is_empty()
}

/// Whether every option of the ability `source` among `sources` is plain, so tapping it can only add plain mana.
fn plain_ability(sources: &[ManaOption], source: ManaSource) -> bool {
    sources.iter().filter(|o| o.source == source).all(is_plain)
}

/// The automatic payment with plain mana only: the pool and abilities whose options are all plain, hybrid and
/// Phyrexian symbols paid with mana, `{S}` with snow mana, no life and no help; lands before artifacts as
/// [`auto_payment_with`] chooses.
pub fn plain_payment(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    spend: &Spend,
) -> Option<Payment> {
    let plain: Vec<ManaOption> = sources
        .iter()
        .filter(|o| plain_ability(sources, o.source))
        .cloned()
        .collect();
    // A payer with no life to spare pays no Phyrexian symbol with life.
    auto_payment_with(cost, pool, &plain, Help::NONE, 0, spend)
}

/// The payment the game makes for the player: plain mana when it can ([`plain_payment`]), otherwise
/// [`auto_payment_with`], with why that one is not plain.
pub fn propose(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    help: Help<'_>,
    life: i32,
    spend: &Spend,
) -> Proposal {
    if let Some(payment) = plain_payment(cost, pool, sources, spend) {
        return Proposal {
            payment: Some(payment),
            asked: None,
        };
    }
    let payment = auto_payment_with(cost, pool, sources, help, life, spend);
    let asked = match &payment {
        Some(payment) => asked_why(cost, sources, payment),
        None => Some(AskedWhy::NoPayment),
    };
    Proposal { payment, asked }
}

/// A kind of source that is not plain: the test of its option and the reason that names its objects.
type Kind = (fn(&ManaOption) -> bool, fn(Vec<ObjectId>) -> AskedWhy);

/// What `payment` spends beside plain mana, the first of: life, help, then the sources by kind; none when it is plain.
fn asked_why(cost: &ManaCost, sources: &[ManaOption], payment: &Payment) -> Option<AskedWhy> {
    if payment.life > 0 {
        let phyrexian = cost.symbols().iter().copied().find(|s| {
            matches!(
                s,
                ManaSymbol::Phyrexian(_)
                    | ManaSymbol::HybridPhyrexian(..)
                    | ManaSymbol::ColorlessPhyrexian
            )
        });
        return phyrexian.map(AskedWhy::Life);
    }
    if !payment.convoke.is_empty() {
        return Some(AskedWhy::Convoke);
    }
    if !payment.improvise.is_empty() {
        return Some(AskedWhy::Improvise);
    }
    if !payment.delve.is_empty() {
        return Some(AskedWhy::Delve);
    }
    let used: Vec<&ManaOption> = payment
        .taps
        .iter()
        .filter_map(|tap| {
            let mut options = sources.iter().filter(|o| o.source == *tap);
            options
                .clone()
                .find(|o| !is_plain(o))
                .or_else(|| options.next())
        })
        .collect();
    let kinds: [Kind; 6] = [
        (ManaOption::gives_up, AskedWhy::GivesUp),
        (|o| o.life > 0, AskedWhy::PaysLife),
        (|o| o.energy > 0, AskedWhy::Energy),
        (ManaOption::is_costly, AskedWhy::PaidAbility),
        (|o| !o.takes.is_empty(), AskedWhy::Takes),
        (|o| o.extra, AskedWhy::DoesMore),
    ];
    kinds.into_iter().find_map(|(holds, why)| {
        let mut objects: Vec<ObjectId> = used
            .iter()
            .filter(|o| holds(o))
            .map(|o| o.source.object)
            .collect();
        objects.dedup();
        (!objects.is_empty()).then(|| why(objects))
    })
}
