//! Goal: a Saga chapter line ("I — ...", "II, III — ...") reads into one chapter ability for each number it names, with the effects its text reads as, writes back as the line it came from, and every near-miss stays with the table.
use super::*;

use crate::compile::read::chapter_block;

fn chapter_of(number: u32, effects: &str) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"Chapter":{{"number":{number}}}}},"effects":{effects}}}}}"#
    ))
}

fn block(text: &str) -> Option<Vec<Ability>> {
    chapter_block(text, false, &Own::default())
}

#[rstest]
#[case::one("I \u{2014} Draw a card.", &[1], r#"[{"Draw":{}}]"#)]
#[case::the_third("III \u{2014} Draw a card.", &[3], r#"[{"Draw":{}}]"#)]
#[case::two_numbers("I, II \u{2014} Draw a card.", &[1, 2], r#"[{"Draw":{}}]"#)]
#[case::three_numbers("I, II, III \u{2014} Draw a card.", &[1, 2, 3], r#"[{"Draw":{}}]"#)]
#[case::apart("II, IV \u{2014} Draw a card.", &[2, 4], r#"[{"Draw":{}}]"#)]
#[case::the_last_of_ten("X \u{2014} Draw a card.", &[10], r#"[{"Draw":{}}]"#)]
fn a_chapter_line_is_a_chapter_ability_for_each_number(
    #[case] text: &str,
    #[case] numbers: &[u32],
    #[case] effects: &str,
) {
    let expected: Vec<Ability> = numbers
        .iter()
        .map(|number| chapter_of(*number, effects))
        .collect();
    assert_eq!(block(text), Some(expected), "{text}");
}

#[rstest]
#[case::draw("I \u{2014} Draw a card.")]
#[case::two_sentences("II \u{2014} Draw a card. Each opponent loses 1 life.")]
#[case::life("I, II \u{2014} You gain 2 life.")]
#[case::a_target("III \u{2014} Destroy target creature.")]
fn a_chapter_ability_writes_back_as_its_line(#[case] text: &str) {
    let Some(abilities) = block(text) else {
        panic!("not read: {text}");
    };
    let (numbers, body) = crate::compile::classify::chapter(text).unwrap_or_default();
    assert_eq!(abilities.len(), numbers.len(), "{text}");
    for (ability, number) in abilities.iter().zip(numbers) {
        let numeral = crate::compile::classify::numeral(number).unwrap_or_default();
        let wanted = format!("{numeral} \u{2014} {body}");
        let written = render_variants(ability);
        assert!(
            written.iter().any(|found| same_text(found, &wanted)),
            "{wanted}: {written:?}"
        );
    }
}

#[test]
fn a_chapter_of_choices_reads_its_modes() {
    let Some(abilities) = block("III \u{2014} Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.")
    else {
        panic!("not read");
    };
    assert!(matches!(
        abilities.as_slice(),
        [Ability::Triggered { effects, .. }] if matches!(effects.as_slice(), [crate::effect::Effect::Modal { .. }])
    ));
}

#[rstest]
#[case::no_dash("I Draw a card.")]
#[case::not_a_numeral("IIII \u{2014} Draw a card.")]
#[case::out_of_order("II, I \u{2014} Draw a card.")]
#[case::twice("I, I \u{2014} Draw a card.")]
#[case::no_text("I \u{2014} ")]
#[case::an_unread_sentence("I \u{2014} Frobnicate the widgets.")]
#[case::half_read("II \u{2014} Draw a card. Frobnicate the widgets.")]
#[case::a_trigger("I \u{2014} Whenever ~ attacks, draw a card.")]
fn a_line_the_reader_does_not_know_stays_with_the_table(#[case] text: &str) {
    assert_eq!(block(text), None, "{text}");
}
