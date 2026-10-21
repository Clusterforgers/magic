//! Abilities as the engine names and stores them: a reference to one, and what one becomes on the stack.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::choose::{Choices, Target};
use crate::object::ObjectRef;
use crate::resolve::Bindings;
use crate::snapshot::Snapshot;
use crate::triggers::DelayedId;

/// What gives an object an ability it does not print: a continuous effect, or one modification of a static ability.
///
/// The name holds as long as the grant does, and it resolves to the very ability the effect grants whether or not the
/// object that has it is still there: a registered effect by its id, a static by the card it is printed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Grant {
    /// A continuous effect that a resolved spell or ability began (`Effect::Continuous`), named by its id.
    Effect(EffectId),
    /// The modification `index` of the static ability `rule` printed on the card of `source`, where `rule` is the
    /// static's index among the rules of that card.
    Static {
        /// The object with the static ability.
        source: ObjectId,
        /// The static's index among the rules of its card.
        rule: usize,
        /// The modification's index among the static's modifications.
        index: usize,
    },
}

/// Where an ability of an object comes from, which with the object and an index names it (`ability_reader`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum AbilitySource {
    /// Printed on the object's card: the index is the ability's index among the card's rules.
    #[default]
    Printed,
    /// Given to the object by an effect: the index is 0, since a modification grants one ability.
    Granted(Grant),
    /// Added by a keyword ability printed on the object's card (`KeywordRules::adds_abilities`): the keyword is the
    /// card's rule with this index, and the address's index is the ability's place among those the keyword adds.
    PrintedKeyword(usize),
    /// Added by a keyword an effect gave the object: the grant names the effect or the static ability that gave
    /// the keyword, and the address's index is the ability's place among those the keyword adds.
    GrantedKeyword(Grant),
}

impl AbilitySource {
    /// Whether this is the printed source, which a serialized [`AbilityRef`] leaves out.
    pub const fn is_printed(&self) -> bool {
        matches!(self, AbilitySource::Printed)
    }
}

/// An ability of an object, named by the object, where the ability comes from and an index.
///
/// A printed ability is serialized as `{"object", "ability"}`; the source is left out while it is `Printed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AbilityRef {
    /// The object with the ability.
    pub object: ObjectId,
    /// For a printed ability the index into the rules of its card (`CardDefinition::rules`); for one a keyword adds,
    /// its place among those the keyword adds.
    pub ability: usize,
    /// Where the ability comes from.
    #[serde(default, skip_serializing_if = "AbilitySource::is_printed")]
    #[cfg_attr(feature = "ts", ts(as = "Option<AbilitySource>", optional))]
    pub source: AbilitySource,
}

impl AbilityRef {
    /// The address of the ability at `ability` among the rules printed on the card of `object`.
    pub const fn printed(object: ObjectId, ability: usize) -> Self {
        AbilityRef {
            object,
            ability,
            source: AbilitySource::Printed,
        }
    }

    /// The address of the ability `object` has because of `grant`.
    pub const fn granted(object: ObjectId, grant: Grant) -> Self {
        AbilityRef {
            object,
            ability: 0,
            source: AbilitySource::Granted(grant),
        }
    }

    /// The address of the ability at `index` among those the keyword `source` names adds to `object`.
    pub const fn keyword(object: ObjectId, source: AbilitySource, index: usize) -> Self {
        AbilityRef {
            object,
            ability: index,
            source,
        }
    }
}

/// What made a triggered ability trigger: the objects and players the event was about.
///
/// Objects are kept as they were when the event happened (CR 603.10a), so the ability can still read them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Cause {
    /// The things the event was about (`Ref::Triggering`).
    pub subjects: Vec<Target>,
    /// The objects among them, as they were.
    pub snapshots: Vec<Snapshot>,
    /// How much the event was (`Amount::Triggering`): the damage dealt, the life gained or lost, the counters put or
    /// removed, how many events a batch held; zero for an event with no amount.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
    pub amount: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(amount: &u32) -> bool {
    *amount == 0
}

/// An ability put on the stack, activated by its controller or triggered: what the `AbilityPutOnStack` event says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PutOnStack {
    /// The ability's new object.
    pub object: ObjectId,
    /// The object it comes from, as it was.
    pub source: ObjectRef,
    /// The source as it was when the ability was put on the stack (a cost may have moved it since).
    pub snapshot: Snapshot,
    /// Its index among the rules of the source's card; 0 for a granted ability; its place among those its keyword
    /// adds for one a keyword added.
    pub ability: usize,
    /// Where the ability comes from: the card, an effect that granted it, or a keyword.
    #[serde(default, skip_serializing_if = "AbilitySource::is_printed")]
    #[cfg_attr(feature = "ts", ts(as = "Option<AbilitySource>", optional))]
    pub origin: AbilitySource,
    /// Who controls it.
    pub controller: PlayerId,
    /// What the controller announced: targets, modes and the like.
    #[serde(default, skip_serializing_if = "Choices::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Choices>", optional))]
    pub choices: Choices,
    /// What triggered it, for a triggered ability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cause: Option<Cause>,
    /// The delayed trigger it is one of, for a delayed triggered ability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub delayed: Option<DelayedId>,
}

/// An activated or triggered ability on the stack: it exists on its own once put there (CR 113.7a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityObject {
    pub(crate) source: ObjectRef,
    pub(crate) snapshot: Snapshot,
    pub(crate) index: usize,
    pub(crate) origin: AbilitySource,
    pub(crate) rules: Arc<Ability>,
    pub(crate) cause: Option<Cause>,
    pub(crate) delayed: Option<DelayedId>,
    pub(crate) bindings: Bindings,
}

impl AbilityObject {
    /// The object the ability came from, as it was when the ability was put on the stack.
    pub const fn source(&self) -> ObjectRef {
        self.source
    }

    /// The source as it was then (last known information).
    pub const fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Its index among the rules of the source's card.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Where the ability comes from: the card, an effect that granted it, or a keyword.
    pub const fn origin(&self) -> AbilitySource {
        self.origin
    }

    /// The rules it runs; the ability needs no source to resolve.
    pub fn rules(&self) -> &Ability {
        &self.rules
    }

    /// The rules it runs, shared with the ability object.
    pub(crate) fn shared_rules(&self) -> Arc<Ability> {
        Arc::clone(&self.rules)
    }

    /// What triggered it, for a triggered ability.
    pub const fn cause(&self) -> Option<&Cause> {
        self.cause.as_ref()
    }

    /// The delayed trigger it is one of, for a delayed triggered ability.
    pub const fn delayed(&self) -> Option<DelayedId> {
        self.delayed
    }

    /// The names a delayed trigger took from the resolution that set it up, which its own resolution starts with.
    pub const fn bindings(&self) -> &Bindings {
        &self.bindings
    }
}

// The rules follow from the source's card and the index (or from the delayed trigger, which the log names), so
// hashing them would only cost time.
impl Hash for AbilityObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.snapshot.hash(state);
        self.index.hash(state);
        // A printed ability hashes as it did before the origin was kept.
        if !self.origin.is_printed() {
            self.origin.hash(state);
        }
        self.cause.hash(state);
        self.delayed.hash(state);
        self.bindings.hash(state);
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the address of a printed ability is written as `{"object", "ability"}` and read from it, whatever the object and the index, exactly as before it had a source; a source spelled out as `Printed` reads the same and is left out when written.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::the_first_ability(r#"{"object":3,"ability":0}"#, 3, 0)]
    #[case::a_later_ability(r#"{"object":3,"ability":4}"#, 3, 4)]
    #[case::the_first_object(r#"{"object":0,"ability":1}"#, 0, 1)]
    #[case::the_largest_ids(
        r#"{"object":18446744073709551615,"ability":9007199254740991}"#,
        u64::MAX,
        9_007_199_254_740_991
    )]
    fn an_address_of_a_printed_ability_reads_and_writes_as_it_always_did(
        #[case] old: &str,
        #[case] object: u64,
        #[case] ability: usize,
    ) {
        let read: AbilityRef = serde_json::from_str(old).unwrap();
        assert_eq!(read, AbilityRef::printed(ObjectId(object), ability));
        assert_eq!(serde_json::to_string(&read).unwrap(), old);
    }

    #[test]
    fn a_source_spelled_out_as_printed_reads_the_same_and_is_not_written() {
        let spelled = r#"{"object":7,"ability":2,"source":"Printed"}"#;
        let read: AbilityRef = serde_json::from_str(spelled).unwrap();
        assert_eq!(read, AbilityRef::printed(ObjectId(7), 2));
        assert_eq!(
            serde_json::to_string(&read).unwrap(),
            r#"{"object":7,"ability":2}"#
        );
    }

    #[test]
    fn a_source_that_is_not_known_is_refused() {
        let read =
            serde_json::from_str::<AbilityRef>(r#"{"object":7,"ability":2,"source":"Gone"}"#);
        assert!(read.is_err());
    }

    #[test]
    fn the_default_source_is_the_printed_one() {
        assert_eq!(AbilitySource::default(), AbilitySource::Printed);
        assert!(AbilitySource::default().is_printed());
    }
}
