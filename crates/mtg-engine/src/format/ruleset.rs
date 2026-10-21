//! The rules a game plays by: the format's values with the house rules laid over them.

use mtg_core::house::{
    AttackRange, EmptyLibrary, HandLimit, HouseRules, MulliganStyle, SkipFirstDraw,
};
use mtg_core::ids::{ObjectId, PlayerId};

use super::{FormatRules, MulliganRule};
use crate::decision::MulliganOptions;
use crate::state::{GameResult, GameState};
use crate::turn::{DRAW_STEP_CARDS, LAND_DROPS_PER_TURN};

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

    /// Life `seat` starts with: its handicap, else the table's rule, else the format's.
    pub fn starting_life(&self, seat: PlayerId) -> i32 {
        self.house
            .handicap(seat)
            .and_then(|handicap| handicap.starting_life)
            .or(self.house.starting_life)
            .unwrap_or_else(|| self.format.starting_life())
    }

    /// The opening-hand mulligan rule of `seat`: the hand size (its handicap, else the table's rule, else the
    /// format's) and the mulligans that cost no card.
    pub fn mulligan_rule(&self, seat: PlayerId) -> MulliganRule {
        let format = self.format.mulligan_rule();
        MulliganRule {
            hand_size: self
                .house
                .handicap(seat)
                .and_then(|handicap| handicap.opening_hand_size)
                .or(self.house.opening_hand_size)
                .map_or(format.hand_size, count),
            free_mulligans: self.house.free_mulligans.unwrap_or(format.free_mulligans),
        }
    }

    /// The most mulligans `seat` may take: the cap if one is set, else its opening hand size, and never more
    /// than the cards a hand can lose after the free ones.
    pub fn max_mulligans(&self, seat: PlayerId) -> u32 {
        let rule = self.mulligan_rule(seat);
        let hand = u32::try_from(rule.hand_size).unwrap_or(u32::MAX);
        self.house
            .mulligan_cap
            .unwrap_or(hand)
            .min(hand.saturating_add(rule.free_mulligans))
    }

    /// The keep-or-mulligan options of `seat` for `hand` after `taken` mulligans, with the size of the next hand, the
    /// bottom counts and the scries worked out.
    pub fn mulligan_options(
        &self,
        seat: PlayerId,
        hand: Vec<ObjectId>,
        taken: u32,
    ) -> MulliganOptions {
        let rule = self.mulligan_rule(seat);
        let next = taken.saturating_add(1);
        let scry = |taken| Some(self.scry_after_keep(taken)).filter(|cards| *cards > 0);
        MulliganOptions {
            hand,
            mulligans_taken: taken,
            can_mulligan: taken < self.max_mulligans(seat),
            free_mulligans: rule.free_mulligans,
            bottom_if_keep: self.bottom_count(seat, taken),
            bottom_if_mulligan: self.bottom_count(seat, next),
            hand_if_mulligan: Some(self.dealt_hand_size(seat, next)),
            scry_if_keep: scry(taken),
            scry_if_mulligan: scry(next),
        }
    }

    /// Which players an attacker may attack.
    pub fn attack_range(&self) -> AttackRange {
        self.house.attack_range.unwrap_or(AttackRange::Any)
    }

    /// The players `attacker` may attack, in turn order from the one after them. The neighbours and the left player
    /// are worked out over the seats still in the game, so a seat that left changes who is next.
    pub fn defending_seats(&self, state: &GameState, attacker: PlayerId) -> Vec<PlayerId> {
        let mut others = state.turn_order_from(attacker);
        others.retain(|seat| *seat != attacker);
        match self.attack_range() {
            AttackRange::Any => others,
            AttackRange::Left => others.into_iter().take(1).collect(),
            AttackRange::Neighbours => {
                let (first, last) = (others.first().copied(), others.last().copied());
                first
                    .into_iter()
                    .chain(last.filter(|_| others.len() > 1))
                    .collect()
            }
        }
    }

    /// Whether every seat sees every hand.
    pub fn open_hands(&self) -> bool {
        self.house.open_hands.unwrap_or(false)
    }

    /// Whether every seat sees the top card of every library.
    pub fn revealed_library_top(&self) -> bool {
        self.house.revealed_library_top.unwrap_or(false)
    }

    /// Whether each opening hand is the better of two draws by its share of lands.
    pub fn opening_hand_smoothing(&self) -> bool {
        self.house.opening_hand_smoothing.unwrap_or(false)
    }

    /// How a mulligan gets the next hand.
    pub fn mulligan_style(&self) -> MulliganStyle {
        self.house.mulligan_style.unwrap_or(MulliganStyle::London)
    }

    /// Cards `seat` puts on the bottom when it keeps a hand after `taken` mulligans: the London count, none in the
    /// styles that draw fewer cards instead.
    pub fn bottom_count(&self, seat: PlayerId, taken: u32) -> usize {
        match self.mulligan_style() {
            MulliganStyle::London => self.mulligan_rule(seat).bottom_count(taken),
            MulliganStyle::Paris | MulliganStyle::Vancouver => 0,
        }
    }

    /// Cards in the hand `seat` is dealt after `taken` mulligans: the full hand, or one card fewer for each mulligan
    /// that is not free in the styles that draw fewer cards.
    pub fn dealt_hand_size(&self, seat: PlayerId, taken: u32) -> usize {
        let rule = self.mulligan_rule(seat);
        match self.mulligan_style() {
            MulliganStyle::London => rule.hand_size,
            MulliganStyle::Paris | MulliganStyle::Vancouver => {
                rule.hand_size.saturating_sub(rule.bottom_count(taken))
            }
        }
    }

    /// Cards a player who keeps a hand after `taken` mulligans scries.
    pub fn scry_after_keep(&self, taken: u32) -> usize {
        usize::from(self.mulligan_style() == MulliganStyle::Vancouver && taken > 0)
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

    /// Cards each player draws in their draw step.
    pub fn draw_step_cards(&self) -> usize {
        self.house.draw_step_cards.map_or(DRAW_STEP_CARDS, count)
    }

    /// Lands each player may play on their turn.
    pub fn land_plays_per_turn(&self) -> u32 {
        self.house
            .land_plays_per_turn
            .unwrap_or(LAND_DROPS_PER_TURN)
    }

    /// Whether mana left in a pool at the end of a step or phase costs its owner 1 life each (CR 106.4 as it was).
    pub fn mana_burn(&self) -> bool {
        self.house.mana_burn.unwrap_or(false)
    }

    /// What a draw from an empty library does.
    pub fn empty_library(&self) -> EmptyLibrary {
        self.house.empty_library.unwrap_or(EmptyLibrary::Lose)
    }

    /// The rounds after which the game ends, if it is limited.
    pub fn turn_limit(&self) -> Option<u32> {
        self.house.turn_limit
    }

    /// The life total that wins the game, if one does.
    pub fn winning_life(&self) -> Option<i32> {
        self.house.winning_life
    }

    /// The poison counters that make a player lose.
    pub fn poison_threshold(&self) -> u32 {
        self.house
            .poison_threshold
            .unwrap_or_else(|| self.format.poison_threshold())
    }

    /// The game result once decided: the format's, else a draw when every player left is at the winning life total
    /// (state-based actions have already removed the others, so one player left is the format's win).
    pub fn game_result(&self, state: &GameState) -> Option<GameResult> {
        self.format.game_result(state).or_else(|| {
            let goal = self.winning_life()?;
            state
                .players()
                .iter()
                .filter(|player| player.is_in_game())
                .all(|player| player.life() >= goal)
                .then_some(GameResult::Draw)
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every rule the house can set wins over the format's value, an unset rule keeps it, and the mulligan options follow the resolved rule, bottom counts and the limit on mulligans included.
    use mtg_core::house::{EmptyLibrary, HandLimit, Handicap, HouseRules, SkipFirstDraw};
    use rstest::rstest;

    use super::*;
    use crate::format::Normal;

    const SEAT: PlayerId = PlayerId(0);

    fn normal(house: &HouseRules) -> RuleSet<'_> {
        RuleSet::new(&Normal, house)
    }

    #[test]
    fn no_house_rule_is_the_format() {
        let house = HouseRules::default();
        let rules = normal(&house);
        assert_eq!(rules.starting_life(SEAT), 20);
        assert_eq!(rules.mulligan_rule(SEAT), Normal.mulligan_rule());
        assert_eq!(rules.max_mulligans(SEAT), 7);
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
        let mulligan = rules.mulligan_rule(SEAT);
        assert_eq!(
            (
                rules.starting_life(SEAT),
                mulligan.hand_size,
                mulligan.free_mulligans,
                rules.max_mulligans(SEAT)
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
        normal(&house).mulligan_options(SEAT, vec![ObjectId(1)], taken)
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

    #[test]
    fn the_rules_of_the_turn_and_life_are_the_formats_when_unset() {
        let house = HouseRules::default();
        let rules = normal(&house);
        assert_eq!(rules.draw_step_cards(), 1);
        assert_eq!(rules.land_plays_per_turn(), 1);
        assert!(!rules.mana_burn());
        assert_eq!(rules.empty_library(), EmptyLibrary::Lose);
        assert_eq!((rules.turn_limit(), rules.winning_life()), (None, None));
        assert_eq!(rules.poison_threshold(), 10);
    }

    #[rstest]
    #[case::draws(HouseRules { draw_step_cards: Some(3), ..HouseRules::default() }, 3, 1, 10)]
    #[case::lands(HouseRules { land_plays_per_turn: Some(4), ..HouseRules::default() }, 1, 4, 10)]
    #[case::poison(HouseRules { poison_threshold: Some(3), ..HouseRules::default() }, 1, 1, 3)]
    #[case::a_value_equal_to_the_formats(HouseRules { draw_step_cards: Some(1), land_plays_per_turn: Some(1), poison_threshold: Some(10), ..HouseRules::default() }, 1, 1, 10)]
    fn a_set_number_wins(
        #[case] house: HouseRules,
        #[case] draws: usize,
        #[case] lands: u32,
        #[case] poison: u32,
    ) {
        let rules = normal(&house);
        assert_eq!(
            (
                rules.draw_step_cards(),
                rules.land_plays_per_turn(),
                rules.poison_threshold()
            ),
            (draws, lands, poison)
        );
    }

    #[test]
    fn the_switches_follow_the_house() {
        let house = HouseRules {
            mana_burn: Some(true),
            empty_library: Some(EmptyLibrary::ReshuffleGraveyard),
            turn_limit: Some(8),
            winning_life: Some(40),
            ..HouseRules::default()
        };
        let rules = normal(&house);
        assert!(rules.mana_burn());
        assert_eq!(rules.empty_library(), EmptyLibrary::ReshuffleGraveyard);
        assert_eq!(
            (rules.turn_limit(), rules.winning_life()),
            (Some(8), Some(40))
        );
    }

    fn handicapped(seat: u64, life: Option<i32>, hand: Option<u32>) -> Handicap {
        Handicap {
            seat: PlayerId(seat),
            starting_life: life,
            opening_hand_size: hand,
        }
    }

    #[rstest]
    #[case::the_handicapped_seat(1, 5, 3)]
    #[case::a_seat_without_one_keeps_the_tables_value(0, 40, 6)]
    #[case::another_seat_too(2, 40, 6)]
    fn a_handicap_is_for_its_seat_only(#[case] seat: u64, #[case] life: i32, #[case] hand: usize) {
        let house = HouseRules {
            starting_life: Some(40),
            opening_hand_size: Some(6),
            handicaps: vec![handicapped(1, Some(5), Some(3))],
            ..HouseRules::default()
        };
        let rules = normal(&house);
        assert_eq!(rules.starting_life(PlayerId(seat)), life);
        assert_eq!(rules.mulligan_rule(PlayerId(seat)).hand_size, hand);
    }

    #[rstest]
    #[case::life_only(handicapped(0, Some(9), None), 9, 7)]
    #[case::hand_only(handicapped(0, None, Some(2)), 20, 2)]
    #[case::both(handicapped(0, Some(1), Some(1)), 1, 1)]
    fn a_handicap_that_sets_one_value_leaves_the_other(
        #[case] handicap: Handicap,
        #[case] life: i32,
        #[case] hand: usize,
    ) {
        let house = HouseRules {
            handicaps: vec![handicap],
            ..HouseRules::default()
        };
        let rules = normal(&house);
        assert_eq!(rules.starting_life(PlayerId(0)), life);
        assert_eq!(rules.mulligan_rule(PlayerId(0)).hand_size, hand);
    }

    #[test]
    fn the_mulligan_options_of_a_seat_use_its_own_hand_size() {
        let house = HouseRules {
            handicaps: vec![handicapped(1, None, Some(3))],
            ..HouseRules::default()
        };
        let rules = normal(&house);
        let own = rules.mulligan_options(PlayerId(1), vec![ObjectId(1)], 3);
        let other = rules.mulligan_options(PlayerId(0), vec![ObjectId(1)], 3);
        // A hand of three can lose at most three cards, a hand of seven seven.
        assert_eq!(
            (own.can_mulligan, own.bottom_if_keep, own.bottom_if_mulligan),
            (false, 3, 4)
        );
        assert_eq!(
            (
                other.can_mulligan,
                other.bottom_if_keep,
                other.bottom_if_mulligan
            ),
            (true, 3, 4)
        );
        assert_eq!(rules.max_mulligans(PlayerId(1)), 3);
        assert_eq!(rules.max_mulligans(PlayerId(0)), 7);
    }
}
