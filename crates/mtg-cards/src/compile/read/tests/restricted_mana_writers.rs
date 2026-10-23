//! Goal: mana added with a restriction is written as the card words it (a list of subtypes, a subtype with a card type, "and/or" between plural kinds, the abilities of the same kinds in each wording), and another kind, use or order is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

/// A tap ability that adds one mana of any color with the uses.
fn any_color(only: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"AnyColor":1}},"only":{only}}}}}]}}}}"#
    )
}

const TURTLE: &str = r#"[{"Spell":{"subtypes":["Ninja","Turtle"]}}]"#;
const HERO: &str = r#"[{"Spell":{"subtypes":["Hero"]}},{"Ability":{"subtypes":["Hero"]}}]"#;
const TIME_LORD: &str = r#"[{"Spell":{"subtypes":["Time Lord","Alien"]}},{"Ability":{"subtypes":["Time Lord","Alien"]}}]"#;

#[rstest]
#[case::turtle_lair(
    TURTLE,
    "{T}: Add one mana of any color. Spend this mana only to cast a Ninja or Turtle spell."
)]
#[case::in_the_plural(
    TURTLE,
    "{T}: Add one mana of any color. Spend this mana only to cast Ninja or Turtle spells."
)]
#[case::haven_of_the_spirit_dragon(
    r#"[{"Spell":{"types":["Creature"],"subtypes":["Dragon"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast a Dragon creature spell."
)]
#[case::avengers_tower(
    HERO,
    "{T}: Add one mana of any color. Spend this mana only to cast a Hero spell or to activate an ability of a Hero source."
)]
#[case::heroes_in_the_plural(
    HERO,
    "{T}: Add one mana of any color. Spend this mana only to cast Hero spells or activate abilities of Heroes."
)]
#[case::gallifrey_council_chamber(
    TIME_LORD,
    "{T}: Add one mana of any color. Spend this mana only to cast a Time Lord or Alien spell or activate an ability of a Time Lord or Alien."
)]
#[case::an_ability_of_an_elemental(
    r#"[{"Spell":{"subtypes":["Elemental"]}},{"Ability":{"subtypes":["Elemental"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast an Elemental spell or activate an ability of an Elemental."
)]
fn restricted_mana_is_written_as_the_card_words_it(#[case] only: &str, #[case] text: &str) {
    let json = any_color(only);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        render_variants(&ability(&json))
    );
}

#[rstest]
#[case::cormela_and_or(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue","Black","Red"]},"only":[{"Spell":{"types":["Instant","Sorcery"]}}]}}]}}"#,
    "{1}, {T}: Add {U}{B}{R}. Spend this mana only to cast instant and/or sorcery spells."
)]
#[case::with_or(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue","Black","Red"]},"only":[{"Spell":{"types":["Instant","Sorcery"]}}]}}]}}"#,
    "{1}, {T}: Add {U}{B}{R}. Spend this mana only to cast instant or sorcery spells."
)]
fn plural_kinds_are_joined_by_or_and_by_and_or(#[case] json: &str, #[case] text: &str) {
    assert!(writes(json, text), "{text}");
}

#[rstest]
#[case::another_subtype(
    TURTLE,
    "{T}: Add one mana of any color. Spend this mana only to cast a Ninja or Mutant spell."
)]
#[case::a_subtype_dropped(
    TURTLE,
    "{T}: Add one mana of any color. Spend this mana only to cast a Ninja spell."
)]
#[case::abilities_added(
    TURTLE,
    "{T}: Add one mana of any color. Spend this mana only to cast a Ninja or Turtle spell or activate an ability of a Ninja or Turtle."
)]
#[case::a_spell_for_an_ability(
    HERO,
    "{T}: Add one mana of any color. Spend this mana only to cast a Hero spell."
)]
#[case::another_kind_of_source(
    HERO,
    "{T}: Add one mana of any color. Spend this mana only to cast a Hero spell or to activate an ability of a Villain source."
)]
#[case::a_creature_for_any_spell(
    r#"[{"Spell":{"types":["Creature"],"subtypes":["Dragon"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast a Dragon spell."
)]
#[case::an_artifact_for_a_creature(
    r#"[{"Spell":{"types":["Artifact"],"subtypes":["Dragon"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast a Dragon creature spell."
)]
fn another_use_is_not_the_text(#[case] only: &str, #[case] text: &str) {
    let json = any_color(only);
    assert!(!writes(&json, text), "{text}");
}

#[rstest]
#[case::troyan(
    r#"[{"Spell":{"min_value":5}},{"Spell":{"facts":["XInCost"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast spells with mana value 5 or greater or spells with {X} in their mana costs."
)]
#[case::thieving_varmint(
    r#"[{"Spell":{"facts":["NotOwned"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast spells you don't own."
)]
#[case::lord_of_the_forsaken(
    r#"[{"Spell":{"facts":["FromGraveyard"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast a spell from your graveyard."
)]
#[case::the_ringhart_crest(
    r#"[{"Spell":{"types":["Creature"],"chosen_type":true}},{"Spell":{"types":["Creature"],"supertypes":["Legendary"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type or a legendary creature spell."
)]
#[case::master_of_dark_rites(
    r#"[{"Spell":{"subtypes":["Vampire","Cleric","Demon"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast Vampire, Cleric, and/or Demon spells."
)]
#[case::vedalken_engineer(
    r#"[{"Spell":{"types":["Artifact"]}},{"Ability":{"types":["Artifact"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast artifact spells or activate abilities of artifacts."
)]
#[case::flamebraider(
    r#"[{"Spell":{"subtypes":["Elemental"]}},{"Ability":{"subtypes":["Elemental"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast Elemental spells or activate abilities of Elemental sources."
)]
#[case::ixallis_lorekeeper(
    r#"[{"Spell":{"subtypes":["Dinosaur"]}},{"Ability":{"subtypes":["Dinosaur"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast a Dinosaur spell or activate an ability of a Dinosaur source."
)]
fn the_restrictions_of_the_spender_cards_are_written_as_they_are_worded(
    #[case] only: &str,
    #[case] text: &str,
) {
    let json = any_color(only);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        render_variants(&ability(&json))
    );
}

#[rstest]
#[case::a_lower_value(
    r#"[{"Spell":{"min_value":5}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast spells with mana value 6 or greater."
)]
#[case::a_hand_for_a_graveyard(
    r#"[{"Spell":{"facts":["FromGraveyard"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast a spell from your hand."
)]
#[case::owned_for_not_owned(
    r#"[{"Spell":{"facts":["NotOwned"]}}]"#,
    "{T}: Add one mana of any color. Spend this mana only to cast spells you own."
)]
fn another_value_zone_or_owner_is_not_the_text(#[case] only: &str, #[case] text: &str) {
    let json = any_color(only);
    assert!(!writes(&json, text), "{text}");
}
