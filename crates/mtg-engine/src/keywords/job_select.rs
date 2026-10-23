//! Job select: "When this Equipment enters, create a 1/1 colorless Hero creature token, then attach this to it."

use mtg_cards::ability::Ability;
use mtg_cards::card::TokenSpec;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype};

use super::KeywordRules;
use super::living_weapon::enters_attached_to;

/// The rules of job select.
#[derive(Debug)]
pub(super) struct JobSelect;

impl KeywordRules for JobSelect {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![enters_attached_to(hero())]
    }
}

/// The 1/1 colorless Hero creature token.
fn hero() -> TokenSpec {
    TokenSpec {
        name: "Hero".into(),
        colors: Vec::new(),
        supertypes: Vec::new(),
        types: vec![CardType::Creature],
        subtypes: vec![Subtype("Hero".into())],
        power: Some(1),
        toughness: Some(1),
        abilities: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: job select adds the living weapon trigger with a 1/1 colorless Hero creature token in place of the Germ.
    use super::*;

    #[test]
    fn it_creates_a_hero_and_attaches_the_source_to_it() {
        let written: Ability = serde_json::from_str(
            r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[
                {"CreateToken":{"token":{"Custom":{"name":"Hero","types":["Creature"],"subtypes":["Hero"],"power":1,"toughness":1}},"as":"germ"}},
                {"Attach":{"what":"This","to":{"Bound":"germ"}}}]}}"#,
        )
        .unwrap();
        assert_eq!(JobSelect.adds_abilities(&Keyword::JobSelect), [written]);
    }
}
