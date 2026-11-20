//! The abilities a way of casting gives the permanent its spell becomes when the card does not print that way: a card
//! given warp in a hand is exiled at the beginning of the next end step and may be cast from exile later (CR 702.185a),
//! one given sneak enters tapped (CR 702.190a), a creature spell given blitz has haste, draws when it dies and is sacrificed
//! at the next end step (CR 702.152a). A card that prints the way prints these riders with it.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::alt_riders::{sneak_riders, warp_riders};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;

use crate::abilities::AbilityRef;
use crate::object::GameObject;

/// The riders of `way`, in order: the abilities its keyword adds, but the keywords a static rider gives, which
/// [`r111_rider_keywords`] lists.
fn riders(way: CastWay) -> Vec<Ability> {
    match way {
        CastWay::Warp => warp_riders(),
        CastWay::Sneak => sneak_riders(),
        CastWay::Blitz => blitz_riders()
            .into_iter()
            .filter(|rider| !matches!(rider, Ability::Static { .. }))
            .collect(),
        _ => Vec::new(),
    }
}

/// What blitz adds to a permanent, whatever its cost.
fn blitz_riders() -> Vec<Ability> {
    let blitz = Keyword::Blitz(mtg_cards::cost::Cost::default());
    crate::keywords::rules(&blitz).adds_abilities(&blitz)
}

/// Whether the card prints the alternative cost of `way`, as an `AltCast` or as its keyword.
fn prints(card: &mtg_cards::CardDefinition, way: CastWay) -> bool {
    crate::ability_reader::printed_rules(card).any(|rules| match &**rules {
        Ability::AltCast { way: own, .. } => *own == way,
        Ability::Keyword(Keyword::Blitz(_)) => way == CastWay::Blitz,
        _ => false,
    })
}

/// The keywords the way `object` was cast gives it when its card does not print that way: blitz's haste.
pub(crate) fn r111_rider_keywords(object: &GameObject) -> Vec<Keyword> {
    match object.cast_way() {
        Some(CastWay::Blitz) if !prints(object.card(), CastWay::Blitz) => blitz_riders()
            .into_iter()
            .filter_map(|rider| match rider {
                Ability::Static { mods, .. } => Some(mods),
                _ => None,
            })
            .flatten()
            .filter_map(|modification| match modification {
                mtg_cards::modify::Modification::AddKeyword(keyword) => Some(keyword),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The riders `object` has: those of the way its spell was cast, when its card prints no alternative cost of that way.
pub(crate) fn r111_riders(object: &GameObject) -> Vec<(AbilityRef, Arc<Ability>)> {
    let Some(way) = object.cast_way() else {
        return Vec::new();
    };
    if prints(object.card(), way) {
        return Vec::new();
    }
    riders(way)
        .into_iter()
        .enumerate()
        .map(|(index, body)| (AbilityRef::r111_rider(object.id(), index), Arc::new(body)))
        .collect()
}

/// Whether the riders of the alternative cost `ability` is, or that a static ability gives, hold an ability `holds` picks:
/// what the state watches for from the start of the game (`setup`).
pub(crate) fn r111_rides(ability: &Ability, holds: impl Fn(&Ability) -> bool) -> bool {
    let rides = |way: CastWay| riders(way).iter().any(&holds);
    match ability {
        Ability::AltCast { way, .. } => rides(*way),
        Ability::Static { mods, .. } => mods.iter().any(|modification| {
            matches!(modification, mtg_cards::modify::Modification::GrantAbility(granted)
                if matches!(&**granted, Ability::AltCast { way, .. } if rides(*way)))
        }),
        _ => false,
    }
}

/// Whether a permanent `player` controls lets them cast `card`, which they own, from the zone `zone` it is in for an
/// alternative cost of `way` that works from a hand (`GameRule::R111CastWayFrom`: "You may cast creature spells from your
/// graveyard using their sneak abilities").
pub(crate) fn r111_cast_from(
    state: &crate::state::GameState,
    player: mtg_core::ids::PlayerId,
    card: &GameObject,
    way: CastWay,
    zone: mtg_cards::refs::ZoneKind,
) -> bool {
    use mtg_cards::modify::GameRule;
    if card.owner() != player {
        return false;
    }
    state
        .zone(crate::zones::Zone::Battlefield)
        .iter()
        .filter(|id| crate::characteristics::controller_of(state, **id) == Some(player))
        .filter_map(|id| state.object(*id))
        .any(|source| {
            crate::ability_reader::abilities_of(state, source.id())
                .iter()
                .any(|(_, ability)| match &**ability {
                    Ability::Rule {
                        rule:
                            GameRule::R111CastWayFrom {
                                way: allowed,
                                from,
                                what,
                            },
                        condition,
                        from: mtg_cards::refs::ZoneKind::Battlefield,
                    } if *allowed == way && *from == zone => {
                        let scope =
                            crate::resolve::Scope::announcing(state, player, source.reference());
                        let mut what = what.clone();
                        what.0.push(mtg_cards::refs::Pred::In(zone));
                        condition
                            .as_ref()
                            .is_none_or(|held| scope.holds(held).unwrap_or(false))
                            && scope.selects(&what, card.reference()).unwrap_or(false)
                    }
                    _ => false,
                })
        })
}

/// The rider at `index` that a permanent of `card` cast for `way` has (as it is, or as it last was), if it has one there.
pub(crate) fn r111_rider_at(
    card: &mtg_cards::CardDefinition,
    way: Option<CastWay>,
    index: usize,
) -> Option<Arc<Ability>> {
    let way = way.filter(|way| !prints(card, *way))?;
    riders(way).into_iter().nth(index).map(Arc::new)
}
