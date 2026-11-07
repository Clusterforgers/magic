//! The cost of blocking (CR 509.1d): "Creatures can't block unless their controller pays {1} for each of those
//! creatures.", "~ can't block creatures with power 3 or greater unless you pay {1}."
//!
//! A declaration is checked against the options first. Each blocker costs the mana and life of every tax in force
//! (`Rule::BlockTax`) that taxes it and an attacker it blocks, once however many attackers it blocks, and the total is
//! locked in. A declaration with a total of nothing is made at once. One with a total is legal only if its player can
//! pay it, without paying down to 0 life, and then it becomes a pending block (`What::Block`) whose payment is the
//! ordinary mana payment: the declaration is made with the payment, and backing out of the payment returns to the
//! declaration, with nothing paid. A requirement to block is never one to pay (CR 509.1d): a taxed creature is bound
//! by none.

use mtg_cards::modify::GameRule;
use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use serde::{Deserialize, Serialize};

use crate::announce::Next;
use crate::answers::Outcome;
use crate::cast::payment_decision;
use crate::decision::{Decision, DecisionKind, PayOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::help::Offered;
use crate::object::ObjectRef;
use crate::priority::mana_options;
use crate::resolve::{ResolveError, Scope};
use crate::rules::{Rule, block_taxes, repeated};
use crate::spend::Spend;
use crate::state::GameState;

/// A tax on blocking: which blockers it taxes, of which attackers, and what each costs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BlockTax {
    /// The blockers it taxes, judged when blockers are declared; every creature when empty.
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub blockers: Selector,
    /// The attackers whose blockers it taxes, judged the same way; every attacker when empty.
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub attackers: Selector,
    /// What each taxed blocker costs, counted when the rule began or the state last changed.
    pub mana: ManaCost,
    /// The life each taxed blocker costs, counted the same way.
    #[serde(default)]
    pub life: u32,
}

/// A tax on blocking in force, with whose it is.
#[derive(Debug, Clone)]
pub(crate) struct BlockTaxing {
    /// The controller of the effect or permanent that taxes, for the selectors' "you".
    pub(crate) controller: PlayerId,
    /// The object that taxes, for the selectors' "this"; none for a rule that no object began.
    pub(crate) source: Option<ObjectRef>,
    /// The tax.
    pub(crate) tax: BlockTax,
}

/// The rule of a `GameRule::BlockTax` in `scope`: the cost repeated `per` times.
pub(crate) fn lower(scope: &Scope<'_>, wanted: &GameRule) -> Result<Rule, ResolveError> {
    let GameRule::BlockTax {
        blockers,
        attackers,
        mana,
        life,
        per,
    } = wanted
    else {
        return Err(ResolveError::Invalid("not a block tax".to_owned()));
    };
    let times = usize::try_from(scope.amount(per)?.max(0)).unwrap_or(usize::MAX);
    Ok(Rule::BlockTax(BlockTax {
        blockers: blockers.clone(),
        attackers: attackers.clone(),
        mana: repeated(mana.rest(), times),
        life: life.saturating_mul(u32::try_from(times).unwrap_or(u32::MAX)),
    }))
}

/// Whether `selector` of the tax picks `object`: anything for an empty one, else what it selects for its controller.
fn picks(state: &GameState, taxing: &BlockTaxing, selector: &Selector, object: ObjectId) -> bool {
    selector.is_empty()
        || taxing
            .source
            .zip(state.object(object))
            .is_some_and(|(source, found)| {
                Scope::announcing(state, taxing.controller, source)
                    .selects(selector, found.reference())
                    .unwrap_or(false)
            })
}

/// The taxes in force that tax `blocker` as it blocks `attacker`.
fn in_force(
    state: &GameState,
    blocker: ObjectId,
    attacker: ObjectId,
) -> impl Iterator<Item = &BlockTaxing> {
    block_taxes(state).iter().filter(move |taxing| {
        picks(state, taxing, &taxing.tax.blockers, blocker)
            && picks(state, taxing, &taxing.tax.attackers, attacker)
    })
}

/// Whether blocking `attacker` with `blocker` costs anything.
pub(crate) fn taxed(state: &GameState, blocker: ObjectId, attacker: ObjectId) -> bool {
    in_force(state, blocker, attacker).next().is_some()
}

/// What each blocker of the declaration costs, in mana and life: every tax that taxes it and an attacker it blocks,
/// once each.
fn costs(state: &GameState, blocks: &[(ObjectId, ObjectId)]) -> (ManaCost, u32) {
    let mut seen: Vec<ObjectId> = Vec::new();
    let mut mana = Vec::new();
    let mut life = 0u32;
    for (blocker, _) in blocks {
        if seen.contains(blocker) {
            continue;
        }
        seen.push(*blocker);
        for taxing in block_taxes(state) {
            let hits = picks(state, taxing, &taxing.tax.blockers, *blocker)
                && blocks.iter().any(|(by, attacker)| {
                    by == blocker && picks(state, taxing, &taxing.tax.attackers, *attacker)
                });
            if hits {
                mana.extend(taxing.tax.mana.symbols().iter().copied());
                life = life.saturating_add(taxing.tax.life);
            }
        }
    }
    (ManaCost::from_symbols(mana), life)
}

/// Whether `player` can pay `life` (CR 119.4, 119.8): any total for nothing, else a life total of at least that much
/// that can change.
fn can_pay_life(state: &GameState, player: PlayerId, life: u32) -> bool {
    life == 0
        || (!crate::rules::life_total_cant_change(state, player)
            && state
                .player(player)
                .is_some_and(|seat| i64::from(seat.life()) >= i64::from(life)))
}

/// The events of a block declared and paid for: the declaration, then the life it costs.
pub(crate) fn made(
    state: &GameState,
    player: PlayerId,
    blocks: &[(ObjectId, ObjectId)],
) -> Vec<Event> {
    let mut events = vec![Event::BlockersDeclared {
        player,
        blocks: blocks.to_vec(),
    }];
    let (_, life) = costs(state, blocks);
    if life > 0 {
        events.push(Event::LifePaid {
            player,
            amount: life,
        });
    }
    events
}

/// The payment of the mana of a declaration, if its player can make it automatically; the object named is the first
/// blocker.
fn payment(
    state: &GameState,
    player: PlayerId,
    blocks: &[(ObjectId, ObjectId)],
) -> Option<PayOptions> {
    let (owed, life) = costs(state, blocks);
    can_pay_life(state, player, life).then_some(())?;
    let options = payment_decision(
        state,
        player,
        blocks.first()?.0,
        owed,
        mana_options(state, player),
        Offered::default(),
        Spend::other(),
        true,
        0,
    )?;
    options.auto_available.then_some(PayOptions {
        block: true,
        ..options
    })
}

/// What a declared block does: with nothing to pay it is made, else it is paid for first (CR 509.1d) or, if its player
/// cannot pay, refused.
pub(crate) fn declare(
    state: &GameState,
    player: PlayerId,
    blocks: Vec<(ObjectId, ObjectId)>,
) -> Result<Outcome, EngineError> {
    if block_taxes(state).is_empty() {
        return Ok(Outcome::Events(vec![Event::BlockersDeclared {
            player,
            blocks,
        }]));
    }
    let (owed, life) = costs(state, &blocks);
    if !can_pay_life(state, player, life) {
        return Err(IllegalAnswer::BlockUnaffordable(format!("{life} life")).into());
    }
    if owed.symbols().is_empty() {
        return Ok(Outcome::Events(made(state, player, &blocks)));
    }
    if payment(state, player, &blocks).is_none() {
        return Err(IllegalAnswer::BlockUnaffordable(owed.to_string()).into());
    }
    Ok(Outcome::BeginBlock(blocks))
}

/// The decision of a block that waits for its payment: `PayMana` for the total cost, which can be backed out of. A block
/// that can no longer be paid is dropped.
pub(crate) fn next(state: &GameState, player: PlayerId, blocks: &[(ObjectId, ObjectId)]) -> Next {
    match payment(state, player, blocks) {
        Some(options) => Next::Ask(Decision {
            player,
            kind: DecisionKind::PayMana(options),
        }),
        None => Next::Drop,
    }
}
