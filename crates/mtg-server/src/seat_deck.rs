//! The decks the seats of a lobby play.
//!
//! A [`ChosenDeck`] is a snapshot taken when a deck is chosen: the cards the engine is given, the
//! recipe's name for them (a built-in deck's id, or the oracle id of every card) with their
//! fingerprint, and the three facts the table is told of it. Editing or deleting the stored deck
//! afterwards changes nothing. [`SeatDecks`] keeps one choice per seat while the game is in its
//! lobby; the decks of the seats go into the game's recipe when it starts.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::deck::Issue;
use mtg_core::house::HouseRules;
use mtg_protocol::SeatDeck;

use crate::cards::with_printings;
use crate::deck_choice::format_issues;
use crate::demo::{self, DemoDeck};
use crate::recipe::{DeckFingerprint, DeckRef, FingerprintError, FormatKind, GameRecipe};

/// The format key the table is told for a built-in deck.
const DEMO_FORMAT: &str = "normal";

/// A deck chosen for a seat, as it was when it was chosen.
#[derive(Debug, Clone)]
pub struct ChosenDeck {
    cards: Arc<[Arc<CardDefinition>]>,
    commanders: Arc<[Arc<CardDefinition>]>,
    companion: Option<Arc<CardDefinition>>,
    deck: DeckRef,
    fingerprint: DeckFingerprint,
    note: SeatDeck,
    own_issues: Arc<[Issue]>,
}

impl ChosenDeck {
    /// A built-in deck. Every card is one the engine fully supports, so the table has nothing to resolve by hand.
    pub fn demo(deck: &DemoDeck) -> Result<ChosenDeck, FingerprintError> {
        let cards = deck.cards();
        Ok(ChosenDeck {
            fingerprint: DeckFingerprint::of(&cards)?,
            cards: cards.into(),
            commanders: Arc::from([]),
            companion: None,
            deck: DeckRef::Demo {
                id: deck.id.to_owned(),
            },
            note: SeatDeck {
                name: deck.name.to_owned(),
                format: DEMO_FORMAT.to_owned(),
                manual_cards: 0,
            },
            own_issues: Arc::from([]),
        })
    }

    /// What a Commander seat holds until its holder chooses a deck; the seat cannot be ready with it.
    pub fn commander_stand_in() -> Result<ChosenDeck, FingerprintError> {
        ChosenDeck::commander_stand_in_of(demo::COMMANDER_DECK)
    }

    /// Like [`ChosenDeck::commander_stand_in`], for a table whose decks hold `size` cards, the
    /// commander included, so the stand-in passes the table's own deck check.
    pub fn commander_stand_in_of(size: usize) -> Result<ChosenDeck, FingerprintError> {
        let (commander, library) = demo::commander_stand_in_of(size);
        Ok(ChosenDeck {
            fingerprint: DeckFingerprint::of_seat(&library, std::slice::from_ref(&commander))?,
            cards: library.into(),
            commanders: Arc::from([commander]),
            companion: None,
            deck: DeckRef::Demo {
                id: demo::commander_stand_in_id(size),
            },
            note: SeatDeck {
                name: "No deck".to_owned(),
                format: "commander".to_owned(),
                manual_cards: 0,
            },
            own_issues: Arc::from([]),
        })
    }

    /// A stored deck as assembled: `oracle_ids` names every card of the library in deck order and
    /// `cards` holds their definitions in the same order. `note` is what the table is told.
    pub fn stored(
        oracle_ids: Vec<String>,
        cards: Vec<Arc<CardDefinition>>,
        note: SeatDeck,
    ) -> Result<ChosenDeck, FingerprintError> {
        ChosenDeck::led(oracle_ids, cards, Vec::new(), Vec::new(), note)
    }

    /// Like [`ChosenDeck::stored`], for a deck led by commanders: `commander_ids` names them and
    /// `commanders` holds their definitions in the same order. The seat starts with them in its
    /// command zone, and they are part of the deck's fingerprint.
    pub fn led(
        oracle_ids: Vec<String>,
        cards: Vec<Arc<CardDefinition>>,
        commander_ids: Vec<String>,
        commanders: Vec<Arc<CardDefinition>>,
        note: SeatDeck,
    ) -> Result<ChosenDeck, FingerprintError> {
        Ok(ChosenDeck {
            fingerprint: DeckFingerprint::of_seat(&cards, &commanders)?,
            cards: cards.into(),
            commanders: commanders.into(),
            companion: None,
            deck: DeckRef::Oracle {
                cards: oracle_ids,
                commanders: commander_ids,
                companion: None,
                printings: Vec::new(),
            },
            note,
            own_issues: Arc::from([]),
        })
    }

    /// The deck with the printing each deck line chose: one entry for every card of the library, then of the
    /// commanders, then the companion, `None` for a card whose line named none. The cards carry it in their identity
    /// and the recipe keeps it for a resumed game; the fingerprint is the same. A built-in deck has no printings and
    /// stays as it is.
    #[must_use]
    pub fn with_printings(mut self, printings: Vec<Option<String>>) -> ChosenDeck {
        let DeckRef::Oracle {
            printings: kept, ..
        } = &mut self.deck
        else {
            return self;
        };
        if printings.iter().all(Option::is_none) {
            return self;
        }
        let (library, leaders) = (self.cards.len(), self.commanders.len());
        let all: Vec<Arc<CardDefinition>> = self
            .cards
            .iter()
            .chain(self.commanders.iter())
            .chain(self.companion.iter())
            .cloned()
            .collect();
        let mut printed = with_printings(&all, &printings);
        self.companion = printed.split_off(library + leaders).pop();
        self.commanders = printed.split_off(library).into();
        self.cards = printed.into();
        *kept = printings;
        self
    }

    /// The deck with `card` (its oracle id is `oracle_id`) as the seat's companion (CR 702.139): the
    /// game keeps it outside the game, and it is part of the deck's fingerprint. A built-in deck has
    /// no oracle ids to name it by and stays as it is.
    pub fn with_companion(
        mut self,
        oracle_id: String,
        card: Arc<CardDefinition>,
    ) -> Result<ChosenDeck, FingerprintError> {
        if let DeckRef::Oracle { companion, .. } = &mut self.deck {
            *companion = Some(oracle_id);
            self.fingerprint =
                DeckFingerprint::of_seat_with(&self.cards, &self.commanders, Some(&card))?;
            self.companion = Some(card);
        }
        Ok(self)
    }

    /// What the table is told of the deck.
    pub fn note(&self) -> &SeatDeck {
        &self.note
    }

    /// How the recipe names the deck.
    pub fn reference(&self) -> &DeckRef {
        &self.deck
    }

    /// The fingerprint of the cards the engine is given.
    pub fn fingerprint(&self) -> &DeckFingerprint {
        &self.fingerprint
    }

    /// The library, in deck order.
    pub fn cards(&self) -> &[Arc<CardDefinition>] {
        &self.cards
    }

    /// The commanders the seat starts with in its command zone; none outside Commander.
    pub fn commanders(&self) -> &[Arc<CardDefinition>] {
        &self.commanders
    }

    /// The companion the seat names, a card the game keeps outside it until the seat pays for it.
    pub fn companion(&self) -> Option<&Arc<CardDefinition>> {
        self.companion.as_ref()
    }

    /// The deck with the issues its own check found (`deck_report` in the deck's format), which a
    /// table's deck rules may lift ([`ChosenDeck::issues_in`]). Only issues of codes a table can lift
    /// belong here: the others refuse the deck before it is chosen.
    #[must_use]
    pub(crate) fn with_own_issues(mut self, issues: Vec<Issue>) -> ChosenDeck {
        self.own_issues = issues.into();
        self
    }

    /// The rules of `format`, under the `house` rules of the game, that the deck breaks, in the words
    /// of the deck check; none when the game accepts the deck.
    ///
    /// A Commander table that relaxed a rule the deck's own check refused for (size, copies, colour
    /// identity, the pair of commanders) leaves that issue to the engine's `validate_seat` under the
    /// table's rules; every other issue of the deck's check stands. The engine judges a deck whose
    /// own check has nothing left.
    pub fn issues_in(&self, format: FormatKind, house: &HouseRules) -> Vec<Issue> {
        let none = HouseRules::default();
        let house = if format == FormatKind::Commander {
            house
        } else {
            &none
        };
        let own: Vec<Issue> = self
            .own_issues
            .iter()
            .filter(|issue| !issue.code.lifted_by(house))
            .cloned()
            .collect();
        if own.is_empty() {
            format_issues(
                format,
                &self.cards,
                &self.commanders,
                self.companion.as_ref(),
                house,
            )
        } else {
            own
        }
    }
}

/// The choice of every seat of a lobby, and the decks the game was made with.
#[derive(Debug)]
pub(crate) struct SeatDecks {
    format: FormatKind,
    house_rules: HouseRules,
    seed: u64,
    /// What each seat plays until its holder chooses, and again once the seat is given back.
    defaults: Vec<Option<ChosenDeck>>,
    current: Vec<Option<ChosenDeck>>,
}

impl SeatDecks {
    /// The lobby of a game made with `defaults`, one entry per seat; a seat without a deck cannot
    /// press Ready.
    pub(crate) fn new(format: FormatKind, seed: u64, defaults: Vec<Option<ChosenDeck>>) -> Self {
        SeatDecks {
            format,
            house_rules: HouseRules::default(),
            seed,
            current: defaults.clone(),
            defaults,
        }
    }

    /// The lobby of a game played under `house_rules`, which every recipe it writes keeps.
    #[must_use]
    pub(crate) fn with_house_rules(mut self, house_rules: HouseRules) -> Self {
        self.house_rules = house_rules;
        self
    }

    /// The lobby of a game stored in `recipe`: only built-in decks come back as chosen, since the
    /// recipe keeps no name for a stored deck; its seat chooses again.
    pub(crate) fn of_recipe(recipe: &GameRecipe) -> Self {
        let defaults = recipe
            .decks
            .iter()
            .map(|deck| match deck {
                DeckRef::Demo { id } => demo::find(id).and_then(|deck| ChosenDeck::demo(deck).ok()),
                DeckRef::Oracle { .. } => None,
            })
            .collect();
        SeatDecks::new(recipe.format, recipe.seed, defaults)
            .with_house_rules(recipe.house_rules.clone())
    }

    /// What the table is told of `seat`'s deck.
    pub(crate) fn note(&self, seat: usize) -> Option<&SeatDeck> {
        self.current.get(seat)?.as_ref().map(ChosenDeck::note)
    }

    /// Whether `seat` has a deck.
    pub(crate) fn has(&self, seat: usize) -> bool {
        self.current.get(seat).is_some_and(Option::is_some)
    }

    /// `seat` plays `deck` from now on; false for a seat the lobby does not have.
    pub(crate) fn choose(&mut self, seat: usize, deck: ChosenDeck) -> bool {
        match self.current.get_mut(seat) {
            Some(slot) => {
                *slot = Some(deck);
                true
            }
            None => false,
        }
    }

    /// `seat` goes back to the deck the game was made with, if it had one; that deck, for the
    /// library to follow.
    pub(crate) fn reset(&mut self, seat: usize) -> Option<&ChosenDeck> {
        let default = self.defaults.get(seat)?.clone();
        *self.current.get_mut(seat)? = default;
        self.current.get(seat)?.as_ref()
    }

    /// Whether any seat plays something else than the deck the game was made with.
    pub(crate) fn changed(&self) -> bool {
        let named = |deck: &Option<ChosenDeck>| {
            deck.as_ref()
                .map(|deck| (deck.deck.clone(), deck.fingerprint))
        };
        self.current
            .iter()
            .map(named)
            .ne(self.defaults.iter().map(named))
    }

    /// The recipe of the game as the seats chose it; none while a seat has no deck.
    pub(crate) fn recipe(&self) -> Option<GameRecipe> {
        let chosen: Option<Vec<&ChosenDeck>> = self.current.iter().map(Option::as_ref).collect();
        let chosen = chosen?;
        Some(GameRecipe {
            format: self.format,
            house_rules: self.house_rules.clone(),
            seed: self.seed,
            decks: chosen.iter().map(|deck| deck.deck.clone()).collect(),
            fingerprints: chosen.iter().map(|deck| deck.fingerprint).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a lobby's decks follow the seats: a seat plays what it chose until it is given back and then what the game was made with, a seat without a deck keeps the recipe from being written, and only a change from the creation decks asks for a new recipe. A commander table lifts the issues of the deck's own check that its relaxed rules replace, and a Normal game lifts none.
    use mtg_cards::deck::IssueCode;
    use mtg_cards::vanilla;
    use mtg_core::color::Color;

    use super::*;

    fn demo_deck(id: &str) -> ChosenDeck {
        ChosenDeck::demo(demo::find(id).unwrap()).unwrap()
    }

    fn stored_deck(name: &str) -> ChosenDeck {
        let cards: Vec<Arc<CardDefinition>> =
            std::iter::repeat_n(vanilla::basic_land(Color::Green), 60).collect();
        let note = SeatDeck {
            name: name.to_owned(),
            format: "modern".to_owned(),
            manual_cards: 3,
        };
        ChosenDeck::stored(vec!["forest".to_owned(); 60], cards, note).unwrap()
    }

    fn issue(code: IssueCode) -> Issue {
        Issue {
            code,
            message: "x".to_owned(),
            zone: None,
            oracle_id: None,
            name: None,
        }
    }

    #[rstest::rstest]
    #[case::no_rules(HouseRules::default(), &[IssueCode::Copies, IssueCode::DeckSize])]
    #[case::one_lifted(deck_rules(|d| d.singleton = Some(false)), &[IssueCode::DeckSize])]
    #[case::both_lifted(deck_rules(|d| { d.singleton = Some(false); d.deck_size = Some(60); }), &[])]
    #[case::another_rule(deck_rules(|d| d.colour_identity = Some(false)), &[IssueCode::Copies, IssueCode::DeckSize])]
    fn a_commander_table_lifts_the_issues_of_the_rules_it_relaxed(
        #[case] house: HouseRules,
        #[case] left: &[IssueCode],
    ) {
        let (commander, library) = demo::commander_stand_in();
        let note = SeatDeck {
            name: "x".to_owned(),
            format: "commander".to_owned(),
            manual_cards: 0,
        };
        let own = vec![issue(IssueCode::Copies), issue(IssueCode::DeckSize)];
        let chosen = ChosenDeck::led(vec![], library, vec![], vec![commander], note)
            .unwrap()
            .with_own_issues(own);
        let codes: Vec<IssueCode> = chosen
            .issues_in(FormatKind::Commander, &house)
            .into_iter()
            .map(|issue| issue.code)
            .collect();
        // What is left of the deck's own check is all that is said; when nothing is left, the engine
        // judges the deck under the table's rules (a 100-card deck against a 60-card table).
        let expected: Vec<IssueCode> = if left.is_empty() {
            vec![IssueCode::DeckSize]
        } else {
            left.to_vec()
        };
        assert_eq!(codes, expected);
        // A Normal game has no deck rules to lift anything.
        let normal: Vec<IssueCode> = chosen
            .issues_in(FormatKind::Normal, &house)
            .into_iter()
            .map(|issue| issue.code)
            .collect();
        assert_eq!(normal, [IssueCode::Copies, IssueCode::DeckSize]);
    }

    fn deck_rules(set: impl FnOnce(&mut mtg_core::house::DeckRules)) -> HouseRules {
        let mut deck = mtg_core::house::DeckRules::default();
        set(&mut deck);
        HouseRules {
            deck,
            ..HouseRules::default()
        }
    }

    fn lobby() -> SeatDecks {
        SeatDecks::new(
            FormatKind::Normal,
            7,
            vec![Some(demo_deck("mono-green")), None],
        )
    }

    #[test]
    fn a_built_in_deck_tells_the_table_its_name_and_no_manual_cards() {
        let deck = demo_deck("mono-red");
        assert_eq!(
            deck.note(),
            &SeatDeck {
                name: "Mono Red Giants".into(),
                format: "normal".into(),
                manual_cards: 0
            }
        );
        assert_eq!(deck.cards().len(), 60);
        assert_eq!(
            deck.fingerprint(),
            &DeckFingerprint::of(deck.cards()).unwrap(),
            "the fingerprint is the one of the cards the engine is given"
        );
    }

    #[test]
    fn a_seat_has_a_deck_once_it_chooses_one() {
        let mut decks = lobby();
        assert!(decks.has(0) && !decks.has(1) && !decks.has(2));
        assert_eq!(decks.note(1), None);
        assert!(decks.choose(1, stored_deck("Burn")));
        assert_eq!(decks.note(1).map(|n| n.name.as_str()), Some("Burn"));
        assert!(!decks.choose(2, stored_deck("Burn")), "no such seat");
    }

    #[test]
    fn a_seat_given_back_returns_to_the_deck_the_game_was_made_with() {
        let mut decks = lobby();
        decks.choose(0, stored_deck("Burn"));
        decks.choose(1, stored_deck("Control"));
        assert_eq!(
            decks.reset(0).map(|deck| deck.note().name.clone()),
            Some("Mono Green Stompers".to_owned())
        );
        assert!(decks.reset(1).is_none(), "it was made without one");
        assert!(!decks.has(1));
    }

    #[test]
    fn the_recipe_waits_for_every_seat_and_names_each_deck_with_its_fingerprint() {
        let mut decks = lobby();
        assert_eq!(decks.recipe(), None, "a seat has no deck");
        decks.choose(1, stored_deck("Burn"));
        let recipe = decks.recipe().unwrap();
        assert_eq!(recipe.seed, 7);
        assert_eq!(
            recipe.decks,
            [
                DeckRef::Demo {
                    id: "mono-green".into()
                },
                DeckRef::oracle(vec!["forest".into(); 60])
            ]
        );
        assert_eq!(recipe.fingerprints.len(), 2);
        assert_eq!(recipe.fingerprints[1], *stored_deck("Burn").fingerprint());
    }

    #[test]
    fn only_a_deck_other_than_the_one_the_game_was_made_with_is_a_change() {
        let mut decks = lobby();
        assert!(!decks.changed());
        decks.choose(0, demo_deck("mono-green"));
        assert!(!decks.changed(), "the same deck again changes nothing");
        decks.choose(0, demo_deck("mono-red"));
        assert!(decks.changed());
        decks.reset(0);
        assert!(!decks.changed());
        decks.choose(1, stored_deck("Burn"));
        assert!(decks.changed(), "a seat made without a deck chose one");
    }

    #[test]
    fn a_lobby_read_back_from_its_recipe_keeps_the_built_in_decks_and_forgets_the_stored_ones() {
        let recipe = GameRecipe::normal(
            3,
            [
                DeckRef::Demo {
                    id: "mono-white".into(),
                },
                DeckRef::oracle(vec!["forest".into(); 60]),
                DeckRef::Demo {
                    id: "no-such-deck".into(),
                },
            ],
        );
        let decks = SeatDecks::of_recipe(&recipe);
        assert_eq!(
            decks.note(0).map(|n| n.name.as_str()),
            Some("Mono White Weenies")
        );
        assert!(!decks.has(1) && !decks.has(2));
        assert!(!decks.changed());
    }
}

#[cfg(test)]
mod printing_tests {
    //! Goal: a chosen deck takes the printing of each deck line for the cards of its library, its commanders and its companion, keeps them in its recipe, keeps its fingerprint, and ignores a list that names none and a built-in deck.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use mtg_cards::vanilla;
    use mtg_core::color::Color;
    use rstest::rstest;

    use super::*;
    use crate::cards::identified;

    fn card(name: &str, id: &str) -> Arc<CardDefinition> {
        let bare = vanilla::creature(name, 1, &[Color::Green], 2, 2);
        Arc::new(identified((*bare).clone(), id))
    }

    fn deck() -> ChosenDeck {
        let note = SeatDeck {
            name: "Greens".to_owned(),
            format: "commander".to_owned(),
            manual_cards: 0,
        };
        let bear = card("Bear", "bear");
        let library = vec![bear.clone(), bear, card("Elk", "elk")];
        ChosenDeck::led(
            vec!["bear".into(), "bear".into(), "elk".into()],
            library,
            vec!["chief".into()],
            vec![card("Chief", "chief")],
            note,
        )
        .unwrap()
        .with_companion("pal".into(), card("Pal", "pal"))
        .unwrap()
    }

    fn printing(card: &CardDefinition) -> Option<&str> {
        card.identity.as_ref()?.printing_id.as_deref()
    }

    #[rstest]
    #[case::each_position(vec![Some("a"), Some("b"), None, Some("c"), Some("d")])]
    #[case::copies_in_one_printing(vec![Some("a"), Some("a"), Some("e"), None, None])]
    #[case::only_the_companion(vec![None, None, None, None, Some("d")])]
    fn a_position_takes_the_printing_of_its_line(#[case] printings: Vec<Option<&str>>) {
        let printings: Vec<Option<String>> = printings
            .into_iter()
            .map(|p| p.map(str::to_owned))
            .collect();
        let plain = deck();
        let printed = deck().with_printings(printings.clone());
        let seen: Vec<Option<&str>> = printed
            .cards()
            .iter()
            .chain(printed.commanders())
            .chain(printed.companion())
            .map(|card| printing(card))
            .collect();
        let wanted: Vec<Option<&str>> = printings.iter().map(Option::as_deref).collect();
        assert_eq!(seen, wanted);
        assert_eq!(printed.fingerprint(), plain.fingerprint());
        assert_eq!(printed.cards().len(), 3);
        assert_eq!(printed.commanders().len(), 1);
        // The recipe of a lobby with the deck names the printings.
        let decks = SeatDecks::new(FormatKind::Commander, 1, vec![Some(printed)]);
        let recipe = decks.recipe().unwrap();
        assert!(
            matches!(&recipe.decks[0], DeckRef::Oracle { printings: kept, .. } if kept == &printings)
        );
    }

    #[test]
    fn a_list_that_names_nothing_and_a_built_in_deck_stay_as_they_are() {
        let plain = deck();
        let same = deck().with_printings(vec![None; 5]);
        assert_eq!(same.reference(), plain.reference());
        assert!(same.cards().iter().all(|card| printing(card).is_none()));
        let demo = ChosenDeck::demo(demo::find("mono-green").unwrap()).unwrap();
        let named = demo.clone().with_printings(vec![Some("x".to_owned()); 60]);
        assert_eq!(named.reference(), demo.reference());
        assert!(named.cards().iter().all(|card| card.identity.is_none()));
    }
}
