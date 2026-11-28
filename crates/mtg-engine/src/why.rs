//! The detail of why a card or ability cannot be played now ([`Why`], `docs/engine.md`, Refusals): each part is read
//! off the check that decided the reason, so the detail never disagrees with it.

use std::collections::BTreeMap;

use mtg_cards::ability::Ability;
use mtg_cards::value::CastWay;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType};

use crate::decision::{HostBy, Why};
use crate::mana::{ManaOption, ManaPool};
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;
use crate::zones::Zone;

/// The target of `rules`, a spell or ability of `source`, that has no legal choice for `player`, in words; `None` when
/// every target can be chosen.
pub(crate) fn target(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    rules: &Ability,
) -> Option<Why> {
    let spec = crate::choose::unmet_target(state, player, source, rules)?;
    let (needs, zone) = crate::wording::target_need(spec);
    Some(Why::Target { needs, zone })
}

/// The rule found by [`crate::prohibit::forbidder`], named by the object that put it in force while every player can
/// see that object: a face-up permanent, a spell or ability on the stack, or an object in the command zone.
pub(crate) fn forbidden_by(state: &GameState, source: Option<ObjectRef>) -> Why {
    named_ban(state, source, None)
}

/// [`forbidden_by`] for the prohibition `ban`, with the number of spells it lets a player cast each turn when it limits
/// them (One Voice, Rule of Law).
pub(crate) fn forbidden_by_ban(state: &GameState, ban: &crate::prohibit::Banned) -> Why {
    named_ban(state, ban.source, ban.cast_limit())
}

/// The rule of `source`, named while every player can see it, and its limit.
fn named_ban(state: &GameState, source: Option<ObjectRef>, limit: Option<u32>) -> Why {
    let seen = source
        .and_then(|source| state.object(source.id))
        .filter(|object| {
            object.face_down().is_none()
                && matches!(
                    object.zone(),
                    Zone::Battlefield | Zone::Stack | Zone::Command(_)
                )
        });
    Why::ForbiddenBy {
        source: seen.map(GameObject::id),
        name: seen.map(|object| object.card().name.clone()),
        limit,
    }
}

/// The spell with split second on the stack, if one is there (CR 702.61a).
pub(crate) fn split_second(state: &GameState) -> Option<Why> {
    let spell = crate::keywords::split_second_spell(state)?;
    Some(Why::SplitSecond {
        spell,
        name: state
            .object(spell)
            .map_or_default(|object| object.card().name.clone()),
    })
}

/// `cost` against the mana at hand: the pool and one use of each source, and the kinds of mana the cost asks for by a
/// symbol of one kind that neither the pool nor any source makes; `tax` is the commander tax `cost` includes.
pub(crate) fn mana(cost: ManaCost, pool: &ManaPool, sources: &[ManaOption], tax: u32) -> Why {
    let at_hand = at_hand(pool, sources, None);
    let mut missing: Vec<ManaType> = Vec::new();
    for symbol in cost.symbols() {
        let kind = match symbol {
            ManaSymbol::Colored(color) => ManaType::from(*color),
            ManaSymbol::Colorless => ManaType::Colorless,
            _ => continue,
        };
        let made = pool.get(kind) > 0 || sources.iter().any(|option| option.produces == kind);
        if !made && !missing.contains(&kind) {
            missing.push(kind);
        }
    }
    Why::Mana {
        cost,
        at_hand,
        missing,
        tax,
    }
}

/// The most mana `pool` and one use of each of `sources` make, of the kind `kind` alone when it is given.
fn at_hand(pool: &ManaPool, sources: &[ManaOption], kind: Option<ManaType>) -> u32 {
    let mut best: BTreeMap<ObjectId, u32> = BTreeMap::new();
    for option in sources
        .iter()
        .filter(|option| kind.is_none_or(|kind| option.produces == kind))
    {
        let most = best.entry(option.source.object).or_default();
        *most = (*most).max(option.count.max(1));
    }
    let floating = kind.map_or_else(|| pool.total(), |kind| pool.get(kind));
    floating.saturating_add(best.values().sum::<u32>())
}

/// The least X of a cost that cannot be paid at that X: `short` is its mana there against the pool and the sources when
/// the mana is what falls short, `None` when another part of the cost is.
pub(crate) fn least_x(least: u32, short: Option<(ManaCost, &ManaPool, &[ManaOption])>) -> Why {
    let (cost, at_hand) = match short {
        Some((cost, pool, sources)) => (Some(cost), Some(self::at_hand(pool, sources, None))),
        None => (None, None),
    };
    Why::LeastX {
        least,
        cost,
        at_hand,
    }
}

/// "Spend only mana of the chosen color" for an ability of `source`: the color it chose, `cost` as that color pays it
/// (`None` when no color is chosen or the cost asks for another), and the mana of that color at hand.
pub(crate) fn chosen_color(
    state: &GameState,
    source: ObjectId,
    cost: Option<ManaCost>,
    pool: &ManaPool,
    sources: &[ManaOption],
) -> Why {
    let color = match state
        .object(source)
        .and_then(GameObject::permanent)
        .and_then(crate::object::PermanentState::chosen)
    {
        Some(crate::object::Chosen::Color(color)) => Some(*color),
        _ => None,
    };
    Why::ChosenColor {
        color,
        cost: color.and(cost),
        at_hand: color.map_or(0, |color| {
            self::at_hand(pool, sources, Some(ManaType::from(color)))
        }),
    }
}

/// The host the spell `id` cast in `way` lacks, in the words of its target: an Aura's enchant ability, a bestowed creature
/// (CR 702.103b) or a mutating one (CR 702.140a); `None` when it has a host or needs none.
pub(crate) fn host(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    way: Option<CastWay>,
) -> Option<Why> {
    let words = |spec: mtg_cards::refs::TargetSpec| crate::wording::target_need(&spec).0;
    let (needs, by, met) = match way {
        Some(CastWay::Bestow) => (
            words(crate::attach::bestow_target()),
            HostBy::Bestow,
            crate::attach::bestow_announceable(state, player, id),
        ),
        // The mutate target in the words of its reminder text: the owner it names is the caster's.
        Some(CastWay::Mutate) => (
            "a non-Human creature you own on the battlefield".to_owned(),
            HostBy::Mutate,
            crate::merged::announceable(state, player, id),
        ),
        _ => (
            words(crate::attach::cast_target(state, id)?),
            HostBy::Enchant,
            crate::attach::announceable(state, player, id),
        ),
    };
    (!met).then_some(Why::Host { needs, by })
}

/// [`host`] for the first way of casting `id` that has no host, the mutate way left out for that among them.
pub(crate) fn host_of_any_way(state: &GameState, player: PlayerId, id: ObjectId) -> Option<Why> {
    let ways = crate::ways::of(state, player, id);
    let unhosted = crate::ways::hx29_unhosted_mutate(state, player, id, &ways);
    ways.iter()
        .chain(&unhosted)
        .find_map(|way| host(state, player, id, way.alternative))
}

/// The first target of `rules` that reads X, in words, for X from `least` to `most`, none of which leaves it a legal
/// choice; `None` when no target reads X.
pub(crate) fn x_target(rules: &Ability, least: u32, most: u32) -> Option<Why> {
    let modes = crate::choose::modal_of(rules);
    let spec = match &modes {
        Some(modal) => modal
            .modes
            .iter()
            .flat_map(|mode| mode.targets.iter())
            .find(|spec| crate::choose::reads_x(spec)),
        None => rules
            .targets()
            .iter()
            .find(|spec| crate::choose::reads_x(spec)),
    }?;
    let (needs, zone) = crate::wording::target_need(spec);
    Some(Why::XTarget {
        needs,
        zone,
        least,
        most,
    })
}
