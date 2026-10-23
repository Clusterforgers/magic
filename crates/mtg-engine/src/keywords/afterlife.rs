//! Afterlife (CR 702.135): "When this permanent is put into a graveyard from the battlefield, create N 1/1 white and black
//! Spirit creature tokens with flying."
//!
//! The keyword adds a dies trigger of the source whose effect creates the tokens for its controller.

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

/// The rules of afterlife.
#[derive(Debug)]
pub(super) struct Afterlife;

impl KeywordRules for Afterlife {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Afterlife(count) = keyword else {
            return Vec::new();
        };
        let count = i32::try_from(*count).unwrap_or(i32::MAX);
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::CreateToken {
                who: Ref::You,
                token: TokenRef::Custom(Box::new(spirit())),
                count: Amount::Fixed(count),
                tapped: false,
                attacking: false,
                bind: None,
            }],
        }]
    }
}

/// The 1/1 white and black Spirit creature token with flying.
fn spirit() -> TokenSpec {
    TokenSpec {
        name: "Spirit".into(),
        colors: vec![Color::White, Color::Black],
        supertypes: Vec::new(),
        types: vec![CardType::Creature],
        subtypes: vec![Subtype("Spirit".into())],
        power: Some(1),
        toughness: Some(1),
        abilities: vec![Ability::Keyword(Keyword::Flying)],
    }
}

#[cfg(test)]
mod tests {
    //! Goal: afterlife adds one dies trigger of the source that creates its number of white and black Spirit tokens with flying, and no other keyword adds it.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::one(1)]
    #[case::two(2)]
    #[case::three(3)]
    fn afterlife_adds_a_dies_trigger_with_its_number_of_spirits(#[case] number: u32) {
        let written: Ability = serde_json::from_str(&format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[
                {{"CreateToken":{{"token":{{"Custom":{{"name":"Spirit","colors":["White","Black"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1,"abilities":[{{"Keyword":"Flying"}}]}}}},"count":{number}}}}}]}}}}"#
        ))
        .unwrap();
        assert_eq!(
            Afterlife.adds_abilities(&Keyword::Afterlife(number)),
            [written]
        );
    }

    #[rstest]
    #[case::fabricate(Keyword::Fabricate(1))]
    #[case::persist(Keyword::Persist)]
    #[case::flying(Keyword::Flying)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Afterlife.adds_abilities(&keyword), []);
    }
}
