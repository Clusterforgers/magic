//! House rules: the values a game's creator sets in place of the format's.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The starting life totals a table may set.
pub const STARTING_LIFE: RangeInclusive<i32> = 1..=1000;
/// The opening hand sizes a table may set.
pub const OPENING_HAND_SIZE: RangeInclusive<u32> = 1..=20;
/// The free mulligans a table may set; 100 outlasts any hand, so no mulligan costs a card.
pub const FREE_MULLIGANS: RangeInclusive<u32> = 0..=100;
/// The caps on mulligans a table may set; 0 forbids them.
pub const MULLIGAN_CAP: RangeInclusive<u32> = 0..=100;
/// The maximum hand sizes a table may set.
pub const MAX_HAND_SIZE: RangeInclusive<u32> = 0..=50;

/// Whether the player who takes the first turn skips their first draw, in place of the format's rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SkipFirstDraw {
    /// In every game, whatever the number of players.
    Always,
    /// In no game.
    Never,
}

/// The most cards a player keeps in hand at the cleanup step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum HandLimit {
    /// No more than this many; 0 discards the whole hand.
    Cards(u32),
    /// Nobody discards at cleanup.
    Unlimited,
}

/// What a game's creator sets in place of the format's values. A rule that is left out keeps the
/// format's value; a value equal to the format's is kept as given.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(default, deny_unknown_fields)]
pub struct HouseRules {
    /// Life each player starts with.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub starting_life: Option<i32>,
    /// Cards in an opening hand.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub opening_hand_size: Option<u32>,
    /// Mulligans that put no card on the bottom; a large number makes every mulligan free.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub free_mulligans: Option<u32>,
    /// The most mulligans a player may take.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mulligan_cap: Option<u32>,
    /// Whether the first player skips the first draw.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub skip_first_draw: Option<SkipFirstDraw>,
    /// The maximum hand size.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_hand_size: Option<HandLimit>,
}

/// Why house rules cannot be played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HouseRuleError {
    /// A value lies outside the range its rule allows.
    #[error("{field}: {got} is not between {min} and {max}")]
    OutOfRange {
        /// The name of the rule on the wire.
        field: &'static str,
        /// The value given.
        got: i64,
        /// The least value allowed.
        min: i64,
        /// The most value allowed.
        max: i64,
    },
}

impl HouseRuleError {
    /// The name of the rule that was refused.
    pub const fn field(&self) -> &'static str {
        match self {
            HouseRuleError::OutOfRange { field, .. } => field,
        }
    }
}

fn within<T: Copy + Into<i64> + PartialOrd>(
    field: &'static str,
    value: Option<T>,
    range: &RangeInclusive<T>,
) -> Result<(), HouseRuleError> {
    match value {
        Some(got) if !range.contains(&got) => Err(HouseRuleError::OutOfRange {
            field,
            got: got.into(),
            min: (*range.start()).into(),
            max: (*range.end()).into(),
        }),
        _ => Ok(()),
    }
}

impl HouseRules {
    /// Whether no rule is set, so the format's values all stand.
    pub fn is_empty(&self) -> bool {
        *self == HouseRules::default()
    }

    /// Checks every value that is set against the range of its rule.
    pub fn validate(&self) -> Result<(), HouseRuleError> {
        within("starting_life", self.starting_life, &STARTING_LIFE)?;
        within(
            "opening_hand_size",
            self.opening_hand_size,
            &OPENING_HAND_SIZE,
        )?;
        within("free_mulligans", self.free_mulligans, &FREE_MULLIGANS)?;
        within("mulligan_cap", self.mulligan_cap, &MULLIGAN_CAP)?;
        let cards = match self.max_hand_size {
            Some(HandLimit::Cards(n)) => Some(n),
            _ => None,
        };
        within("max_hand_size", cards, &MAX_HAND_SIZE)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every rule is checked against its range and named in the error, the wire form leaves unset rules out and loads without any, and an unknown rule is refused.
    use proptest::prelude::*;
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    fn rules(field: &str, value: &serde_json::Value) -> Result<HouseRules, serde_json::Error> {
        serde_json::from_value(json!({ field: value }))
    }

    #[rstest]
    #[case::life_low("starting_life", json!(0), Some("starting_life"))]
    #[case::life_negative("starting_life", json!(-5), Some("starting_life"))]
    #[case::life_least("starting_life", json!(1), None)]
    #[case::life_most("starting_life", json!(1000), None)]
    #[case::life_high("starting_life", json!(1001), Some("starting_life"))]
    #[case::hand_low("opening_hand_size", json!(0), Some("opening_hand_size"))]
    #[case::hand_least("opening_hand_size", json!(1), None)]
    #[case::hand_most("opening_hand_size", json!(20), None)]
    #[case::hand_high("opening_hand_size", json!(21), Some("opening_hand_size"))]
    #[case::free_none("free_mulligans", json!(0), None)]
    #[case::free_most("free_mulligans", json!(100), None)]
    #[case::free_high("free_mulligans", json!(101), Some("free_mulligans"))]
    #[case::cap_none("mulligan_cap", json!(0), None)]
    #[case::cap_most("mulligan_cap", json!(100), None)]
    #[case::cap_high("mulligan_cap", json!(101), Some("mulligan_cap"))]
    #[case::limit_none("max_hand_size", json!({"cards": 0}), None)]
    #[case::limit_most("max_hand_size", json!({"cards": 50}), None)]
    #[case::limit_high("max_hand_size", json!({"cards": 51}), Some("max_hand_size"))]
    #[case::unlimited("max_hand_size", json!("unlimited"), None)]
    #[case::always("skip_first_draw", json!("always"), None)]
    #[case::never("skip_first_draw", json!("never"), None)]
    fn each_rule_has_its_range(
        #[case] field: &str,
        #[case] value: serde_json::Value,
        #[case] refused: Option<&str>,
    ) {
        let parsed = rules(field, &value).unwrap();
        assert_eq!(parsed.validate().err().map(|e| e.field()), refused);
    }

    #[test]
    fn the_error_says_the_value_and_the_range() {
        let error = HouseRules {
            starting_life: Some(5000),
            ..HouseRules::default()
        }
        .validate()
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "starting_life: 5000 is not between 1 and 1000"
        );
    }

    #[test]
    fn the_first_rule_out_of_range_is_named() {
        let both = HouseRules {
            starting_life: Some(0),
            mulligan_cap: Some(500),
            ..HouseRules::default()
        };
        assert_eq!(both.validate().unwrap_err().field(), "starting_life");
    }

    #[rstest]
    #[case::nothing(json!({}), true)]
    #[case::null_rule(json!({"starting_life": null}), true)]
    #[case::one_rule(json!({"starting_life": 40}), false)]
    #[case::a_value_equal_to_the_formats(json!({"starting_life": 20}), false)]
    fn a_game_with_no_rule_set_has_none(#[case] body: serde_json::Value, #[case] empty: bool) {
        let rules: HouseRules = serde_json::from_value(body).unwrap();
        assert_eq!(rules.is_empty(), empty);
    }

    #[test]
    fn an_empty_rule_set_serializes_to_nothing_and_a_set_one_to_its_rules_only() {
        assert_eq!(
            serde_json::to_value(HouseRules::default()).unwrap(),
            json!({})
        );
        let set = HouseRules {
            starting_life: Some(30),
            max_hand_size: Some(HandLimit::Cards(9)),
            skip_first_draw: Some(SkipFirstDraw::Never),
            ..HouseRules::default()
        };
        assert_eq!(
            serde_json::to_value(set).unwrap(),
            json!({"starting_life": 30, "skip_first_draw": "never", "max_hand_size": {"cards": 9}})
        );
    }

    #[test]
    fn an_unknown_rule_is_refused() {
        assert!(serde_json::from_value::<HouseRules>(json!({"mana_burn": true})).is_err());
    }

    fn any_rules() -> impl Strategy<Value = HouseRules> {
        (
            proptest::option::of(any::<i32>()),
            proptest::option::of(any::<u32>()),
            proptest::option::of(any::<u32>()),
            proptest::option::of(any::<u32>()),
            proptest::option::of(prop_oneof![
                Just(SkipFirstDraw::Always),
                Just(SkipFirstDraw::Never)
            ]),
            proptest::option::of(prop_oneof![
                any::<u32>().prop_map(HandLimit::Cards),
                Just(HandLimit::Unlimited)
            ]),
        )
            .prop_map(|(life, hand, free, cap, first, limit)| HouseRules {
                starting_life: life,
                opening_hand_size: hand,
                free_mulligans: free,
                mulligan_cap: cap,
                skip_first_draw: first,
                max_hand_size: limit,
            })
    }

    proptest! {
        #[test]
        fn rules_survive_json_and_validation_only_ever_fails_by_range(rules in any_rules()) {
            let back: HouseRules = serde_json::from_str(&serde_json::to_string(&rules).unwrap()).unwrap();
            prop_assert_eq!(back, rules);
            let in_range = rules.starting_life.is_none_or(|n| STARTING_LIFE.contains(&n))
                && rules.opening_hand_size.is_none_or(|n| OPENING_HAND_SIZE.contains(&n))
                && rules.free_mulligans.is_none_or(|n| FREE_MULLIGANS.contains(&n))
                && rules.mulligan_cap.is_none_or(|n| MULLIGAN_CAP.contains(&n))
                && match rules.max_hand_size {
                    Some(HandLimit::Cards(n)) => MAX_HAND_SIZE.contains(&n),
                    _ => true,
                };
            prop_assert_eq!(rules.validate().is_ok(), in_range);
        }
    }
}
