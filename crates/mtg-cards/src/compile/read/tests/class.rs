//! Goal: a level of a Class ("{3}{U}: Level 2" and the lines under it) reads into the level up ability and one ability for each line, every one working from that level on; "becomes level N" reads and writes back as the card words it, and every near-miss stays with the table.
use super::*;

use crate::compile::clauses;
use crate::compile::read::class_block;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;
use crate::value::{Cmp, Condition};

fn block(text: &str) -> Option<Vec<Ability>> {
    class_block(text, false, &Own::default())
}

/// The condition that the Class is at least this level, as data.
fn from_level(level: u32) -> String {
    format!(r#"{{"Level":{{"of":"This","op":"Ge","level":{level}}}}}"#)
}

#[rstest]
#[case::the_second("{2}{U}: Level 2", 2, "{2}{U}")]
#[case::the_third("{5}{U}: Level 3", 3, "{5}{U}")]
#[case::hybrid("{B/G}: Level 2", 2, "{B/G}")]
#[case::two_colors("{3}{R}{G}: Level 3", 3, "{3}{R}{G}")]
fn a_header_is_the_level_up_ability_for_its_cost(
    #[case] header: &str,
    #[case] level: u32,
    #[case] cost: &str,
) {
    let text = format!("{header}\nYou have no maximum hand size.");
    let abilities = block(&text).unwrap_or_default();
    assert_eq!(
        abilities.first(),
        Some(&ability(&format!(
            r#"{{"Activated":{{"cost":[{{"Mana":"{cost}"}}],"timing":"Sorcery","condition":{{"Level":{{"of":"This","op":"Eq","level":{}}}}},"effects":[{{"BecomeLevel":{{"level":{level}}}}}]}}}}"#,
            level - 1
        ))),
        "{text}"
    );
    assert_eq!(clauses::class_header(header), Some((level, cost)));
}

#[test]
fn a_trigger_of_the_level_is_conditioned_on_the_level() {
    let text = "{2}{U}: Level 2\nWhen ~ becomes level 2, draw two cards.";
    let draw_two = ability(&format!(
        r#"{{"Triggered":{{"when":{{"BecomesLevel":{{"what":["This"],"level":2}}}},"if":{},"effects":[{{"Draw":{{"count":2}}}}]}}}}"#,
        from_level(2)
    ));
    let abilities = block(text).unwrap_or_default();
    assert_eq!(abilities.len(), 2);
    assert_eq!(abilities[1], draw_two);
}

#[rstest]
#[case::a_static("{3}{W}: Level 2\nCreatures you control get +1/+1.", 1)]
#[case::a_trigger("{3}{W}: Level 3\nWhenever you gain life, draw a card.", 1)]
#[case::two_lines(
    "{1}{W}: Level 2\nWhenever you gain life, draw a card.\nCreatures you control get +1/+1.",
    2
)]
fn every_line_works_from_the_level_on(#[case] text: &str, #[case] lines: usize) {
    let level: u32 = text
        .lines()
        .next()
        .and_then(clauses::class_header)
        .map_or(0, |(level, _)| level);
    let abilities = block(text).unwrap_or_default();
    assert_eq!(abilities.len(), lines + 1, "{text}");
    assert!(matches!(
        abilities[0],
        Ability::Activated { ref effects, .. } if *effects == [Effect::BecomeLevel { level }]
    ));
    let wanted = Condition::Level {
        of: Ref::This,
        op: Cmp::Ge,
        level,
    };
    for ability in &abilities[1..] {
        let held = match ability {
            Ability::Static { condition, .. } => condition.as_ref(),
            Ability::Triggered { intervening_if, .. } => intervening_if.as_ref(),
            other => panic!("{other:?}"),
        };
        assert_eq!(held, Some(&wanted), "{text}");
    }
}

#[rstest]
#[case::a_header_alone("{2}{U}: Level 2")]
#[case::an_unread_line("{2}{U}: Level 2\nThe goblin dances at midnight.")]
#[case::an_unread_line_after_a_read_one(
    "{2}{U}: Level 2\nYou have no maximum hand size.\nSomething unknown happens."
)]
#[case::not_a_header("Draw a card.")]
#[case::level_up_is_a_keyword("Level up {1}{U}\nYou have no maximum hand size.")]
#[case::a_cost_in_words("Pay 2 life: Level 2\nYou have no maximum hand size.")]
#[case::no_number("{2}{U}: Level\nYou have no maximum hand size.")]
#[case::the_level_of_a_creature("LEVEL 2-3\n3/3")]
fn what_is_not_read_stays_with_the_table(#[case] text: &str) {
    assert_eq!(block(text), None, "{text}");
}

#[rstest]
#[case::the_second("When ~ becomes level 2, draw two cards.")]
#[case::the_third("When ~ becomes level 3, you gain 3 life.")]
fn becoming_a_level_writes_back_as_the_card_words_it(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    assert!(
        matches!(
            &read,
            Ability::Triggered {
                when: TriggerPattern::BecomesLevel { .. },
                ..
            }
        ),
        "{read:?}"
    );
}

#[rstest]
#[case::no_number("When ~ becomes level, draw a card.")]
fn a_near_miss_of_becoming_a_level_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
