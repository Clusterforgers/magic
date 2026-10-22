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

#[cfg(test)]
mod tests {
    //! Goal: embalm and eternalize each add one sorcery-speed ability from a graveyard whose cost is the keyword cost and the exile of the card, making one token copy of the card that is a Zombie of the right colour with no mana cost (and a 4/4 for eternalize); no other keyword adds one.
    use mtg_cards::effect::{Effect, TokenRef};
    use rstest::rstest;

    use super::super::eternalize::Eternalize;
    use super::*;

    fn mana() -> Cost {
        Cost::mana("{2}{W}".parse().unwrap())
    }

    fn zombie(color: Color) -> Vec<Modification> {
        vec![
            Modification::SetColors(vec![color]),
            Modification::AddSubtype(Subtype("Zombie".to_owned())),
        ]
    }

    #[rstest]
    #[case::embalm(Keyword::Embalm(mana()), zombie(Color::White))]
    #[case::eternalize(
        Keyword::Eternalize(mana()),
        [
            zombie(Color::Black),
            vec![Modification::SetPT { power: Amount::Fixed(4), toughness: Amount::Fixed(4) }],
        ]
        .concat()
    )]
    fn the_ability_exiles_the_card_for_the_cost_and_makes_a_zombie_copy(
        #[case] keyword: Keyword,
        #[case] except: Vec<Modification>,
    ) {
        let added = match &keyword {
            Keyword::Embalm(_) => Embalm.adds_abilities(&keyword),
            _ => Eternalize.adds_abilities(&keyword),
        };
        let [
            Ability::Activated {
                cost,
                timing,
                from,
                targets,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one activated ability: {added:?}");
        };
        assert_eq!(
            cost.0,
            [
                CostItem::Mana("{2}{W}".parse().unwrap()),
                CostItem::ExileSelf
            ]
        );
        assert_eq!(*timing, Timing::Sorcery);
        assert_eq!(*from, ZoneKind::Graveyard);
        assert!(targets.is_empty());
        let [
            Effect::CreateToken {
                token:
                    TokenRef::CopyOf {
                        of,
                        except: made,
                        no_mana_cost,
                    },
                count,
                ..
            },
        ] = effects.as_slice()
        else {
            panic!("one token copy: {effects:?}");
        };
        assert_eq!(
            (of, made, *no_mana_cost, count),
            (&Ref::This, &except, true, &Amount::one())
        );
    }

    #[rstest]
    #[case::other_keyword(Keyword::Flying)]
    #[case::unearth(Keyword::Unearth(mana()))]
    #[case::the_other_of_the_pair(Keyword::Eternalize(mana()))]
    fn embalm_adds_nothing_for_another_keyword(#[case] keyword: Keyword) {
        assert!(Embalm.adds_abilities(&keyword).is_empty());
    }

    #[test]
    fn eternalize_adds_nothing_for_embalm() {
        assert!(
            Eternalize
                .adds_abilities(&Keyword::Embalm(mana()))
                .is_empty()
        );
    }
}
