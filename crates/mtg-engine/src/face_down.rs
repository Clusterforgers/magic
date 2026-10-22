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

use crate::object::FaceDown;
use crate::price::Price;
use crate::state::GameState;
use crate::zones::Zone;

/// The mana a card is cast face down for: {3} (CR 702.37c, 702.168a).
pub(crate) fn cast_cost() -> ManaCost {
    ManaCost::from_symbols([ManaSymbol::Generic(3)])
}

/// A 2/2 creature with no name, mana cost, colour or subtype and no abilities (CR 708.2a); `ward` gives it ward {2}.
fn creature(ward: bool) -> CardDefinition {
    let mut shown = CardDefinition::new("");
    shown.types.insert(CardType::Creature);
    shown.power = Some(2);
    shown.toughness = Some(2);
    if ward {
        let two = ManaCost::from_symbols([ManaSymbol::Generic(2)]);
        shown
            .rules
            .push(Arc::new(Ability::Keyword(Keyword::Ward(Cost::mana(
                mtg_cards::mana::ManaCostX::new(0, two),
            )))));
    }
    shown
}

static MORPH: LazyLock<CardDefinition> = LazyLock::new(|| creature(false));
static DISGUISE: LazyLock<CardDefinition> = LazyLock::new(|| creature(true));

impl FaceDown {
    /// The characteristics a spell or permanent that lies face down this way has (CR 708.2a), or `None` for a card that
    /// keeps its own, like a foretold card in exile.
    pub(crate) fn characteristics(self) -> Option<&'static CardDefinition> {
        match self {
            FaceDown::Foretold { .. } => None,
            FaceDown::Morph => Some(&MORPH),
            FaceDown::Disguise => Some(&DISGUISE),
        }
    }

    /// Whether the characteristics of an object that lies face down this way are public: every seat reads a 2/2 creature
    /// and only the card behind it is hidden.
    pub fn shows_characteristics(self) -> bool {
        self.characteristics().is_some()
    }

    /// How the card with this definition lies when it is cast face down: with ward {2} for a card with disguise.
    pub(crate) fn cast_as(card: &CardDefinition) -> FaceDown {
        let disguise = card
            .rules
            .iter()
            .any(|rules| matches!(&**rules, Ability::Keyword(Keyword::Disguise(_))));
        if disguise {
            FaceDown::Disguise
        } else {
            FaceDown::Morph
        }
    }
}

/// What turning the face-down permanent `id` face up takes: its morph, megamorph or disguise cost, paid with mana alone,
/// and whether it is a megamorph, whose permanent gets a +1/+1 counter (CR 702.37b). `None` for a permanent that is not
/// face down or has no such cost.
pub(crate) fn turn_up_cost(state: &GameState, id: ObjectId) -> Option<(ManaCost, bool)> {
    let object = state.object(id)?;
    if object.zone() != Zone::Battlefield || !object.face_down()?.shows_characteristics() {
        return None;
    }
    object.whole_card().rules.iter().find_map(|rules| {
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
