//! The types of mana an ability picks from when they depend on the game: the colours of a commander's identity
//! (CR 903.4) and the types of mana permanents could produce (CR 106.7).

use mtg_cards::AbilityDef;
use mtg_cards::assemble::{ManaProduced, mana_shape};
use mtg_cards::effect::Produce;
use mtg_cards::refs::Selector;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;

use crate::ability_reader::abilities_of;
use crate::characteristics::{characteristics, controller_of_object};
use crate::object::{Chosen, PermanentState};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The five colours, in the order of the wheel.
pub(crate) const COLORS: [ManaType; 5] = [
    ManaType::White,
    ManaType::Blue,
    ManaType::Black,
    ManaType::Red,
    ManaType::Green,
];

/// Every way to pick `count` mana among `types` (all five colors when none are given), the mana of a pick in the order of
/// the types: the combinations of a multiset, so `{W}{U}` and `{U}{W}` are one.
pub(crate) fn combinations(types: &[ManaType], count: u32) -> Vec<Vec<ManaType>> {
    let pool: &[ManaType] = if types.is_empty() { &COLORS } else { types };
    let mut found: Vec<Vec<ManaType>> = vec![Vec::new()];
    for _ in 0..count {
        let mut next = Vec::new();
        for picked in &found {
            // Only a type at or after the last one picked, so each multiset is made once.
            let from = picked
                .last()
                .and_then(|last| pool.iter().position(|t| t == last))
                .unwrap_or(0);
            for kind in &pool[from..] {
                let mut longer = picked.clone();
                longer.push(*kind);
                next.push(longer);
            }
        }
        found = next;
    }
    found
}

/// Every way to pick mana among `types` (all five colors when none are given) for each count in `counts`, the picks of
/// the first count first and no more than `limit` in all. A count with more ways than are left offers only the picks
/// of one type and the picks of one type but one mana, and ends the list: the engine keeps a source for each way.
pub(crate) fn combinations_of(
    types: &[ManaType],
    counts: impl IntoIterator<Item = u32>,
    limit: usize,
) -> Vec<Vec<ManaType>> {
    let pool: &[ManaType] = if types.is_empty() { &COLORS } else { types };
    let mut found: Vec<Vec<ManaType>> = Vec::new();
    for count in counts {
        if found.len() >= limit {
            break;
        }
        let ways = mtg_cards::assemble::combinations(pool.len(), count);
        if ways <= limit - found.len() {
            found.extend(combinations(types, count));
            continue;
        }
        found.extend(pool.iter().map(|kind| vec![*kind; count as usize]));
        for (at, spare) in pool.iter().enumerate() {
            for (other, kind) in pool.iter().enumerate() {
                if at != other {
                    let mut pick = vec![*kind; (count as usize).saturating_sub(1)];
                    pick.push(*spare);
                    pick.sort_by_key(|mana| pool.iter().position(|t| t == mana));
                    found.push(pick);
                }
            }
        }
        found.truncate(limit);
        break;
    }
    found
}

/// The mana of a colour.
pub(crate) const fn mana_of(color: Color) -> ManaType {
    match color {
        Color::White => ManaType::White,
        Color::Blue => ManaType::Blue,
        Color::Black => ManaType::Black,
        Color::Red => ManaType::Red,
        Color::Green => ManaType::Green,
    }
}

/// The colours in the identity of `player`'s commanders, in the order of the wheel; none outside a Commander game.
pub(crate) fn identity_types(state: &GameState, player: PlayerId) -> Vec<ManaType> {
    let mut colors = Vec::new();
    for id in state.commanders() {
        let Some(object) = state.object(id).filter(|o| o.owner() == player) else {
            continue;
        };
        colors.extend(object.card().color_identity().iter());
    }
    COLORS
        .into_iter()
        .filter(|mana| colors.iter().any(|color| mana_of(*color) == *mana))
        .collect()
}

/// The types of mana the permanents that match `among` could produce, for the ability `place` of `source` that `player`
/// controls (no `place` means any of its abilities), in the order of the wheel and colourless last: colourless counts
/// only with `colorless`.
///
/// Costs and restrictions are ignored (CR 106.7); an ability that looks at what permanents could produce and depends
/// on itself adds nothing.
pub(crate) fn producible_types(
    state: &GameState,
    player: PlayerId,
    (source, place): (ObjectId, Option<usize>),
    among: &Selector,
    colorless: bool,
) -> Vec<ManaType> {
    let mut visiting = vec![(source, place.unwrap_or(usize::MAX))];
    let mut found = Vec::new();
    collect(state, player, source, among, &mut visiting, &mut found);
    let all = COLORS
        .into_iter()
        .chain(colorless.then_some(ManaType::Colorless));
    all.filter(|mana| found.contains(mana)).collect()
}

fn collect(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    among: &Selector,
    visiting: &mut Vec<(ObjectId, usize)>,
    found: &mut Vec<ManaType>,
) {
    let Some(origin) = state.object(source) else {
        return;
    };
    let scope = Scope::announcing(state, player, origin.reference());
    let candidates: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| scope.selects(among, o.reference()).unwrap_or(false))
        })
        .collect();
    for id in candidates {
        types_of(state, id, visiting, found);
    }
}

/// Adds to `found` the types the permanent `id` could produce.
fn types_of(
    state: &GameState,
    id: ObjectId,
    visiting: &mut Vec<(ObjectId, usize)>,
    found: &mut Vec<ManaType>,
) {
    let (Some(object), Some(chars)) = (state.object(id), characteristics(state, id)) else {
        return;
    };
    for def in &chars.abilities {
        let AbilityDef::Mana(mana) = def;
        found.push(mana.produces);
    }
    let controller = controller_of_object(state, object);
    for (place, (_, rules)) in abilities_of(state, id).into_iter().enumerate() {
        let Some(shape) = mana_shape(&rules) else {
            continue;
        };
        match shape.produces {
            ManaProduced::Fixed { mana, .. } => found.push(mana),
            ManaProduced::Mixed(list) | ManaProduced::OneOf(list) => found.extend_from_slice(list),
            ManaProduced::OneOfLists(lists) => found.extend(lists.iter().flatten().copied()),
            ManaProduced::AnyColor(_) | ManaProduced::AnyCombination { among: [], .. } => {
                found.extend(COLORS);
            }
            // Either branch could be added.
            ManaProduced::Instead { base, instead, .. } => {
                for produce in [base, instead] {
                    match produce {
                        Produce::Fixed(list) | Produce::OneOf(list) => {
                            found.extend_from_slice(list)
                        }
                        Produce::OneOfLists(lists) => found.extend(lists.iter().flatten().copied()),
                        _ => found.extend(COLORS),
                    }
                }
            }
            ManaProduced::AnyCombination { among, .. } => found.extend_from_slice(among),
            ManaProduced::ChosenColor(_) => {
                if let Some(Chosen::Color(color)) =
                    object.permanent().and_then(PermanentState::chosen)
                {
                    found.push(mana_of(*color));
                }
            }
            ManaProduced::OrChosenColor(printed) => {
                found.push(printed);
                if let Some(Chosen::Color(color)) =
                    object.permanent().and_then(PermanentState::chosen)
                {
                    found.push(mana_of(*color));
                }
            }
            ManaProduced::CommanderIdentity(_) => found.extend(identity_types(state, controller)),
            ManaProduced::ProducedBy {
                among, colorless, ..
            } => {
                if visiting
                    .iter()
                    .any(|(v, p)| *v == id && (*p == place || *p == usize::MAX))
                {
                    continue;
                }
                visiting.push((id, place));
                let mut inner = Vec::new();
                collect(state, controller, id, among, visiting, &mut inner);
                visiting.pop();
                found.extend(
                    inner
                        .into_iter()
                        .filter(|mana| colorless || *mana != ManaType::Colorless),
                );
            }
        }
    }
}
