//! Priority: who may act, what they may do, and when a round of passes is complete.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::ability::{Ability, Activators, LoyaltyCost, Timing};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::abilities::AbilityRef;
use crate::ability_reader::{abilities_of, provided_here, working_here};
use crate::announce::some_x_has_targets;
use crate::answers::Outcome;
use crate::characteristics::{characteristics, controller_of_object};
use crate::choose::announceable;
use crate::decision::{
    FaceOption, InactiveAbility, PriorityAction, PriorityOptions, Refusal, Unplayable, Why,
};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::faces;
use crate::free_activation;
use crate::keywords;
use crate::loyalty;
use crate::mana::{ManaOption, ManaPool, can_afford, can_afford_with};
use crate::mana_ability;
use crate::manual;
use crate::object::GameObject;
use crate::player::Player;
use crate::prepared;
use crate::price::{Leaves, Price};
use crate::resolve::{Scope, zone_kind};
use crate::spend::Spend;
use crate::state::GameState;
use crate::unplayable;
use crate::ways;
use crate::zones::Zone;

/// Who holds priority and who has passed since the last action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PriorityState {
    pub(crate) holder: PlayerId,
    pub(crate) passed: BTreeSet<PlayerId>,
}

impl PriorityState {
    /// The player who may act now.
    pub const fn holder(&self) -> PlayerId {
        self.holder
    }

    /// Players who passed in succession.
    pub const fn passed(&self) -> &BTreeSet<PlayerId> {
        &self.passed
    }
}

/// True once every living player has passed in succession.
pub(crate) fn all_passed(state: &GameState) -> bool {
    state
        .priority()
        .is_some_and(|p| state.alive_players().all(|a| p.passed.contains(&a)))
}

/// The mana abilities `player` can activate right now to pay for something (`mana_ability.rs`): without those that may be
/// activated only as an instant.
pub(crate) fn mana_options(state: &GameState, player: PlayerId) -> Vec<ManaOption> {
    let mut found = mana_ability::options(state, player);
    found.retain(|option| !option.priority_only);
    found
}

/// The mana abilities `player` can activate while holding priority: every one, those activated only as an instant too.
fn priority_mana(state: &GameState, player: PlayerId) -> Vec<ManaOption> {
    mana_ability::options(state, player)
}

/// Whether `player` could cast a sorcery now: their own turn, a main step and an empty stack (CR 307.1).
pub(crate) fn sorcery_timing(state: &GameState, player: PlayerId) -> bool {
    state.is_active(player) && state.turn().step().is_main() && state.stack().is_empty()
}

/// The answer that plots `object`, when it is offered.
/// Unlocking a door and ending an effect by paying are legal when the offer lists them.
fn e15g_special_answer(
    options: &PriorityOptions,
    action: &PriorityAction,
) -> Result<Outcome, EngineError> {
    match *action {
        PriorityAction::UnlockDoor(object, door) => {
            if !options
                .unlockable
                .iter()
                .any(|option| option.card == object && option.face == door)
            {
                return Err(IllegalAnswer::NotOffered("unlock").into());
            }
            Ok(Outcome::BeginUnlock(object, door))
        }
        PriorityAction::EndEffect(effect) => {
            let offered = options
                .endable
                .iter()
                .find(|option| option.effect == effect)
                .ok_or(IllegalAnswer::NotOffered("end effect"))?;
            Ok(Outcome::BeginEndEffect(offered.object, effect))
        }
        _ => Err(IllegalAnswer::NotOffered("special action").into()),
    }
}

/// The refusal of a play of `card` the offer does not list: with the reason the offer gives for it, else `NotOffered`.
fn refused_card(options: &PriorityOptions, card: ObjectId, what: &'static str) -> EngineError {
    options
        .unplayable
        .iter()
        .find(|entry| entry.card == card)
        .map_or(IllegalAnswer::NotOffered(what), |entry| {
            IllegalAnswer::Refused(Box::new(Refusal {
                object: card,
                ability: None,
                reason: entry.reason,
                why: entry.why.clone(),
            }))
        })
        .into()
}

/// The refusal of an activation the offer does not list: with the reason the offer gives for it, else `NotOffered`.
fn refused_ability(options: &PriorityOptions, ability: AbilityRef) -> EngineError {
    options
        .inactive
        .iter()
        .find(|inactive| inactive.ability == ability)
        .map_or(IllegalAnswer::NotOffered("ability"), |inactive| {
            IllegalAnswer::Refused(Box::new(Refusal {
                object: ability.object,
                ability: Some(ability),
                reason: inactive.reason,
                why: inactive.why.clone(),
            }))
        })
        .into()
}

fn plot_answer(options: &PriorityOptions, object: ObjectId) -> Result<Outcome, EngineError> {
    if options.plottable.contains(&object) {
        Ok(Outcome::BeginPlot(object))
    } else {
        Err(IllegalAnswer::NotOffered("plot").into())
    }
}

/// Whether `player` has a land play left this turn (CR 305.2): the turn's plays so far are fewer than the ruleset and
/// the rules in play allow.
pub(crate) fn land_play_left(state: &GameState, player: PlayerId) -> bool {
    // Each player of a team that shares the turn has their own land plays (CR 305.2).
    let plays_made = if state.shares_turns() {
        state.tallied(player, crate::state::Tally::LandsPlayed)
    } else {
        state.turn().lands_played()
    };
    !crate::prohibit::forbidden(state, crate::prohibit::Doing::PlayLand { player })
        && plays_made
            < state
                .ruleset()
                .land_plays_per_turn()
                .saturating_add(crate::rules::extra_land_plays(state, player))
}

/// Whether `player` could play a land card now while the ability `resolving` resolves (CR 305.1, 702.75a): it is their
/// turn and a main phase, nothing else is on the stack and they have a land play left.
pub(crate) fn may_play_land_while_resolving(
    state: &GameState,
    player: PlayerId,
    resolving: ObjectId,
) -> bool {
    state.is_active(player)
        && state.turn().step().is_main()
        && state.stack().iter().all(|id| *id == resolving)
        && land_play_left(state, player)
}

fn hand_land(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|o| o.base_types().contains(CardType::Land))
}

/// Whether the cost can take the source from where it is, as it leaves, and no rule forbids it.
fn leaves_from_here(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    leaves: Leaves,
) -> bool {
    match leaves {
        Leaves::Stays => true,
        Leaves::Exiled => crate::prohibit::may_exile(state, object.id()),
        Leaves::Sacrificed => {
            object.zone() == Zone::Battlefield
                && crate::prohibit::may_sacrifice(state, player, object.id())
        }
        Leaves::Discarded => {
            matches!(object.zone(), Zone::Hand(_))
                && crate::prohibit::may_discard(state, player, object.id())
        }
    }
}

/// Whether the ability `rules`, which `address` names among the abilities of the permanent `object`, could be
/// activated by `player` now ([`activation`]).
fn can_activate(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    address: AbilityRef,
    rules: &Ability,
) -> bool {
    matches!(
        activation(state, player, object, address, rules),
        Activation::Ready
    )
}

/// Whether an ability can be activated now, and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Activation {
    /// It can be activated now.
    Ready,
    /// It cannot now, for this reason.
    Hindered(Unplayable, Option<Why>),
    /// It is never offered here: a mana ability, an ability that works from another zone, a cost the engine does not
    /// pay, or a cost a player who does not control the source cannot pay.
    Never,
}

/// The [`Activation`] of the ability `rules`, which `address` names among the abilities of the permanent `object`, for
/// `player` now: the first check that fails gives the reason.
///
/// The engine pays `{T}`, mana, the loss of the source and the parts of `Price::parts` for an ability and nothing else;
/// `Caps` keeps abilities with other costs out of the game. The mana must be affordable with the floating mana and the other
/// permanents the player taps.
#[allow(clippy::too_many_lines)]
pub(crate) fn activation(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    address: AbilityRef,
    rules: &Ability,
) -> Activation {
    use Activation::{Hindered, Never, Ready};
    if let Some(source) = crate::prohibit::forbidder(
        state,
        crate::prohibit::Doing::Activate {
            player,
            source: object.id(),
            kind: crate::activation_kind::specific(state, address, rules),
            taps: matches!(rules, Ability::Activated { cost, .. }
                if cost.0.contains(&mtg_cards::cost::CostItem::Tap)),
        },
    ) {
        return Hindered(
            Unplayable::Restricted,
            Some(crate::why::forbidden_by(state, source)),
        );
    }
    let targets = || {
        Hindered(
            Unplayable::Targets,
            crate::why::target(state, player, object.id(), rules),
        )
    };
    // A loyalty ability costs counters, not a cost the price below knows (CR 606).
    if let Ability::Loyalty { cost, .. } = rules {
        // A `-X` removes X counters, at most those the planeswalker holds (CR 606.6).
        let counters = object
            .permanent()
            .map_or(0, |permanent| permanent.counters_of(&CounterKind::Loyalty));
        if !loyalty::can_activate(state, player, object, *cost) {
            return if object.zone() != Zone::Battlefield {
                Never
            } else if !sorcery_timing(state, player) {
                Hindered(Unplayable::Timing, Some(Why::SorcerySpeed))
            } else if state.loyalty_spent(object.reference()) {
                Hindered(Unplayable::Restricted, Some(Why::LimitSpent))
            } else {
                Hindered(Unplayable::Cost, Some(Why::OtherCost))
            };
        }
        if !loyalty_mana_affordable(state, player, object, address, rules) {
            return Hindered(Unplayable::Mana, None);
        }
        if !announceable(state, player, object.id(), rules) {
            return targets();
        }
        return if some_x_has_targets(state, player, object.id(), rules, |x| {
            x == 0 || (*cost == LoyaltyCost::MinusX && x <= counters)
        }) {
            Ready
        } else {
            Hindered(Unplayable::Targets, None)
        };
    }
    let Ability::Activated {
        cost,
        timing,
        condition,
        limit,
        from,
        ..
    } = rules
    else {
        return Never;
    };
    // An ability is activated from the zone it works from, and a cost that moves its source needs it in the right one.
    if *from != zone_kind(object.zone()) {
        return Never;
    }
    if let Some(limit) = limit
        && state.limit_spent(*limit, object.reference(), address.source, address.ability)
    {
        return Hindered(Unplayable::Restricted, Some(Why::LimitSpent));
    }
    let cost = crate::free_activation::cost(state, player, (address, rules), cost);
    let Ok(price) =
        Price::announced_in(&cost, state, player, object.id(), address.source.granter())
    else {
        return Never;
    };
    // A player who does not control the permanent can neither tap it nor give it up as a cost (CR 602.2).
    let foreign = controller_of_object(state, object) != player;
    if (rules.is_mana_ability() && !crate::r281_mana_at_priority::r281_at_priority(rules))
        || (foreign && (price.tap || price.untap || price.leaves != Leaves::Stays))
    {
        return Never;
    }
    let other_cost = Hindered(Unplayable::Cost, Some(Why::OtherCost));
    if !leaves_from_here(state, player, object, price.leaves) {
        return other_cost;
    }
    // Life, counters and the objects the other parts take must be there to pay (CR 118.3).
    let Some(claimed) = price.claimed(state, player, object.id()) else {
        return other_cost;
    };
    if *timing == Timing::Sorcery && !free_activation::timing_open(state, player, (address, rules))
    {
        return Hindered(Unplayable::Timing, Some(Why::SorcerySpeed));
    }
    // "Activate only if ..." is judged as the ability is activated (CR 602.5b).
    if let Some(condition) = condition
        && !Scope::announcing(state, player, object.reference())
            .holds(condition)
            .unwrap_or(false)
    {
        return Hindered(Unplayable::Restricted, Some(Why::OwnCondition));
    }
    if let Some(why) = source_turns(state, object, &price) {
        return Hindered(Unplayable::Cost, Some(why));
    }
    if let Some(mana) =
        crate::c11a_less::c11a_mana(state, player, (object.id(), address, rules), &price, None)
    {
        let mana = &mana;
        let Some(seat) = state.player(player) else {
            return Never;
        };
        let mut sources = price.sources(state, player, object.id());
        // What the other parts of the cost take is not there to make mana or to be taken by a mana ability's cost (a
        // Cadaverous Bloom), and the life they pay is not there to spend.
        crate::mana_takes::leave(&mut sources, &claimed);
        let life = seat
            .life()
            .saturating_sub(i32::try_from(price.life()).unwrap_or(i32::MAX));
        let spend = Spend::for_ability(state, object.id(), price.x > 0);
        let helpers = waterbend_help(state, player, object.id(), &price, &claimed);
        if !can_afford_with(mana, seat.mana(), &sources, helpers.help(), life, &spend) {
            return Hindered(
                Unplayable::Mana,
                Some(crate::why::mana(mana.clone(), seat.mana(), &sources)),
            );
        }
    }
    let affords = |x: u32| {
        x == 0
            || ((price.x > 0 || price.counted())
                && price.at(x).claimed(state, player, object.id()).is_some()
                && can_pay_x(state, player, object, &price, &claimed, x))
    };
    // "X can't be 0": the least X must be paid for.
    let least = crate::announce::least_of(&cost);
    if least != 0 && !affords(least) {
        return Hindered(Unplayable::Mana, None);
    }
    if !announceable(state, player, object.id(), rules) {
        return targets();
    }
    if some_x_has_targets(state, player, object.id(), rules, affords) {
        Ready
    } else {
        Hindered(Unplayable::Targets, None)
    }
}

/// Why the source cannot be tapped (`{T}`) or untapped (`{Q}`) as the price asks, `None` when it can: it must be a
/// permanent in the other state, and a creature free of summoning sickness (CR 302.6); a price with neither asks
/// nothing of it.
fn source_turns(state: &GameState, object: &GameObject, price: &Price) -> Option<Why> {
    if !price.tap && !price.untap {
        return None;
    }
    let Some(permanent) = object.permanent() else {
        return Some(Why::OtherCost);
    };
    let sick = keywords::sick_for_abilities(state, object.id())
        && characteristics(state, object.id()).is_some_and(|c| c.is_creature());
    let ready = if price.untap {
        // A permanent that can't become untapped can't pay {Q}.
        permanent.tapped()
            && !price.tap
            && !crate::prohibit::forbidden(
                state,
                crate::prohibit::Doing::Untap {
                    object: object.id(),
                },
            )
    } else {
        !permanent.tapped()
    };
    if !ready {
        Some(Why::Tapped)
    } else if sick {
        Some(Why::SummoningSick)
    } else {
        None
    }
}

/// What may help pay the waterbend part of the price of an ability of `source`: the artifacts and creatures the cost does not
/// take, nothing for a price without one.
fn waterbend_help(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    price: &Price,
    claimed: &[ObjectId],
) -> crate::help::Offered {
    if price.waterbend == 0 {
        return crate::help::Offered::default();
    }
    let mut barred = claimed.to_vec();
    if price.tap || price.moves_source() {
        barred.push(source);
    }
    crate::help::waterbend(state, player, &barred)
}

/// Whether `player` can pay the mana that cost modifiers add to a loyalty ability (CR 606.4, 601.2f), if any do.
fn loyalty_mana_affordable(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    address: AbilityRef,
    rules: &Ability,
) -> bool {
    let Some((mana, price)) =
        crate::c11a_less::loyalty_mana(state, player, (object.id(), address, rules), None)
    else {
        return true;
    };
    let Some(seat) = state.player(player) else {
        return false;
    };
    let sources = price.sources(state, player, object.id());
    let spend = Spend::for_ability(state, object.id(), false);
    can_afford(&mana, seat.mana(), &sources, seat.life(), &spend)
}

/// Whether `player` can pay the mana of `price` with `x` more generic mana for each `{X}`, as `can_activate` judges the
/// mana: from what floats and the permanents they can tap other than those the other parts of the cost take.
fn can_pay_x(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    price: &Price,
    claimed: &[ObjectId],
    x: u32,
) -> bool {
    let Some(seat) = state.player(player) else {
        return false;
    };
    let mana = price.mana.clone().unwrap_or(ManaCost::ZERO);
    let mana = crate::price::with_x(mana, x.saturating_mul(price.x), price.x_color);
    let mut sources = price.at(x).sources(state, player, object.id());
    sources.retain(|option| !claimed.contains(&option.source.object));
    let life = seat
        .life()
        .saturating_sub(i32::try_from(price.life()).unwrap_or(i32::MAX));
    let spend = Spend::for_ability(state, object.id(), price.x > 0);
    can_afford(&mana, seat.mana(), &sources, life, &spend)
}

/// The cards outside the battlefield whose abilities `player` may use: the owner of a card in a hand, a graveyard or the
/// command zone, or in exile, activates its abilities (CR 602.2).
fn own_cards_elsewhere(state: &GameState, player: PlayerId) -> impl Iterator<Item = &GameObject> {
    [
        Zone::Hand(player),
        Zone::Graveyard(player),
        Zone::Command(player),
        Zone::Exile,
    ]
    .into_iter()
    .flat_map(|zone| state.zone(zone).iter())
    .filter_map(|id| state.object(*id))
    .filter(move |object| object.owner() == player)
}

/// Activated abilities of `player`'s permanents, and of the cards they own in a hand, a graveyard, exile or the command
/// zone, that are not mana abilities and could be used now.
#[cfg(test)]
pub(crate) fn other_abilities(state: &GameState, player: PlayerId) -> Vec<AbilityRef> {
    abilities_now(state, player).0
}

/// [`other_abilities`], and the activated abilities of the permanents `player` controls that cannot be activated now,
/// each with why (`PriorityOptions.inactive`).
pub(crate) fn abilities_now(
    state: &GameState,
    player: PlayerId,
) -> (Vec<AbilityRef>, Vec<InactiveAbility>) {
    let mut found = Vec::new();
    let mut inactive = Vec::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        let controller = controller_of_object(state, object);
        if controller != player && !state.possible.others_activate {
            continue;
        }
        for (address, rules) in abilities_of(state, id) {
            let by = match &*rules {
                Ability::Activated { by, .. } => *by,
                _ => Activators::Controller,
            };
            let allowed = match by {
                Activators::Controller => controller == player,
                Activators::Anyone => true,
                Activators::Opponents => state.is_opponent(controller, player),
                Activators::Owner => object.owner() == player,
            };
            if !allowed {
                continue;
            }
            match activation(state, player, object, address, &rules) {
                Activation::Ready => found.push(address),
                Activation::Hindered(reason, why) if controller == player => {
                    inactive.push(InactiveAbility {
                        ability: address,
                        reason,
                        why,
                    });
                }
                Activation::Hindered(..) | Activation::Never => {}
            }
        }
    }
    if state.possible.elsewhere {
        for object in own_cards_elsewhere(state, player) {
            let printed = working_here(object).map(|(address, rules)| (address, Arc::clone(rules)));
            for (address, rules) in printed.chain(provided_here(state, object)) {
                if can_activate(state, player, object, address, &rules) {
                    found.push(address);
                }
            }
        }
    }
    (found, inactive)
}

/// Whether `player` can do anything worth stopping for: play a land, cast a spell or use an ability that is not a mana ability.
///
/// `castable` already holds only the spells the floating mana and the untapped sources can pay for. Mana
/// abilities alone never count: tapping a land with nothing to spend the mana on is not an action.
pub(crate) fn can_act(
    playable_lands: &[ObjectId],
    castable: &[ObjectId],
    abilities: &[AbilityRef],
    special_actions: &[ObjectId],
) -> bool {
    !playable_lands.is_empty()
        || !castable.is_empty()
        || !abilities.is_empty()
        || !special_actions.is_empty()
}

/// The faces of the cards with several faces that can be cast now (`castable`) or played as a land, each with its cost.
fn face_options(
    state: &GameState,
    player: PlayerId,
    castable: &[ObjectId],
    may_play_land: bool,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<FaceOption> {
    let mut found = Vec::new();
    let hand = state.zone(Zone::Hand(player));
    for &card in hand.iter().chain(castable) {
        if !faces::is_multi_face(state, card) || found.iter().any(|o: &FaceOption| o.card == card) {
            continue;
        }
        let listed = |face: Option<u8>, land: bool| {
            let (name, cost) = faces::describe(state, card, face);
            FaceOption {
                card,
                face: face.unwrap_or(0),
                name,
                cost: if land { None } else { cost },
                land,
            }
        };
        if may_play_land && hand.contains(&card) {
            found.extend(
                faces::land_faces(state, card)
                    .into_iter()
                    .map(|face| listed(face, true)),
            );
        }
        if castable.contains(&card) {
            found.extend(
                faces::castable_faces(state, player, card, pool, sources, life)
                    .into_iter()
                    .map(|face| listed(face, false)),
            );
        }
    }
    found
}

/// The abilities offered and explained while split second forbids every activation but mana abilities (`silenced`):
/// none is offered, and each one that could be activated says so.
fn silence(
    state: &GameState,
    silenced: bool,
    (abilities, inactive): (Vec<AbilityRef>, Vec<InactiveAbility>),
) -> (Vec<AbilityRef>, Vec<InactiveAbility>) {
    match crate::why::split_second(state).filter(|_| silenced) {
        Some(why) => (
            Vec::new(),
            abilities
                .into_iter()
                .map(|ability| InactiveAbility {
                    ability,
                    reason: Unplayable::Restricted,
                    why: Some(why.clone()),
                })
                .chain(inactive)
                .collect(),
        ),
        None => (abilities, inactive),
    }
}

/// Everything `player` may do while holding priority.
pub(crate) fn priority_options(state: &GameState, player: PlayerId) -> PriorityOptions {
    let hand = state.zone(Zone::Hand(player));
    let all_mana = mana_options(state, player);
    let pool = state
        .player(player)
        .map(|p| p.mana().clone())
        .unwrap_or_default();
    let life = state.player(player).map_or(0, Player::life);
    let may_play_land = sorcery_timing(state, player) && land_play_left(state, player);
    let playable_lands: Vec<ObjectId> = if may_play_land {
        hand.iter()
            .copied()
            .filter(|id| hand_land(state, *id))
            .chain(ways::lands_elsewhere(state, player))
            .collect()
    } else {
        Vec::new()
    };
    // CR 601.2c: a spell that needs a target is cast only when it has a legal one; CR 700.2a: a modal spell only when
    // enough of its modes can be chosen. A card is offered when some way to cast it can be used now (`ways`).
    // Split second: no spell is cast and no ability that is not a mana ability is activated (CR 702.61a). Special
    // actions and mana abilities stay.
    let silenced = keywords::split_second_on_stack(state);
    let restricted = silenced || crate::w17b_statics::w17b_cant_cast(state, player);
    let judged = unplayable::judge(state, player, &pool, &all_mana, life, restricted);
    let castable = unplayable::castable(state, player, &judged, restricted, &pool, &all_mana, life);
    // A special action: no stack, so no target or modes are needed (CR 116.2f).
    let suspendable: Vec<ObjectId> = hand
        .iter()
        .copied()
        .filter(|id| ways::can_suspend(state, player, *id, &pool, &all_mana, life))
        .collect();
    let foretellable: Vec<ObjectId> = hand
        .iter()
        .copied()
        .filter(|id| ways::can_foretell(state, player, *id, &pool, &all_mana, life))
        .collect();
    let plottable = ways::plottable(state, player, hand, &pool, &all_mana, life);
    let companion = crate::companions::can_bring(state, player, &pool, &all_mana, life);
    let turnable: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| ways::can_turn_up(state, player, *id, &pool, &all_mana, life))
        .collect();
    let unlockable = crate::dk1c5_rooms::dk1c5_unlockable(state, player, &pool, &all_mana, life);
    let endable = crate::e15g_paid::endable(state, player, &pool, &all_mana, life);
    let (abilities, inactive) = silence(state, silenced, abilities_now(state, player));
    let mut faces = face_options(
        state,
        player,
        &castable,
        may_play_land,
        &pool,
        &all_mana,
        life,
    );
    if !silenced {
        faces.extend(prepared::options(state, player, &pool, &all_mana, life));
    }
    let mana_abilities =
        crate::costly_mana::at_priority(state, &priority_mana(state, player), &pool, life);
    let offered = unplayable::Offered {
        lands: &playable_lands,
        castable: &castable,
        faces: &faces,
        special: [&suspendable, &foretellable, &plottable],
    };
    let at_hand = (&pool, all_mana.as_slice(), life);
    let unplayable = unplayable::unplayable(state, player, &judged, &offered, at_hand, restricted);
    PriorityOptions {
        can_act: can_act(
            &playable_lands,
            &castable,
            &abilities,
            &[
                suspendable.as_slice(),
                foretellable.as_slice(),
                plottable.as_slice(),
                turnable.as_slice(),
            ]
            .concat(),
        ) || !faces.is_empty()
            || !unlockable.is_empty()
            || !endable.is_empty()
            || companion,
        playable_lands,
        castable,
        mana_abilities,
        abilities,
        suspendable,
        foretellable,
        plottable,
        turnable,
        unlockable,
        endable,
        companion,
        faces,
        unplayable,
        inactive,
        can_pass: true,
    }
}

/// Validates a priority action and turns it into events (or a cast in progress).
pub(crate) fn answer(
    state: &GameState,
    player: PlayerId,
    options: &PriorityOptions,
    action: PriorityAction,
) -> Result<Outcome, EngineError> {
    match action {
        PriorityAction::Pass => Ok(Outcome::Events(vec![Event::PriorityPassed { player }])),
        PriorityAction::PlayLand(object) => {
            if !options.playable_lands.contains(&object) {
                return Err(refused_card(options, object, "land"));
            }
            Ok(Outcome::Events(crate::stack::land_played(
                state, player, object, None,
            )))
        }
        PriorityAction::PlayLandFace(object, face) => {
            let offered = options
                .faces
                .iter()
                .any(|option| option.land && option.card == object && option.face == face);
            if !offered {
                return Err(IllegalAnswer::NotOffered("land").into());
            }
            // The front face of an adventure or modal double-faced card is the card itself.
            Ok(Outcome::Events(crate::stack::land_played(
                state,
                player,
                object,
                (face != 0).then_some(face),
            )))
        }
        PriorityAction::CastSpell(object) => {
            if !options.castable.contains(&object) {
                return Err(refused_card(options, object, "spell"));
            }
            Ok(Outcome::BeginCast(object))
        }
        PriorityAction::Activate(ability) => {
            if !options.abilities.contains(&ability) {
                return Err(refused_ability(options, ability));
            }
            Ok(Outcome::BeginActivation(ability))
        }
        PriorityAction::Suspend(object) => {
            if !options.suspendable.contains(&object) {
                return Err(IllegalAnswer::NotOffered("suspend").into());
            }
            Ok(Outcome::BeginSuspend(object))
        }
        PriorityAction::Foretell(object) => {
            if !options.foretellable.contains(&object) {
                return Err(IllegalAnswer::NotOffered("foretell").into());
            }
            Ok(Outcome::BeginForetell(object))
        }
        PriorityAction::Plot(object) => plot_answer(options, object),
        PriorityAction::BringCompanion => {
            let outside = crate::companions::outside(state, player)
                .filter(|_| options.companion)
                .ok_or(IllegalAnswer::NotOffered("companion"))?;
            Ok(Outcome::BeginCompanion(outside))
        }
        PriorityAction::TurnFaceUp(object) => {
            if !options.turnable.contains(&object) {
                return Err(IllegalAnswer::NotOffered("turn face up").into());
            }
            Ok(Outcome::BeginTurnUp(object))
        }
        special @ (PriorityAction::UnlockDoor(..) | PriorityAction::EndEffect(_)) => {
            e15g_special_answer(options, &special)
        }
        PriorityAction::CastPrepared(object) => {
            let offered = options.faces.iter().any(|option| {
                option.card == object && option.face == prepared::SPELL_FACE && !option.land
            }) && state
                .object(object)
                .is_some_and(|held| held.zone() == Zone::Battlefield);
            if !offered {
                return Err(IllegalAnswer::NotOffered("prepared spell").into());
            }
            Ok(Outcome::BeginPrepared(object))
        }
        PriorityAction::Manual(action) => manual::events(state, player, &action)
            .map(Outcome::Events)
            .map_err(|refusal| IllegalAnswer::Manual(refusal).into()),
        PriorityAction::ActivateMana(source) => {
            let option = options
                .mana_abilities
                .iter()
                .find(|o| o.source == source)
                .ok_or(IllegalAnswer::NotOffered("mana ability"))?;
            if !option.is_costly() {
                return Ok(Outcome::Events(mana_ability::activation(
                    state,
                    player,
                    option,
                    &[],
                )?));
            }
            let all = mana_options(state, player);
            let mut events = crate::costly_mana::cost_events(state, player, option, &all)?;
            events.extend(mana_ability::activation(state, player, option, &[])?);
            mana_ability::r5g_distinct(state, &mut events);
            Ok(Outcome::Events(events))
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an activated ability is offered when its whole price can be paid now: a `{T}` needs an untapped source, mana needs floating mana or untapped mana sources (the source of an ability that taps it is none of them), an `{X}` is offered when X = 0 can be paid, and a part the engine does not pay is never offered.
    use std::sync::Arc;

    use mtg_cards::AbilityDef;
    use mtg_cards::CardDefinition;
    use mtg_cards::definition::ManaAbility;
    use mtg_core::card::CardType;
    use mtg_core::color::Color;
    use mtg_core::mana::ManaType;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::state;
    use crate::fold::apply_event;

    fn gadget(cost: &str, makes_mana: bool) -> Arc<CardDefinition> {
        let rules = format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"Draw":{{}}}}]}}}}"#);
        let card = CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(&rules).unwrap());
        Arc::new(if makes_mana {
            card.with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Colorless,
            }))
        } else {
            card
        })
    }

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::basic_land(Some(Color::Green)))
    }

    fn offered(game: &GameState) -> bool {
        !other_abilities(game, PlayerId(0)).is_empty()
    }

    #[rstest]
    #[case::free("[]", 0, true)]
    #[case::a_tap("[\"Tap\"]", 0, true)]
    #[case::mana_without_a_land("\"{1}\"", 0, false)]
    #[case::mana_with_a_land("\"{1}\"", 1, true)]
    #[case::two_mana_with_one_land("\"{2}\"", 1, false)]
    #[case::two_mana_with_two_lands("\"{2}\"", 2, true)]
    #[case::a_colour_the_lands_do_not_make("\"{R}\"", 3, false)]
    #[case::mana_and_a_tap("[{\"Mana\":\"{1}\"},\"Tap\"]", 1, true)]
    #[case::mana_and_a_tap_without_a_land("[{\"Mana\":\"{1}\"},\"Tap\"]", 0, false)]
    #[case::x("[{\"Mana\":\"{X}\"}]", 3, true)]
    #[case::x_of_nothing("[{\"Mana\":\"{X}\"}]", 0, true)]
    #[case::x_with_a_colour_that_cannot_be_paid("[{\"Mana\":\"{X}{R}\"}]", 3, false)]
    #[case::x_and_a_tap_without_a_source("[{\"Mana\":\"{X}\"},\"Tap\"]", 0, true)]
    #[case::life_as_well("[{\"Mana\":\"{1}\"},{\"PayLife\":1}]", 3, true)]
    #[case::more_life_than_the_player_has("[{\"Mana\":\"{1}\"},{\"PayLife\":21}]", 3, false)]
    fn an_ability_is_offered_when_its_price_can_be_paid(
        #[case] cost: &str,
        #[case] lands: usize,
        #[case] expected: bool,
    ) {
        let mut cards = vec![gadget(cost, false)];
        cards.extend(vec![forest(); lands]);
        let game = state(vec![vec![], vec![]], vec![cards, vec![]]);
        assert_eq!(offered(&game), expected);
    }

    #[rstest]
    #[case::taps_itself_for_its_own_ability("\"{1}\"", true)]
    #[case::cannot_tap_twice("[{\"Mana\":\"{1}\"},\"Tap\"]", false)]
    fn the_source_pays_for_its_ability_only_when_it_is_not_tapped_for_the_cost(
        #[case] cost: &str,
        #[case] expected: bool,
    ) {
        let game = state(vec![vec![], vec![]], vec![vec![gadget(cost, true)], vec![]]);
        assert_eq!(offered(&game), expected);
    }

    #[test]
    fn floating_mana_pays_and_a_tapped_source_cannot_tap() {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![gadget("[{\"Mana\":\"{1}\"},\"Tap\"]", false)], vec![]],
        );
        assert!(!offered(&game));
        apply_event(
            &mut game,
            &Event::ManaAdded {
                player: PlayerId(0),
                mana: ManaType::Red,
                source: None,
                only: Vec::new(),
                lasts: mtg_cards::spend::ManaLasts::Step,
            },
        )
        .unwrap();
        assert!(offered(&game));
        let gadget = game.zone(Zone::Battlefield)[0];
        apply_event(&mut game, &Event::PermanentTapped { object: gadget }).unwrap();
        assert!(!offered(&game));
    }

    #[test]
    fn the_other_seat_is_not_offered_the_ability() {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![gadget("[]", false)], vec![forest()]],
        );
        assert_eq!(
            other_abilities(&game, PlayerId(1)),
            Vec::<AbilityRef>::new()
        );
    }
}
