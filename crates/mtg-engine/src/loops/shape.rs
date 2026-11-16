//! The fingerprint of a priority point: its shape (turn, step, holder, the stack by name) must repeat exactly, and its
//! counts (life, zone sizes, permanents by what they are) may grow by the same amount each cycle. Object ids are left
//! out, so a token made new each cycle counts as one more of its kind.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use mtg_core::ids::PlayerId;

use crate::state::GameState;
use crate::zones::Zone;

/// What a count measures; life totals are told apart, as a loop that only lowers life ends with the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Measure {
    /// A player's life total.
    Life,
    /// Anything else.
    Other,
}

/// The counts of a priority point, sorted by key.
pub(crate) type Counts = Vec<((Measure, u64), i64)>;

fn hash_of(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// The shape of the point where `holder` has priority: what must be the same each cycle.
pub(crate) fn shape(state: &GameState, holder: PlayerId) -> u64 {
    let mut hasher = DefaultHasher::new();
    let turn = state.turn();
    (turn.number(), turn.step(), state.active_player(), holder).hash(&mut hasher);
    for id in state.stack() {
        if let Some(object) = state.object(*id) {
            (object.kind(), &object.card().name, object.controller()).hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// The names of the objects on the stack, bottom first.
pub(crate) fn stack_names(state: &GameState) -> Vec<String> {
    state
        .stack()
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|object| object.card().name.clone())
        .collect()
}

/// The counts of the point: per player life, poison and other counters, zone sizes and floating mana; per kind of
/// permanent (name, controller, owner, token, tapped, damage, counters, face down, attached) how many there are; the
/// exile, the delayed triggers and the effects in force.
pub(crate) fn counts(state: &GameState) -> Counts {
    let mut counts: Counts = Vec::new();
    let mut add = |measure: Measure, key: u64, value: i64| counts.push(((measure, key), value));
    for player in state.players() {
        let id = player.id;
        add(Measure::Life, hash_of(("life", id)), i64::from(player.life));
        add(
            Measure::Other,
            hash_of(("mana", id)),
            i64::from(player.mana.total()),
        );
        for (kind, count) in &player.counters {
            add(Measure::Other, hash_of(("counter", id, kind)), i64::from(*count));
        }
        for (name, zone) in [
            ("hand", Zone::Hand(id)),
            ("library", Zone::Library(id)),
            ("graveyard", Zone::Graveyard(id)),
            ("command", Zone::Command(id)),
        ] {
            let size = i64::try_from(state.zone(zone).len()).unwrap_or(i64::MAX);
            add(Measure::Other, hash_of((name, id)), size);
        }
    }
    for id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(*id) else {
            continue;
        };
        let permanent = object.permanent();
        let key = hash_of((
            &object.card().name,
            object.controller(),
            object.owner(),
            object.kind(),
            object.face_down.is_some(),
            permanent.map(|p| (p.tapped, p.damage, &p.counters, p.attached_to.is_some())),
        ));
        add(Measure::Other, key, 1);
    }
    let sizes = [
        ("exile", state.zone(Zone::Exile).len()),
        ("delayed", state.delayed().len()),
        ("effects", state.effects().len()),
        ("rules", state.rule_effects().len()),
    ];
    for (name, size) in sizes {
        add(
            Measure::Other,
            hash_of(name),
            i64::try_from(size).unwrap_or(i64::MAX),
        );
    }
    counts.sort_unstable_by_key(|(key, _)| *key);
    counts.dedup_by(|later, earlier| {
        let same = later.0 == earlier.0;
        if same {
            earlier.1 += later.1;
        }
        same
    });
    counts
}

/// The change from `before` to `after`, the keys whose value moved, sorted.
pub(crate) fn delta(before: &Counts, after: &Counts) -> Counts {
    let mut change: std::collections::BTreeMap<(Measure, u64), i64> = std::collections::BTreeMap::new();
    for (key, value) in before {
        *change.entry(*key).or_default() -= value;
    }
    for (key, value) in after {
        *change.entry(*key).or_default() += value;
    }
    change.into_iter().filter(|(_, value)| *value != 0).collect()
}

/// The key a loop is known by in its turn: the shapes of its points, sorted, so it is the same wherever it is found.
pub(crate) fn loop_key(shapes: &[u64]) -> u64 {
    hash_of(shapes)
}
