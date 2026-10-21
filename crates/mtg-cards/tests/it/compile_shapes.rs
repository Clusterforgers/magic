//! Goal: over many wordings and generated texts, the compiler's output loads, matches the imported text, and keeps its clauses, abilities and status consistent.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::card::ImplStatus;
use mtg_cards::compile::ClauseKind::{
    self, Activated, Keyword, Loyalty, Modal, Spell, Static, Triggered,
};
use mtg_cards::compile::{Compiled, compile};
use mtg_cards::load::{Library, Source, canonical, slug};
use mtg_cards::support::{PrintedCard, PrintedFace, has_rules_text};
use rstest::rstest;

const ID: &str = "4457ed35-7c10-48c8-9776-456485fdf070";

fn face(name: &str, type_line: &str, text: &str) -> PrintedFace {
    PrintedFace {
        name: name.into(),
        type_line: type_line.into(),
        oracle_text: text.into(),
        ..PrintedFace::default()
    }
}

fn card_of(faces: Vec<PrintedFace>) -> PrintedCard {
    let name = faces
        .iter()
        .map(|face| face.name.as_str())
        .collect::<Vec<_>>()
        .join(" // ");
    PrintedCard {
        oracle_id: ID.into(),
        name,
        keywords: vec![],
        faces,
    }
}

/// Compiles the card and checks what the card files require of the result.
fn check(printed: &PrintedCard) -> Compiled {
    let compiled = compile(printed);
    let live = printed
        .faces
        .iter()
        .any(|face| has_rules_text(&face.oracle_text));
    assert_eq!(compiled.card.is_some(), live, "{printed:?}");
    for (at, clause) in compiled.clauses.iter().enumerate() {
        let text = &printed.faces[clause.face].oracle_text;
        assert!(clause.span.slice(text).is_some(), "{clause:?} in {text:?}");
        assert!(
            compiled.clauses[..at]
                .iter()
                .filter(|earlier| earlier.face == clause.face)
                .all(|earlier| !earlier.span.overlaps(&clause.span)),
            "{clause:?} overlaps an earlier clause of {text:?}"
        );
    }
    let Some(card) = &compiled.card else {
        return compiled;
    };
    let read = compiled.clauses.iter().filter(|clause| clause.read).count();
    let unread = compiled.clauses.len() - read;
    let read_abilities = card
        .abilities()
        .filter(|(_, ability)| !ability.is_manual())
        .count();
    let manual = card
        .abilities()
        .filter(|(_, ability)| ability.is_manual())
        .count();
    match card.impl_status {
        ImplStatus::Full => assert!(unread == 0 && read > 0, "{compiled:?}"),
        ImplStatus::Partial => assert!(read > 0 && unread > 0, "{compiled:?}"),
        ImplStatus::Manual => assert_eq!((read, read_abilities, manual), (0, 0, 0), "{compiled:?}"),
        ImplStatus::Unplayable => panic!("the compiler never gives a card up: {compiled:?}"),
    }
    if card.impl_status != ImplStatus::Manual {
        // The spell clauses of a face read into one ability, the others one each.
        assert_eq!(manual, unread, "{compiled:?}");
        assert!(read_abilities <= read, "{compiled:?}");
    }
    let source = Source {
        path: format!("cards/{}.json", slug(&card.name)),
        text: canonical(card),
    };
    let library = Library::from_sources(&[source]).unwrap_or_else(|errors| panic!("{errors}"));
    let problems = library.check_printed(std::slice::from_ref(printed));
    assert!(problems.is_empty(), "{problems:?} for {printed:?}");
    compiled
}

#[rstest]
#[case::quoted_mana_ability("Enchanted land has \"{T}: Add {G}.\"", "Enchantment \u{2014} Aura", &[Static])]
#[case::quote_then_keyword("Creatures you control have \"{T}: Add one mana of any color.\"\nFlying", "Enchantment", &[Static, Keyword])]
#[case::quoted_trigger("Creatures you control have \"Whenever this creature attacks, draw a card.\"", "Enchantment", &[Static])]
#[case::quoted_keyword_list("Creatures you control have \"Flying, haste\"", "Enchantment", &[Static])]
#[case::ability_word_trigger("Landfall \u{2014} Whenever a land you control enters, draw a card.\nFlying", "Creature \u{2014} Elf", &[Triggered, Keyword])]
#[case::ability_word_activated("Boast \u{2014} {1}: Draw a card.", "Creature \u{2014} Elf", &[Activated])]
#[case::ability_word_static("Threshold \u{2014} As long as seven cards are in your graveyard, ~ gets +1/+1.", "Creature \u{2014} Elf", &[Static])]
#[case::modal_spell("Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} Gain 2 life.", "Instant", &[Modal])]
#[case::modal_after_a_keyword("Kicker {2}\nChoose one \u{2014}\n\u{2022} A.\n\u{2022} B.", "Sorcery", &[Keyword, Modal])]
#[case::modal_trigger("Whenever ~ attacks, choose one \u{2014}\n\u{2022} A.\n\u{2022} B.\nFlying", "Creature \u{2014} Elf", &[Triggered, Keyword])]
#[case::spree("Spree\n+ {1} \u{2014} Draw a card.\n+ {2} \u{2014} Gain 2 life.", "Instant", &[Modal])]
#[case::loyalty_with_minus_x("+1: Draw a card.\n\u{2212}X: ~ deals X damage.\n0: Scry 1.\n\u{2212}12: Win.", "Planeswalker \u{2014} Test", &[Loyalty, Loyalty, Loyalty, Loyalty])]
#[case::level_up("Level up {1}{U}\nLEVEL 1-2\n1/1\nFlying\nLEVEL 3+\n2/2\n{T}: Draw a card.", "Creature \u{2014} Wizard", &[Keyword, Static, Static])]
#[case::saga("I \u{2014} Draw a card.\nII, III \u{2014} Gain 2 life.", "Enchantment \u{2014} Saga", &[Triggered, Triggered])]
#[case::spell_text("Draw two cards.\nDiscard a card.", "Sorcery", &[Spell, Spell])]
#[case::spell_cost_rule("This spell costs {1} less to cast.\nDraw two cards.", "Sorcery", &[Static, Spell])]
#[case::same_text_on_a_permanent("Draw two cards.", "Enchantment", &[Static])]
#[case::keyword_list("Flying, first strike, vigilance", "Creature \u{2014} Angel", &[Keyword, Keyword, Keyword])]
#[case::keyword_as_a_subject("Flying creatures you control get +1/+1.", "Enchantment", &[Static])]
#[case::reminder_line_between("Flying\n(This is a reminder.)\nHaste", "Creature \u{2014} Elf", &[Keyword, Keyword])]
#[case::colon_after_prose("Draw a card: this is not a cost.", "Enchantment", &[Static])]
#[case::cost_words("Sacrifice a creature: Draw a card.", "Enchantment", &[Activated])]
fn clauses_have_the_kind_their_shape_says(
    #[case] text: &str,
    #[case] type_line: &str,
    #[case] expected: &[ClauseKind],
) {
    let compiled = check(&card_of(vec![face("Test Card", type_line, text)]));
    let kinds: Vec<_> = compiled.clauses.iter().map(|clause| clause.kind).collect();
    assert_eq!(kinds, expected, "{text:?}");
}

#[rstest]
#[case::aura("Enchantment \u{2014} Aura", ImplStatus::Full)]
#[case::not_an_aura("Enchantment", ImplStatus::Partial)]
#[case::creature_aura("Enchantment Creature \u{2014} Aura Spirit", ImplStatus::Full)]
fn enchant_is_read_on_an_aura_only(#[case] type_line: &str, #[case] expected: ImplStatus) {
    let text = "Enchant creature\nEnchanted creature can't attack or block.";
    let compiled = check(&card_of(vec![face("Test Card", type_line, text)]));
    assert_eq!(compiled.card.unwrap().impl_status, expected);
}

#[rstest]
#[case::flash_on_an_instant("Instant", "Flash")]
#[case::keywords_on_a_sorcery("Sorcery", "Cascade\nStorm")]
#[case::keyword_list_on_an_instant("Instant", "Convoke, delve")]
fn a_spell_of_only_keywords_is_left_to_the_table(#[case] type_line: &str, #[case] text: &str) {
    let compiled = check(&card_of(vec![face("Test Card", type_line, text)]));
    let card = compiled.card.unwrap();
    assert_eq!(card.impl_status, ImplStatus::Manual);
    assert!(compiled.clauses.iter().all(|clause| !clause.read));
}

#[rstest]
#[case::five_faces(&["A", "B", "C", "D", "E"])]
#[case::repeated_name(&["Soldier", "Soldier"])]
fn a_card_the_data_cannot_hold_lists_its_clauses_but_has_no_rules(#[case] names: &[&str]) {
    let faces = names
        .iter()
        .map(|name| face(name, "Creature \u{2014} Elf", "Flying"))
        .collect();
    let compiled = compile(&card_of(faces));
    assert_eq!(compiled.card, None);
    assert_eq!(compiled.clauses.len(), names.len());
}

#[test]
fn a_partner_card_names_a_card_that_must_exist() {
    let text = "Partner with Pir, Imaginative Rascal (When this creature enters, target player may search.)\nHaste";
    let toothy = card_of(vec![face(
        "Toothy, Imaginary Friend",
        "Legendary Creature \u{2014} Illusion",
        text,
    )]);
    let card = compile(&toothy).card.unwrap();
    assert_eq!(card.impl_status, ImplStatus::Full);
    let source = Source {
        path: format!("cards/{}.json", slug(&card.name)),
        text: canonical(&card),
    };
    let library = Library::from_sources(&[source]).unwrap();
    let mut pir = card_of(vec![face(
        "Pir, Imaginative Rascal",
        "Legendary Creature \u{2014} Human",
        "",
    )]);
    pir.oracle_id = "5f5e0b10-c8cf-450c-bfd3-bcb0528ec330".into();
    let alone = library.check_printed(std::slice::from_ref(&toothy));
    assert_eq!(alone.len(), 1, "{alone:?}");
    assert!(
        alone[0].message.contains("Pir, Imaginative Rascal"),
        "{alone:?}"
    );
    let together = library.check_printed(&[toothy, pir]);
    assert!(together.is_empty(), "{together:?}");
}

mod generated {
    //! Goal: over generated oracle texts of one or two faces, the compiled card always loads and matches its imported text, with consistent clauses, abilities and status.
    use super::*;
    use proptest::prelude::*;

    const LINES: &[&str] = &[
        "Flying",
        "Vigilance, deathtouch, haste",
        "First strike, lifelink",
        "Haste (This creature can attack as soon as it comes under your control.)",
        "Equip {1}",
        "Kicker {4} (You may pay an additional {4}.)",
        "Ward {2}",
        "Ward\u{2014}Pay 3 life.",
        "Cycling {2}",
        "Flashback {2}{R}",
        "Suspend 4\u{2014}{U}",
        "Foretell {1}{U} (During your turn, you may pay {2} and exile this card from your hand face down. Cast it on a later turn for its foretell cost.)",
        "Enchant creature",
        "Undying",
        "Level up {1}{U}",
        "Companion \u{2014} Each nonland card in your starting deck has a different name.",
        "When ~ enters, draw a card.",
        "Whenever Tess attacks, you gain 1 life. (Reminder.)",
        "At the beginning of your upkeep, scry 1.",
        "Landfall \u{2014} Whenever a land you control enters, draw a card.",
        "{T}: Add {G}.",
        "{2}, {T}, Sacrifice Tess, Test Card: Draw a card.",
        "Enchanted land has \"{T}: Add {G}.\"",
        "+2: Look at the top card.",
        "\u{2212}X: Tess deals X damage to target creature.",
        "0: Draw a card.",
        "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} Gain 2 life.",
        "Spree\n+ {1} \u{2014} Draw a card.\n+ {2} \u{2014} Gain 2 life.",
        "LEVEL 2-3\n2/4\n{U}{U}, {T}: Draw a card.",
        "Equipped creature gets +2/+0.",
        "Destroy target creature.",
        "Tess, Test Card deals 2 damage to any target.",
        "This spell costs {1} less to cast.",
        "I \u{2014} Draw a card.",
        "(Only reminder text.)",
        "(An unclosed reminder",
        "",
        "   ",
        "Flying (",
        "Partner",
        "Fuse",
        "Multikicker {1}{G}",
    ];

    const TYPE_LINES: [&str; 6] = [
        "Creature \u{2014} Elf",
        "Instant",
        "Sorcery",
        "Enchantment \u{2014} Aura",
        "Legendary Creature \u{2014} Elf",
        "Legendary Planeswalker \u{2014} Tess",
    ];

    fn text() -> impl Strategy<Value = String> {
        prop::collection::vec(prop::sample::select(LINES.to_vec()), 0..7)
            .prop_map(|lines| lines.join("\n"))
    }

    fn type_line() -> impl Strategy<Value = &'static str> {
        prop::sample::select(TYPE_LINES.to_vec())
    }

    proptest! {
        #[test]
        fn single_faced_cards_compile_to_valid_consistent_rules(
            text in text(),
            type_line in type_line(),
        ) {
            check(&card_of(vec![face("Tess, Test Card", type_line, &text)]));
        }

        #[test]
        fn double_faced_cards_compile_to_valid_consistent_rules(
            front in text(),
            back in text(),
            front_type in type_line(),
            back_type in type_line(),
        ) {
            check(&card_of(vec![
                face("Front Face", front_type, &front),
                face("Back Face", back_type, &back),
            ]));
        }
    }
}
