//! Goal: "as long as you control your commander" (Lieutenant) and the keywords melee and myriad given by a static ability read into the condition and the keyword and write back as the same text, and the ability word before them changes nothing; near-misses stay with the table.
use mtg_core::card::{CardType, Subtype, Supertype};

use super::*;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition};

fn creature() -> Pred {
    Pred::Type(CardType::Creature)
}

fn commander_held() -> Condition {
    Condition::Compare {
        left: Amount::Count(Selector(vec![
            Pred::Commander,
            Pred::OwnedBy(Ref::You),
            Pred::ControlledBy(Ref::You),
        ])),
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    }
}

fn gives(
    affected: Ref,
    mods: Vec<Modification>,
    condition: Option<Condition>,
) -> Ability {
    Ability::Static {
        affected,
        mods,
        condition,
        defining: false,
        from: ZoneKind::Battlefield,
    }
}

fn group(preds: Vec<Pred>) -> Ref {
    Ref::Each(Selector(preds))
}

#[rstest]
#[case::other_creatures_melee(
    "Other creatures you control have melee.",
    group(vec![creature(), Pred::Other, Pred::ControlledBy(Ref::You)]),
    vec![Modification::AddKeyword(Keyword::Melee)],
    None
)]
#[case::creatures_myriad(
    "Creatures you control have myriad.",
    group(vec![creature(), Pred::ControlledBy(Ref::You)]),
    vec![Modification::AddKeyword(Keyword::Myriad)],
    None
)]
#[case::a_subtype_melee(
    "Other Spirits you control have melee.",
    group(vec![Pred::Subtype(Subtype("Spirit".to_owned())), Pred::Other, Pred::ControlledBy(Ref::You)]),
    vec![Modification::AddKeyword(Keyword::Melee)],
    None
)]
#[case::nonlegendary_artifact_creatures(
    "Nonlegendary artifact creatures you control have myriad.",
    group(vec![
        Pred::Type(CardType::Artifact),
        creature(),
        Pred::Not(Box::new(Pred::Supertype(Supertype::Legendary))),
        Pred::ControlledBy(Ref::You),
    ]),
    vec![Modification::AddKeyword(Keyword::Myriad)],
    None
)]
#[case::equipped_creature_myriad(
    "Equipped creature has myriad.",
    Ref::Attached,
    vec![Modification::AddKeyword(Keyword::Myriad)],
    None
)]
#[case::melee_with_the_commander(
    "As long as you control your commander, other creatures you control have melee.",
    group(vec![creature(), Pred::Other, Pred::ControlledBy(Ref::You)]),
    vec![Modification::AddKeyword(Keyword::Melee)],
    Some(commander_held())
)]
#[case::a_bonus_with_the_commander(
    "~ gets +2/+2 as long as you control your commander.",
    Ref::This,
    vec![Modification::ModifyPT { power: Amount::Fixed(2), toughness: Amount::Fixed(2) }],
    Some(commander_held())
)]
#[case::a_keyword_with_the_commander(
    "As long as you control your commander, ~ has hexproof.",
    Ref::This,
    vec![Modification::AddKeyword(Keyword::Hexproof)],
    Some(commander_held())
)]
fn a_static_ability_reads_with_its_keyword_and_condition(
    #[case] text: &str,
    #[case] affected: Ref,
    #[case] mods: Vec<Modification>,
    #[case] condition: Option<Condition>,
) {
    assert_eq!(
        golden(text, ClauseKind::Static, false),
        gives(affected, mods, condition),
        "{text}"
    );
}

#[rstest]
#[case::lieutenant(
    "Lieutenant \u{2014} As long as you control your commander, ~ gets +2/+2 and has flying.",
    "As long as you control your commander, ~ gets +2/+2 and has flying."
)]
#[case::lieutenant_melee(
    "Lieutenant \u{2014} As long as you control your commander, other creatures you control have melee.",
    "As long as you control your commander, other creatures you control have melee."
)]
fn the_ability_word_is_flavor(#[case] with: &str, #[case] without: &str) {
    let read = clause(with, ClauseKind::Static, false);
    assert!(read.is_some(), "{with}");
    assert_eq!(read, clause(without, ClauseKind::Static, false), "{with}");
}

#[rstest]
#[case::a_commander_you_do_not_control("As long as your opponents control your commander, ~ has flying.")]
#[case::someone_elses_commander("As long as you control their commander, ~ has flying.")]
#[case::a_count_of_commanders("As long as you control two or more of your commander, ~ has flying.")]
#[case::riot_is_not_given_yet("Nontoken creatures you control have riot.")]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
