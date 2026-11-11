//! Splice (CR 702.47): as a spell of the kind a card names is cast, its caster may reveal that card from their hand and
//! pay its splice cost; the spell then has the card's effects after its own, with targets of their own (702.47b), and the
//! card stays in the hand.
//!
//! The announcement offers the cards of the hand that may be spliced (`spliceable`) in its splice step, keeps the cards
//! chosen in `Choices::spliced`, adds their costs to the total cost and reveals them as the spell goes on the stack. The
//! spell ability is read with the spliced cards' effects added wherever it is read ([`with_spliced`]), so the targets are
//! chosen, checked and resolved like any others, and a copy of the spell copies them with its choices (CR 707.10).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::assemble::spliced_spell;
use mtg_cards::keyword::{Keyword, SpliceOnto};
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use super::KeywordRules;
use crate::ability_reader;
use crate::characteristics::characteristics;
use crate::price::Price;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of splice: it adds no ability; the announcement asks [`spliceable`].
#[derive(Debug)]
pub(super) struct Splice;

impl KeywordRules for Splice {}

/// A card of the caster's hand that may be spliced onto the spell being cast, and its splice cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spliceable {
    /// The card.
    pub(crate) card: ObjectId,
    /// "Splice <name> <cost>", as the choice offers it.
    pub(crate) label: String,
    /// The mana of the splice cost, added to the total cost.
    pub(crate) mana: ManaCost,
}

/// Whether a spell with the characteristics of `spell` is of the kind `onto` names.
fn is_onto(state: &GameState, spell: ObjectId, onto: SpliceOnto) -> bool {
    let Some(printed) = state.object(spell).map(|object| &object.card().subtypes) else {
        return false;
    };
    characteristics(state, spell).is_some_and(|chars| match onto {
        SpliceOnto::Arcane => chars.has_subtype(printed, &Subtype("Arcane".to_owned())),
        SpliceOnto::InstantOrSorcery => {
            chars.types.contains(CardType::Instant) || chars.types.contains(CardType::Sorcery)
        }
    })
}

/// The splice cost a card prints for spells like `spell`, when the engine pays it: mana alone (CR 702.47a).
fn splice_cost(state: &GameState, card: ObjectId, spell: ObjectId) -> Option<ManaCost> {
    let object = state.object(card)?;
    ability_reader::printed_rules(object.card()).find_map(|rule| match &**rule {
        Ability::Keyword(Keyword::Splice { onto, cost }) if is_onto(state, spell, *onto) => {
            let price = Price::announced(cost).ok()?;
            let mana_only = !price.tap
                && !price.untap
                && price.x == 0
                && price.parts.is_empty()
                && price.changes.is_empty();
            mana_only.then(|| price.mana.unwrap_or_default())
        }
        _ => None,
    })
}

/// The spell ability a card adds when it is spliced.
fn spell_of(state: &GameState, card: ObjectId) -> Option<&Arc<Ability>> {
    let object = state.object(card)?;
    ability_reader::printed_rules(object.card())
        .find(|rule| matches!(***rule, Ability::Spell { .. }))
}

/// The cards of `player`'s hand that may be spliced onto `spell` as it is cast, in the order of the hand: each other card
/// whose splice names the spell's kind, that has effects to add and a cost the engine pays (CR 702.47a).
pub(crate) fn spliceable(state: &GameState, player: PlayerId, spell: ObjectId) -> Vec<Spliceable> {
    state
        .zone(Zone::Hand(player))
        .iter()
        .filter(|card| **card != spell)
        .filter_map(|card| {
            let mana = splice_cost(state, *card, spell)?;
            spell_of(state, *card)?;
            let name = &state.object(*card)?.card().name;
            Some(Spliceable {
                card: *card,
                label: format!("Splice {name} {mana}"),
                mana,
            })
        })
        .collect()
}

/// The spell ability `rules` with the effects of the `spliced` cards added in order (CR 702.47b); the rules as they are
/// when nothing is spliced or a card has no spell ability.
pub(crate) fn with_spliced(
    state: &GameState,
    rules: Arc<Ability>,
    spliced: &[ObjectId],
) -> Arc<Ability> {
    spliced.iter().fold(rules, |spell, card| {
        spell_of(state, *card)
            .and_then(|added| spliced_spell(&spell, added))
            .map_or(spell, Arc::new)
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a card of the hand is offered for splicing exactly onto spells of the kind it names, never onto itself and only for a mana cost; and the spliced spell has the card's targets and effects after its own.
    use mtg_cards::CardDefinition;
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::state;

    const ME: PlayerId = PlayerId(0);

    fn spell(
        name: &str,
        kind: CardType,
        arcane: bool,
        splice: Option<(SpliceOnto, Cost)>,
    ) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new(name)
            .with_type(kind)
            .with_rules(serde_json::from_str::<Ability>(
                r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
            ).unwrap());
        if arcane {
            card = card.with_subtype("Arcane");
        }
        if let Some((onto, cost)) = splice {
            card = card.with_rules(Ability::Keyword(Keyword::Splice { onto, cost }));
        }
        Arc::new(card)
    }

    fn mana(cost: &str) -> Cost {
        Cost::mana(cost.parse().unwrap())
    }

    /// The hand: the spell cast (0) and the card that may be spliced (1).
    fn hand(
        cast: Arc<CardDefinition>,
        card: Arc<CardDefinition>,
    ) -> (GameState, ObjectId, ObjectId) {
        let game = state(vec![vec![cast, card], vec![]], vec![vec![], vec![]]);
        let hand = game.zone(Zone::Hand(ME)).to_vec();
        (game, hand[0], hand[1])
    }

    #[rstest]
    #[case::arcane_onto_arcane(CardType::Instant, true, SpliceOnto::Arcane, true)]
    #[case::arcane_onto_a_plain_instant(CardType::Instant, false, SpliceOnto::Arcane, false)]
    #[case::instant_or_sorcery_onto_a_sorcery(
        CardType::Sorcery,
        false,
        SpliceOnto::InstantOrSorcery,
        true
    )]
    #[case::instant_or_sorcery_onto_a_creature(
        CardType::Creature,
        false,
        SpliceOnto::InstantOrSorcery,
        false
    )]
    fn a_card_is_offered_onto_the_kind_it_names(
        #[case] kind: CardType,
        #[case] arcane: bool,
        #[case] onto: SpliceOnto,
        #[case] offered: bool,
    ) {
        let (game, cast, card) = hand(
            spell("Cast", kind, arcane, None),
            spell(
                "Splicer",
                CardType::Instant,
                true,
                Some((onto, mana("{1}{R}"))),
            ),
        );
        let found = spliceable(&game, ME, cast);
        assert_eq!(
            found.iter().map(|s| s.card).collect::<Vec<_>>(),
            if offered { vec![card] } else { Vec::new() }
        );
        if offered {
            assert_eq!(found[0].label, "Splice Splicer {1}{R}");
        }
    }

    #[test]
    fn a_card_is_never_spliced_onto_itself() {
        let (game, _, card) = hand(
            spell("Cast", CardType::Instant, true, None),
            spell(
                "Splicer",
                CardType::Instant,
                true,
                Some((SpliceOnto::Arcane, mana("{1}"))),
            ),
        );
        assert_eq!(spliceable(&game, ME, card), Vec::new());
    }

    #[test]
    fn a_cost_that_is_not_mana_is_not_offered() {
        let life: Cost = serde_json::from_str(r#"[{"PayLife":2}]"#).unwrap();
        let (game, cast, _) = hand(
            spell("Cast", CardType::Instant, true, None),
            spell(
                "Splicer",
                CardType::Instant,
                true,
                Some((SpliceOnto::Arcane, life)),
            ),
        );
        assert_eq!(spliceable(&game, ME, cast), Vec::new());
    }

    #[test]
    fn the_spliced_spell_has_the_cards_targets_after_its_own() {
        let (game, cast, card) = hand(
            spell("Cast", CardType::Instant, true, None),
            spell(
                "Splicer",
                CardType::Instant,
                true,
                Some((SpliceOnto::Arcane, mana("{1}"))),
            ),
        );
        let rules = Arc::clone(spell_of(&game, cast).unwrap());
        let spliced = with_spliced(&game, Arc::clone(&rules), &[card]);
        assert_eq!(spliced.targets().len(), 2);
        assert_eq!(spliced.effects().len(), 2);
        assert_eq!(with_spliced(&game, Arc::clone(&rules), &[]), rules);
    }
}
