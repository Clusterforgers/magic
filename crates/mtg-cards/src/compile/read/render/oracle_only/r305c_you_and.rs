//! Wordings of a static ability that gives a keyword to players and to permanents together: "You and permanents you
//! control have protection from Salamanders.", written from the twin that names the permanents alone.

use super::super::super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Ref};

/// Adds "You and <permanents> have <keyword>." for a static ability over `You` and one selector.
pub(super) fn r305c_add_you_and(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected: Ref::Union(parts),
        mods,
        condition: None,
        defining: false,
        from,
    } = ability
    else {
        return;
    };
    let [Ref::You, Ref::Each(selector)] = parts.as_slice() else {
        return;
    };
    // "protection from the chosen card type": the card's own replacement chooses a card type as it enters.
    if let [Modification::AddKeyword(Keyword::Protection(quality))] = mods.as_slice()
        && quality.0 == [Pred::ChosenSubtype]
        && let Some(objects) = phrase_text(selector, Form::All)
    {
        texts.push(format!(
            "You and {objects} have protection from the chosen card type."
        ));
    }
    let twin = Ability::Static {
        affected: Ref::Each(selector.clone()),
        mods: mods.clone(),
        condition: None,
        defining: false,
        from: *from,
    };
    for text in super::super::variants(&twin) {
        let mut letters = text.chars();
        let Some(first) = letters.next() else {
            continue;
        };
        if !text.contains(" have ") {
            continue;
        }
        texts.push(format!(
            "You and {}{}",
            first.to_lowercase(),
            letters.as_str()
        ));
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the union of you and permanents reads as printed; another union or a lone player is not written this way.
    use super::*;
    use rstest::rstest;

    fn written(json: &str) -> Vec<String> {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let mut texts = Vec::new();
        r305c_add_you_and(&ability, &mut texts);
        texts
    }

    #[rstest]
    #[case::salamanders(r#"{"Static":{"affected":{"Union":["You",{"Each":[{"ControlledBy":"You"}]}]},"mods":[{"AddKeyword":{"Protection":[{"Subtype":"Salamander"}]}}]}}"#, "You and permanents you control have protection from Salamanders.", true)]
    #[case::creatures_hexproof(r#"{"Static":{"affected":{"Union":["You",{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}]},"mods":[{"AddKeyword":"Hexproof"}]}}"#, "You and creatures you control have hexproof.", true)]
    #[case::chosen_card_type(r#"{"Static":{"affected":{"Union":["You",{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}]},"mods":[{"AddKeyword":{"Protection":["ChosenSubtype"]}}]}}"#, "You and creatures you control have protection from the chosen card type.", true)]
    #[case::near_miss_chosen_color(r#"{"Static":{"affected":{"Union":["You",{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}]},"mods":[{"AddKeyword":{"Protection":["ChosenColor"]}}]}}"#, "You and creatures you control have protection from the chosen card type.", false)]
    #[case::near_miss_opponents(r#"{"Static":{"affected":{"Union":["Opponents",{"Each":[{"ControlledBy":"You"}]}]},"mods":[{"AddKeyword":"Hexproof"}]}}"#, "You and permanents you control have hexproof.", false)]
    #[case::near_miss_no_union(
        r#"{"Static":{"affected":"You","mods":[{"AddKeyword":"Hexproof"}]}}"#,
        "You and permanents you control have hexproof.",
        false
    )]
    fn r305c_you_and_reads_back(#[case] json: &str, #[case] text: &str, #[case] expected: bool) {
        let texts = written(json);
        assert_eq!(texts.iter().any(|t| t == text), expected, "{texts:#?}");
    }
}
