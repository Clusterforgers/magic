//! "As this creature enters, it becomes your choice of a 3/3 creature, a 2/2 creature with flying, or a 1/6 creature
//! with defender": the wording of a choice of forms (`ReplaceBy::BecomesChoice`) and the phrase of one form.

use mtg_core::card::CardType;

use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

use super::selectors::keyword_text;

/// The phrase of one form ("a 1/6 Wall artifact creature with defender"): a fixed power and toughness, then the
/// subtypes, the card types and the keywords it adds, in the order the data lists them.
pub fn form_text(form: &[Modification]) -> Option<String> {
    let mut size = None;
    let (mut subtypes, mut types, mut keywords) = (Vec::new(), Vec::new(), Vec::new());
    for change in form {
        match change {
            Modification::SetPT {
                power: Amount::Fixed(power),
                toughness: Amount::Fixed(toughness),
            } if size.is_none() => size = Some((*power, *toughness)),
            Modification::AddSubtype(subtype) => subtypes.push(subtype.0.clone()),
            Modification::AddType(kind) => types.push(type_word(*kind)?),
            Modification::AddKeyword(keyword) => keywords.push(keyword_text(keyword)?),
            _ => return None,
        }
    }
    let (power, toughness) = size?;
    if types.is_empty() {
        return None;
    }
    let article = if matches!(power, 8 | 11 | 18 | 80..=89) {
        "an"
    } else {
        "a"
    };
    let mut words = vec![format!("{article} {power}/{toughness}")];
    words.extend(subtypes);
    words.extend(types.into_iter().map(str::to_owned));
    let mut text = words.join(" ");
    if !keywords.is_empty() {
        text.push_str(" with ");
        text.push_str(&listed(&keywords, "and"));
    }
    Some(text)
}

/// The word of a card type a form adds.
fn type_word(kind: CardType) -> Option<&'static str> {
    Some(match kind {
        CardType::Artifact => "artifact",
        CardType::Creature => "creature",
        CardType::Enchantment => "enchantment",
        CardType::Land => "land",
        _ => return None,
    })
}

/// "A, B, or C": a list joined by commas and the last word.
fn listed(items: &[String], last: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [one, two] => format!("{one} {last} {two}"),
        [rest @ .., final_item] => format!("{}, {last} {final_item}", rest.join(", ")),
    }
}

/// Adds "As ~ enters, it becomes your choice of A, B, or C." for a choice of forms of the card itself as it enters,
/// with and without "in addition to its other types" (the forms only add types either way).
pub(super) fn r303_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when: ReplaceWhen::Enters { what },
        by: ReplaceBy::BecomesChoice(forms),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    if *what != Selector::this() {
        return;
    }
    let Some(phrases) = forms
        .iter()
        .map(|form| form_text(form))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let choice = listed(&phrases, "or");
    texts.push(format!("As ~ enters, it becomes your choice of {choice}."));
    texts.push(format!(
        "As ~ enters, it becomes your choice of {choice} in addition to its other types."
    ));
}

#[cfg(test)]
mod tests {
    //! Goal: a form is worded from its changes alone, and a change no copy effect makes has no wording.
    use super::form_text;
    use crate::modify::Modification;
    use rstest::rstest;

    #[rstest]
    #[case::plain(
        r#"[{"SetPT":{"power":3,"toughness":3}},{"AddType":"Creature"}]"#,
        Some("a 3/3 creature")
    )]
    #[case::wall(
        r#"[{"SetPT":{"power":1,"toughness":6}},{"AddSubtype":"Wall"},{"AddType":"Artifact"},{"AddType":"Creature"},{"AddKeyword":"Defender"}]"#,
        Some("a 1/6 Wall artifact creature with defender")
    )]
    #[case::eight(
        r#"[{"SetPT":{"power":8,"toughness":8}},{"AddType":"Creature"}]"#,
        Some("an 8/8 creature")
    )]
    #[case::no_size(r#"[{"AddType":"Creature"},{"AddKeyword":"Flying"}]"#, None)]
    #[case::no_type(
        r#"[{"SetPT":{"power":2,"toughness":2}},{"AddKeyword":"Flying"}]"#,
        None
    )]
    #[case::pump(
        r#"[{"ModifyPT":{"power":2,"toughness":2}},{"AddType":"Creature"}]"#,
        None
    )]
    fn a_form_reads_as_its_changes(#[case] form: &str, #[case] expected: Option<&str>) {
        let form: Vec<Modification> = serde_json::from_str(form).unwrap();
        assert_eq!(form_text(&form).as_deref(), expected);
    }
}
