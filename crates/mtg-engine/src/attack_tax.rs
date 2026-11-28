//! The cost of attacking (CR 508.1h-i): "Creatures can't attack you unless their controller pays {2} for each creature
//! they control that's attacking you."
//!
//! A declaration is checked against the options first. The cost of each attacker is the mana of every tax in force
//! (`Rule::AttackTax`) that protects what it attacks and taxes it, and the total is locked in. A declaration with a
//! total of nothing is made at once. One with a total is legal only if its player can pay it, without paying down to 0
//! life, with the attackers tapped that tap to attack (CR 508.1f), and then it becomes a pending attack
//! (`What::Attack`) whose payment is the ordinary
//! mana payment (`PayMana`, mana abilities included): the declaration is made with the payment, and backing out of the
//! payment returns to the declaration, with nothing paid.

use std::collections::BTreeMap;

use mtg_cards::modify::Attacked;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use mtg_cards::cost::CostItem;

use crate::announce::r6400_combat_costs::{
    R6400Share, r6400_parts, r6400_shares_events, r6400_shares_next, r6400_shares_payable,
};
use crate::announce::{Done, Next};
use crate::answers::Outcome;
use crate::cast::payment_decision;
use crate::characteristics::controller_of_object;
use crate::choose::Choices;
use crate::combat::{AttackExtras, AttackTarget, attack_events, defending_player};
use crate::decision::{AttackOptions, Decision, DecisionKind, PayOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::fold::apply_event;
use crate::help::Offered;
use crate::object::GameObject;
use crate::parts::Part;
use crate::priority::mana_options;
use crate::resolve::Scope;
use crate::rules::{Taxing, attack_taxes};
use crate::spend::Spend;
use crate::state::GameState;
use crate::zones::Zone;

/// What `object` costs to attack `target`: the mana of every tax in force that protects the player attacked, or the
/// player who controls the planeswalker attacked, and taxes the creature.
pub(crate) fn cost_of(state: &GameState, object: ObjectId, target: AttackTarget) -> ManaCost {
    in_force(state, object, target).fold(ManaCost::ZERO, |sum, taxing| {
        if taxing.tax.per_counter {
            let counters = state
                .object(object)
                .and_then(GameObject::permanent)
                .map_or(0, |held| {
                    held.counters().map(|(_, count)| count).sum::<u32>()
                });
            let times = usize::try_from(counters).unwrap_or(usize::MAX);
            plus(&sum, &crate::rules::repeated(&taxing.tax.mana, times))
        } else {
            plus(&sum, &taxing.tax.mana)
        }
    })
}

/// What `object` costs to attack `target` in life: the life of the same taxes as `cost_of`.
pub(crate) fn life_of(state: &GameState, object: ObjectId, target: AttackTarget) -> u32 {
    in_force(state, object, target).fold(0, |sum, taxing| sum.saturating_add(taxing.tax.life))
}

/// The taxes in force that protect the player attacked, or the player who controls the planeswalker attacked, and tax
/// the creature `object`.
fn in_force(
    state: &GameState,
    object: ObjectId,
    target: AttackTarget,
) -> impl Iterator<Item = &Taxing> {
    let defender = defending_player(state, target);
    let walker = matches!(target, AttackTarget::Planeswalker(_));
    // A tax protects players and their planeswalkers; the battle a player protects is neither.
    let battle =
        matches!(target, AttackTarget::Planeswalker(id) if crate::battle::is_battle(state, id));
    attack_taxes(state).iter().filter(move |taxing| {
        let tax = &taxing.tax;
        let protects = match tax.attacked {
            Attacked::Both => true,
            Attacked::Player => !walker,
            Attacked::Planeswalkers => walker,
        };
        protects
            && !battle
            && defender.is_some_and(|defender| tax.players.contains(&defender))
            && taxes_creature(state, taxing, object)
    })
}

/// The life an attack costs: what each of its attackers costs in life, added up (CR 508.1h).
pub(crate) fn life_total(state: &GameState, attackers: &[(ObjectId, AttackTarget)]) -> u32 {
    attackers.iter().fold(0, |sum, (object, target)| {
        sum.saturating_add(life_of(state, *object, *target))
    })
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

/// The attack an attack's paying owes for: each controller of its attackers with their own creatures, the attacking
/// player first. Where a team shares its turns each player pays the costs of their own creatures of the team's combined
/// attack; else the attacking player pays for every attacker (CR 508.1h).
fn shares(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Vec<(PlayerId, Vec<(ObjectId, AttackTarget)>)> {
    let mut shares = vec![(player, Vec::new())];
    let split = crate::party_attacks::splits(state);
    for (object, target) in attackers {
        let paying = if split {
            crate::characteristics::controller_of(state, *object).unwrap_or(player)
        } else {
            player
        };
        match shares.iter_mut().find(|(seat, _)| *seat == paying) {
            Some((_, own)) => own.push((*object, *target)),
            None => shares.push((paying, vec![(*object, *target)])),
        }
    }
    shares
}

/// The teammates who pay mana for their own creatures of a team's attack, in the order they are asked, each with
/// their creatures; the attacking player pays last, with the declaration.
fn teammates(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Vec<(PlayerId, Vec<(ObjectId, AttackTarget)>)> {
    let mut mates: Vec<_> = shares(state, player, attackers)
        .into_iter()
        .filter(|(paying, own)| *paying != player && !total(state, own).symbols().is_empty())
        .collect();
    mates.sort_by_key(|(paying, _)| *paying);
    mates
}

/// The creatures of an attack the attacking player pays for themselves.
fn own_share(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Vec<(ObjectId, AttackTarget)> {
    shares(state, player, attackers)
        .into_iter()
        .find(|(paying, _)| *paying == player)
        .map_or_default(|(_, own)| own)
}

/// The events of an attack declared and paid for: the teammates' payments of a team's attack (`paid`), the
/// declaration, the bands (CR 702.22c), the exerts (CR 701.43), then the life it costs, each player paying for their
/// own creatures, and its other costs with the permanents picked for them (`picked`); `None` when a pick is missing.
pub(crate) fn made(
    state: &GameState,
    player: PlayerId,
    (attackers, extras): (&[(ObjectId, AttackTarget)], &AttackExtras),
    picked: &Choices,
    paid: &[Event],
) -> Option<Vec<Event>> {
    let mut events = paid.to_vec();
    events.extend(attack_events(state, player, attackers));
    events.extend(crate::keywords::banding::band_events(player, &extras.bands));
    events.extend(crate::w11a_exert::w11a_exert_events(player, &extras.exert));
    events.extend(crate::r114_enlist::r114_enlist_events(
        player,
        &extras.r114_enlist,
    ));
    for (paying, own) in shares(state, player, attackers) {
        let life = life_total(state, &own);
        if life > 0 {
            events.push(Event::LifePaid {
                player: paying,
                amount: life,
            });
        }
    }
    events.extend(r6400_shares_events(
        state,
        &part_shares(state, player, attackers),
        picked,
    )?);
    Some(events)
}

/// The parts of an attack beyond mana and life by who pays them: each teammate of a team's attack for their own
/// creatures in turn order, then the attacking player for theirs (CR 508.1g-h).
fn part_shares(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Vec<R6400Share> {
    let mut shares = shares(state, player, attackers);
    shares.sort_by_key(|(paying, _)| (*paying == player, *paying));
    shares
        .into_iter()
        .filter_map(|(paying, own)| {
            let parts = other_parts(state, &own);
            (!parts.is_empty()).then_some(R6400Share {
                player: paying,
                source: own.first()?.0,
                parts,
            })
        })
        .collect()
}

/// The creatures of a declaration.
fn declared(attackers: &[(ObjectId, AttackTarget)]) -> Vec<ObjectId> {
    attackers.iter().map(|(object, _)| *object).collect()
}

/// The parts an attack costs beyond mana and life (CR 508.1g): the other costs of every tax on each of its attackers,
/// added up.
pub(crate) fn other_parts(state: &GameState, attackers: &[(ObjectId, AttackTarget)]) -> Vec<Part> {
    let items: Vec<CostItem> = attackers
        .iter()
        .flat_map(|(object, target)| {
            in_force(state, *object, *target).flat_map(|taxing| taxing.tax.costs.iter().cloned())
        })
        .collect();
    if items.is_empty() {
        Vec::new()
    } else {
        r6400_parts(&items)
    }
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

/// Whether `player` controls an untapped permanent with rules the table plays by hand: the mana it makes is added
/// by hand, which the engine cannot see, so the attack waits for it instead of being refused.
fn by_hand(state: &GameState, player: PlayerId) -> bool {
    state.zone(Zone::Battlefield).iter().any(|id| {
        state.object(*id).is_some_and(|object| {
            !object.card().manual.is_empty()
                && controller_of_object(state, object) == player
                && object.permanent().is_some_and(|perm| !perm.tapped())
        })
    })
}

/// The payment `paying` makes for their creatures `own` of the attack `attackers`, if they can make it. The mana
/// abilities offered are those of the permanents the attack leaves untapped; the object named is the first of `own`.
///
/// The attack is payable when an automatic payment exists, which never pays life down to 0: an attack that only a
/// payment taking the player's last life would cover is not made, so a payment decision always has the automatic answer
/// beside backing out and a player who backs out is not offered the same attack for nothing.
fn payment(
    state: &GameState,
    (player, paying): (PlayerId, PlayerId),
    attackers: &[(ObjectId, AttackTarget)],
    own: &[(ObjectId, AttackTarget)],
) -> Option<PayOptions> {
    let owed = total(state, own);
    can_pay_life(state, paying, life_total(state, own)).then_some(())?;
    let after = tapped(state, player, attackers)?;
    let options = payment_decision(
        &after,
        paying,
        own.first()?.0,
        owed,
        mana_options(&after, paying),
        Offered::default(),
        Spend::other().r111_by(state, player),
        true,
        0,
    )?;
    (options.auto_available || by_hand(&after, paying)).then_some(PayOptions {
        attack: true,
        ..options
    })
}

/// Whether `paying` can pay for their creatures `own` of the attack `attackers`: the life and the mana they cost.
fn check_share(
    state: &GameState,
    (player, paying): (PlayerId, PlayerId),
    attackers: &[(ObjectId, AttackTarget)],
    own: &[(ObjectId, AttackTarget)],
) -> Result<(), EngineError> {
    let life = life_total(state, own);
    if !can_pay_life(state, paying, life) {
        return Err(IllegalAnswer::AttackUnaffordable(format!("{life} life")).into());
    }
    let owed = total(state, own);
    if !owed.symbols().is_empty() && payment(state, (player, paying), attackers, own).is_none() {
        return Err(IllegalAnswer::AttackUnaffordable(owed.to_string()).into());
    }
    Ok(())
}

/// Whether every player can give up the permanents the attack takes for their own creatures.
fn check_parts(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Result<(), EngineError> {
    if r6400_shares_payable(
        state,
        &declared(attackers),
        &part_shares(state, player, attackers),
    ) {
        Ok(())
    } else {
        Err(IllegalAnswer::AttackUnaffordable("the permanents it takes".to_owned()).into())
    }
}

/// Whether `player` can pay for their own choice `own` of a team's attack, checked as they choose, so a choice only
/// its own player can mend is refused to them (CR 508.1h).
pub(crate) fn check_pick(
    state: &GameState,
    player: PlayerId,
    own: &[(ObjectId, AttackTarget)],
) -> Result<(), EngineError> {
    check_share(state, (player, player), own, own)?;
    check_parts(state, player, own)
}

/// What a declared attack does: with nothing to pay it is made, else it is paid for first (CR 508.1h) or, if a player
/// who pays for it cannot pay, refused.
pub(crate) fn declare(
    state: &GameState,
    player: PlayerId,
    attackers: Vec<(ObjectId, AttackTarget)>,
    extras: AttackExtras,
) -> Result<Outcome, EngineError> {
    for (paying, own) in shares(state, player, &attackers) {
        check_share(state, (player, paying), &attackers, &own)?;
    }
    check_parts(state, player, &attackers)?;
    if total(state, &attackers).symbols().is_empty() && other_parts(state, &attackers).is_empty() {
        let events = made(
            state,
            player,
            (&attackers, &extras),
            &Choices::default(),
            &[],
        )
        .ok_or_else(|| EngineError::InvalidEvent("an attack without its picks".into()))?;
        return Ok(Outcome::Events(events));
    }
    Ok(Outcome::BeginAttack(player, attackers, extras))
}

/// The decision of an attack that waits for its payment: the permanents its other costs take, each player picking those
/// of their own creatures, then `PayMana` of each
/// teammate for their own creatures of a team's attack, then `PayMana` of the attacking player for theirs, each of
/// which can be backed out of. An attack that can no longer be paid is dropped.
pub(crate) fn next(
    state: &GameState,
    player: PlayerId,
    (attackers, extras): (&[(ObjectId, AttackTarget)], &AttackExtras),
    picks: (&mut Choices, &mut Done),
) -> Next {
    let parts = part_shares(state, player, attackers);
    let (choices, done) = picks;
    if let Some(next) = r6400_shares_next(state, &declared(attackers), &parts, (choices, done)) {
        return next;
    }
    if let Some((mate, own)) = teammates(state, player, attackers).get(usize::from(done.party_paid))
    {
        return match payment(state, (player, *mate), attackers, own) {
            Some(options) => {
                done.asking_teammate();
                Next::Ask(Decision {
                    player: *mate,
                    kind: DecisionKind::PayMana(options),
                })
            }
            None => Next::Drop,
        };
    }
    let own = own_share(state, player, attackers);
    if total(state, &own).symbols().is_empty() {
        return made(
            state,
            player,
            (attackers, extras),
            choices,
            done.assist_events.as_deref().unwrap_or_default(),
        )
        .map_or(Next::Drop, Next::Commit);
    }
    match payment(state, (player, player), attackers, &own) {
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
        option.life = option
            .defenders
            .iter()
            .map(|target| (*target, life_of(state, option.object, *target)))
            .filter(|(_, life)| *life > 0)
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
