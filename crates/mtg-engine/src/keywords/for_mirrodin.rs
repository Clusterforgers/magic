//! For Mirrodin! (CR 702.163): "When this Equipment enters, create a 2/2 red Rebel creature token, then attach this to it."

use mtg_cards::ability::Ability;
use mtg_cards::card::TokenSpec;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;

use super::KeywordRules;
use super::living_weapon::enters_attached_to;

/// The rules of For Mirrodin!.
#[derive(Debug)]
pub(super) struct ForMirrodin;

impl KeywordRules for ForMirrodin {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![enters_attached_to(rebel())]
    }
}

/// The 2/2 red Rebel creature token.
fn rebel() -> TokenSpec {
    TokenSpec {
        name: "Rebel".into(),
        colors: vec![Color::Red],
        supertypes: Vec::new(),
        types: vec![CardType::Creature],
        subtypes: vec![Subtype("Rebel".into())],
        power: Some(2),
        toughness: Some(2),
        abilities: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: For Mirrodin! adds the living weapon trigger with a 2/2 red Rebel creature token in place of the Germ.
    use super::*;

    #[test]
    fn it_creates_a_rebel_and_attaches_the_source_to_it() {
        let written: Ability = serde_json::from_str(
            r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[
                {"CreateToken":{"token":{"Custom":{"name":"Rebel","colors":["Red"],"types":["Creature"],"subtypes":["Rebel"],"power":2,"toughness":2}},"as":"germ"}},
                {"Attach":{"what":"This","to":{"Bound":"germ"}}}]}}"#,
        )
        .unwrap();
        assert_eq!(ForMirrodin.adds_abilities(&Keyword::ForMirrodin), [written]);
    }
}
