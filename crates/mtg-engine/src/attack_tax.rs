//! The cost of attacking (CR 508.1h-i): "Creatures can't attack you unless their controller pays {2} for each creature
//! they control that's attacking you."
//!
//! A declaration is checked against the options first. The cost of each attacker is the mana of every tax in force
//! (`Rule::AttackTax`) that protects what it attacks and taxes it, and the total is locked in. A declaration with a
//! total of nothing is made at once. One with a total is legal only if its player can pay it with the attackers tapped
//! that tap to attack (CR 508.1f), and then it becomes a pending attack (`What::Attack`) whose payment is the ordinary
//! mana payment (`PayMana`, mana abilities included): the declaration is made with the payment, and backing out of the
//! payment returns to the declaration, with nothing paid.

use std::collections::BTreeMap;

use mtg_cards::modify::Attacked;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::announce::Next;
use crate::answers::Outcome;
use crate::cast::payment_decision;
use crate::combat::{AttackTarget, attack_events, defending_player};
use crate::decision::{AttackOptions, Decision, DecisionKind, PayOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::fold::apply_event;
use crate::help::Offered;
use crate::mana::can_afford_with;
use crate::priority::mana_options;
use crate::resolve::Scope;
use crate::rules::{Taxing, attack_taxes};
use crate::spend::Spend;
use crate::state::GameState;

/// What `object` costs to attack `target`: the mana of every tax in force that protects the player attacked, or the
/// player who controls the planeswalker attacked, and taxes the creature.
pub(crate) fn cost_of(state: &GameState, object: ObjectId, target: AttackTarget) -> ManaCost {
    let taxes = attack_taxes(state);
    if taxes.is_empty() {
        return ManaCost::ZERO;
    }
    let Some(defender) = defending_player(state, target) else {
        return ManaCost::ZERO;
    };
    let walker = matches!(target, AttackTarget::Planeswalker(_));
    taxes
        .iter()
        .filter(|taxing| {
            let tax = &taxing.tax;
            let protects = match tax.attacked {
                Attacked::Both => true,
                Attacked::Player => !walker,
                Attacked::Planeswalkers => walker,
            };
            protects && tax.players.contains(&defender) && taxes_creature(state, taxing, object)
        })
        .fold(ManaCost::ZERO, |sum, taxing| plus(&sum, &taxing.tax.mana))
}

/// Whether the tax reaches the creature `object`: any creature for a tax that names none, else those its selector picks.
fn taxes_creature(state: &GameState, taxing: &Taxing, object: ObjectId) -> bool {
    taxing.tax.creatures.is_empty()
        || taxing
            .source
            .zip(state.object(object))
            .is_some_and(|(source, creature)| {
                Scope::announcing(state, taxing.controller, source)
                    .selects(&taxing.tax.creatures, creature.reference())
                    .unwrap_or(false)
            })
}

/// Two costs paid together.
fn plus(left: &ManaCost, right: &ManaCost) -> ManaCost {
    ManaCost::from_symbols(left.symbols().iter().chain(right.symbols()).copied())
}

/// The total cost of an attack: what each of its attackers costs, added up and locked in (CR 508.1h).
pub(crate) fn total(state: &GameState, attackers: &[(ObjectId, AttackTarget)]) -> ManaCost {
    attackers
        .iter()
        .fold(ManaCost::ZERO, |sum, (object, target)| {
            plus(&sum, &cost_of(state, *object, *target))
        })
}

/// The mana the attacking player can surely spend on an attack, whichever creatures attack: the mana floating and, for
/// each untapped permanent that is not a creature that may attack (the declaration would tap it), the most its mana
/// abilities that cost nothing add, since a payment taps a permanent once. A bot that stays within it
/// declares an attack it can pay for, as long as the cost is generic.
pub(crate) fn budget(state: &GameState, player: PlayerId, may_attack: &[ObjectId]) -> u32 {
    let floating = state.player(player).map_or(0, |seat| seat.mana().total());
    let mut best: BTreeMap<ObjectId, u32> = BTreeMap::new();
    for option in mana_options(state, player).iter().filter(|option| {
        option.cost.symbols().is_empty()
            && option.takes.is_empty()
            && option.life == 0
            && option.energy == 0
            && option.only.is_empty()
            && !may_attack.contains(&option.source.object)
    }) {
        let made = u32::try_from(option.made().len()).unwrap_or(u32::MAX);
        let slot = best.entry(option.source.object).or_default();
        *slot = (*slot).max(made);
    }
    best.values()
        .fold(floating, |sum, made| sum.saturating_add(*made))
}

/// The state once the attack taps its attackers that tap to attack (CR 508.1f), in which the mana abilities of the
/// payment are offered; `None` if a tap does not fold.
fn tapped(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Option<GameState> {
    let mut after = state.clone();
    for event in attack_events(state, player, attackers) {
        if matches!(event, Event::PermanentTapped { .. }) {
            apply_event(&mut after, &event).ok()?;
        }
    }
    Some(after)
}

/// The payment of the total cost of an attack, if its player can make it. The mana abilities offered are those of the
/// permanents the attack leaves untapped; the object named is the first attacker.
fn payment(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Option<PayOptions> {
    let owed = total(state, attackers);
    let after = tapped(state, player, attackers)?;
    let options = payment_decision(
        &after,
        player,
        attackers.first()?.0,
        owed,
        mana_options(&after, player),
        Offered::default(),
        Spend::other(),
        true,
        0,
    )?;
    can_afford_with(
        &options.cost,
        &options.pool,
        &options.sources,
        options.help(),
        options.life,
        &options.spend,
    )
    .then_some(options)
}

/// What a declared attack does: with nothing to pay it is made, else it is paid for first (CR 508.1h) or, if its player
/// cannot pay, refused.
pub(crate) fn declare(
    state: &GameState,
    player: PlayerId,
    attackers: Vec<(ObjectId, AttackTarget)>,
) -> Result<Outcome, EngineError> {
    let owed = total(state, &attackers);
    if owed.symbols().is_empty() {
        return Ok(Outcome::Events(attack_events(state, player, &attackers)));
    }
    if payment(state, player, &attackers).is_none() {
        return Err(IllegalAnswer::AttackUnaffordable(owed.to_string()).into());
    }
    Ok(Outcome::BeginAttack(attackers))
}

/// The decision of an attack that waits for its payment: `PayMana` for the total cost, which can be backed out of. An
/// attack that can no longer be paid is dropped.
pub(crate) fn next(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Next {
    match payment(state, player, attackers) {
        Some(options) => Next::Ask(Decision {
            player,
            kind: DecisionKind::PayMana(options),
        }),
        None => Next::Drop,
    }
}

/// Fills the costs and the budget of the options an attack is offered with.
pub(crate) fn describe(state: &GameState, player: PlayerId, options: &mut AttackOptions) {
    if attack_taxes(state).is_empty() {
        return;
    }
    for option in &mut options.attackers {
        option.tax = option
            .defenders
            .iter()
            .map(|target| (*target, cost_of(state, option.object, *target)))
            .filter(|(_, cost)| !cost.symbols().is_empty())
            .collect();
    }
    if options
        .attackers
        .iter()
        .any(|option| !option.tax.is_empty())
    {
        let may_attack: Vec<ObjectId> = options.attackers.iter().map(|o| o.object).collect();
        options.mana = budget(state, player, &may_attack);
    }
}
