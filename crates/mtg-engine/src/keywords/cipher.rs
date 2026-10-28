//! Cipher (CR 702.99): "Then you may exile this spell card encoded on a creature you control. Whenever that creature
//! deals combat damage to a player, its controller may cast a copy of the encoded card without paying its mana cost."
//!
//! The spell ability gains a last step (`encoded`): its controller chooses up to one creature they control and the card
//! is exiled encoded on it (`Effect::Encode`), unless the spell is a copy, which is no card. The keyword adds the
//! trigger that works from exile: it watches the creature the card is encoded on (`Pred::EncodedWith`) and is
//! controlled by that creature's controller (`triggers::detect`), who copies the card (`Effect::CopyCard`) and may cast
//! the copy for nothing. The link is state of the exiled card (`GameObject::encoded_on`): a zone change of the card
//! clears it, and a creature that has left the battlefield is a new object that no card is encoded on.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;

use super::KeywordRules;
use crate::ability_reader;
use crate::object::{GameObject, ObjectKind};

/// The name the creature chosen to carry the card is bound to.
const CARRIER: &str = "encoded on";
/// The name the copy of the encoded card is bound to.
const COPY: &str = "copy of the encoded card";

/// The rules of cipher.
#[derive(Debug)]
pub(super) struct Cipher;

impl KeywordRules for Cipher {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Cipher) {
            return Vec::new();
        }
        vec![Ability::Triggered {
            when: TriggerPattern::DealsDamage {
                source: Selector(vec![Pred::EncodedWith(Ref::This)]),
                to: Some(Ref::EachPlayer),
                combat: Some(true),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Exile,
            targets: Vec::new(),
            effects: vec![
                Effect::CopyCard {
                    who: Ref::You,
                    what: Ref::This,
                    bind: COPY.to_owned(),
                },
                Effect::CastFromZone {
                    who: Ref::You,
                    what: Ref::Bound(COPY.to_owned()),
                    free: true,
                    play: false,
                    way: None,
                    bind: None,
                    otherwise: Vec::new(),
                },
            ],
        }]
    }
}

/// The rules of the spell `object` runs, with the encoding last when its card has cipher: "Then you may exile this
/// spell card encoded on a creature you control" (CR 702.99a). A copy of the spell is no card and is not encoded; other
/// rules come back as they are.
pub(crate) fn encoded(object: &GameObject, rules: Arc<Ability>) -> Arc<Ability> {
    if object.kind() != ObjectKind::Card
        || !ability_reader::printed_rules(object.card())
            .any(|rule| matches!(**rule, Ability::Keyword(Keyword::Cipher)))
    {
        return rules;
    }
    let Ability::Spell { targets, effects } = &*rules else {
        return rules;
    };
    let mut effects = effects.clone();
    effects.extend([
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::You),
            ])),
            count: Bounds::up_to(Some(Amount::one())),
            bind: CARRIER.to_owned(),
            rest_as: None,
        },
        Effect::Encode {
            what: Ref::This,
            on: Ref::Bound(CARRIER.to_owned()),
        },
    ]);
    Arc::new(Ability::Spell {
        targets: targets.clone(),
        effects,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: cipher adds one trigger that works from exile and watches the combat damage of the creature the card is encoded on, copies the card and casts the copy for nothing; a card with the keyword ends its spell ability with the choice of a creature and the encoding, a copy of the spell and a card without the keyword keep theirs, and no other keyword adds a trigger.
    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::state;
    use crate::resolve::spell_ability;
    use crate::zones::Zone;

    fn spell(keyword: bool) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new("Cipher Spell").with_type(CardType::Sorcery);
        card = card.with_rules(Ability::Spell {
            targets: Vec::new(),
            effects: vec![Effect::Draw {
                who: Ref::You,
                count: Amount::one(),
            }],
        });
        if keyword {
            card = card.with_rules(Ability::Keyword(Keyword::Cipher));
        }
        Arc::new(card)
    }

    fn effects_of(rules: &Ability) -> &[Effect] {
        match rules {
            Ability::Spell { effects, .. } => effects,
            other => panic!("not a spell ability: {other:?}"),
        }
    }

    #[test]
    fn cipher_adds_the_trigger_of_the_encoded_creature_that_works_from_exile() {
        let added = Cipher.adds_abilities(&Keyword::Cipher);
        let [
            Ability::Triggered {
                when,
                from,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one trigger, got {added:?}");
        };
        assert_eq!(*from, ZoneKind::Exile);
        assert!(matches!(
            when,
            TriggerPattern::DealsDamage {
                combat: Some(true),
                to: Some(Ref::EachPlayer),
                ..
            }
        ));
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::CopyCard { .. },
                Effect::CastFromZone { free: true, .. }
            ]
        ));
        assert_eq!(Cipher.adds_abilities(&Keyword::Demonstrate), []);
    }

    #[rstest]
    #[case::with_the_keyword(true, 3)]
    #[case::without_it(false, 1)]
    fn a_card_with_cipher_ends_its_spell_ability_with_the_encoding(
        #[case] keyword: bool,
        #[case] steps: usize,
    ) {
        let game = state(vec![vec![spell(keyword)], vec![]], vec![vec![], vec![]]);
        let id = game.zone(Zone::Hand(PlayerId(0)))[0];
        let card = game.object(id).unwrap();
        let (_, rules) = spell_ability(&game, id).unwrap();
        let ran = encoded(card, rules);
        assert_eq!(effects_of(&ran).len(), steps);
        assert_eq!(
            matches!(
                effects_of(&ran),
                [
                    Effect::Draw { .. },
                    Effect::Choose { .. },
                    Effect::Encode { .. }
                ]
            ),
            keyword
        );
    }
}
