//! What mana added with a restriction may be spent on ("Spend this mana only to cast creature spells").

use mtg_core::card::{CardType, CardTypes, Subtype, Supertype};
use serde::{Deserialize, Serialize};

/// A kind of spell, or of the source of an ability: every part that is listed must hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SpendFilter {
    /// Has at least one of these card types; none listed means any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CardType>>", optional))]
    pub types: Vec<CardType>,
    /// Has none of these card types ("noncreature").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CardType>>", optional))]
    pub without: Vec<CardType>,
    /// Has at least one of these supertypes; none listed means any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Supertype>>", optional))]
    pub supertypes: Vec<Supertype>,
    /// Has at least one of these subtypes; none listed means any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Subtype>>", optional))]
    pub subtypes: Vec<Subtype>,
    /// Has two or more colors.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub multicolored: bool,
    /// Has the creature type chosen as the source entered the battlefield; the engine puts that type in `subtypes` when the mana is added.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub chosen_type: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

/// What a spell or an ability source is, as far as a restriction reads it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SpendTraits {
    /// Its card types.
    pub types: CardTypes,
    /// Its supertypes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Supertype>>", optional))]
    pub supertypes: Vec<Supertype>,
    /// Its subtypes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Subtype>>", optional))]
    pub subtypes: Vec<Subtype>,
    /// How many colors it has.
    pub colors: u8,
}

impl SpendFilter {
    /// A filter of one card type ("creature spells").
    pub fn of_type(card_type: CardType) -> Self {
        SpendFilter {
            types: vec![card_type],
            ..SpendFilter::default()
        }
    }

    /// The filter with the chosen creature type put in its subtypes; `None` when it asks for one and none was chosen.
    pub fn resolved(&self, chosen: Option<&Subtype>) -> Option<SpendFilter> {
        if !self.chosen_type {
            return Some(self.clone());
        }
        Some(SpendFilter {
            subtypes: vec![chosen?.clone()],
            chosen_type: false,
            ..self.clone()
        })
    }

    /// Whether something with these traits is of the kind; a filter whose chosen type is not resolved holds for nothing.
    pub fn holds(&self, traits: &SpendTraits) -> bool {
        let any = |wanted: &[CardType]| wanted.iter().any(|t| traits.types.contains(*t));
        !self.chosen_type
            && (self.types.is_empty() || any(&self.types))
            && !any(&self.without)
            && (self.supertypes.is_empty()
                || self
                    .supertypes
                    .iter()
                    .any(|s| traits.supertypes.contains(s)))
            && (self.subtypes.is_empty()
                || self.subtypes.iter().any(|s| traits.subtypes.contains(s)))
            && (!self.multicolored || traits.colors >= 2)
    }
}

/// One thing mana may be spent on; a restriction lists the uses that are allowed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub enum SpendOn {
    /// Casting a spell of the kind.
    Spell(SpendFilter),
    /// Activating an ability whose source is of the kind; a mana ability is not paid for, so none counts.
    Ability(SpendFilter),
    /// Paying a cost that has {X} in it, of a spell or of an ability.
    CostWithX,
}

impl SpendOn {
    /// The use with the chosen creature type put in its filter; `None` when it asks for a type and none was chosen.
    pub fn resolved(&self, chosen: Option<&Subtype>) -> Option<SpendOn> {
        Some(match self {
            SpendOn::Spell(filter) => SpendOn::Spell(filter.resolved(chosen)?),
            SpendOn::Ability(filter) => SpendOn::Ability(filter.resolved(chosen)?),
            SpendOn::CostWithX => SpendOn::CostWithX,
        })
    }

    /// "Cast creature spells."
    pub fn spells(filter: SpendFilter) -> Self {
        SpendOn::Spell(filter)
    }

    /// "Activate abilities."
    pub fn abilities() -> Self {
        SpendOn::Ability(SpendFilter::default())
    }
}

/// The uses with the chosen creature type put in them; the uses that ask for a type when none was chosen are dropped,
/// and `None` when that leaves a restriction no use satisfies.
pub fn resolve_uses(only: &[SpendOn], chosen: Option<&Subtype>) -> Option<Vec<SpendOn>> {
    let kept: Vec<SpendOn> = only.iter().filter_map(|on| on.resolved(chosen)).collect();
    (only.is_empty() || !kept.is_empty()).then_some(kept)
}

#[cfg(test)]
mod tests {
    //! Goal: a spend filter holds for exactly the things whose traits satisfy every part it lists, and a use serializes as the card files write it.
    use rstest::rstest;

    use super::*;

    fn traits(
        types: &[CardType],
        supertypes: &[Supertype],
        subtypes: &[&str],
        colors: u8,
    ) -> SpendTraits {
        SpendTraits {
            types: types.iter().copied().collect(),
            supertypes: supertypes.to_vec(),
            subtypes: subtypes.iter().map(|s| Subtype((*s).to_owned())).collect(),
            colors,
        }
    }

    #[rstest]
    #[case::any(r"{}", &[CardType::Land], &[], &[], 0, true)]
    #[case::type_held(r#"{"types":["Creature"]}"#, &[CardType::Creature], &[], &[], 1, true)]
    #[case::type_missing(r#"{"types":["Creature"]}"#, &[CardType::Artifact], &[], &[], 0, false)]
    #[case::one_of_two_types(r#"{"types":["Instant","Sorcery"]}"#, &[CardType::Sorcery], &[], &[], 1, true)]
    #[case::artifact_creature_is_both(r#"{"types":["Artifact"]}"#, &[CardType::Artifact, CardType::Creature], &[], &[], 0, true)]
    #[case::noncreature(r#"{"without":["Creature"]}"#, &[CardType::Instant], &[], &[], 1, true)]
    #[case::noncreature_refuses_a_creature(r#"{"without":["Creature"]}"#, &[CardType::Artifact, CardType::Creature], &[], &[], 0, false)]
    #[case::legendary(r#"{"supertypes":["Legendary"]}"#, &[CardType::Creature], &[Supertype::Legendary], &[], 1, true)]
    #[case::subtype(r#"{"types":["Creature"],"subtypes":["Dragon"]}"#, &[CardType::Creature], &[], &["Dragon"], 1, true)]
    #[case::wrong_subtype(r#"{"types":["Creature"],"subtypes":["Dragon"]}"#, &[CardType::Creature], &[], &["Elf"], 1, false)]
    #[case::multicolored(r#"{"multicolored":true}"#, &[CardType::Creature], &[], &[], 2, true)]
    #[case::one_color_is_not_multicolored(r#"{"multicolored":true}"#, &[CardType::Creature], &[], &[], 1, false)]
    fn a_filter_holds_when_every_listed_part_does(
        #[case] json: &str,
        #[case] types: &[CardType],
        #[case] supertypes: &[Supertype],
        #[case] subtypes: &[&str],
        #[case] colors: u8,
        #[case] holds: bool,
    ) {
        let filter: SpendFilter = serde_json::from_str(json).unwrap();
        assert_eq!(
            filter.holds(&traits(types, supertypes, subtypes, colors)),
            holds
        );
    }

    fn elf() -> Subtype {
        Subtype("Elf".to_owned())
    }

    fn chosen_creature() -> SpendFilter {
        SpendFilter {
            types: vec![CardType::Creature],
            chosen_type: true,
            ..SpendFilter::default()
        }
    }

    #[rstest]
    #[case::the_type_chosen(Some(elf()), &["Elf"], true)]
    #[case::another_type(Some(elf()), &["Goblin"], false)]
    #[case::no_type_chosen(None, &["Elf"], false)]
    fn a_filter_of_the_chosen_type_holds_for_that_type_once_resolved(
        #[case] chosen: Option<Subtype>,
        #[case] subtypes: &[&str],
        #[case] holds: bool,
    ) {
        let filter = chosen_creature();
        let creature = traits(&[CardType::Creature], &[], subtypes, 1);
        assert!(!filter.holds(&creature), "unresolved, it holds for nothing");
        let resolved = filter.resolved(chosen.as_ref());
        assert_eq!(
            resolved.is_some_and(|resolved| resolved.holds(&creature)),
            holds
        );
    }

    #[test]
    fn uses_without_a_chosen_type_are_dropped_and_a_restriction_left_empty_is_refused() {
        let uses = [SpendOn::Spell(chosen_creature()), SpendOn::abilities()];
        assert_eq!(resolve_uses(&uses, None), Some(vec![SpendOn::abilities()]));
        assert_eq!(resolve_uses(&uses[..1], None), None);
        let chosen = elf();
        let kept = resolve_uses(&uses[..1], Some(&chosen)).unwrap();
        assert_eq!(kept.len(), 1);
        assert!(matches!(&kept[0], SpendOn::Spell(f) if f.subtypes == [elf()] && !f.chosen_type));
        assert_eq!(resolve_uses(&[], None), Some(Vec::new()));
    }

    #[rstest]
    #[case::spells(
        SpendOn::spells(SpendFilter::of_type(CardType::Creature)),
        r#"{"Spell":{"types":["Creature"]}}"#
    )]
    #[case::abilities(SpendOn::abilities(), r#"{"Ability":{}}"#)]
    #[case::x(SpendOn::CostWithX, r#""CostWithX""#)]
    fn a_use_is_written_without_its_empty_parts(#[case] on: SpendOn, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&on).unwrap(), json);
        assert_eq!(serde_json::from_str::<SpendOn>(json).unwrap(), on);
    }
}
