//! Living weapon (CR 702.92): "When this permanent enters, create a 0/0 black Phyrexian Germ creature token, then attach
//! this permanent to it."

use mtg_cards::ability::Ability;
use mtg_cards::card::TokenSpec;
use mtg_cards::effect::{Effect, TokenRef};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;

use super::KeywordRules;

/// The rules of living weapon.
#[derive(Debug)]
pub(super) struct LivingWeapon;

impl KeywordRules for LivingWeapon {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![
                Effect::CreateToken {
                    who: Ref::You,
                    token: TokenRef::Custom(Box::new(germ())),
                    count: Amount::Fixed(1),
                    tapped: false,
                    attacking: false,
                    bind: Some("germ".into()),
                },
                Effect::Attach {
                    what: Ref::This,
                    to: Ref::Bound("germ".into()),
                },
            ],
        }]
    }
}

/// The 0/0 black Phyrexian Germ creature token.
fn germ() -> TokenSpec {
    TokenSpec {
        name: "Phyrexian Germ".into(),
        colors: vec![Color::Black],
        supertypes: Vec::new(),
        types: vec![CardType::Creature],
        subtypes: vec![Subtype("Phyrexian".into()), Subtype("Germ".into())],
        power: Some(0),
        toughness: Some(0),
        abilities: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: living weapon adds one triggered ability that waits for its source to enter the battlefield, creates a 0/0 black Phyrexian Germ creature token that it names and attaches its source to that token.
    use super::*;

    #[test]
    fn it_creates_a_germ_and_attaches_the_source_to_it() {
        let written: Ability = serde_json::from_str(
            r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[
                {"CreateToken":{"token":{"Custom":{"name":"Phyrexian Germ","colors":["Black"],"types":["Creature"],"subtypes":["Phyrexian","Germ"],"power":0,"toughness":0}},"as":"germ"}},
                {"Attach":{"what":"This","to":{"Bound":"germ"}}}]}}"#,
        )
        .unwrap();
        assert_eq!(
            LivingWeapon.adds_abilities(&Keyword::LivingWeapon),
            [written]
        );
    }
}
