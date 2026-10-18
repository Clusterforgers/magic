//! Card assembly: the printed side from the imported record and the rules from the DSL become the
//! definition the engine plays, with the support record that says how far to trust it.
//!
//! Design record: `docs/cards-dsl.md`, "Printed side and rules". The definition is that of the front
//! face; a card with several faces is `Manual` until the engine plays the others.

use std::collections::BTreeSet;

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::{ManaCost, ManaType, ParseManaCostError};

use crate::ability::{Ability, Timing};
use crate::card::Card;
use crate::cost::CostItem;
use crate::definition::{AbilityDef, CardDefinition, ManaAbility};
use crate::effect::{Effect, Produce};
use crate::mana::ManaCostX;
use crate::refs::{Ref, ZoneKind};
use crate::support::{
    Caps, Clause, PrintedCard, PrintedFace, Support, SupportRecord, support, whole_text,
};
use crate::validate::used_primitives;

/// The one primitive a plain `{T}: Add {X}` stands for: the engine runs that exact shape, one fixed
/// mana for a tap, not the general `Ability.Activated`, `Cost.Tap` and `Effect.AddMana`.
pub const PLAIN_MANA_ABILITY: &str = "Ability.ManaAbility";
/// The engine pays `{X}` in the mana cost of a spell.
pub const X_COST: &str = "Card.XCost";
/// The engine plays every face of a card that has more than one.
pub const FACES: &str = "Card.Faces";
/// The engine knows a power, toughness, loyalty or defense that is not a number (`*`).
pub const VARIABLE_STATS: &str = "Card.VariableStats";

/// The basic land types in the order of CR 305.6, each with the mana its intrinsic ability adds.
const BASIC_LAND_TYPES: [(&str, ManaType); 5] = [
    ("Plains", ManaType::White),
    ("Island", ManaType::Blue),
    ("Swamp", ManaType::Black),
    ("Mountain", ManaType::Red),
    ("Forest", ManaType::Green),
];

/// A card as the engine plays it, and how far the engine can be trusted with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assembled {
    /// The front face's printed characteristics and the abilities the engine runs.
    pub definition: CardDefinition,
    /// The state the card is in and the clauses left to the table.
    pub support: SupportRecord,
}

/// Builds the definition of one oracle card from its imported record and its rules, if it has any.
///
/// The printed side always comes from `printed`. The abilities of the rules are attached only when
/// the support record says the engine runs them (`Verified` or `Partial`), and then only those the
/// engine runs; every other card is `Manual` and has the printed side alone. The mana abilities the
/// basic land types give are part of the printed side (CR 305.6), so they are always there.
pub fn assemble(printed: &PrintedCard, rules: Option<&Card>, caps: &Caps) -> Assembled {
    let record = support(rules, printed, caps);
    let mut definition = match printed.faces.first() {
        Some(front) => printed_side(front),
        None => CardDefinition::new(printed.name.clone()),
    };
    definition
        .abilities
        .extend(intrinsic_abilities(&definition));
    if matches!(record.support, Support::Verified | Support::Partial)
        && let Some(front) = rules.and_then(|card| card.faces.first())
    {
        let runs = |ability: &&Ability| caps.missing(&used_primitives(ability)).is_empty();
        definition
            .abilities
            .extend(front.abilities.iter().filter(runs).filter_map(lower));
    }
    Assembled {
        definition,
        support: record,
    }
}

/// The mana a plain `{T}: Add {X}` adds: no targets, no limit, usable at instant speed from the
/// battlefield, cost exactly `{T}`, and one fixed mana added once to the controller's pool.
pub fn plain_mana_ability(ability: &Ability) -> Option<ManaType> {
    let Ability::Activated {
        cost,
        timing: Timing::Instant,
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    let [CostItem::Tap] = cost.0.as_slice() else {
        return None;
    };
    let [
        Effect::AddMana {
            who: Ref::You,
            mana: Produce::Fixed(mana),
            times,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    let [one] = mana.as_slice() else {
        return None;
    };
    (targets.is_empty() && times.is_one()).then_some(*one)
}

/// The engine-facing form of an ability, if the engine has one: only the plain mana ability so far.
///
/// Every primitive the engine's `Caps` lists must be carried here, or a `Verified` card would lose
/// the ability on its way into the game.
pub fn lower(ability: &Ability) -> Option<AbilityDef> {
    plain_mana_ability(ability).map(|produces| AbilityDef::Mana(ManaAbility { produces }))
}

/// The mana abilities the basic land types of a land give.
fn intrinsic_abilities(definition: &CardDefinition) -> Vec<AbilityDef> {
    if !definition.is_type(CardType::Land) {
        return Vec::new();
    }
    BASIC_LAND_TYPES
        .iter()
        .filter(|(name, _)| definition.subtypes.iter().any(|subtype| subtype.0 == *name))
        .map(|&(_, produces)| AbilityDef::Mana(ManaAbility { produces }))
        .collect()
}

/// The clauses for printed characteristics the engine cannot play with, on the front face and on
/// every face the engine does not play.
pub(crate) fn printed_gaps(printed: &PrintedCard, caps: &Caps) -> Vec<Clause> {
    let mut clauses = Vec::new();
    if let Some(front) = printed.faces.first() {
        for gap in read_face(front).gaps {
            if gap.needs.is_none_or(|name| !caps.runs(name)) {
                clauses.push(Clause {
                    face: 0,
                    span: whole_text(&front.oracle_text),
                    reason: gap.reason,
                });
            }
        }
    }
    if !caps.runs(FACES) {
        for (face, shown) in printed.faces.iter().enumerate().skip(1) {
            clauses.push(Clause {
                face,
                span: whole_text(&shown.oracle_text),
                reason: format!(
                    "the engine plays only the front face of a card, so {} cannot be cast or turned over",
                    shown.name
                ),
            });
        }
    }
    clauses
}

/// A printed characteristic the definition cannot hold or the engine cannot play with.
struct Gap {
    /// The capability that lifts the gap once the engine has it; `None` when the definition cannot hold the value.
    needs: Option<&'static str>,
    reason: String,
}

/// What a face's strings say, read once for the definition and for the gaps.
struct Read {
    cost: Option<ManaCostX>,
    types: TypeLine,
    power: Option<i32>,
    toughness: Option<i32>,
    loyalty: Option<u32>,
    defense: Option<u32>,
    gaps: Vec<Gap>,
}

/// The words of a type line.
#[derive(Default)]
struct TypeLine {
    supertypes: BTreeSet<Supertype>,
    types: BTreeSet<CardType>,
    subtypes: Vec<Subtype>,
    unknown: Vec<String>,
}

/// A printed power, toughness, loyalty or defense.
enum Stat {
    Absent,
    Number(i64),
    /// Not a number: `*`, `1+*`, `X`.
    Variable,
}

fn read_stat(text: Option<&str>) -> Stat {
    match text.map(str::trim) {
        None | Some("") => Stat::Absent,
        // Newer data may print the sign of a negative number as U+2212.
        Some(text) => text
            .replace('\u{2212}', "-")
            .parse()
            .map_or(Stat::Variable, Stat::Number),
    }
}

fn supertype_of(word: &str) -> Option<Supertype> {
    Some(match word {
        "Basic" => Supertype::Basic,
        "Legendary" => Supertype::Legendary,
        "Snow" => Supertype::Snow,
        "World" => Supertype::World,
        _ => return None,
    })
}

fn card_type_of(word: &str) -> Option<CardType> {
    Some(match word {
        "Artifact" => CardType::Artifact,
        "Battle" => CardType::Battle,
        "Creature" => CardType::Creature,
        "Enchantment" => CardType::Enchantment,
        "Instant" => CardType::Instant,
        // The old name of the type, kept by cards printed before the rename.
        "Kindred" | "Tribal" => CardType::Kindred,
        "Land" => CardType::Land,
        "Planeswalker" => CardType::Planeswalker,
        "Sorcery" => CardType::Sorcery,
        _ => return None,
    })
}

fn read_type_line(line: &str) -> TypeLine {
    let (head, tail) = line.split_once('\u{2014}').unwrap_or((line, ""));
    let mut read = TypeLine::default();
    for word in head.split_whitespace() {
        if let Some(supertype) = supertype_of(word) {
            read.supertypes.insert(supertype);
        } else if let Some(card_type) = card_type_of(word) {
            read.types.insert(card_type);
        } else {
            read.unknown.push(word.to_owned());
        }
    }
    let words: Vec<&str> = tail.split_whitespace().collect();
    let mut at = 0;
    while let Some(&word) = words.get(at) {
        // The one creature type of two words (CR 205.3m).
        if word == "Time" && words.get(at + 1) == Some(&"Lord") {
            read.subtypes.push(Subtype("Time Lord".to_owned()));
            at += 2;
        } else {
            read.subtypes.push(Subtype(word.to_owned()));
            at += 1;
        }
    }
    read
}

/// The Scryfall letters of a face as a color set; letters that are no color are skipped.
fn letters(colors: &[String]) -> ColorSet {
    colors
        .iter()
        .filter_map(|text| text.chars().next())
        .filter_map(Color::from_symbol)
        .collect()
}

fn read_face(face: &PrintedFace) -> Read {
    let mut gaps = Vec::new();
    let cost = if face.mana_cost.trim().is_empty() {
        None
    } else {
        match face.mana_cost.parse::<ManaCostX>() {
            Ok(cost) => Some(cost),
            Err(error) => {
                gaps.push(Gap {
                    needs: None,
                    reason: match error {
                        ParseManaCostError::Unsupported(symbol) => format!(
                            "the mana cost has the symbol {{{symbol}}}, which the engine does not know yet"
                        ),
                        ParseManaCostError::Malformed(_) => {
                            format!("the mana cost {:?} cannot be read", face.mana_cost)
                        }
                    },
                });
                None
            }
        }
    };
    if cost.as_ref().is_some_and(ManaCostX::has_x) {
        gaps.push(Gap {
            needs: Some(X_COST),
            reason: "the engine cannot pay {X} in a mana cost yet".to_owned(),
        });
    }
    let types = read_type_line(&face.type_line);
    if !types.unknown.is_empty() {
        gaps.push(Gap {
            needs: None,
            reason: format!(
                "the type line has words the engine does not know: {}",
                types.unknown.join(", ")
            ),
        });
    }
    let mut variable = Vec::new();
    let mut number = |name: &'static str, text: Option<&str>| -> Option<i64> {
        match read_stat(text) {
            Stat::Absent => None,
            Stat::Number(value) => Some(value),
            Stat::Variable => {
                variable.push(name);
                None
            }
        }
    };
    let power = number("power", face.power.as_deref()).and_then(|n| i32::try_from(n).ok());
    let toughness =
        number("toughness", face.toughness.as_deref()).and_then(|n| i32::try_from(n).ok());
    let loyalty = number("loyalty", face.loyalty.as_deref()).and_then(|n| u32::try_from(n).ok());
    let defense = number("defense", face.defense.as_deref()).and_then(|n| u32::try_from(n).ok());
    if !variable.is_empty() {
        let (verb, what, source) = if variable.len() == 1 {
            ("is", "a number", "it comes")
        } else {
            ("are", "numbers", "they come")
        };
        gaps.push(Gap {
            needs: Some(VARIABLE_STATS),
            reason: format!(
                "the printed {} {verb} not {what}, so {source} from an ability the engine does not run yet",
                variable.join(" and "),
            ),
        });
    }
    Read {
        cost,
        types,
        power,
        toughness,
        loyalty,
        defense,
        gaps,
    }
}

/// The printed characteristics of a face; its colors come from the cost unless the record gives
/// other ones, which only a color indicator does.
fn printed_side(face: &PrintedFace) -> CardDefinition {
    let read = read_face(face);
    let (mana_cost, x_in_cost) = read
        .cost
        .as_ref()
        .map_or((None, 0), |cost| (Some(cost.rest().clone()), cost.x()));
    let from_cost = mana_cost.as_ref().map(ManaCost::colors).unwrap_or_default();
    // An empty list is no evidence: records of split and adventure faces often leave colors out.
    let shown = letters(&face.colors);
    CardDefinition {
        name: face.name.clone(),
        mana_cost,
        x_in_cost,
        supertypes: read.types.supertypes,
        types: read.types.types,
        subtypes: read.types.subtypes,
        power: read.power,
        toughness: read.toughness,
        loyalty: read.loyalty,
        defense: read.defense,
        color_indicator: (!shown.is_empty() && shown != from_cost).then_some(shown),
        abilities: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the type line, the stats and the cost of a face read into a definition, the plain mana ability is the one shape the engine carries, and characteristics the definition cannot hold make the card `Manual`.
    use super::*;
    use rstest::rstest;

    use crate::card::{Face, ImplStatus};
    use crate::support::oracle_fingerprint;

    fn face(name: &str, cost: &str, type_line: &str) -> PrintedFace {
        PrintedFace {
            name: name.into(),
            mana_cost: cost.into(),
            type_line: type_line.into(),
            ..PrintedFace::default()
        }
    }

    fn card(front: PrintedFace) -> PrintedCard {
        PrintedCard {
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: front.name.clone(),
            keywords: vec![],
            faces: vec![front],
        }
    }

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    #[rstest]
    #[case::creature("Creature \u{2014} Elf Druid", &[], &["Creature"], &["Elf", "Druid"])]
    #[case::legendary("Legendary Creature \u{2014} Beast", &["Legendary"], &["Creature"], &["Beast"])]
    #[case::basic_land("Basic Land \u{2014} Forest", &["Basic"], &["Land"], &["Forest"])]
    #[case::several_types("Artifact Creature \u{2014} Golem", &[], &["Artifact", "Creature"], &["Golem"])]
    #[case::no_subtypes("Instant", &[], &["Instant"], &[])]
    #[case::snow("Snow Land", &["Snow"], &["Land"], &[])]
    #[case::old_tribal("Tribal Instant \u{2014} Goblin", &[], &["Kindred", "Instant"], &["Goblin"])]
    #[case::two_word_type("Legendary Creature \u{2014} Time Lord Doctor", &["Legendary"], &["Creature"], &["Time Lord", "Doctor"])]
    #[case::empty("", &[], &[], &[])]
    fn type_lines_read_into_supertypes_types_and_subtypes(
        #[case] line: &str,
        #[case] supertypes: &[&str],
        #[case] types: &[&str],
        #[case] subtypes: &[&str],
    ) {
        let read = read_type_line(line);
        let names = |set: Vec<String>| -> BTreeSet<String> { set.into_iter().collect() };
        let expected = |words: &[&str]| -> BTreeSet<String> {
            words.iter().map(|w| (*w).to_owned()).collect()
        };
        assert_eq!(
            names(read.supertypes.iter().map(|s| format!("{s:?}")).collect()),
            expected(supertypes)
        );
        assert_eq!(
            names(read.types.iter().map(|t| format!("{t:?}")).collect()),
            expected(types)
        );
        let subtypes_read: Vec<&str> = read.subtypes.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(subtypes_read, subtypes);
        assert!(read.unknown.is_empty(), "{line}");
    }

    #[test]
    fn a_word_of_no_known_kind_is_kept_apart() {
        assert_eq!(
            read_type_line("Plane \u{2014} Dominaria").unknown,
            ["Plane"]
        );
    }

    #[rstest]
    #[case::none(None, "absent")]
    #[case::blank(Some(" "), "absent")]
    #[case::number(Some("3"), "3")]
    #[case::zero(Some("0"), "0")]
    #[case::negative(Some("-1"), "-1")]
    #[case::unicode_minus(Some("\u{2212}2"), "-2")]
    #[case::star(Some("*"), "variable")]
    #[case::star_plus(Some("1+*"), "variable")]
    #[case::x(Some("X"), "variable")]
    fn stats_are_numbers_or_variable(#[case] text: Option<&str>, #[case] expected: &str) {
        let shown = match read_stat(text) {
            Stat::Absent => "absent".to_owned(),
            Stat::Number(n) => n.to_string(),
            Stat::Variable => "variable".to_owned(),
        };
        assert_eq!(shown, expected);
    }

    #[test]
    fn a_face_reads_into_the_definition_it_prints() {
        let mut front = face(
            "Jace, the Mind Sculptor",
            "{2}{U}{U}",
            "Legendary Planeswalker \u{2014} Jace",
        );
        front.loyalty = Some("3".into());
        let shown = assemble(&card(front), None, &Caps::none()).definition;
        assert_eq!(shown.name, "Jace, the Mind Sculptor");
        assert_eq!(shown.mana_cost, Some(ManaCost::parse("{2}{U}{U}").unwrap()));
        assert_eq!(
            (shown.loyalty, shown.defense, shown.x_in_cost),
            (Some(3), None, 0)
        );
        assert!(shown.supertypes.contains(&Supertype::Legendary));
        assert!(shown.is_type(CardType::Planeswalker));
        assert_eq!(shown.subtypes, [Subtype("Jace".into())]);
        assert_eq!((shown.power, shown.toughness), (None, None));
    }

    #[rstest]
    #[case::no_cost("", None, 0)]
    #[case::free("{0}", Some("{0}"), 0)]
    #[case::plain("{2}{R}", Some("{2}{R}"), 0)]
    #[case::x("{X}{R}", Some("{R}"), 1)]
    #[case::two_x("{X}{X}", Some(""), 2)]
    #[case::hybrid("{R/W}{R/W}", Some("{R/W}{R/W}"), 0)]
    fn a_cost_keeps_its_x_beside_the_symbols_the_engine_pays(
        #[case] text: &str,
        #[case] paid: Option<&str>,
        #[case] x: u8,
    ) {
        let shown = assemble(&card(face("C", text, "Sorcery")), None, &Caps::none()).definition;
        assert_eq!(shown.mana_cost, paid.map(|p| ManaCost::parse(p).unwrap()));
        assert_eq!(shown.x_in_cost, x);
    }

    #[rstest]
    #[case::the_cost_says_it("{1}{R}", &["R"], false)]
    #[case::no_evidence("{1}{R}", &[], false)]
    #[case::indicator_without_a_cost("", &["U"], true)]
    #[case::indicator_over_a_cost("{4}", &["W", "U", "B", "R", "G"], true)]
    #[case::two_colors_in_either_order("{U}{R}", &["R", "U"], false)]
    fn a_color_indicator_is_kept_only_where_the_cost_does_not_say_the_colors(
        #[case] cost: &str,
        #[case] colors: &[&str],
        #[case] indicator: bool,
    ) {
        let mut front = face("C", cost, "Creature \u{2014} Elf");
        front.colors = colors.iter().map(|c| (*c).to_owned()).collect();
        let shown = assemble(&card(front.clone()), None, &Caps::none()).definition;
        assert_eq!(shown.color_indicator.is_some(), indicator);
        if !colors.is_empty() {
            assert_eq!(shown.colors(), letters(&front.colors));
        }
    }

    #[rstest]
    #[case::forest("Basic Land \u{2014} Forest", &[ManaType::Green])]
    #[case::plains("Basic Land \u{2014} Plains", &[ManaType::White])]
    #[case::dual("Land \u{2014} Mountain Forest", &[ManaType::Red, ManaType::Green])]
    #[case::listed_in_the_order_of_the_rules("Land \u{2014} Forest Island", &[ManaType::Blue, ManaType::Green])]
    #[case::no_land_type("Land", &[])]
    #[case::wastes("Basic Land", &[])]
    #[case::not_a_land("Creature \u{2014} Forest Dryad", &[])]
    fn land_types_give_their_mana_abilities_to_every_card_with_them(
        #[case] type_line: &str,
        #[case] mana: &[ManaType],
    ) {
        let shown = assemble(&card(face("L", "", type_line)), None, &Caps::none()).definition;
        let expected: Vec<AbilityDef> = mana
            .iter()
            .map(|&produces| AbilityDef::Mana(ManaAbility { produces }))
            .collect();
        assert_eq!(shown.abilities, expected);
    }

    #[rstest]
    #[case::llanowar(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        Some(ManaType::Green)
    )]
    #[case::colorless(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#, Some(ManaType::Colorless))]
    #[case::sol_ring_adds_two(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#, None)]
    #[case::any_color(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        None
    )]
    #[case::cradle_adds_many_times(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Type":"Creature"}]}}}]}}"#, None)]
    #[case::adds_twice(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":2}}]}}"#, None)]
    #[case::for_an_opponent(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"who":"Opponents","mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::costs_more(r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::costs_mana(
        r#"{"Activated":{"cost":"{1}","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        None
    )]
    #[case::at_sorcery_speed(r#"{"Activated":{"cost":["Tap"],"timing":"Sorcery","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::once_a_turn(r#"{"Activated":{"cost":["Tap"],"limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::from_the_graveyard(r#"{"Activated":{"cost":["Tap"],"from":"Graveyard","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::and_draws(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}},{"Draw":{}}]}}"#, None)]
    #[case::not_activated(r#"{"Keyword":"Flying"}"#, None)]
    fn only_a_plain_tap_for_one_mana_is_a_mana_ability_the_engine_carries(
        #[case] json: &str,
        #[case] expected: Option<ManaType>,
    ) {
        let parsed = ability(json);
        assert_eq!(plain_mana_ability(&parsed), expected);
        assert_eq!(
            lower(&parsed),
            expected.map(|produces| AbilityDef::Mana(ManaAbility { produces }))
        );
        let used = used_primitives(&parsed);
        assert_eq!(used.contains(PLAIN_MANA_ABILITY), expected.is_some());
        if expected.is_some() {
            assert_eq!(used.len(), 1, "{used:?}");
        }
    }

    const MANA_TEXT: &str = "{T}: Add {G}.";
    const MANA_ABILITY: &str =
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#;

    fn elf_rules(status: ImplStatus, abilities: Vec<Ability>) -> Card {
        Card {
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: "Elf".into(),
            impl_status: status,
            faces: vec![Face {
                name: "Elf".into(),
                oracle_fingerprint: oracle_fingerprint(MANA_TEXT),
                abilities,
            }],
            notes: None,
        }
    }

    fn elf() -> PrintedCard {
        let mut front = face("Elf", "{G}", "Creature \u{2014} Elf Druid");
        front.oracle_text = MANA_TEXT.into();
        front.power = Some("1".into());
        front.toughness = Some("1".into());
        card(front)
    }

    #[rstest]
    #[case::the_engine_runs_it(Caps::from_names([PLAIN_MANA_ABILITY]), Support::Verified, 1)]
    #[case::the_engine_runs_everything(Caps::everything(), Support::Verified, 1)]
    #[case::the_engine_runs_nothing(Caps::none(), Support::Manual, 0)]
    fn rules_are_attached_only_where_the_support_says_the_engine_runs_them(
        #[case] caps: Caps,
        #[case] expected: Support,
        #[case] attached: usize,
    ) {
        let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        let assembled = assemble(&elf(), Some(&rules), &caps);
        assert_eq!(assembled.support.support, expected);
        assert_eq!(assembled.definition.abilities.len(), attached);
        assert_eq!(
            assembled.definition.power,
            Some(1),
            "the printed side is always there"
        );
    }

    #[test]
    fn rules_without_a_current_text_are_not_attached() {
        let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        let mut errata = elf();
        errata.faces[0].oracle_text = "{T}: Add {R}.".into();
        let assembled = assemble(&errata, Some(&rules), &Caps::everything());
        assert_eq!(assembled.support.support, Support::Stale);
        assert!(
            assembled.definition.abilities.is_empty(),
            "no rules for a Manual card"
        );
        let no_file = assemble(&elf(), None, &Caps::everything());
        assert_eq!(no_file.support.support, Support::Manual);
        assert!(no_file.definition.abilities.is_empty(), "no file, no rules");
    }

    #[test]
    fn a_partial_card_carries_what_the_engine_runs_and_leaves_the_clauses() {
        let clause = Ability::Manual {
            reason: "needs a rule the data lacks".into(),
            span: crate::card::Span { start: 0, end: 4 },
        };
        let rules = elf_rules(ImplStatus::Partial, vec![ability(MANA_ABILITY), clause]);
        let caps = Caps::from_names([PLAIN_MANA_ABILITY]);
        let assembled = assemble(&elf(), Some(&rules), &caps);
        assert_eq!(assembled.support.support, Support::Partial);
        assert_eq!(assembled.support.clauses.len(), 1);
        assert_eq!(assembled.definition.abilities.len(), 1);
    }

    #[rstest]
    #[case::star_power(Some("*"), Some("2"), "power is not a number")]
    #[case::star_toughness(Some("2"), Some("1+*"), "toughness is not a number")]
    #[case::both(Some("*"), Some("*"), "power and toughness are not numbers")]
    fn a_stat_that_is_not_a_number_makes_the_card_manual(
        #[case] power: Option<&str>,
        #[case] toughness: Option<&str>,
        #[case] reason: &str,
    ) {
        let mut shown = elf();
        shown.faces[0].power = power.map(str::to_owned);
        shown.faces[0].toughness = toughness.map(str::to_owned);
        let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        let caps = Caps::from_names([PLAIN_MANA_ABILITY]);
        let assembled = assemble(&shown, Some(&rules), &caps);
        assert_eq!(assembled.support.support, Support::Manual);
        assert_eq!(assembled.support.clauses.len(), 1);
        assert!(
            assembled.support.clauses[0].reason.contains(reason),
            "{:?}",
            assembled.support.clauses
        );
        assert!(
            assembled.definition.abilities.is_empty(),
            "no rules for a Manual card"
        );
        // Once the engine knows variable stats the gap is gone.
        let lifted = Caps::from_names([PLAIN_MANA_ABILITY, VARIABLE_STATS]);
        assert_eq!(
            assemble(&shown, Some(&rules), &lifted).support.support,
            Support::Verified
        );
    }

    #[rstest]
    #[case::x_in_the_cost("{X}{G}", "Creature \u{2014} Elf", X_COST, "{X}")]
    #[case::snow_in_the_cost("{1}{S}", "Creature \u{2014} Elf", "", "{S}")]
    #[case::garbage_cost("1G", "Creature \u{2014} Elf", "", "cannot be read")]
    #[case::a_type_of_no_game_here("{G}", "Plane \u{2014} Dominaria", "", "Plane")]
    fn what_the_definition_cannot_hold_or_play_makes_the_card_manual(
        #[case] cost: &str,
        #[case] type_line: &str,
        #[case] lifted_by: &'static str,
        #[case] reason: &str,
    ) {
        let mut shown = elf();
        shown.faces[0].mana_cost = cost.into();
        shown.faces[0].type_line = type_line.into();
        let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        let caps = Caps::from_names([PLAIN_MANA_ABILITY]);
        let assembled = assemble(&shown, Some(&rules), &caps);
        assert_eq!(assembled.support.support, Support::Manual);
        assert!(
            assembled
                .support
                .clauses
                .iter()
                .any(|c| c.reason.contains(reason)),
            "{:?}",
            assembled.support.clauses
        );
        assert!(
            assembled.definition.abilities.is_empty(),
            "no rules for a Manual card"
        );
        let can_be_lifted = !lifted_by.is_empty();
        let lifted = Caps::from_names([PLAIN_MANA_ABILITY, lifted_by]);
        let state = assemble(&shown, Some(&rules), &lifted).support.support;
        assert_eq!(state == Support::Verified, can_be_lifted, "{state:?}");
    }

    #[test]
    fn faces_the_engine_does_not_play_make_the_card_manual_and_each_gets_a_clause() {
        let mut shown = elf();
        shown
            .faces
            .push(face("Back", "", "Creature \u{2014} Elf Wolf"));
        shown
            .faces
            .push(face("Third", "", "Creature \u{2014} Elf Bear"));
        let mut rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        for name in ["Back", "Third"] {
            rules.faces.push(Face {
                name: name.into(),
                oracle_fingerprint: oracle_fingerprint(""),
                abilities: vec![],
            });
        }
        let caps = Caps::from_names([PLAIN_MANA_ABILITY]);
        let assembled = assemble(&shown, Some(&rules), &caps);
        assert_eq!(assembled.support.support, Support::Manual);
        let faces: Vec<usize> = assembled.support.clauses.iter().map(|c| c.face).collect();
        assert_eq!(faces, [1, 2]);
        assert_eq!(
            assembled.definition.name, "Elf",
            "the front face is the card"
        );
        assert!(
            assembled.definition.abilities.is_empty(),
            "no rules for a Manual card"
        );
        let lifted = Caps::from_names([PLAIN_MANA_ABILITY, FACES]);
        let state = assemble(&shown, Some(&rules), &lifted);
        assert_eq!(state.support.support, Support::Verified);
        assert_eq!(state.definition.abilities.len(), 1);
    }

    #[test]
    fn a_record_without_faces_still_makes_a_named_definition() {
        let bare = PrintedCard {
            oracle_id: "x".into(),
            name: "Nothing".into(),
            ..PrintedCard::default()
        };
        let assembled = assemble(&bare, None, &Caps::none());
        assert_eq!(assembled.definition, CardDefinition::new("Nothing"));
        assert_eq!(assembled.support.support, Support::Manual);
    }

    mod generated {
        //! Goal: over generated faces, a card with rules the engine runs is Verified exactly when nothing printed on it is out of reach, and the definition keeps the X of a readable cost and the numbers of the stats.
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn verified_exactly_when_nothing_printed_is_out_of_reach(
                symbols in prop::collection::vec(
                    prop::sample::select(vec!["{1}", "{G}", "{W/U}", "{2/R}", "{G/P}", "{C}", "{0}"]),
                    0..5,
                ),
                x in 0usize..3,
                unknown_symbol in prop::option::of(prop::sample::select(vec!["{S}", "{Z}", "{HW}"])),
                power in prop::sample::select(vec![None, Some("0"), Some("7"), Some("-1"), Some("*"), Some("1+*")]),
                toughness in prop::sample::select(vec![None, Some("1"), Some("12"), Some("X")]),
                type_word in prop::sample::select(vec!["Creature", "Artifact", "Land", "Plane", "Scheme"]),
            ) {
                let cost = format!("{}{}{}", "{X}".repeat(x), symbols.concat(), unknown_symbol.unwrap_or(""));
                let mut shown = elf();
                shown.faces[0].mana_cost = cost;
                shown.faces[0].type_line = format!("{type_word} \u{2014} Elf");
                shown.faces[0].power = power.map(str::to_owned);
                shown.faces[0].toughness = toughness.map(str::to_owned);
                let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
                let caps = Caps::from_names([PLAIN_MANA_ABILITY]);

                let assembled = assemble(&shown, Some(&rules), &caps);

                let reachable = x == 0
                    && unknown_symbol.is_none()
                    && matches!(type_word, "Creature" | "Artifact" | "Land")
                    && matches!(power, None | Some("0" | "7" | "-1"))
                    && matches!(toughness, None | Some("1" | "12"));
                prop_assert_eq!(assembled.support.support == Support::Verified, reachable);
                prop_assert_eq!(assembled.definition.abilities.is_empty(), !reachable);
                if unknown_symbol.is_none() {
                    prop_assert_eq!(usize::from(assembled.definition.x_in_cost), x);
                    prop_assert_eq!(assembled.definition.mana_cost.is_some(), x > 0 || !symbols.is_empty());
                } else {
                    prop_assert_eq!(assembled.definition.mana_cost, None);
                }
                prop_assert_eq!(assembled.definition.power, power.and_then(|p| p.parse().ok()));
                prop_assert_eq!(assembled.definition.toughness, toughness.and_then(|t| t.parse().ok()));
            }

            #[test]
            fn a_number_reads_back_as_the_number(n in -100_000i64..100_000) {
                let text = n.to_string();
                prop_assert!(matches!(read_stat(Some(&text)), Stat::Number(m) if m == n));
            }
        }
    }
}
