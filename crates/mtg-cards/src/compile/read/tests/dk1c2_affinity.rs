//! Goal: affinity given to the spells a selector names ("Enchantment spells you cast have affinity for Auras.") reads for each
//! kind of spell and permanent and writes back as the same text, the cast of a spell that targets a modified permanent you
//! control reads and writes, and a near-miss (no kind counted, another keyword, a keyword that helps nothing) stays unread.
use super::*;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Ref};

#[rstest]
#[case::auras("Enchantment spells you cast have affinity for Auras.")]
#[case::equipment("Creature spells you cast have affinity for Equipment.")]
#[case::artifacts("Noncreature spells you cast have affinity for artifacts.")]
#[case::a_land_type("Artifact spells you cast have affinity for Islands.")]
#[case::tokens("Spells you cast have affinity for tokens.")]
#[case::with_another_keyword("Artifact spells you cast have convoke and affinity for artifacts.")]
fn affinity_given_to_spells_reads_and_writes_back(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, false);
    let Ability::Static {
        affected: Ref::Each(spells),
        mods,
        ..
    } = read
    else {
        panic!("{text}: {read:?}");
    };
    assert_eq!(spells.0.first(), Some(&Pred::Spell), "{text}");
    assert!(
        mods.iter()
            .any(|given| matches!(given, Modification::AddKeyword(Keyword::Affinity(_)))),
        "{text}"
    );
}

#[rstest]
#[case::no_kind("Enchantment spells you cast have affinity.")]
#[case::no_kind_after_for("Enchantment spells you cast have affinity for.")]
#[case::an_unread_kind("Enchantment spells you cast have affinity for the best.")]
#[case::another_keyword("Enchantment spells you cast have flying.")]
#[case::trample_and_affinity("Enchantment spells you cast have trample and affinity for Auras.")]
#[case::not_spells("Enchantments you control have affinity for Auras.")]
fn affinity_near_misses_stay_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert!(
        !matches!(
            &read,
            Some(Ability::Static { mods, .. })
                if mods.iter().any(|given| matches!(given, Modification::AddKeyword(Keyword::Affinity(_))))
        ),
        "{text}: {read:?}"
    );
}

const DRAW: &str = r#"[{"Draw":{}}]"#;

fn cast_targeting(what: &str, effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":{what},"by":"You"}}}},"effects":{effects}}}}}"#
    )
}

#[rstest]
#[case::the_deck_card(
    "Whenever you cast an Aura spell that targets a modified permanent you control, draw a card.",
    r#"[{"Subtype":"Aura"},{"Targeting":["Modified",{"ControlledBy":"You"}]}]"#
)]
#[case::a_creature(
    "Whenever you cast an Aura spell that targets a modified creature you control, draw a card.",
    r#"[{"Subtype":"Aura"},{"Targeting":[{"Type":"Creature"},"Modified",{"ControlledBy":"You"}]}]"#
)]
#[case::any_spell(
    "Whenever you cast a spell that targets a modified creature you control, draw a card.",
    r#"[{"Targeting":[{"Type":"Creature"},"Modified",{"ControlledBy":"You"}]}]"#
)]
#[case::an_equipment_spell(
    "Whenever you cast an Equipment spell that targets a modified permanent you control, draw a card.",
    r#"[{"Subtype":"Equipment"},{"Targeting":["Modified",{"ControlledBy":"You"}]}]"#
)]
fn a_spell_that_targets_a_modified_permanent_reads_and_writes(
    #[case] text: &str,
    #[case] what: &str,
) {
    reads_as(
        text,
        ClauseKind::Triggered,
        false,
        &cast_targeting(what, DRAW),
    );
}

#[rstest]
#[case::the_wrong_word(
    "Whenever you cast an Aura spell that targets a modifying permanent you control, draw a card."
)]
#[case::a_player("Whenever you cast an Aura spell that targets a modified player, draw a card.")]
#[case::no_target_phrase(
    "Whenever you cast an Aura spell with a modified permanent you control, draw a card."
)]
fn a_modified_target_near_misses_stay_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    assert!(read.is_none(), "{text}: {read:?}");
}
