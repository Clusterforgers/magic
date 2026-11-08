//! Goal: the writers say two abilities of one printed sentence, the graveyard activation, an artifact that becomes an artifact creature and a card returned "this way" as the cards print them, and a near miss of any of them never reads back.
use super::*;

/// Whether some wording of the ability is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// Whether some wording of the two abilities as one sentence is the clause.
fn paired(first: &str, second: &str, clause: &str) -> bool {
    let (first, second) = (ability(first), ability(second));
    let mut texts = static_pair_texts(&first, &second);
    texts.extend(either_way_pair_texts(&first, &second));
    texts.iter().any(|wording| same_text(wording, clause))
}

const ATTACK: &str = r#"{"Rule":{"rule":{"MustAttack":"This"}}}"#;
const BLOCK: &str = r#"{"Rule":{"rule":{"MustBlock":"This"}}}"#;
const NOT_TARGETED: &str =
    r#"{"Rule":{"rule":{"Cant":{"action":{"BeTargeted":["Spell"]},"who":"Attached"}}}}"#;
const NOT_ENCHANTED: &str = r#"{"Rule":{"rule":{"Cant":{"action":{"BeAttachedBy":[{"Subtype":"Aura"},"Other"]},"who":"Attached"}}}}"#;
const CHEAPER: &str = r#"{"CostModifier":{"applies_to":["Spell",{"Compare":{"op":"Ge","stat":"ManaValue","to":5}},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}"}}"#;
const UNCOUNTERABLE: &str = r#"{"Rule":{"rule":{"CantBeCountered":{"Each":["Spell",{"Compare":{"op":"Ge","stat":"ManaValue","to":5}},{"WasCast":{"by":"You"}}]}}}}"#;
const DOUBLE_PLAYER: &str =
    r#"{"Replacement":{"by":{"Multiply":2},"when":{"Damage":{"to":"Opponents"}}}}"#;
const DOUBLE_PERMANENT: &str = r#"{"Replacement":{"by":{"Multiply":2},"when":{"Damage":{"to":{"Each":[{"ControlledBy":"Opponents"}]}}}}}"#;
const HALF_PLAYER: &str = r#"{"Replacement":{"by":"PreventHalf","when":{"Damage":{"to":"You"}}}}"#;
const HALF_PERMANENT: &str = r#"{"Replacement":{"by":"PreventHalf","when":{"Damage":{"to":{"Each":[{"ControlledBy":"You"}]}}}}}"#;

#[rstest]
#[case::attacks_or_blocks(ATTACK, BLOCK, "This creature attacks or blocks each combat if able.")]
#[case::cant_both(
    NOT_TARGETED,
    NOT_ENCHANTED,
    "Enchanted creature can't be the target of spells and can't be enchanted by other Auras."
)]
#[case::cheaper_and_uncounterable(
    CHEAPER,
    UNCOUNTERABLE,
    "Spells you cast with mana value 5 or greater cost {1} less to cast and can't be countered."
)]
#[case::double_damage(
    DOUBLE_PLAYER,
    DOUBLE_PERMANENT,
    "If a source would deal damage to an opponent or a permanent an opponent controls, that source deals double that damage to that player or permanent instead."
)]
#[case::half_damage(
    HALF_PLAYER,
    HALF_PERMANENT,
    "If a source would deal damage to you or a permanent you control, prevent half that damage, rounded up."
)]
fn two_abilities_read_as_one_sentence(
    #[case] first: &str,
    #[case] second: &str,
    #[case] clause: &str,
) {
    assert!(paired(first, second, clause), "{clause}");
}

#[rstest]
#[case::another_creature_blocks(ATTACK, &BLOCK.replace("This", "Attached"))]
#[case::must_block_first(BLOCK, ATTACK)]
#[case::another_prohibited(NOT_TARGETED, &NOT_ENCHANTED.replace(r#""who":"Attached""#, r#""who":"This""#))]
#[case::spells_controlled_not_cast(CHEAPER, &UNCOUNTERABLE.replace(r#"{"WasCast":{"by":"You"}}"#, r#"{"ControlledBy":"Opponents"}"#))]
#[case::another_size(CHEAPER, &UNCOUNTERABLE.replace('5', "6"))]
#[case::another_replacement(DOUBLE_PLAYER, HALF_PERMANENT)]
#[case::permanents_of_you(DOUBLE_PLAYER, &DOUBLE_PERMANENT.replace("Opponents", "You"))]
fn a_near_miss_of_two_abilities_never_reads_as_one_sentence(
    #[case] first: &str,
    #[case] second: &str,
) {
    for clause in [
        "This creature attacks or blocks each combat if able.",
        "Enchanted creature can't be the target of spells and can't be enchanted by other Auras.",
        "Spells you cast with mana value 5 or greater cost {1} less to cast and can't be countered.",
        "If a source would deal damage to an opponent or a permanent an opponent controls, that source deals double that damage to that player or permanent instead.",
        "If a source would deal damage to you or a permanent you control, prevent half that damage, rounded up.",
    ] {
        assert!(!paired(first, second, clause), "{clause}");
    }
}

const GLORY: &str = r#"{"Activated":{"cost":"{2}{W}","effects":[{"Draw":{}}],"from":"Graveyard"}}"#;

#[test]
fn an_ability_of_the_graveyard_says_where_it_is_activated_from() {
    assert!(written(
        GLORY,
        "{2}{W}: Draw a card. Activate only if ~ is in your graveyard."
    ));
    assert!(!written(
        &GLORY.replace(r#","from":"Graveyard""#, ""),
        "{2}{W}: Draw a card. Activate only if ~ is in your graveyard."
    ));
}

const SUIT_UP: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetPT":{"power":4,"toughness":5}},{"AddType":"Artifact"},{"AddType":"Creature"}]}},{"Draw":{}}],"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Subtype":"Vehicle"}]}]}}]}}"#;
const MIMIC: &str = r#"{"Activated":{"cost":"{2}","effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"SetPT":{"power":3,"toughness":3}},{"SetCreatureTypes":["Shapeshifter"]},{"AddType":"Artifact"},{"AddType":"Creature"}]}}]}}"#;
const SPHERE: &str = r#"{"Activated":{"cost":"{2}","effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"SetPT":{"power":3,"toughness":2}},{"SetCreatureTypes":["Construct"]},{"AddType":"Artifact"},{"AddType":"Creature"},{"RemoveKeyword":"Flying"}]}}]}}"#;
const MAJESTIC: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetPT":{"power":4,"toughness":4}},{"SetCreatureTypes":["Angel"]},{"AddType":"Artifact"},{"AddType":"Creature"},{"AddKeyword":"Flying"}]}},{"Draw":{}}],"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}]}}]}}"#;

#[rstest]
#[case::size_as_base(
    SUIT_UP,
    "Until end of turn, target creature or Vehicle becomes an artifact creature with base power and toughness 4/5.\nDraw a card."
)]
#[case::types_and_base_size(
    MIMIC,
    "{2}: This artifact becomes a Shapeshifter artifact creature with base power and toughness 3/3 until end of turn."
)]
#[case::loses_a_keyword(
    SPHERE,
    "{2}: Until end of turn, this artifact becomes a 3/2 Construct artifact creature and loses flying."
)]
#[case::gains_a_keyword(
    MAJESTIC,
    "Until end of turn, target artifact or creature becomes a 4/4 Angel artifact creature and gains flying.\nDraw a card."
)]
fn an_artifact_that_becomes_a_creature_is_said_as_printed(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_size(
    SUIT_UP,
    "Until end of turn, target creature or Vehicle becomes an artifact creature with base power and toughness 4/6.\nDraw a card."
)]
#[case::another_type(
    MIMIC,
    "{2}: This artifact becomes a Golem artifact creature with base power and toughness 3/3 until end of turn."
)]
#[case::gains_not_loses(
    SPHERE,
    "{2}: Until end of turn, this artifact becomes a 3/2 Construct artifact creature and gains flying."
)]
#[case::no_land(
    MIMIC,
    "{2}: This artifact becomes a Shapeshifter artifact creature with base power and toughness 3/3 until end of turn. It's still a land."
)]
fn a_near_miss_of_an_artifact_creature_never_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

const FOUL_RENEWAL: &str = r#"{"Spell":{"effects":[{"Move":{"as":"moved","to":"Hand","what":{"Target":0}}},{"Continuous":{"affected":{"Target":1},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":{"Times":{"by":-1,"of":{"Stat":{"of":{"Bound":"moved"},"stat":"Toughness"}}}},"toughness":{"Times":{"by":-1,"of":{"Stat":{"of":{"Bound":"moved"},"stat":"Toughness"}}}}}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"OwnedBy":"You"},{"In":"Graveyard"}]}},{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;

#[test]
fn a_card_returned_is_named_by_its_toughness() {
    assert!(written(
        FOUL_RENEWAL,
        "Return target creature card from your graveyard to your hand. Target creature gets -X/-X until end of turn, where X is the toughness of the card returned this way."
    ));
    assert!(!written(
        FOUL_RENEWAL,
        "Return target creature card from your graveyard to your hand. Target creature gets -X/-X until end of turn, where X is the power of the card returned this way."
    ));
}

const LEGEND: &str = r#"{"Static":{"affected":"Attached","mods":[{"AddType":"Creature"},{"SetPT":{"power":5,"toughness":5}}]}}"#;

#[test]
fn an_aura_that_adds_the_creature_type_without_a_creature_type_of_its_own() {
    assert!(
        super::aura_additive::texts(&ability_mods(LEGEND)).contains(
            &"is a creature with base power and toughness 5/5 in addition to its other types"
                .to_owned()
        )
    );
}

/// The changes of a static ability.
fn ability_mods(json: &str) -> Vec<crate::modify::Modification> {
    match ability(json) {
        Ability::Static { mods, .. } => mods,
        _ => Vec::new(),
    }
}
