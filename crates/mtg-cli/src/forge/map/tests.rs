//! Goal: the map file reads into rules where the most specific rule that holds wins, an item no rule names is unmapped and a keyword stands for the keyword of its name; keys ask for further verbs and tell uses apart; and the embedded map names only verbs the DSL has.
use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{GameRule, Modification};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use rstest::rstest;
use serde::de::DeserializeOwned;

use super::*;

const SMALL: &str = "\
# a small map
effect Zap => Effect.Alpha
effect Zap Mode=Hand|Gone => Effect.Beta
effect Zap Mode=Hand Flag => Effect.Gamma Effect.Delta   # two verbs
effect Mend => -
effect Bookkeep => ~
trigger Zap => Trigger.Echo
keyword Odd => -
";

fn small() -> Map {
    Map::parse(SMALL).unwrap_or_else(|error| panic!("{error}"))
}

fn item(kind: Kind, name: &str, body: &str) -> Item {
    Item {
        kind,
        name: name.to_owned(),
        params: Params::parse(body),
    }
}

fn verbs(names: &[&str]) -> Lookup {
    Lookup::Verbs(names.iter().map(|name| (*name).to_owned()).collect())
}

#[rstest]
#[case::the_general_rule(Kind::Effect, "Zap", "SP$ Zap", verbs(&["Effect.Alpha"]))]
#[case::a_condition_on_a_value(Kind::Effect, "Zap", "SP$ Zap | Mode$ Hand", verbs(&["Effect.Beta"]))]
#[case::another_listed_value(Kind::Effect, "Zap", "SP$ Zap | Mode$ Gone", verbs(&["Effect.Beta"]))]
#[case::a_value_not_listed(Kind::Effect, "Zap", "SP$ Zap | Mode$ Other", verbs(&["Effect.Alpha"]))]
#[case::two_conditions_beat_one(Kind::Effect, "Zap", "SP$ Zap | Mode$ Hand | Flag$ True", verbs(&["Effect.Gamma", "Effect.Delta"]))]
#[case::half_of_the_conditions(Kind::Effect, "Zap", "SP$ Zap | Flag$ True", verbs(&["Effect.Alpha"]))]
#[case::the_name_in_any_case(Kind::Effect, "zAP", "SP$ Zap", verbs(&["Effect.Alpha"]))]
#[case::no_verb_in_the_dsl(Kind::Effect, "Mend", "DB$ Mend", Lookup::NoVerb)]
#[case::bookkeeping(Kind::Effect, "Bookkeep", "DB$ Bookkeep", Lookup::Skip)]
#[case::the_same_name_in_another_family(Kind::Trigger, "Zap", "Mode$ Zap", verbs(&["Trigger.Echo"]))]
#[case::a_name_no_rule_has(Kind::Effect, "Unheard", "DB$ Unheard", Lookup::Unmapped)]
#[case::a_family_no_rule_has(Kind::Static, "Zap", "Mode$ Zap", Lookup::Unmapped)]
fn the_most_specific_rule_that_holds_decides(
    #[case] kind: Kind,
    #[case] name: &str,
    #[case] body: &str,
    #[case] expected: Lookup,
) {
    assert_eq!(small().lookup(&item(kind, name, body)), expected);
}

#[rstest]
#[case::a_one_word_keyword("Flying", &["Keyword.Flying"])]
#[case::two_words("First Strike", &["Keyword.FirstStrike", "Keyword.First"])]
#[case::a_keyword_with_a_kind("Enchant creature", &["Keyword.EnchantCreature", "Keyword.Enchant"])]
#[case::a_hyphen("Jump-start", &["Keyword.JumpStart"])]
#[case::an_apostrophe("Doctor's companion", &["Keyword.DoctorSCompanion", "Keyword.DoctorS"])]
fn a_keyword_with_no_rule_stands_for_the_keyword_of_its_name(
    #[case] name: &str,
    #[case] expected: &[&str],
) {
    let item = item(Kind::Keyword, name, "");
    assert_eq!(small().lookup(&item), verbs(expected));
}

#[test]
fn a_keyword_rule_beats_the_keyword_of_the_same_name() {
    assert_eq!(
        small().lookup(&item(Kind::Keyword, "Odd", "")),
        Lookup::NoVerb
    );
}

#[test]
fn a_key_asks_for_a_verb_of_its_own() {
    let map = Map::parse(
        "also effect Zap Extra => Mod.X Mod.Y\nalso effect * Wide => Effect.May\nalso trigger Zap Extra => Mod.Z",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let wanted = |kind, name, body| {
        map.wanted(&item(kind, name, body))
            .into_iter()
            .map(|wanted| (wanted.key, wanted.verbs))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        wanted(Kind::Effect, "Zap", "SP$ Zap | Extra$ 1 | Wide$ 2"),
        [
            (
                "Extra".to_owned(),
                vec!["Mod.X".to_owned(), "Mod.Y".to_owned()]
            ),
            ("Wide".to_owned(), vec!["Effect.May".to_owned()]),
        ]
    );
    assert_eq!(
        wanted(Kind::Effect, "Other", "SP$ Other | Wide$ 2").len(),
        1
    );
    assert_eq!(wanted(Kind::Effect, "Zap", "SP$ Zap").len(), 0);
    assert_eq!(
        wanted(Kind::Trigger, "Zap", "Mode$ Zap | Extra$ 1").len(),
        1
    );
}

#[rstest]
#[case::the_general_rule("SP$ Zap | Extra$ 1", &["Mod.X"])]
#[case::a_condition_that_holds("SP$ Zap | Extra$ 1 | Own$ Self", &["Mod.Y"])]
#[case::a_condition_with_another_value("SP$ Zap | Extra$ 1 | Own$ Other", &["Mod.X"])]
#[case::two_conditions_beat_one("SP$ Zap | Extra$ 1 | Own$ Self | Flag$ True", &["Mod.Z"])]
#[case::a_second_key_keeps_its_own_rule("SP$ Zap | Extra$ 1 | Wide$ 2", &["Mod.X", "Mod.W"])]
fn the_rule_of_a_key_with_the_most_conditions_that_hold_decides(
    #[case] body: &str,
    #[case] expected: &[&str],
) {
    let map = Map::parse(
        "also effect Zap Extra => Mod.X\nalso effect Zap Extra Own=Self => Mod.Y\nalso effect Zap Extra Own=Self Flag => Mod.Z\nalso effect Zap Wide => Mod.W",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let wanted: Vec<String> = map
        .wanted(&item(Kind::Effect, "Zap", body))
        .into_iter()
        .map(|wanted| wanted.verbs.join(" "))
        .collect();
    assert_eq!(wanted, expected);
}

#[test]
fn a_variant_key_is_named_with_its_value() {
    let map = Map::parse("variant effect Zap Mode\nvariant effect Zap Origin")
        .unwrap_or_else(|e| panic!("{e}"));
    let variants = |body| map.variants(&item(Kind::Effect, "Zap", body));
    assert_eq!(
        variants("SP$ Zap | Origin$ Gone | Mode$ Hand"),
        ["Mode=Hand", "Origin=Gone"]
    );
    assert_eq!(variants("SP$ Zap").len(), 0);
    let other = map.variants(&item(Kind::Effect, "Mend", "SP$ Mend | Mode$ Hand"));
    assert_eq!(other.len(), 0);
}

#[rstest]
#[case::no_family("bogus Zap => X", "line 1: `bogus` is not a family")]
#[case::no_arrow("effect Zap", "line 1: no `=>`")]
#[case::no_verbs("effect Zap =>", "line 1: no verbs after `=>`")]
#[case::no_name("effect", "line 1: no name")]
#[case::no_key_for_an_extra("also effect Zap => Mod.X", "line 1: no key")]
#[case::no_key_for_a_variant("variant effect Zap", "line 1: no key")]
fn a_line_that_does_not_read_is_named(#[case] line: &str, #[case] expected: &str) {
    assert_eq!(
        Map::parse(line),
        Err(expected.to_owned()),
        "for the line `{line}`"
    );
    let second = Map::parse(&format!("# fine\n{line}"));
    assert_eq!(second, Err(expected.replace("line 1", "line 2")));
}

#[test]
fn names_are_compared_in_lowercase_without_punctuation() {
    assert_eq!(norm("First Strike"), norm("first-strike"));
    assert_eq!(norm("Effect.Move"), "effect.move");
}

/// Whether the DSL type `T` has a variant called `tag`: serde says so unless it reports an unknown variant.
fn has_variant<T: DeserializeOwned>(tag: &str) -> bool {
    let mut object = serde_json::Map::new();
    object.insert(tag.to_owned(), serde_json::json!({}));
    serde_json::from_value::<T>(serde_json::Value::Object(object))
        .err()
        .is_none_or(|error| !error.to_string().contains("unknown variant"))
}

/// Whether `verb` (`Group.Tag` and what follows) names a variant of the DSL.
fn in_the_dsl(verb: &str) -> bool {
    let mut parts = verb.split('.');
    let (Some(group), Some(tag)) = (parts.next(), parts.next()) else {
        return false;
    };
    match group {
        "Ability" => has_variant::<Ability>(tag),
        "Effect" => has_variant::<Effect>(tag),
        "Trigger" => has_variant::<TriggerPattern>(tag),
        "Mod" => has_variant::<Modification>(tag),
        "Keyword" => has_variant::<Keyword>(tag),
        "Rule" => has_variant::<GameRule>(tag),
        "ReplaceWhen" => has_variant::<ReplaceWhen>(tag),
        "ReplaceBy" => has_variant::<ReplaceBy>(tag),
        "Cost" => has_variant::<CostItem>(tag),
        _ => false,
    }
}

#[rstest]
#[case("Effect.Draw", true)]
#[case("Effect.CreateToken.Copy", true)]
#[case("Trigger.ZoneChange", true)]
#[case("Keyword.Flying", true)]
#[case("Cost.Tap", true)]
#[case("Effect.Zork", false)]
#[case("Trigger.Draw", false)]
#[case("Keyword.Flyng", false)]
#[case("Zork.Draw", false)]
#[case("Draw", false)]
fn the_check_of_the_vocabulary_tells_a_verb_of_the_dsl_from_a_misspelling(
    #[case] verb: &str,
    #[case] expected: bool,
) {
    assert_eq!(in_the_dsl(verb), expected);
}

#[test]
fn the_embedded_map_reads_and_names_only_verbs_the_dsl_has() {
    let map = Map::standard().unwrap_or_else(|error| panic!("{error}"));
    for kind in Kind::ALL {
        assert!(map.rules_of(kind) > 0, "no rule for {}", kind.word());
    }
    let unknown: Vec<&str> = map.verbs().filter(|verb| !in_the_dsl(verb)).collect();
    assert!(unknown.is_empty(), "not in the DSL: {unknown:?}");
}

#[test]
fn the_embedded_map_names_the_sixty_most_used_apis() {
    let map = Map::standard().unwrap_or_else(|error| panic!("{error}"));
    // The APIs of the survey's ranking (`docs/reference-engines.md`): names only.
    let top = [
        "ChangeZone",
        "Pump",
        "Draw",
        "Token",
        "PutCounter",
        "Cleanup",
        "DealDamage",
        "Mana",
        "Effect",
        "GainLife",
        "Destroy",
        "Tap",
        "LoseLife",
        "Discard",
        "PumpAll",
        "Animate",
        "Dig",
        "Sacrifice",
        "Charm",
        "ChangeZoneAll",
        "Mill",
        "Counter",
        "Untap",
        "DelayedTrigger",
        "Scry",
        "ChooseCard",
        "DamageAll",
        "CopyPermanent",
        "DestroyAll",
        "RepeatEach",
        "SetState",
        "ImmediateTrigger",
        "Play",
        "GainControl",
        "PutCounterAll",
        "Regenerate",
        "CopySpellAbility",
        "Attach",
        "Surveil",
        "RemoveCounter",
        "MakeCard",
        "ChooseType",
        "Clone",
        "DigUntil",
        "PeekAndReveal",
        "GenericChoice",
        "ReplaceEffect",
        "Fight",
        "SacrificeAll",
        "Investigate",
        "RollDice",
        "AnimateAll",
        "StoreSVar",
        "ChooseColor",
        "ChoosePlayer",
        "AlterAttribute",
        "UntapAll",
        "PreventDamage",
        "Branch",
        "Seek",
    ];
    assert_eq!(top.len(), 60);
    for api in top {
        let item = item(Kind::Effect, api, &format!("DB$ {api}"));
        assert_ne!(
            map.lookup(&item),
            Lookup::Unmapped,
            "{api} is not in the map"
        );
    }
}
