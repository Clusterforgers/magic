//! Embalm (CR 702.128): "[Cost], Exile this card from your graveyard: Create a token that's a copy of it, except it's a white
//! Zombie in addition to its other types and it has no mana cost. Activate only as a sorcery."
//!
//! An activated ability that works from a graveyard. The card is exiled as a cost, so the copy is made from what the
//! card was (last known information, CR 707.2). Eternalize (`eternalize.rs`) differs in the colour and the power and
//! toughness of the copy.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Effect, TokenRef};
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::card::Subtype;
use mtg_core::color::Color;

use super::KeywordRules;

/// The rules of embalm.
#[derive(Debug)]
pub(super) struct Embalm;

impl KeywordRules for Embalm {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Embalm(cost) = keyword else {
            return Vec::new();
        };
        vec![zombie_copy(cost, Vec::new(), Color::White)]
    }
}

/// The sorcery-speed ability from a graveyard that exiles the card for `cost` and makes a token copy of it that is a Zombie
/// of `color` with no mana cost, and the changes in `more` (shared with eternalize).
pub(super) fn zombie_copy(cost: &Cost, more: Vec<Modification>, color: Color) -> Ability {
    let mut paid = cost.0.clone();
    paid.push(CostItem::ExileSelf);
    let mut except = vec![
        Modification::SetColors(vec![color]),
        Modification::AddSubtype(Subtype("Zombie".to_owned())),
    ];
    except.extend(more);
    Ability::Activated {
        cost: Cost(paid),
        timing: Timing::Sorcery,
        condition: None,
        limit: None,
        from: ZoneKind::Graveyard,
        targets: Vec::new(),
        effects: vec![Effect::CreateToken {
            who: Ref::You,
            token: TokenRef::CopyOf {
                of: Ref::This,
                except,
                no_mana_cost: true,
            },
            count: Amount::one(),
            tapped: false,
            attacking: false,
            bind: None,
        }],
    }
}
