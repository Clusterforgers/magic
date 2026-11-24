//! Mana abilities of a shape the payment planner does not take (a discard, a mill, an untap or an X in the cost, a
//! choice of player): activated at priority like any ability, they resolve at once, before any player receives
//! priority, and their controller keeps priority (CR 605.3a, 605.3b, 117.3c).

use mtg_cards::ability::Ability;
use mtg_cards::assemble::{mana_shape, plain_mana_ability};
use mtg_cards::cost::CostItem;
use mtg_cards::effect::{Effect, Produce};
use mtg_cards::refs::Ref;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;

use crate::abilities::AbilityRef;
use crate::ability_reader;
use crate::events::Event;
use crate::state::GameState;

/// Whether `rules` is a mana ability that the planner never activates, so it is activated at priority and resolves
/// at once.
pub fn r281_at_priority(rules: &Ability) -> bool {
    rules.is_mana_ability() && plain_mana_ability(rules).is_none() && mana_shape(rules).is_none()
}

/// The player who keeps priority once the ability object `id` stops resolving: the controller of a mana ability that
/// resolved at once; `None` for any other object.
pub(crate) fn r281_keeps_priority(state: &GameState, id: ObjectId) -> Option<PlayerId> {
    let object = state.objects.get(&id)?;
    let ability = object.ability()?;
    r281_at_priority(ability.rules()).then(|| object.controller())
}

/// Ends the events of an activation, mana paid, with the start of its resolution when the ability put on the stack is a
/// mana ability activated at priority (CR 605.3a).
pub(crate) fn r281_resolve_at_once(state: &GameState, events: &mut Vec<Event>) {
    let at_once = events.iter().find_map(|event| match event {
        Event::AbilityPutOnStack(put) if put.cause.is_none() => {
            let address = AbilityRef {
                object: put.source.id,
                ability: put.ability,
                source: put.origin,
            };
            ability_reader::ability(state, address)
                .is_some_and(|rules| r281_at_priority(&rules))
                .then_some(put.object)
        }
        _ => None,
    });
    if let Some(object) = at_once {
        events.push(Event::ResolutionBegan { object });
    }
}

/// The mana abilities of this kind `player` may activate while paying for `paying` (CR 601.2g): each one offered at
/// priority now but the ability being paid for; none while a resolution is under way, whose cast asks its payment in
/// the middle of it. A discard cost may not take the card `aside` of the cast being paid for.
pub(crate) fn r281_while_paying(
    state: &GameState,
    player: PlayerId,
    paying: Option<AbilityRef>,
    aside: Option<ObjectId>,
) -> Vec<AbilityRef> {
    if state.resolving().is_some() {
        return Vec::new();
    }
    let hand = state
        .zone(crate::zones::Zone::Hand(player))
        .len()
        .saturating_sub(r281_spare(state, player, aside));
    crate::priority::abilities_now(state, player)
        .0
        .into_iter()
        .filter(|address| Some(*address) != paying)
        .filter(|address| {
            ability_reader::ability(state, *address).is_some_and(|rules| {
                r281_at_priority(&rules) && r281_discards(&rules).is_none_or(|count| count <= hand)
            })
        })
        .collect()
}

/// Where the payment sources of [`r281_mana`] begin among a source's ability numbers: far above any real ability, so
/// they never stand for one.
const R281_SOURCE_BASE: usize = usize::MAX / 2;

/// The mana `player`'s abilities among `announced` would add, as payment sources for judging whether a cost can be
/// paid (a spell priority offers, a payment asked): never offered or tapped, since each is activated apart from the
/// payment (CR 601.2g). `spare` hand cards are kept from a discard cost (the spell cast from the hand). An ability whose
/// mana or cost the planner cannot count (an X, an amount counted as it resolves) adds none.
pub(crate) fn r281_mana(
    state: &GameState,
    player: PlayerId,
    announced: &[AbilityRef],
    spare: usize,
) -> Vec<crate::mana::ManaOption> {
    let hand = state.zone(crate::zones::Zone::Hand(player)).len();
    announced
        .iter()
        .enumerate()
        .flat_map(|(place, address)| {
            ability_reader::ability(state, *address)
                .and_then(|rules| r281_options(&rules, hand.saturating_sub(spare)))
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(move |(variant, (made, shape))| {
                    let mut option = crate::mana::ManaOption::new(
                        crate::mana::ManaSource {
                            object: address.object,
                            ability: R281_SOURCE_BASE + place * ManaType::ALL.len() + variant,
                        },
                        made,
                    );
                    option.count = shape.count;
                    option.also = shape.also;
                    option.tapless = !shape.tap;
                    option.sacrifice = shape.sacrifice;
                    option.extra = true;
                    option.nonland = true;
                    option.cost = shape.cost.clone();
                    option
                })
        })
        .collect()
}

/// The sources a cast offered at priority is judged with: `sources` and the mana of the announced mana abilities among
/// `abilities`, one hand card kept for the spell.
pub(crate) fn r281_cast_mana<'a>(
    state: &GameState,
    player: PlayerId,
    abilities: &[AbilityRef],
    sources: &'a [crate::mana::ManaOption],
) -> std::borrow::Cow<'a, [crate::mana::ManaOption]> {
    let announced: Vec<AbilityRef> = abilities
        .iter()
        .copied()
        .filter(|address| {
            ability_reader::ability(state, *address).is_some_and(|rules| r281_at_priority(&rules))
        })
        .collect();
    let more = r281_mana(state, player, &announced, 1);
    if more.is_empty() {
        return std::borrow::Cow::Borrowed(sources);
    }
    std::borrow::Cow::Owned([sources, more.as_slice()].concat())
}

/// How many cards the cost of `rules` discards; `None` when the number is counted as it is paid.
fn r281_discards(rules: &Ability) -> Option<usize> {
    let Ability::Activated { cost, .. } = rules else {
        return Some(0);
    };
    cost.0.iter().try_fold(0usize, |sum, item| match item {
        CostItem::Discard { count, .. } | CostItem::DiscardRandom(count) => {
            Some(sum.saturating_add(usize::try_from(count.fixed()?).ok()?))
        }
        _ => Some(sum),
    })
}

/// The hand cards a discard cost of a mana ability activated now may not take: the card `aside` of a cast whose payment
/// waits for it, while it is still in `player`'s hand.
pub(crate) fn r281_spare(state: &GameState, player: PlayerId, aside: Option<ObjectId>) -> usize {
    usize::from(
        aside.is_some_and(|card| state.zone(crate::zones::Zone::Hand(player)).contains(&card)),
    )
}

/// What one use of an announced mana ability gives a payment: how many mana, its own mana cost, and whether it taps or
/// sacrifices its source.
#[derive(Clone)]
struct R281Use {
    count: u32,
    also: Vec<ManaType>,
    cost: mtg_core::mana::ManaCost,
    tap: bool,
    sacrifice: bool,
}

/// Each kind of mana `rules` may add with one use, with that use; `None` when the planner cannot count it or the
/// `hand` cards free for a discard do not pay its cost.
fn r281_options(rules: &Ability, hand: usize) -> Option<Vec<(ManaType, R281Use)>> {
    let Ability::Activated { cost, effects, .. } = rules else {
        return None;
    };
    if cost.has_x() {
        return None;
    }
    let mut shape = R281Use {
        count: 1,
        also: Vec::new(),
        cost: mtg_core::mana::ManaCost::ZERO,
        tap: false,
        sacrifice: false,
    };
    for item in &cost.0 {
        match item {
            CostItem::Tap => shape.tap = true,
            CostItem::SacrificeSelf => shape.sacrifice = true,
            CostItem::Mana(mana) => shape.cost = mana.rest().clone(),
            _ => {}
        }
    }
    if r281_discards(rules)? > hand {
        return None;
    }
    let (mana, times) = effects.iter().find_map(|effect| match effect {
        Effect::AddMana {
            who: Ref::You,
            mana,
            times,
            only,
            ..
        } if only.is_empty() => Some((mana, times.fixed()?)),
        _ => None,
    })?;
    let additions: Vec<Vec<ManaType>> = match mana {
        Produce::Fixed(list) => vec![list.clone()],
        Produce::AnyColor(count) => {
            let count = usize::try_from(count.fixed()?).ok()?;
            ManaType::ALL[..5]
                .iter()
                .map(|mana| vec![*mana; count])
                .collect()
        }
        Produce::OneOf(types) => types.iter().map(|mana| vec![*mana]).collect(),
        _ => return None,
    };
    let times = usize::try_from(times).ok()?;
    Some(
        additions
            .into_iter()
            .filter_map(|addition| {
                let made: Vec<ManaType> = (0..times).flat_map(|_| addition.clone()).collect();
                let (mana, count, also) = crate::mana_ability::split(&made)?;
                Some((
                    mana,
                    R281Use {
                        count,
                        also,
                        ..shape.clone()
                    },
                ))
            })
            .collect(),
    )
}
