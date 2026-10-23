//! Face-down spells and permanents (CR 708): the characteristics the rules give them, how a card comes to lie face down
//! and what turning it face up costs.
//!
//! A face-down object keeps its card (`GameObject::whole_card`); the object reads as the characteristics below
//! (`GameObject::card`), so the layers, the abilities, the copy rules and the views need no other rule for it.

use std::sync::{Arc, LazyLock};

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaSymbol};

use crate::ability_reader::printed_rules;
use crate::object::FaceDown;
use crate::price::Price;
use crate::state::GameState;
use crate::zones::Zone;

/// The mana a card is cast face down for: {3} (CR 702.37c, 702.168a).
pub(crate) fn cast_cost() -> ManaCost {
    ManaCost::from_symbols([ManaSymbol::Generic(3)])
}

/// A 2/2 creature with no name, mana cost, colour or subtype and no abilities (CR 708.2a); `ward` gives it ward {2}.
fn creature(ward_wanted: bool) -> CardDefinition {
    let two = ManaCost::from_symbols([ManaSymbol::Generic(2)]);
    let ward = Ability::Keyword(Keyword::Ward(Cost::mana(mtg_cards::mana::ManaCostX::new(
        0, two,
    ))));
    let mut shown = CardDefinition {
        rules: if ward_wanted {
            vec![Arc::new(ward)]
        } else {
            Vec::new()
        },
        ..CardDefinition::new("")
    };
    shown.types.insert(CardType::Creature);
    shown.power = Some(2);
    shown.toughness = Some(2);
    shown
}

static MORPH: LazyLock<CardDefinition> = LazyLock::new(|| creature(false));
static DISGUISE: LazyLock<CardDefinition> = LazyLock::new(|| creature(true));

impl FaceDown {
    /// The characteristics a spell or permanent that lies face down this way has (CR 708.2a), or `None` for a card that
    /// keeps its own, like a foretold or hidden card in exile.
    pub(crate) fn characteristics(self) -> Option<&'static CardDefinition> {
        match self {
            FaceDown::Foretold { .. } | FaceDown::Hideaway { .. } => None,
            FaceDown::Morph | FaceDown::Manifest => Some(&MORPH),
            FaceDown::Disguise | FaceDown::Cloak => Some(&DISGUISE),
        }
    }

    /// Whether the characteristics of an object that lies face down this way are public: every seat reads a 2/2 creature
    /// and only the card behind it is hidden.
    pub fn shows_characteristics(self) -> bool {
        self.characteristics().is_some()
    }

    /// Whether an effect put the card onto the battlefield face down (manifest, cloak), so it is turned face up for its mana
    /// cost when it is a creature card, and not a spell cast face down.
    pub(crate) fn is_put_face_down_by_an_effect(self) -> bool {
        matches!(self, FaceDown::Manifest | FaceDown::Cloak)
    }

    /// How the card with this definition lies when it is cast face down: with ward {2} for a card with disguise.
    pub(crate) fn cast_as(card: &CardDefinition) -> FaceDown {
        let disguise = printed_rules(card)
            .any(|rules| matches!(&**rules, Ability::Keyword(Keyword::Disguise(_))));
        if disguise {
            FaceDown::Disguise
        } else {
            FaceDown::Morph
        }
    }
}

/// What turning the face-down permanent `id` face up takes: its morph, megamorph or disguise cost, paid with mana alone,
/// and whether it is a megamorph, whose permanent gets a +1/+1 counter (CR 702.37b). A manifested or cloaked creature card
/// can also be turned face up for its mana cost, an {X} in it counting as 0 (CR 701.40a); a card with both is turned up
/// for the cheaper. `None` for a permanent that is not face down or has no such cost.
pub(crate) fn turn_up_cost(state: &GameState, id: ObjectId) -> Option<(ManaCost, bool)> {
    let object = state.object(id)?;
    let how = object.face_down()?;
    if object.zone() != Zone::Battlefield || !how.shows_characteristics() {
        return None;
    }
    let card = object.whole_card();
    let printed = keyword_cost(card);
    let natural = how
        .is_put_face_down_by_an_effect()
        .then(|| card.types.contains(&CardType::Creature))
        .filter(|creature| *creature)
        .and_then(|_| card.mana_cost.clone())
        .map(|cost| (cost, false));
    match (printed, natural) {
        (Some(printed), Some(natural)) => {
            Some(if printed.0.mana_value() <= natural.0.mana_value() {
                printed
            } else {
                natural
            })
        }
        (printed, natural) => printed.or(natural),
    }
}

/// The morph, megamorph or disguise cost of `card` if it is mana alone, and whether it is a megamorph's.
fn keyword_cost(card: &CardDefinition) -> Option<(ManaCost, bool)> {
    printed_rules(card).find_map(|rules| {
        let (cost, megamorph) = match &**rules {
            Ability::Keyword(Keyword::Morph(cost) | Keyword::Disguise(cost)) => (cost, false),
            Ability::Keyword(Keyword::Megamorph(cost)) => (cost, true),
            _ => return None,
        };
        let price = Price::of(cost).ok()?;
        if price.tap || price.moves_source() || price.x > 0 || !price.parts.is_empty() {
            return None;
        }
        Some((price.mana?, megamorph))
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a face-down object (morph, disguise, manifest, cloak) reads as the 2/2 creature of CR 708.2a (no name, cost, colour, other type or ability, ward {2} for a disguise or a cloak) through the layers, keeps its card for turning it face up, is copied as that 2/2 and not as its card, is looked at by its controller alone, and costs its morph, megamorph or disguise cost in mana to turn face up, or for a manifested or cloaked creature card its mana cost.
    use mtg_cards::definition::CardDefinition;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::characteristics::characteristics;
    use crate::copies::copiable_values;
    use crate::fixtures::state;
    use crate::keywords::keywords_of;
    use crate::object::GameObject;

    fn mana(text: &str) -> Cost {
        Cost::mana(text.parse().unwrap())
    }

    /// A red 4/4 flyer with the given keyword for turning it face up, on seat 0's battlefield face down this way.
    fn lies(kind: FaceDown, keyword: Keyword) -> (GameState, ObjectId) {
        let mut def = CardDefinition::new("Sky Whale")
            .with_type(CardType::Creature)
            .with_type(CardType::Artifact)
            .with_power_toughness(4, 4);
        def.mana_cost = Some(ManaCost::parse("{3}{R}{R}").unwrap());
        let def = CardDefinition {
            rules: vec![
                Arc::new(Ability::Keyword(Keyword::Flying)),
                Arc::new(Ability::Keyword(keyword)),
            ],
            ..def
        };
        let mut game = state(vec![vec![], vec![]], vec![vec![Arc::new(def)], vec![]]);
        let id = game
            .objects()
            .find(|object| object.whole_card().name == "Sky Whale")
            .map(GameObject::id)
            .unwrap();
        game.objects.get_mut(&id).unwrap().face_down = Some(kind);
        game.layers.reset();
        (game, id)
    }

    #[rstest]
    #[case::morph(FaceDown::Morph, false)]
    #[case::disguise(FaceDown::Disguise, true)]
    #[case::manifest(FaceDown::Manifest, false)]
    #[case::cloak(FaceDown::Cloak, true)]
    fn a_face_down_creature_is_a_nameless_colourless_2_2_with_no_abilities_but_a_disguises_ward(
        #[case] kind: FaceDown,
        #[case] ward: bool,
    ) {
        let (game, id) = lies(kind, Keyword::Morph(mana("{2}{R}")));
        let chars = characteristics(&game, id).unwrap();
        assert_eq!(game.object(id).unwrap().card().name, "");
        assert!(chars.types.contains(CardType::Creature));
        assert!(
            !chars.types.contains(CardType::Artifact),
            "the card is an artifact creature"
        );
        assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
        assert!(chars.colors.is_empty() && chars.subtypes.is_empty());
        assert!(game.object(id).unwrap().card().mana_cost.is_none());
        let keywords = keywords_of(&game, id);
        assert_eq!(keywords.len(), usize::from(ward));
        assert!(
            keywords
                .iter()
                .all(|keyword| matches!(keyword, Keyword::Ward(_)))
        );
        // The card is still there, for the player who turns it face up.
        assert_eq!(game.object(id).unwrap().whole_card().name, "Sky Whale");
    }

    #[test]
    fn a_copy_of_a_face_down_creature_copies_the_2_2_and_not_the_card() {
        let (game, id) = lies(FaceDown::Morph, Keyword::Morph(mana("{2}{R}")));
        let reference = game.object(id).unwrap().reference();
        let values = copiable_values(&game, reference, None).unwrap();
        assert_eq!(values.name, "");
        assert_eq!((values.power, values.toughness), (Some(2), Some(2)));
        assert_eq!(printed_rules(&values).count(), 0);
    }

    #[test]
    fn a_face_down_creature_that_has_left_is_copied_as_the_2_2_it_was() {
        let (mut game, id) = lies(FaceDown::Disguise, Keyword::Disguise(mana("{R}")));
        let shot = crate::snapshot::snapshot(&game, id).unwrap();
        let reference = shot.reference();
        let died = crate::events::Event::ZoneChanged {
            object: id,
            from: Zone::Battlefield,
            to: Zone::Graveyard(PlayerId::from_index(0)),
            cause: crate::events::ZoneCause::Destroyed,
        };
        crate::fold::apply_event(&mut game, &died).unwrap();
        assert_eq!(
            game.object(id).unwrap().face_down(),
            None,
            "its card is shown"
        );
        let values = copiable_values(&game, reference, Some(&shot)).unwrap();
        assert_eq!(values.name, "");
        assert_eq!((values.power, values.toughness), (Some(2), Some(2)));
    }

    #[rstest]
    #[case::the_controller(0, true)]
    #[case::an_opponent(1, false)]
    #[case::a_spectator(7, false)]
    fn only_the_controller_looks_at_a_face_down_creature(
        #[case] viewer: usize,
        #[case] looks: bool,
    ) {
        let (game, id) = lies(FaceDown::Morph, Keyword::Morph(mana("{2}{R}")));
        assert_eq!(game.may_look_at(PlayerId::from_index(viewer), id), looks);
    }

    #[rstest]
    #[case::morph(Keyword::Morph(mana("{2}{R}")), Some(("{2}{R}", false)))]
    #[case::megamorph(Keyword::Megamorph(mana("{1}{R}")), Some(("{1}{R}", true)))]
    #[case::disguise(Keyword::Disguise(mana("{R}")), Some(("{R}", false)))]
    #[case::a_morph_cost_with_an_x(Keyword::Morph(mana("{X}{R}")), None)]
    #[case::no_such_keyword(Keyword::Flying, None)]
    fn turning_a_card_face_up_costs_its_keyword_cost_in_mana(
        #[case] keyword: Keyword,
        #[case] expected: Option<(&str, bool)>,
    ) {
        let (game, id) = lies(FaceDown::Morph, keyword);
        let cost = turn_up_cost(&game, id);
        let expected: Option<(ManaCost, bool)> =
            expected.map(|(cost, megamorph)| (ManaCost::parse(cost).unwrap(), megamorph));
        assert_eq!(cost, expected);
    }

    #[rstest]
    #[case::its_mana_cost(FaceDown::Manifest, Keyword::Flying, Some(("{3}{R}{R}", false)))]
    #[case::cloaked_for_its_mana_cost(FaceDown::Cloak, Keyword::Flying, Some(("{3}{R}{R}", false)))]
    #[case::a_cheaper_morph_cost(FaceDown::Manifest, Keyword::Morph(mana("{2}{R}")), Some(("{2}{R}", false)))]
    #[case::a_dearer_morph_cost(FaceDown::Manifest, Keyword::Morph(mana("{4}{R}{R}")), Some(("{3}{R}{R}", false)))]
    #[case::a_megamorph_cost(FaceDown::Cloak, Keyword::Megamorph(mana("{1}{R}")), Some(("{1}{R}", true)))]
    #[case::a_disguise_cost(FaceDown::Cloak, Keyword::Disguise(mana("{R}")), Some(("{R}", false)))]
    #[case::a_morph_cost_with_an_x_leaves_the_mana_cost(FaceDown::Manifest, Keyword::Morph(mana("{X}{R}")), Some(("{3}{R}{R}", false)))]
    #[case::a_spell_cast_face_down_has_no_mana_cost_to_pay(FaceDown::Morph, Keyword::Flying, None)]
    #[case::a_disguise_cast_face_down_has_no_mana_cost_to_pay(
        FaceDown::Disguise,
        Keyword::Flying,
        None
    )]
    fn a_manifested_or_cloaked_creature_card_is_turned_face_up_for_its_mana_cost_or_its_keyword_cost(
        #[case] kind: FaceDown,
        #[case] keyword: Keyword,
        #[case] expected: Option<(&str, bool)>,
    ) {
        let (game, id) = lies(kind, keyword);
        let expected: Option<(ManaCost, bool)> =
            expected.map(|(cost, megamorph)| (ManaCost::parse(cost).unwrap(), megamorph));
        assert_eq!(turn_up_cost(&game, id), expected);
    }

    #[rstest]
    #[case::an_artifact_that_is_no_creature(CardType::Artifact)]
    #[case::a_land(CardType::Land)]
    #[case::a_sorcery(CardType::Sorcery)]
    fn a_manifested_card_that_is_no_creature_card_is_never_turned_face_up(
        #[case] kind: CardType,
        #[values(FaceDown::Manifest, FaceDown::Cloak)] how: FaceDown,
    ) {
        let mut def = CardDefinition::new("Plain Thing").with_type(kind);
        def.mana_cost = Some(ManaCost::parse("{2}").unwrap());
        let mut game = state(vec![vec![], vec![]], vec![vec![Arc::new(def)], vec![]]);
        let id = game.zone(Zone::Battlefield)[0];
        game.objects.get_mut(&id).unwrap().face_down = Some(how);
        assert_eq!(turn_up_cost(&game, id), None);
    }

    #[test]
    fn a_face_up_permanent_and_a_foretold_card_are_not_turned_face_up() {
        let (mut game, id) = lies(FaceDown::Morph, Keyword::Morph(mana("{1}")));
        game.objects.get_mut(&id).unwrap().face_down = Some(FaceDown::Foretold { turn: 1 });
        assert_eq!(turn_up_cost(&game, id), None);
        game.objects.get_mut(&id).unwrap().face_down = None;
        assert_eq!(turn_up_cost(&game, id), None);
        assert_eq!(characteristics(&game, id).unwrap().colors.iter().count(), 1);
    }
}
