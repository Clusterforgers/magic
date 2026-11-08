//! Haunt (CR 702.55): "When this creature dies, exile it haunting target creature." On an instant or sorcery: "When
//! this spell card is put into a graveyard after resolving, exile it haunting target creature."
//!
//! The keyword adds both triggers; a creature card never resolves into a graveyard and a spell card never dies, so only
//! the one that fits its card triggers. Each exiles the card from its graveyard (`Effect::Haunt`), and the link is state
//! of the exiled card (`GameObject::haunting`): a zone change of the card clears it. The card's own "when the creature it
//! haunts dies" ability works from exile and watches `Pred::HauntedBy`.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of haunt.
#[derive(Debug)]
pub(super) struct Haunt;

impl KeywordRules for Haunt {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Haunt) {
            return Vec::new();
        }
        let dies = TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        };
        let resolved = TriggerPattern::ResolvedToGraveyard {
            what: Selector::this(),
        };
        vec![
            exile_haunting(dies, ZoneKind::Battlefield),
            exile_haunting(resolved, ZoneKind::Graveyard),
        ]
    }
}

/// "When <event>, exile it haunting target creature": the card, now in its owner's graveyard, is exiled linked to the
/// target.
fn exile_haunting(when: TriggerPattern, from: ZoneKind) -> Ability {
    Ability::Triggered {
        when,
        intervening_if: None,
        limit: None,
        from,
        targets: vec![TargetSpec {
            what: Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)])),
            bounds: Bounds::default(),
            divide: None,
        }],
        effects: vec![Effect::Haunt {
            what: Ref::Each(Selector(vec![Pred::In(ZoneKind::Graveyard), Pred::This])),
            on: Ref::Target(0),
        }],
    }
}
