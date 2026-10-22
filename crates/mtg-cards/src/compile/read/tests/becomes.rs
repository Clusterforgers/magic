//! Goal: a permanent that becomes a creature until end of turn (the manlands: size, colors, creature types, artifact, keywords, all creature types and an ability in quotes, the duration first or last, "It's still a land." and "It can't be blocked this turn.") reads into one `Continuous` effect of the card, writes back as the same text, and the near-misses stay with the table.
use super::*;
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;
use crate::value::Amount;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

/// The modifications of the first effect of an activated ability.
fn mods_of(text: &str) -> Vec<Modification> {
    let Some(Ability::Activated { effects, .. }) = activated(text) else {
        panic!("not read: {text}");
    };
    let Some(Effect::Continuous {
        affected: Ref::This,
        mods,
        duration: Duration::UntilEndOfTurn,
    }) = effects.into_iter().next()
    else {
        panic!("not a continuous effect of the card: {text}");
    };
    mods
}

fn size(power: i32, toughness: i32) -> Modification {
    Modification::SetPT {
        power: Amount::Fixed(power),
        toughness: Amount::Fixed(toughness),
    }
}

#[rstest]
#[case::all_creature_types(
    "{1}: This land becomes a 2/2 creature with all creature types until end of turn. It's still a land."
)]
#[case::artifact_subtype(
    "{1}: This land becomes a 2/2 Assembly-Worker artifact creature until end of turn. It's still a land."
)]
#[case::duration_first_two_colors_two_keywords(
    "{3}{W}{U}: Until end of turn, this land becomes a 4/4 white and blue Elemental creature with flying and vigilance. It's still a land."
)]
#[case::one_color_one_keyword(
    "{1}{U}: This land becomes a 2/1 blue Faerie creature with flying until end of turn. It's still a land."
)]
#[case::two_subtypes_and_infect(
    "{1}: This land becomes a 1/1 Phyrexian Blinkmoth artifact creature with flying and infect until end of turn. It's still a land."
)]
#[case::a_keyword_and_all_creature_types(
    "{4}: Until end of turn, this land becomes a 3/3 creature with vigilance and all creature types. It's still a land."
)]
#[case::no_color_no_keyword(
    "{4}: This land becomes a 4/2 Golem artifact creature until end of turn. It's still a land."
)]
#[case::an_eight(
    "{8}: This land becomes an 8/8 green Hydra creature with trample until end of turn. It's still a land."
)]
#[case::ward(
    "{5}{U}: Until end of turn, this land becomes a 7/7 blue Giant creature with ward {3}. It's still a land."
)]
#[case::three_colors(
    "{4}: This land becomes a 1/1 white, blue, and red Human creature until end of turn. It's still a land."
)]
#[case::an_artifact_that_is_not_a_land(
    "{2}: This artifact becomes a 4/4 red and green Dragon artifact creature with flying until end of turn."
)]
#[case::unblockable(
    "{1}{U}{B}: Until end of turn, this land becomes a 3/2 blue and black Elemental creature. It's still a land. It can't be blocked this turn."
)]
#[case::a_quoted_trigger(
    "{3}{R}: Until end of turn, this land becomes a 3/2 red Goblin creature with \"Whenever this creature attacks, create a 1/1 red Goblin creature token that's tapped and attacking.\" It's still a land."
)]
#[case::a_quoted_ability(
    "{1}{B}{R}: Until end of turn, this land becomes a 2/2 black and red Elemental creature with \"{X}: This creature gets +X/+0 until end of turn.\" It's still a land."
)]
fn a_card_that_becomes_a_creature_reads_and_writes_back(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[test]
fn the_manland_wording_is_the_layered_modifications_of_the_card() {
    assert_eq!(
        mods_of(
            "{1}: This land becomes a 2/2 Assembly-Worker artifact creature until end of turn. It's still a land."
        ),
        vec![
            size(2, 2),
            Modification::SetCreatureTypes(vec![Subtype("Assembly-Worker".into())]),
            Modification::AddType(CardType::Artifact),
            Modification::AddType(CardType::Creature),
        ]
    );
    assert_eq!(
        mods_of(
            "{3}{W}{U}: Until end of turn, this land becomes a 4/4 white and blue Elemental creature with flying and vigilance."
        ),
        vec![
            size(4, 4),
            Modification::SetColors(vec![Color::White, Color::Blue]),
            Modification::SetCreatureTypes(vec![Subtype("Elemental".into())]),
            Modification::AddType(CardType::Creature),
            Modification::AddKeyword(Keyword::Flying),
            Modification::AddKeyword(Keyword::Vigilance),
        ]
    );
    assert_eq!(
        mods_of("{1}: This land becomes a 2/2 creature with all creature types until end of turn."),
        vec![
            size(2, 2),
            Modification::AddType(CardType::Creature),
            Modification::AddAllCreatureTypes,
        ]
    );
}

#[test]
fn it_cant_be_blocked_is_a_second_effect_of_the_card() {
    let Some(Ability::Activated { effects, .. }) = activated(
        "{1}{U}{B}: Until end of turn, this land becomes a 3/2 blue and black Elemental creature. It's still a land. It can't be blocked this turn.",
    ) else {
        panic!("not read");
    };
    assert!(matches!(
        effects.as_slice(),
        [
            Effect::Continuous { .. },
            Effect::Continuous { mods, .. }
        ] if mods == &[Modification::CantBeBlocked]
    ));
}

#[rstest]
#[case::no_duration("{1}: This land becomes a 2/2 creature. It's still a land.")]
#[case::two_durations(
    "{1}: Until end of turn, this land becomes a 2/2 creature until end of turn. It's still a land."
)]
#[case::no_size("{1}: This land becomes a creature until end of turn. It's still a land.")]
#[case::a_size_of_x(
    "{X}{G}: Until end of turn, this land becomes an X/X green Hydra creature. It's still a land."
)]
#[case::another_permanent(
    "{T}: Target land you control becomes a 3/3 Elemental creature with haste until end of turn."
)]
#[case::still_another_kind_of_land(
    "{2}: This land becomes a 3/3 Elemental creature until end of turn. It's still a Cave land."
)]
#[case::lasting("{4}: This land becomes a 4/2 Golem artifact creature that's still a land.")]
#[case::an_ability_before_a_keyword(
    "{1}: This land becomes a 2/2 creature with all creature types and flying until end of turn."
)]
#[case::a_quote_without_its_period(
    "{1}{B}: This land becomes a 1/1 black Skeleton creature with \"{B}: Regenerate this creature\" until end of turn. It's still a land."
)]
#[case::a_word_after_the_sentences(
    "{1}: This land becomes a 2/2 creature until end of turn. It's still a land. Mill is fun."
)]
fn near_misses_of_a_card_becoming_a_creature_stay_with_the_table(#[case] text: &str) {
    assert!(activated(text).is_none(), "{text}");
}
