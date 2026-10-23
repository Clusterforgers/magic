//! Goal: a Vehicle that becomes an artifact creature until end of turn (the card or another target Vehicle, with the size it prints) and a Mount that becomes saddled read and write back, and a permanent that becomes a creature until end of turn (the manlands: size, colors, creature types, artifact, keywords, all creature types and an ability in quotes, the duration first or last, "It's still a land." and "It can't be blocked this turn.") reads into one `Continuous` effect of the card, writes back as the same text, and the near-misses stay with the table; a land that enters tapped "if you control two or more other lands" reads and writes back too.
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
#[case::a_quoted_trigger_about_the_creature(
    "{2}{R}{G}: Until end of turn, this land becomes a 3/3 red and green Elemental creature with \"Whenever this creature attacks, put a +1/+1 counter on it.\" It's still a land."
)]
#[case::a_keyword_and_a_quoted_trigger(
    "{3}{R}: Until end of turn, this land becomes a 3/3 red Beholder creature with menace and \"Whenever this creature attacks, draw a card.\" It's still a land."
)]
#[case::a_quoted_ability(
    "{1}{B}{R}: Until end of turn, this land becomes a 2/2 black and red Elemental creature with \"{X}: This creature gets +X/+0 until end of turn.\" It's still a land."
)]
fn a_card_that_becomes_a_creature_reads_and_writes_back(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[test]
fn the_pump_beside_a_manland_names_its_creature_type() {
    golden(
        "{1}, {T}: Target attacking Assembly-Worker gets +2/+2 until end of turn.",
        ClauseKind::Activated,
        false,
    );
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

#[rstest]
#[case::two_other_lands("If you control two or more other lands, this land enters tapped.")]
#[case::a_plains("If you control a Plains, ~ enters tapped.")]
#[case::opponents_lands("If your opponents control eight or more lands, this land enters tapped.")]
fn a_land_that_enters_tapped_when_it_controls_other_lands(#[case] text: &str) {
    golden(text, ClauseKind::Static, false);
}

#[rstest]
#[case::untapped("If you control two or more other lands, this land enters untapped.")]
#[case::a_spell("If you control two or more other lands, draw a card.")]
#[case::a_life_total("If an opponent has 10 or less life, this land enters tapped.")]
#[case::without_the_comma("If you control two or more other lands this land enters tapped.")]
fn near_misses_of_a_land_entering_tapped_stay_with_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Static, false).is_none(), "{text}");
}

#[rstest]
#[case::the_card_with_energy(
    "Pay {E}{E}{E}{E}: This Vehicle becomes an artifact creature until end of turn."
)]
#[case::tapped_tokens(
    "Tap two untapped tokens you control: This Vehicle becomes an artifact creature until end of turn."
)]
#[case::another_vehicle(
    "{1}{W}: Another target Vehicle you control becomes an artifact creature until end of turn."
)]
#[case::a_vehicle_you_control(
    "{2}, {T}: Target Vehicle you control becomes an artifact creature until end of turn."
)]
#[case::a_mount_saddled(
    "{2}, {T}: Target Mount you control becomes saddled until end of turn. Activate only as a sorcery."
)]
#[case::the_card_itself("{2}: This artifact becomes an artifact creature until end of turn.")]
fn a_vehicle_that_becomes_an_artifact_creature_or_a_mount_saddled_reads_and_writes_back(
    #[case] text: &str,
) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::the_card("{1}: This Vehicle becomes an artifact creature until end of turn.", Ref::This)]
#[case::a_target(
    "{1}: Another target Vehicle you control becomes an artifact creature until end of turn.",
    Ref::Target(0)
)]
fn a_vehicle_keeps_its_printed_size_while_it_is_an_artifact_creature(
    #[case] text: &str,
    #[case] affected: Ref,
) {
    let Some(Ability::Activated { effects, .. }) = activated(text) else {
        panic!("not read: {text}");
    };
    assert_eq!(
        effects,
        [Effect::Continuous {
            affected,
            mods: vec![
                Modification::AddType(CardType::Artifact),
                Modification::AddType(CardType::Creature),
            ],
            duration: Duration::UntilEndOfTurn,
        }]
    );
}

#[rstest]
#[case::lasting("{1}: This Vehicle becomes an artifact creature.")]
#[case::not_an_artifact("{1}: This Vehicle becomes a creature until end of turn.")]
#[case::two_steps(
    "{1}: Target Vehicle becomes an artifact creature until end of turn and untaps target land."
)]
#[case::saddled_for_good("{1}: This Mount becomes saddled.")]
fn near_misses_of_a_vehicle_becoming_a_creature_stay_with_the_table(#[case] text: &str) {
    assert!(activated(text).is_none(), "{text}");
}
