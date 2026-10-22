//! The Commander format (CR 903): a 100-card singleton deck led by one commander or a pair of partners, played from a
//! command zone, with 40 life, a free first mulligan, a tax on casting the commander and commander damage.

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::PlayerId;

use super::{DeckError, ExtraZone, FormatRules, MulliganRule};
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

/// The commanders are one legendary creature (or a card that can be) or two that are partners.
fn check_commanders(commanders: &[Arc<CardDefinition>]) -> Result<(), DeckError> {
    match commanders {
        [] => return Err(DeckError::NoCommander),
        [_] => {}
        [first, second] => {
            if !first.partners_with(second) {
                return Err(DeckError::NotPartners {
                    first: first.name.clone(),
                    second: second.name.clone(),
                });
            }
        }
        more => return Err(DeckError::TooManyCommanders { got: more.len() }),
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
        self.validate_seat(deck, &[])
    }

    fn validate_seat(
        &self,
        deck: &[Arc<CardDefinition>],
        commanders: &[Arc<CardDefinition>],
    ) -> Result<(), DeckError> {
        check_commanders(commanders)?;
        let size = deck.len() + commanders.len();
        if size != COMMANDER_DECK {
            return Err(DeckError::WrongSize {
                expected: COMMANDER_DECK,
                got: size,
            });
        }
        let mut copies: BTreeMap<&str, (usize, Option<u32>)> = BTreeMap::new();
        for card in commanders.iter().chain(deck) {
            let entry = copies
                .entry(card.name.as_str())
                .or_insert((0, card.singleton_limit()));
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
    //! Goal: the Commander format checks a seat's commanders and deck (one commander or a partner pair of legendary creatures, exactly 100 cards with the commanders, one copy of each card but basic lands and cards that allow more, every card inside the commanders' color identity by cost, hybrid and Phyrexian symbols and activated abilities) with a typed error for each rule, and reports its constants: 40 life, a free first mulligan, a command zone, a tax of 2, 21 commander damage, and a skipped first draw in two-player games only.
    use mtg_cards::ability::Ability;
    use mtg_core::card::{CardType, Supertype};
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;

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
        let result = Commander.validate_seat(&spells(others), &red_leader());
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
        let rats = Arc::new(CardDefinition::new("Rats").with_manual(
            "A deck can have any number of cards named Rats.",
            "",
        ));
        let mut deck = spells(40);
        deck.extend(mountains(50));
        deck.extend(vec![rats; 9]);
        assert_eq!(Commander.validate_seat(&deck, &red_leader()), Ok(()));
    }

    #[rstest]
    #[case::a_second_copy(2)]
    #[case::five_copies(5)]
    fn a_second_copy_of_a_card_is_refused(#[case] copies: usize) {
        let twin = card("Twin", "{R}");
        let mut deck = spells(99 - copies);
        deck.extend(vec![twin; copies]);
        assert_eq!(
            Commander.validate_seat(&deck, &red_leader()),
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
            Commander.validate_seat(&deck, &red_leader()),
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
        let result = Commander.validate_seat(&with_one(cost), &red_leader());
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
        let ability: Ability = serde_json::from_str(
            r#"{"Activated":{"cost":"{2}{U}","effects":[{"Draw":{}}]}}"#,
        )
        .unwrap();
        let mut deck = spells(98);
        deck.push(Arc::new(
            (*card("Drawer", "{R}")).clone().with_rules(ability),
        ));
        assert!(matches!(
            Commander.validate_seat(&deck, &red_leader()),
            Err(DeckError::OutsideColorIdentity { outside, .. }) if outside == "U"
        ));
    }

    #[test]
    fn the_identity_of_a_pair_is_the_two_together() {
        let pair = vec![partner("Red Partner", "{R}"), partner("Blue Partner", "{U}")];
        let mut deck = spells(96);
        deck.push(card("Blue", "{U}"));
        deck.push(card("Both", "{R}{U}"));
        assert_eq!(Commander.validate_seat(&deck, &pair), Ok(()));
        deck.push(card("Green", "{G}"));
        assert!(matches!(
            Commander.validate_seat(&deck[1..], &pair),
            Err(DeckError::OutsideColorIdentity { identity, .. }) if identity == "UR" || identity == "RU"
        ));
    }

    #[test]
    fn a_colorless_commander_allows_colorless_cards_only() {
        let colorless = vec![leader("Gray Leader", "{4}")];
        let mut deck = vec![card("Rock", "{2}"); 1];
        deck.extend((0..98).map(|i| card(&format!("Gray {i}"), "{1}")));
        assert_eq!(Commander.validate_seat(&deck, &colorless), Ok(()));
        deck[0] = card("Red", "{R}");
        assert!(matches!(
            Commander.validate_seat(&deck, &colorless),
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
        DeckError::TooManyCommanders { got: 3 }
    )]
    fn the_commanders_are_one_legendary_creature_or_a_pair_of_partners(
        #[case] commanders: Vec<Arc<CardDefinition>>,
        #[case] error: DeckError,
    ) {
        let deck = spells(100 - commanders.len());
        assert_eq!(Commander.validate_seat(&deck, &commanders), Err(error));
    }

    #[test]
    fn partners_make_a_legal_pair() {
        let pair = vec![partner("A", "{R}"), partner("B", "{R}")];
        assert_eq!(Commander.validate_seat(&spells(98), &pair), Ok(()));
    }

    #[test]
    fn a_deck_without_commanders_is_refused_and_normal_refuses_commanders() {
        assert_eq!(
            Commander.validate_deck(&spells(100)),
            Err(DeckError::NoCommander)
        );
        assert_eq!(
            crate::format::Normal.validate_seat(&spells(60), &red_leader()),
            Err(DeckError::CommandersNotAllowed)
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
