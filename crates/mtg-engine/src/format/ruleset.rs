//! The rules a game plays by: the format's values with the house rules laid over them.

use mtg_core::house::{HandLimit, HouseRules, SkipFirstDraw};
use mtg_core::ids::ObjectId;

use super::{FormatRules, MulliganRule};
use crate::decision::MulliganOptions;

/// The format and the house rules of one game, resolved. Every engine call site that needs a rule the
/// house can change reads it here and never from the format.
#[derive(Debug, Clone, Copy)]
pub struct RuleSet<'a> {
    format: &'a dyn FormatRules,
    house: &'a HouseRules,
}

fn count(n: u32) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

impl<'a> RuleSet<'a> {
    /// The rules of `format` with `house` laid over them.
    pub fn new(format: &'a dyn FormatRules, house: &'a HouseRules) -> Self {
        RuleSet { format, house }
    }

    /// Life each player starts with.
    pub fn starting_life(&self) -> i32 {
        self.house
            .starting_life
            .unwrap_or_else(|| self.format.starting_life())
    }

    /// The opening-hand mulligan rule: the hand size and the mulligans that cost no card.
    pub fn mulligan_rule(&self) -> MulliganRule {
        let format = self.format.mulligan_rule();
        MulliganRule {
            hand_size: self.house.opening_hand_size.map_or(format.hand_size, count),
            free_mulligans: self.house.free_mulligans.unwrap_or(format.free_mulligans),
        }
    }

    /// The most mulligans a player may take: the cap if one is set, else the opening hand size, and never more
    /// than the cards a hand can lose after the free ones.
    pub fn max_mulligans(&self) -> u32 {
        let rule = self.mulligan_rule();
        let hand = u32::try_from(rule.hand_size).unwrap_or(u32::MAX);
        self.house
            .mulligan_cap
            .unwrap_or(hand)
            .min(hand.saturating_add(rule.free_mulligans))
    }

    /// The keep-or-mulligan options for `hand` after `taken` mulligans, with the bottom counts worked out.
    pub fn mulligan_options(&self, hand: Vec<ObjectId>, taken: u32) -> MulliganOptions {
        let rule = self.mulligan_rule();
        MulliganOptions {
            hand,
            mulligans_taken: taken,
            can_mulligan: taken < self.max_mulligans(),
            free_mulligans: rule.free_mulligans,
            bottom_if_keep: rule.bottom_count(taken),
            bottom_if_mulligan: rule.bottom_count(taken.saturating_add(1)),
        }
    }

    /// Whether the starting player skips their first draw in a game of `player_count` players.
    pub fn skips_first_draw(&self, player_count: usize) -> bool {
        match self.house.skip_first_draw {
            Some(SkipFirstDraw::Always) => true,
            Some(SkipFirstDraw::Never) => false,
            None => self.format.skips_first_draw(player_count),
        }
    }

    /// The most cards a player keeps in hand at cleanup; `None` when there is no limit.
    pub fn max_hand_size(&self) -> Option<usize> {
        match self.house.max_hand_size {
            Some(HandLimit::Cards(n)) => Some(count(n)),
            Some(HandLimit::Unlimited) => None,
            None => Some(self.format.max_hand_size()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every rule the house can set wins over the format's value, an unset rule keeps it, and the mulligan options follow the resolved rule, bottom counts and the limit on mulligans included.
    use mtg_core::house::{HandLimit, HouseRules, SkipFirstDraw};
    use rstest::rstest;

    use super::*;
    use crate::format::Normal;

    fn normal(house: &HouseRules) -> RuleSet<'_> {
        RuleSet::new(&Normal, house)
    }

    #[test]
    fn no_house_rule_is_the_format() {
        let house = HouseRules::default();
        let rules = normal(&house);
        assert_eq!(rules.starting_life(), 20);
        assert_eq!(rules.mulligan_rule(), Normal.mulligan_rule());
        assert_eq!(rules.max_mulligans(), 7);
        assert_eq!(rules.max_hand_size(), Some(7));
        assert!(rules.skips_first_draw(2) && !rules.skips_first_draw(4));
    }

    #[rstest]
    #[case::life(HouseRules { starting_life: Some(40), ..HouseRules::default() }, 40, 7, 0, 7)]
    #[case::hand(HouseRules { opening_hand_size: Some(5), ..HouseRules::default() }, 20, 5, 0, 5)]
    #[case::free(HouseRules { free_mulligans: Some(2), ..HouseRules::default() }, 20, 7, 2, 7)]
    #[case::all(HouseRules { starting_life: Some(1), opening_hand_size: Some(1), free_mulligans: Some(1), mulligan_cap: Some(1), ..HouseRules::default() }, 1, 1, 1, 1)]
    #[case::equal_to_the_formats(HouseRules { starting_life: Some(20), opening_hand_size: Some(7), free_mulligans: Some(0), ..HouseRules::default() }, 20, 7, 0, 7)]
    fn a_set_value_wins(
        #[case] house: HouseRules,
        #[case] life: i32,
        #[case] hand: usize,
        #[case] free: u32,
        #[case] most: u32,
    ) {
        let rules = normal(&house);
        let mulligan = rules.mulligan_rule();
        assert_eq!(
            (
                rules.starting_life(),
                mulligan.hand_size,
                mulligan.free_mulligans,
                rules.max_mulligans()
            ),
            (life, hand, free, most)
        );
    }

    #[rstest]
    #[case::unset_two(None, 2, true)]
    #[case::unset_four(None, 4, false)]
    #[case::always_two(Some(SkipFirstDraw::Always), 2, true)]
    #[case::always_four(Some(SkipFirstDraw::Always), 4, true)]
    #[case::never_two(Some(SkipFirstDraw::Never), 2, false)]
    #[case::never_four(Some(SkipFirstDraw::Never), 4, false)]
    fn the_first_draw_follows_the_house_or_the_format(
        #[case] rule: Option<SkipFirstDraw>,
        #[case] players: usize,
        #[case] skips: bool,
    ) {
        let house = HouseRules {
            skip_first_draw: rule,
            ..HouseRules::default()
        };
        assert_eq!(normal(&house).skips_first_draw(players), skips);
    }

    #[rstest]
    #[case::format(None, Some(7))]
    #[case::nine(Some(HandLimit::Cards(9)), Some(9))]
    #[case::none_kept(Some(HandLimit::Cards(0)), Some(0))]
    #[case::unlimited(Some(HandLimit::Unlimited), None)]
    fn the_hand_limit_is_a_number_or_none(
        #[case] limit: Option<HandLimit>,
        #[case] most: Option<usize>,
    ) {
        let house = HouseRules {
            max_hand_size: limit,
            ..HouseRules::default()
        };
        assert_eq!(normal(&house).max_hand_size(), most);
    }

    fn options(
        hand: Option<u32>,
        free: Option<u32>,
        cap: Option<u32>,
        taken: u32,
    ) -> MulliganOptions {
        let house = HouseRules {
            opening_hand_size: hand,
            free_mulligans: free,
            mulligan_cap: cap,
            ..HouseRules::default()
        };
        normal(&house).mulligan_options(vec![ObjectId(1)], taken)
    }

    #[rstest]
    #[case::normal_fresh(None, None, None, 0, true, 0, 1)]
    #[case::normal_six(None, None, None, 6, true, 6, 7)]
    #[case::normal_seven_is_the_last(None, None, None, 7, false, 7, 8)]
    #[case::free_fresh(None, Some(1), None, 0, true, 0, 0)]
    #[case::free_then_paid(None, Some(1), None, 2, true, 1, 2)]
    #[case::no_card_is_ever_lost(None, Some(100), None, 6, true, 0, 0)]
    #[case::no_card_lost_to_the_last_mulligan(None, Some(100), None, 7, false, 0, 0)]
    #[case::a_cap_removes_the_mulligan(None, None, Some(2), 2, false, 2, 3)]
    #[case::under_the_cap(None, None, Some(2), 1, true, 1, 2)]
    #[case::cap_zero_forbids_them(None, None, Some(0), 0, false, 0, 1)]
    #[case::a_cap_above_the_hand_needs_free_mulligans(None, Some(100), Some(20), 19, true, 0, 0)]
    #[case::a_cap_above_what_a_hand_can_lose(None, None, Some(20), 7, false, 7, 8)]
    #[case::small_hand(Some(5), None, None, 4, true, 4, 5)]
    #[case::small_hand_last(Some(5), None, None, 5, false, 5, 6)]
    #[case::free_cap_and_hand(Some(3), Some(1), Some(10), 4, false, 3, 4)]
    fn the_options_follow_the_resolved_rule(
        #[case] hand: Option<u32>,
        #[case] free: Option<u32>,
        #[case] cap: Option<u32>,
        #[case] taken: u32,
        #[case] can: bool,
        #[case] keep: usize,
        #[case] mulligan: usize,
    ) {
        let got = options(hand, free, cap, taken);
        assert_eq!(
            (
                got.mulligans_taken,
                got.can_mulligan,
                got.bottom_if_keep,
                got.bottom_if_mulligan
            ),
            (taken, can, keep, mulligan)
        );
    }

    #[rstest]
    fn the_next_mulligan_costs_a_card_only_when_no_free_one_is_left(
        #[values(0, 1, 2)] free: u32,
        #[values(0, 1, 2, 3, 4)] taken: u32,
    ) {
        let got = options(None, Some(free), None, taken);
        assert_eq!(
            got.bottom_if_mulligan - got.bottom_if_keep,
            usize::from(taken >= free)
        );
    }
}
