//! House rules: the values a game's creator sets in place of the format's.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::PlayerId;

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
/// The cards a player may draw in each draw step.
pub const DRAW_STEP_CARDS: RangeInclusive<u32> = 1..=10;
/// The lands a player may play each turn.
pub const LAND_PLAYS: RangeInclusive<u32> = 1..=10;
/// The turn limits a table may set, in rounds.
pub const TURN_LIMIT: RangeInclusive<u32> = 1..=1000;
/// The life totals that win the game a table may set.
pub const WINNING_LIFE: RangeInclusive<i32> = 1..=10_000;
/// The poison thresholds a table may set.
pub const POISON_THRESHOLD: RangeInclusive<u32> = 1..=100;

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

/// What happens to a player who draws from an empty library, in place of the format's rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum EmptyLibrary {
    /// They lose the game at the next state-based check.
    Lose,
    /// Their graveyard is shuffled into the library and they draw; they lose when the graveyard is empty too.
    ReshuffleGraveyard,
}

/// What one seat starts with in place of the table's values; a value that is left out keeps the table's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct Handicap {
    /// The seat, counted from 0 in table order.
    pub seat: PlayerId,
    /// Life this seat starts with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub starting_life: Option<i32>,
    /// Cards in this seat's opening hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub opening_hand_size: Option<u32>,
}

/// What a game's creator sets in place of the format's values. A rule that is left out keeps the
/// format's value; a value equal to the format's is kept as given.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Cards each player draws in their draw step; a skipped first draw skips all of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draw_step_cards: Option<u32>,
    /// Lands each player may play on their turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub land_plays_per_turn: Option<u32>,
    /// Whether mana left in a pool at the end of a step or phase costs its owner 1 life each.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mana_burn: Option<bool>,
    /// What drawing from an empty library does.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub empty_library: Option<EmptyLibrary>,
    /// Rounds after which the game ends and the highest life total wins; a round is one turn of each seat in play.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub turn_limit: Option<u32>,
    /// A life total that wins the game for a player who has it when state-based actions are checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub winning_life: Option<i32>,
    /// Poison counters that make a player lose.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub poison_threshold: Option<u32>,
    /// What single seats start with, at most one entry for each seat.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(optional, as = "Option<Vec<Handicap>>"))]
    pub handicaps: Vec<Handicap>,
}

/// Why house rules cannot be played.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HouseRuleError {
    /// A value lies outside the range its rule allows.
    #[error("{field}: {got} is not between {min} and {max}")]
    OutOfRange {
        /// The name of the rule on the wire.
        field: String,
        /// The value given.
        got: i64,
        /// The least value allowed.
        min: i64,
        /// The most value allowed.
        max: i64,
    },
    /// Two handicaps name the same seat.
    #[error("handicaps: seat {} has more than one entry", seat.get())]
    DuplicateHandicap {
        /// The seat.
        seat: PlayerId,
    },
    /// A handicap sets no value.
    #[error("handicaps: the entry for seat {} sets nothing", seat.get())]
    EmptyHandicap {
        /// The seat.
        seat: PlayerId,
    },
    /// A handicap names a seat the table does not have.
    #[error("handicaps: seat {} is not at a table of {seats}", seat.get())]
    NoSuchSeat {
        /// The seat.
        seat: PlayerId,
        /// The seats the table has.
        seats: usize,
    },
}

impl HouseRuleError {
    /// The name of the rule that was refused.
    pub fn field(&self) -> &str {
        match self {
            HouseRuleError::OutOfRange { field, .. } => field,
            HouseRuleError::DuplicateHandicap { .. }
            | HouseRuleError::EmptyHandicap { .. }
            | HouseRuleError::NoSuchSeat { .. } => "handicaps",
        }
    }
}

fn within<T: Copy + Into<i64> + PartialOrd>(
    field: &str,
    value: Option<T>,
    range: &RangeInclusive<T>,
) -> Result<(), HouseRuleError> {
    match value {
        Some(got) if !range.contains(&got) => Err(HouseRuleError::OutOfRange {
            field: field.to_owned(),
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
        within("max_hand_size", cards, &MAX_HAND_SIZE)?;
        within("draw_step_cards", self.draw_step_cards, &DRAW_STEP_CARDS)?;
        within(
            "land_plays_per_turn",
            self.land_plays_per_turn,
            &LAND_PLAYS,
        )?;
        within("turn_limit", self.turn_limit, &TURN_LIMIT)?;
        within("winning_life", self.winning_life, &WINNING_LIFE)?;
        within(
            "poison_threshold",
            self.poison_threshold,
            &POISON_THRESHOLD,
        )?;
        for (index, handicap) in self.handicaps.iter().enumerate() {
            if self.handicaps[..index]
                .iter()
                .any(|earlier| earlier.seat == handicap.seat)
            {
                return Err(HouseRuleError::DuplicateHandicap { seat: handicap.seat });
            }
            if handicap.starting_life.is_none() && handicap.opening_hand_size.is_none() {
                return Err(HouseRuleError::EmptyHandicap { seat: handicap.seat });
            }
            within(
                &format!("handicaps[{index}].starting_life"),
                handicap.starting_life,
                &STARTING_LIFE,
            )?;
            within(
                &format!("handicaps[{index}].opening_hand_size"),
                handicap.opening_hand_size,
                &OPENING_HAND_SIZE,
            )?;
        }
        Ok(())
    }

    /// Checks every value like [`validate`](HouseRules::validate), and that every handicap names a seat of a table
    /// of `seats`.
    pub fn validate_for(&self, seats: usize) -> Result<(), HouseRuleError> {
        self.validate()?;
        match self.handicaps.iter().find(|h| h.seat.index() >= seats) {
            Some(handicap) => Err(HouseRuleError::NoSuchSeat {
                seat: handicap.seat,
                seats,
            }),
            None => Ok(()),
        }
    }

    /// What `seat` starts with in place of the table's values, if a handicap names it.
    pub fn handicap(&self, seat: PlayerId) -> Option<&Handicap> {
        self.handicaps.iter().find(|h| h.seat == seat)
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
    #[case::draws_none("draw_step_cards", json!(0), Some("draw_step_cards"))]
    #[case::draws_least("draw_step_cards", json!(1), None)]
    #[case::draws_most("draw_step_cards", json!(10), None)]
    #[case::draws_high("draw_step_cards", json!(11), Some("draw_step_cards"))]
    #[case::lands_none("land_plays_per_turn", json!(0), Some("land_plays_per_turn"))]
    #[case::lands_least("land_plays_per_turn", json!(1), None)]
    #[case::lands_most("land_plays_per_turn", json!(10), None)]
    #[case::lands_high("land_plays_per_turn", json!(11), Some("land_plays_per_turn"))]
    #[case::burn_on("mana_burn", json!(true), None)]
    #[case::burn_off("mana_burn", json!(false), None)]
    #[case::library_loses("empty_library", json!("lose"), None)]
    #[case::library_reshuffles("empty_library", json!("reshuffle_graveyard"), None)]
    #[case::limit_none_at_all("turn_limit", json!(0), Some("turn_limit"))]
    #[case::limit_least("turn_limit", json!(1), None)]
    #[case::limit_most_rounds("turn_limit", json!(1000), None)]
    #[case::limit_high("turn_limit", json!(1001), Some("turn_limit"))]
    #[case::wins_at_zero("winning_life", json!(0), Some("winning_life"))]
    #[case::wins_negative("winning_life", json!(-1), Some("winning_life"))]
    #[case::wins_least("winning_life", json!(1), None)]
    #[case::wins_most("winning_life", json!(10_000), None)]
    #[case::wins_high("winning_life", json!(10_001), Some("winning_life"))]
    #[case::poison_none("poison_threshold", json!(0), Some("poison_threshold"))]
    #[case::poison_least("poison_threshold", json!(1), None)]
    #[case::poison_most("poison_threshold", json!(100), None)]
    #[case::poison_high("poison_threshold", json!(101), Some("poison_threshold"))]
    #[case::no_handicaps("handicaps", json!([]), None)]
    #[case::a_handicap("handicaps", json!([{"seat": 1, "starting_life": 30}]), None)]
    #[case::two_seats("handicaps", json!([{"seat": 0, "opening_hand_size": 5}, {"seat": 1, "starting_life": 30}]), None)]
    #[case::handicap_life_low("handicaps", json!([{"seat": 0, "starting_life": 0}]), Some("handicaps[0].starting_life"))]
    #[case::handicap_life_high("handicaps", json!([{"seat": 0, "starting_life": 1}, {"seat": 1, "starting_life": 1001}]), Some("handicaps[1].starting_life"))]
    #[case::handicap_hand_low("handicaps", json!([{"seat": 2, "opening_hand_size": 0}]), Some("handicaps[0].opening_hand_size"))]
    #[case::handicap_hand_high("handicaps", json!([{"seat": 2, "opening_hand_size": 21}]), Some("handicaps[0].opening_hand_size"))]
    #[case::the_same_seat_twice("handicaps", json!([{"seat": 1, "starting_life": 30}, {"seat": 1, "opening_hand_size": 5}]), Some("handicaps"))]
    #[case::a_handicap_that_sets_nothing("handicaps", json!([{"seat": 1}]), Some("handicaps"))]
    fn each_rule_has_its_range(
        #[case] field: &str,
        #[case] value: serde_json::Value,
        #[case] refused: Option<&str>,
    ) {
        let parsed = rules(field, &value).unwrap();
        let error = parsed.validate().err();
        assert_eq!(error.as_ref().map(HouseRuleError::field), refused);
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
    fn the_wire_form_of_the_turn_and_life_rules() {
        let set = HouseRules {
            draw_step_cards: Some(2),
            land_plays_per_turn: Some(3),
            mana_burn: Some(true),
            empty_library: Some(EmptyLibrary::ReshuffleGraveyard),
            turn_limit: Some(10),
            winning_life: Some(40),
            poison_threshold: Some(5),
            handicaps: vec![Handicap {
                seat: PlayerId(1),
                starting_life: Some(30),
                opening_hand_size: None,
            }],
            ..HouseRules::default()
        };
        let wire = json!({
            "draw_step_cards": 2, "land_plays_per_turn": 3, "mana_burn": true,
            "empty_library": "reshuffle_graveyard", "turn_limit": 10, "winning_life": 40,
            "poison_threshold": 5, "handicaps": [{"seat": 1, "starting_life": 30}]
        });
        assert_eq!(serde_json::to_value(&set).unwrap(), wire);
        assert_eq!(serde_json::from_value::<HouseRules>(wire).unwrap(), set);
    }

    #[rstest]
    #[case::the_last_seat(json!([{"seat": 3, "starting_life": 5}]), 4, None)]
    #[case::the_first_seat(json!([{"seat": 0, "starting_life": 5}]), 2, None)]
    #[case::one_past_the_last(json!([{"seat": 4, "starting_life": 5}]), 4, Some(4))]
    #[case::far_past(json!([{"seat": 1, "starting_life": 5}, {"seat": 90, "starting_life": 5}]), 2, Some(90))]
    fn a_handicap_names_a_seat_of_the_table(
        #[case] handicaps: serde_json::Value,
        #[case] seats: usize,
        #[case] refused: Option<u64>,
    ) {
        let rules: HouseRules = serde_json::from_value(json!({ "handicaps": handicaps })).unwrap();
        assert_eq!(rules.validate(), Ok(()));
        let error = rules.validate_for(seats).err();
        assert_eq!(
            error,
            refused.map(|seat| HouseRuleError::NoSuchSeat {
                seat: PlayerId(seat),
                seats
            })
        );
    }

    #[test]
    fn a_handicap_is_found_by_seat() {
        let rules = HouseRules {
            handicaps: vec![Handicap {
                seat: PlayerId(2),
                starting_life: Some(9),
                opening_hand_size: Some(4),
            }],
            ..HouseRules::default()
        };
        assert_eq!(rules.handicap(PlayerId(2)).and_then(|h| h.starting_life), Some(9));
        assert!(rules.handicap(PlayerId(1)).is_none());
    }

    #[test]
    fn an_unknown_rule_is_refused() {
        assert!(serde_json::from_value::<HouseRules>(json!({"banana": true})).is_err());
        assert!(
            serde_json::from_value::<HouseRules>(json!({"handicaps": [{"seat": 0, "banana": 1}]}))
                .is_err()
        );
    }

    fn any_handicap() -> impl Strategy<Value = Handicap> {
        (
            0..6u64,
            proptest::option::of(any::<i32>()),
            proptest::option::of(any::<u32>()),
        )
            .prop_map(|(seat, life, hand)| Handicap {
                seat: PlayerId(seat),
                starting_life: life,
                opening_hand_size: hand,
            })
    }

    fn any_rules() -> impl Strategy<Value = HouseRules> {
        let scalars = (
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
        );
        let turn = (
            proptest::option::of(0..=12u32),
            proptest::option::of(0..=12u32),
            proptest::option::of(any::<bool>()),
            proptest::option::of(prop_oneof![
                Just(EmptyLibrary::Lose),
                Just(EmptyLibrary::ReshuffleGraveyard)
            ]),
            proptest::option::of(0..=1002u32),
            proptest::option::of(-2..=10_002i32),
            proptest::option::of(0..=102u32),
            proptest::collection::vec(any_handicap(), 0..4),
        );
        (scalars, turn).prop_map(
            |(
                (life, hand, free, cap, first, limit),
                (draws, lands, burn, library, turns, wins, poison, handicaps),
            )| HouseRules {
                starting_life: life,
                opening_hand_size: hand,
                free_mulligans: free,
                mulligan_cap: cap,
                skip_first_draw: first,
                max_hand_size: limit,
                draw_step_cards: draws,
                land_plays_per_turn: lands,
                mana_burn: burn,
                empty_library: library,
                turn_limit: turns,
                winning_life: wins,
                poison_threshold: poison,
                handicaps,
            },
        )
    }

    fn handicaps_fit(rules: &HouseRules) -> bool {
        rules.handicaps.iter().enumerate().all(|(index, h)| {
            rules.handicaps[..index].iter().all(|e| e.seat != h.seat)
                && (h.starting_life.is_some() || h.opening_hand_size.is_some())
                && h.starting_life.is_none_or(|n| STARTING_LIFE.contains(&n))
                && h.opening_hand_size.is_none_or(|n| OPENING_HAND_SIZE.contains(&n))
        })
    }

    proptest! {
        #[test]
        fn rules_survive_json_and_validation_only_ever_fails_by_range(rules in any_rules()) {
            let back: HouseRules = serde_json::from_str(&serde_json::to_string(&rules).unwrap()).unwrap();
            prop_assert_eq!(&back, &rules);
            let in_range = rules.starting_life.is_none_or(|n| STARTING_LIFE.contains(&n))
                && rules.opening_hand_size.is_none_or(|n| OPENING_HAND_SIZE.contains(&n))
                && rules.free_mulligans.is_none_or(|n| FREE_MULLIGANS.contains(&n))
                && rules.mulligan_cap.is_none_or(|n| MULLIGAN_CAP.contains(&n))
                && match rules.max_hand_size {
                    Some(HandLimit::Cards(n)) => MAX_HAND_SIZE.contains(&n),
                    _ => true,
                }
                && rules.draw_step_cards.is_none_or(|n| DRAW_STEP_CARDS.contains(&n))
                && rules.land_plays_per_turn.is_none_or(|n| LAND_PLAYS.contains(&n))
                && rules.turn_limit.is_none_or(|n| TURN_LIMIT.contains(&n))
                && rules.winning_life.is_none_or(|n| WINNING_LIFE.contains(&n))
                && rules.poison_threshold.is_none_or(|n| POISON_THRESHOLD.contains(&n))
                && handicaps_fit(&rules);
            prop_assert_eq!(rules.validate().is_ok(), in_range);
        }
    }
}
