//! What changed in a viewer's objects between the state before a batch and the state after it.

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::ObjectId;

use crate::view::ObjectView;

/// The object half of an event batch: what to insert or replace and what to drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectDiff {
    /// Objects of the new view that the old view lacks or shows differently.
    pub(crate) objects: Vec<ObjectView>,
    /// Ids the old view lists that the new view does not.
    pub(crate) gone: Vec<ObjectId>,
}

/// Compares two views of one viewer's objects, each in id order, and keeps that order.
///
/// With `everything`, `objects` is the whole new view rather than only what changed.
pub(crate) fn diff(before: &[ObjectView], after: &[ObjectView], everything: bool) -> ObjectDiff {
    let known: BTreeMap<ObjectId, &ObjectView> = before.iter().map(|o| (o.id, o)).collect();
    let kept: BTreeSet<ObjectId> = after.iter().map(|o| o.id).collect();
    ObjectDiff {
        objects: after
            .iter()
            .filter(|&o| everything || known.get(&o.id).copied() != Some(o))
            .cloned()
            .collect(),
        gone: before
            .iter()
            .map(|o| o.id)
            .filter(|id| !kept.contains(id))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the object diff lists exactly the new or changed objects and exactly the vanished ids,
    //! whatever the two views hold, and applying it to the old view yields the new one.

    use super::*;
    use mtg_core::ids::PlayerId;
    use mtg_engine::object::ObjectKind;
    use mtg_engine::zones::Zone;
    use proptest::prelude::*;
    use rstest::rstest;

    use crate::view::PermanentView;

    fn object(id: u64, zone: Zone, power: i32) -> ObjectView {
        ObjectView {
            id: ObjectId(id),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone,
            name: format!("Card {id}"),
            mana_cost: None,
            supertypes: Vec::new(),
            types: Vec::new(),
            subtypes: Vec::new(),
            colors: Vec::new(),
            power: Some(power),
            toughness: Some(power),
            abilities: Vec::new(),
            keywords: Vec::new(),
            permanent: (zone == Zone::Battlefield).then_some(PermanentView {
                tapped: false,
                damage: 0,
                summoning_sick: false,
                prepared: false,
                level: None,
                doors: None,
            }),
            face_down: false,
            face: None,
            copy: None,
            merged: Vec::new(),
            kind: ObjectKind::Card,
            source: None,
            targets: Vec::new(),
            targets_legal: Vec::new(),
            counters: Vec::new(),
            loyalty_abilities: Vec::new(),
            attached_to: None,
            attached_player: None,
            paired_with: None,
            attachments: Vec::new(),
            gives: None,
            manual: Vec::new(),
            read: false,
            note: None,
            identity: None,
            activated: Vec::new(),
        }
    }

    fn ids(objects: &[ObjectView]) -> Vec<u64> {
        objects.iter().map(|o| o.id.get()).collect()
    }

    fn gone(diff: &ObjectDiff) -> Vec<u64> {
        diff.gone.iter().map(|id| id.get()).collect()
    }

    const FIELD: Zone = Zone::Battlefield;
    const HAND: Zone = Zone::Hand(PlayerId(0));

    #[rstest]
    #[case::nothing_changed(&[(1, FIELD, 2), (2, FIELD, 3)], &[(1, FIELD, 2), (2, FIELD, 3)], &[], &[])]
    #[case::power_changed(&[(1, FIELD, 2), (2, FIELD, 3)], &[(1, FIELD, 2), (2, FIELD, 4)], &[2], &[])]
    #[case::moved_between_zones(&[(1, HAND, 2)], &[(1, FIELD, 2)], &[1], &[])]
    #[case::appeared(&[(1, FIELD, 2)], &[(1, FIELD, 2), (5, HAND, 1)], &[5], &[])]
    #[case::vanished(&[(1, FIELD, 2), (2, FIELD, 3)], &[(2, FIELD, 3)], &[], &[1])]
    #[case::swapped(&[(1, FIELD, 2)], &[(2, FIELD, 2)], &[2], &[1])]
    #[case::from_empty(&[], &[(3, HAND, 1), (4, HAND, 1)], &[3, 4], &[])]
    #[case::to_empty(&[(3, HAND, 1), (4, HAND, 1)], &[], &[], &[3, 4])]
    #[case::both_empty(&[], &[], &[], &[])]
    fn lists_what_changed_and_what_vanished(
        #[case] before: &[(u64, Zone, i32)],
        #[case] after: &[(u64, Zone, i32)],
        #[case] changed: &[u64],
        #[case] vanished: &[u64],
    ) {
        let build = |list: &[(u64, Zone, i32)]| -> Vec<ObjectView> {
            list.iter()
                .map(|&(id, zone, p)| object(id, zone, p))
                .collect()
        };
        let result = diff(&build(before), &build(after), false);
        assert_eq!(ids(&result.objects), changed);
        assert_eq!(gone(&result), vanished);
        let everything = diff(&build(before), &build(after), true);
        assert_eq!(ids(&everything.objects), ids(&build(after)));
        assert_eq!(gone(&everything), vanished, "gone ignores `everything`");
    }

    fn views() -> impl Strategy<Value = Vec<ObjectView>> {
        // Zone and power vary per id, so some ids repeat unchanged between two draws and some differ.
        prop::collection::btree_map(0u64..24, (0u8..3, 0i32..3), 0..16).prop_map(|map| {
            map.into_iter()
                .map(|(id, (zone, power))| {
                    let zone = [FIELD, HAND, Zone::Exile][usize::from(zone)];
                    object(id, zone, power)
                })
                .collect()
        })
    }

    proptest! {
        #[test]
        fn replaying_the_diff_on_the_old_view_gives_the_new_one(before in views(), after in views()) {
            let result = diff(&before, &after, false);
            let mut replayed: BTreeMap<ObjectId, ObjectView> =
                before.iter().map(|o| (o.id, o.clone())).collect();
            for object in &result.objects {
                replayed.insert(object.id, object.clone());
            }
            for id in &result.gone {
                replayed.remove(id);
            }
            let expected: BTreeMap<ObjectId, ObjectView> =
                after.iter().map(|o| (o.id, o.clone())).collect();
            prop_assert_eq!(replayed, expected);
            let old: BTreeMap<ObjectId, &ObjectView> = before.iter().map(|o| (o.id, o)).collect();
            for object in &result.objects {
                prop_assert!(old.get(&object.id).copied() != Some(object), "unchanged object listed");
            }
            for id in &result.gone {
                prop_assert!(after.iter().all(|o| o.id != *id), "listed as gone but still visible");
            }
        }
    }
}
