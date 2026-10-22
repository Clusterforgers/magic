//! The Commander format (CR 903): a 100-card singleton deck led by one commander or a pair of partners, played from a
//! command zone, with 40 life, a free first mulligan, a tax on casting the commander and commander damage.

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::{Color, ColorSet};
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;

use super::{DeckError, ExtraZone, FormatRules, MulliganRule, RuleSet};
use crate::commanders::damage_dealt_to;
use crate::player::LossReason;
use crate::state::GameState;

/// Cards in a Commander deck, its commanders included.
pub const COMMANDER_DECK: usize = 100;
/// Life each player starts with in Commander.
pub const COMMANDER_LIFE: i32 = 40;
/// Combat damage from one commander that makes a player lose.
pub const COMMANDER_DAMAGE: u32 = 21;
/// Generic mana a commander costs more for each cast from the command zone.
pub const COMMANDER_TAX: u32 = 2;

/// The Commander format.
#[derive(Debug, Clone, Copy, Default)]
pub struct Commander;

/// The letters of the colors in `set`, in WUBRG order; `C` for none.
fn letters(set: ColorSet) -> String {
    let word: String = Color::ALL
        .iter()
        .filter(|color| set.contains(**color))
        .map(|color| match color {
            Color::White => 'W',
            Color::Blue => 'U',
            Color::Black => 'B',
            Color::Red => 'R',
            Color::Green => 'G',
        })
        .collect();
    if word.is_empty() { "C".into() } else { word }
}

/// The commanders are one legendary creature (or a card that can be) or two that are partners, as the game's rules
/// allow.
fn check_commanders(
    commanders: &[Arc<CardDefinition>],
    rules: &RuleSet<'_>,
) -> Result<(), DeckError> {
    let most = rules.most_commanders();
    match commanders {
        [] => return Err(DeckError::NoCommander),
        [_] => {}
        [first, second] if most >= 2 => {
            if rules.pair_needs_partner() && !first.partners_with(second) {
                return Err(DeckError::NotPartners {
                    first: first.name.clone(),
                    second: second.name.clone(),
                });
            }
        }
        more => {
            return Err(DeckError::TooManyCommanders {
                max: most,
                got: more.len(),
            });
        }
    }
    match commanders.iter().find(|card| !card.can_be_commander()) {
        Some(card) => Err(DeckError::NotACommander {
            name: card.name.clone(),
        }),
        None => Ok(()),
    }
}

impl FormatRules for Commander {
    fn name(&self) -> &'static str {
        "commander"
    }

    fn starting_life(&self) -> i32 {
        COMMANDER_LIFE
    }

    fn validate_deck(&self, deck: &[Arc<CardDefinition>]) -> Result<(), DeckError> {
        self.validate_seat(deck, &[], &RuleSet::new(self, &HouseRules::default()))
    }

    fn validate_seat(
        &self,
        deck: &[Arc<CardDefinition>],
        commanders: &[Arc<CardDefinition>],
        rules: &RuleSet<'_>,
    ) -> Result<(), DeckError> {
        check_commanders(commanders, rules)?;
        let size = deck.len() + commanders.len();
        let expected = rules.deck_size(COMMANDER_DECK);
        if size != expected {
            return Err(DeckError::WrongSize {
                expected,
                got: size,
            });
        }
        let mut copies: BTreeMap<&str, (usize, Option<u32>)> = BTreeMap::new();
        for card in commanders.iter().chain(deck) {
            let entry = copies
                .entry(card.name.as_str())
                .or_insert_with(|| (0, rules.copies_limit(card.singleton_limit())));
            entry.0 += 1;
        }
        let over = copies
            .into_iter()
            .find(|(_, (got, limit))| limit.is_some_and(|max| *got > max as usize));
        if let Some((name, (got, Some(max)))) = over {
            return Err(DeckError::TooManyCopies {
                name: name.to_owned(),
                max: max as usize,
                got,
            });
        }
        if !rules.checks_colour_identity() {
            return Ok(());
        }
        let identity = commanders.iter().fold(ColorSet::EMPTY, |all, card| {
            card.color_identity().iter().fold(all, ColorSet::with)
        });
        for card in deck {
            let own = card.color_identity();
            if !own.is_subset_of(identity) {
                let outside = own
                    .iter()
                    .filter(|color| !identity.contains(*color))
                    .fold(ColorSet::EMPTY, ColorSet::with);
                return Err(DeckError::OutsideColorIdentity {
                    name: card.name.clone(),
                    outside: letters(outside),
                    identity: letters(identity),
                });
            }
        }
        Ok(())
    }

    fn mulligan_rule(&self) -> MulliganRule {
        // The first mulligan is free (CR 103.5c).
        MulliganRule {
            hand_size: 7,
            free_mulligans: 1,
        }
    }

    fn extra_zones(&self) -> &'static [ExtraZone] {
        &[ExtraZone::Command]
    }

    // Only the first player of a two-player game skips the draw (CR 103.8a, 103.8c).
    fn skips_first_draw(&self, player_count: usize) -> bool {
        player_count == 2
    }

    fn commander_tax(&self) -> u32 {
        COMMANDER_TAX
    }

    fn commander_damage(&self) -> Option<u32> {
        Some(COMMANDER_DAMAGE)
    }

    fn loss_condition(&self, state: &GameState, player: PlayerId) -> Option<LossReason> {
        let threshold = state.ruleset().commander_damage()?;
        (damage_dealt_to(state, player) >= threshold).then_some(LossReason::CommanderDamage)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the Commander format checks a seat's commanders and deck (one commander or a partner pair of legendary creatures, exactly 100 cards with the commanders, one copy of each card but basic lands and cards that allow more, every card inside the commanders' color identity by cost, hybrid and Phyrexian symbols and activated abilities) with a typed error for each rule, and reports its constants: 40 life, a free first mulligan, a command zone, a tax of 2, 21 commander damage, and a skipped first draw in two-player games only; the house rules for the deck relax the size, the singleton rule, the colour identity and the commanders a seat starts with, each apart from the others.
    use mtg_cards::ability::Ability;
    use mtg_core::card::{CardType, Supertype};
    use mtg_core::house::DeckRules;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;

    /// The seat checked under the format's own rules.
    fn check(
        deck: &[Arc<CardDefinition>],
        commanders: &[Arc<CardDefinition>],
    ) -> Result<(), DeckError> {
        under(&HouseRules::default(), deck, commanders)
    }

    /// The seat checked under `house`.
    fn under(
        house: &HouseRules,
        deck: &[Arc<CardDefinition>],
        commanders: &[Arc<CardDefinition>],
    ) -> Result<(), DeckError> {
        Commander.validate_seat(deck, commanders, &RuleSet::new(&Commander, house))
    }

    fn deck_rules(set: impl FnOnce(&mut DeckRules)) -> HouseRules {
        let mut deck = DeckRules::default();
        set(&mut deck);
        HouseRules {
            deck,
            ..HouseRules::default()
        }
    }

    fn card(name: &str, cost: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_mana_cost(ManaCost::parse(cost).unwrap()),
        )
    }

    fn leader(name: &str, cost: &str) -> Arc<CardDefinition> {
        let base = CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_supertype(Supertype::Legendary)
            .with_mana_cost(ManaCost::parse(cost).unwrap());
        Arc::new(base)
    }

    fn partner(name: &str, cost: &str) -> Arc<CardDefinition> {
        let rules: Ability = serde_json::from_str(r#"{"Keyword":"Partner"}"#).unwrap();
        let base = leader(name, cost);
        Arc::new((*base).clone().with_rules(rules))
    }

    /// `n` distinct red cards.
    fn spells(n: usize) -> Vec<Arc<CardDefinition>> {
        (0..n).map(|i| card(&format!("Red {i}"), "{R}")).collect()
    }

    fn mountains(n: usize) -> Vec<Arc<CardDefinition>> {
        let mountain = Arc::new(CardDefinition::basic_land(Some(Color::Red)));
        vec![mountain; n]
    }

    fn red_leader() -> Vec<Arc<CardDefinition>> {
        vec![leader("Red Leader", "{1}{R}")]
    }

    #[rstest]
    #[case::exactly_100(99, true)]
    #[case::one_short(98, false)]
    #[case::one_over(100, false)]
    #[case::empty(0, false)]
    fn a_deck_has_100_cards_with_its_commander(#[case] others: usize, #[case] legal: bool) {
        let result = check(&spells(others), &red_leader());
        assert_eq!(result.is_ok(), legal, "{result:?}");
        if !legal {
            assert_eq!(
                result,
                Err(DeckError::WrongSize {
                    expected: 100,
                    got: others + 1
                })
            );
        }
    }

    #[test]
    fn basic_lands_and_cards_that_allow_it_are_exempt_from_the_singleton_rule() {
        let rats = Arc::new(
            CardDefinition::new("Rats")
                .with_manual("A deck can have any number of cards named Rats.", ""),
        );
        let mut deck = spells(40);
        deck.extend(mountains(50));
        deck.extend(vec![rats; 9]);
        assert_eq!(check(&deck, &red_leader()), Ok(()));
    }

    #[rstest]
    #[case::a_second_copy(2)]
    #[case::five_copies(5)]
    fn a_second_copy_of_a_card_is_refused(#[case] copies: usize) {
        let twin = card("Twin", "{R}");
        let mut deck = spells(99 - copies);
        deck.extend(vec![twin; copies]);
        assert_eq!(
            check(&deck, &red_leader()),
            Err(DeckError::TooManyCopies {
                name: "Twin".into(),
                max: 1,
                got: copies
            })
        );
    }

    #[test]
    fn the_commander_counts_toward_the_singleton_rule() {
        let mut deck = spells(98);
        deck.push(leader("Red Leader", "{1}{R}"));
        assert!(matches!(
            check(&deck, &red_leader()),
            Err(DeckError::TooManyCopies { got: 2, .. })
        ));
    }

    fn with_one(cost: &str) -> Vec<Arc<CardDefinition>> {
        let mut deck = spells(98);
        deck.push(card("Odd", cost));
        deck
    }

    #[rstest]
    #[case::inside("{R}", true)]
    #[case::colorless("{3}", true)]
    #[case::hybrid_with_one_color_inside("{R/G}", false)]
    #[case::phyrexian_outside("{U/P}", false)]
    #[case::phyrexian_inside("{R/P}", true)]
    #[case::mono_hybrid_outside("{2/W}", false)]
    #[case::outside("{1}{G}", false)]
    fn a_card_is_inside_the_color_identity_of_the_commander(
        #[case] cost: &str,
        #[case] legal: bool,
    ) {
        let result = check(&with_one(cost), &red_leader());
        assert_eq!(result.is_ok(), legal, "{result:?}");
        if let Err(error) = result {
            assert!(
                matches!(&error, DeckError::OutsideColorIdentity { name, identity, .. } if name == "Odd" && identity == "R"),
                "{error}"
            );
        }
    }

    #[test]
    fn mana_in_an_activated_ability_is_part_of_the_identity() {
        let ability: Ability =
            serde_json::from_str(r#"{"Activated":{"cost":"{2}{U}","effects":[{"Draw":{}}]}}"#)
                .unwrap();
        let mut deck = spells(98);
        deck.push(Arc::new(
            (*card("Drawer", "{R}")).clone().with_rules(ability),
        ));
        assert!(matches!(
            check(&deck, &red_leader()),
            Err(DeckError::OutsideColorIdentity { outside, .. }) if outside == "U"
        ));
    }

    #[test]
    fn the_identity_of_a_pair_is_the_two_together() {
        let pair = vec![
            partner("Red Partner", "{R}"),
            partner("Blue Partner", "{U}"),
        ];
        let mut deck = spells(96);
        deck.push(card("Blue", "{U}"));
        deck.push(card("Both", "{R}{U}"));
        assert_eq!(check(&deck, &pair), Ok(()));
        deck.push(card("Green", "{G}"));
        assert!(matches!(
            check(&deck[1..], &pair),
            Err(DeckError::OutsideColorIdentity { identity, .. }) if identity == "UR" || identity == "RU"
        ));
    }

    #[test]
    fn a_colorless_commander_allows_colorless_cards_only() {
        let colorless = vec![leader("Gray Leader", "{4}")];
        let mut deck = vec![card("Rock", "{2}"); 1];
        deck.extend((0..98).map(|i| card(&format!("Gray {i}"), "{1}")));
        assert_eq!(check(&deck, &colorless), Ok(()));
        deck[0] = card("Red", "{R}");
        assert!(matches!(
            check(&deck, &colorless),
            Err(DeckError::OutsideColorIdentity { identity, .. }) if identity == "C"
        ));
    }

    #[rstest]
    #[case::none(vec![], DeckError::NoCommander)]
    #[case::not_legendary(vec![card("Bear", "{R}")], DeckError::NotACommander { name: "Bear".into() })]
    #[case::two_without_partner(
        vec![leader("A", "{R}"), leader("B", "{R}")],
        DeckError::NotPartners { first: "A".into(), second: "B".into() }
    )]
    #[case::three(
        vec![partner("A", "{R}"), partner("B", "{R}"), partner("C", "{R}")],
        DeckError::TooManyCommanders { max: 2, got: 3 }
    )]
    fn the_commanders_are_one_legendary_creature_or_a_pair_of_partners(
        #[case] commanders: Vec<Arc<CardDefinition>>,
        #[case] error: DeckError,
    ) {
        let deck = spells(100 - commanders.len());
        assert_eq!(check(&deck, &commanders), Err(error));
    }

    #[test]
    fn partners_make_a_legal_pair() {
        let pair = vec![partner("A", "{R}"), partner("B", "{R}")];
        assert_eq!(check(&spells(98), &pair), Ok(()));
    }

    #[test]
    fn a_deck_without_commanders_is_refused_and_normal_refuses_commanders() {
        assert_eq!(
            Commander.validate_deck(&spells(100)),
            Err(DeckError::NoCommander)
        );
        assert_eq!(
            crate::format::Normal.validate_seat(
                &spells(60),
                &red_leader(),
                &RuleSet::new(&crate::format::Normal, &HouseRules::default())
            ),
            Err(DeckError::CommandersNotAllowed)
        );
    }

    #[rstest]
    #[case::sixty(60, 59, true)]
    #[case::forty(40, 39, true)]
    #[case::the_most(250, 249, true)]
    #[case::a_hundred_where_sixty_is_set(60, 99, false)]
    #[case::one_short(60, 58, false)]
    #[case::one_over(40, 40, false)]
    fn a_table_sets_the_exact_deck_size(
        #[case] size: u32,
        #[case] others: usize,
        #[case] legal: bool,
    ) {
        let house = deck_rules(|d| d.deck_size = Some(size));
        let result = under(&house, &spells(others), &red_leader());
        assert_eq!(result.is_ok(), legal, "{result:?}");
        if !legal {
            assert_eq!(
                result,
                Err(DeckError::WrongSize {
                    expected: size as usize,
                    got: others + 1
                })
            );
        }
        // The same deck is refused under the format's own size unless it has 100 cards.
        assert_eq!(check(&spells(others), &red_leader()).is_ok(), others == 99);
    }

    fn with_copies(copies: usize) -> Vec<Arc<CardDefinition>> {
        let mut deck = spells(99 - copies);
        deck.extend(vec![card("Twin", "{R}"); copies]);
        deck
    }

    #[rstest]
    #[case::the_formats_singleton(deck_rules(|_| {}), 2, Some(1))]
    #[case::singleton_on(deck_rules(|d| d.singleton = Some(true)), 2, Some(1))]
    #[case::singleton_off_allows_any(deck_rules(|d| d.singleton = Some(false)), 40, None)]
    #[case::four_copies_where_four_are_allowed(deck_rules(|d| { d.singleton = Some(false); d.max_copies = Some(4); }), 4, None)]
    #[case::five_copies_where_four_are_allowed(deck_rules(|d| { d.singleton = Some(false); d.max_copies = Some(4); }), 5, Some(4))]
    #[case::one_copy_where_one_is_allowed(deck_rules(|d| { d.singleton = Some(false); d.max_copies = Some(1); }), 2, Some(1))]
    #[case::ninety_nine(deck_rules(|d| { d.singleton = Some(false); d.max_copies = Some(99); }), 99, None)]
    fn a_table_relaxes_the_singleton_rule(
        #[case] house: HouseRules,
        #[case] copies: usize,
        #[case] refused_at: Option<usize>,
    ) {
        let result = under(&house, &with_copies(copies), &red_leader());
        match refused_at {
            None => assert_eq!(result, Ok(())),
            Some(max) => assert_eq!(
                result,
                Err(DeckError::TooManyCopies {
                    name: "Twin".into(),
                    max,
                    got: copies
                })
            ),
        }
    }

    #[test]
    fn a_relaxed_singleton_keeps_the_limit_of_a_card_that_asks_for_more_and_exempts_basic_lands() {
        let house = deck_rules(|d| {
            d.singleton = Some(false);
            d.max_copies = Some(3);
        });
        let dwarves = Arc::new(
            CardDefinition::new("Dwarves")
                .with_manual("A deck can have up to seven cards named Dwarves.", ""),
        );
        let mut deck = spells(40);
        deck.extend(vec![dwarves; 7]);
        deck.extend(mountains(52));
        assert_eq!(under(&house, &deck, &red_leader()), Ok(()));
    }

    #[rstest]
    #[case::the_formats_identity(HouseRules::default(), false)]
    #[case::identity_on(deck_rules(|d| d.colour_identity = Some(true)), false)]
    #[case::identity_off(deck_rules(|d| d.colour_identity = Some(false)), true)]
    fn a_table_lifts_the_colour_identity(#[case] house: HouseRules, #[case] legal: bool) {
        let result = under(&house, &with_one("{1}{G}"), &red_leader());
        assert_eq!(result.is_ok(), legal, "{result:?}");
        assert!(matches!(
            result,
            Ok(()) | Err(DeckError::OutsideColorIdentity { .. })
        ));
    }

    #[test]
    fn the_relaxations_are_independent_of_each_other() {
        let house = deck_rules(|d| d.colour_identity = Some(false));
        assert!(matches!(
            under(&house, &spells(98), &red_leader()),
            Err(DeckError::WrongSize { .. })
        ));
        assert!(matches!(
            under(&house, &with_copies(2), &red_leader()),
            Err(DeckError::TooManyCopies { .. })
        ));
        // The commanders are still checked.
        assert_eq!(
            under(&house, &spells(100), &[]),
            Err(DeckError::NoCommander)
        );
        assert_eq!(
            under(&house, &spells(99), &[card("Bear", "{R}")]),
            Err(DeckError::NotACommander {
                name: "Bear".into()
            })
        );
    }

    fn per_seat(most: u32) -> HouseRules {
        HouseRules {
            commanders_per_seat: Some(most),
            ..HouseRules::default()
        }
    }

    #[rstest]
    #[case::the_formats_pair_needs_partners(HouseRules::default(), 2, false, Some(DeckError::NotPartners { first: "A".into(), second: "B".into() }))]
    #[case::two_allow_any_two(per_seat(2), 2, false, None)]
    #[case::two_allow_partners_too(per_seat(2), 2, true, None)]
    #[case::two_allow_one(per_seat(2), 1, false, None)]
    #[case::two_refuse_three(per_seat(2), 3, true, Some(DeckError::TooManyCommanders { max: 2, got: 3 }))]
    #[case::one_refuses_partners(per_seat(1), 2, true, Some(DeckError::TooManyCommanders { max: 1, got: 2 }))]
    #[case::one_allows_one(per_seat(1), 1, true, None)]
    fn a_table_sets_the_commanders_a_seat_starts_with(
        #[case] house: HouseRules,
        #[case] count: usize,
        #[case] partners: bool,
        #[case] error: Option<DeckError>,
    ) {
        let make = |name: &str| {
            if partners {
                partner(name, "{R}")
            } else {
                leader(name, "{R}")
            }
        };
        let commanders: Vec<_> = ["A", "B", "C"][..count].iter().map(|n| make(n)).collect();
        let deck = spells(100 - count);
        assert_eq!(under(&house, &deck, &commanders), error.map_or(Ok(()), Err));
    }

    #[test]
    fn two_commanders_that_are_no_partners_still_must_be_commanders() {
        let commanders = vec![leader("A", "{R}"), card("Bear", "{R}")];
        assert_eq!(
            under(&per_seat(2), &spells(98), &commanders),
            Err(DeckError::NotACommander {
                name: "Bear".into()
            })
        );
    }

    #[rstest]
    #[case(2, true)]
    #[case(3, false)]
    #[case(6, false)]
    fn only_a_two_player_game_skips_the_first_draw(#[case] seats: usize, #[case] skips: bool) {
        assert_eq!(Commander.skips_first_draw(seats), skips);
    }

    #[test]
    fn commander_constants() {
        assert_eq!(Commander.name(), "commander");
        assert_eq!(Commander.starting_life(), 40);
        assert_eq!(
            Commander.mulligan_rule(),
            MulliganRule {
                hand_size: 7,
                free_mulligans: 1
            }
        );
        assert_eq!(Commander.mulligan_rule().bottom_count(1), 0);
        assert_eq!(Commander.mulligan_rule().bottom_count(2), 1);
        assert_eq!(Commander.extra_zones(), &[ExtraZone::Command]);
        assert_eq!(Commander.commander_tax(), 2);
        assert_eq!(Commander.commander_damage(), Some(21));
        assert_eq!(Commander.max_hand_size(), 7);
    }
}
