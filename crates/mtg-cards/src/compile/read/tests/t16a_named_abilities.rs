//! Goal: a line of the form "<Flavour name> — <ability>" reads as the ability after the name whatever the name is (a name with the card's own name, a comma, a question mark or a hyphen included) and a clause or a keyword line that only looks like one stays unread; "enters from a graveyard" is the event of a card put onto the battlefield from a graveyard.
use super::*;
use crate::card::ImplStatus;
use crate::compile::classify;
use crate::support::{PrintedCard, PrintedFace};

const DASH: &str = "\u{2014}";

#[rstest]
#[case::triggered(
    "Heavy Rock Cutter",
    "Whenever this creature attacks, destroy up to one target artifact.",
    ClauseKind::Triggered
)]
#[case::triggered_with_a_hyphen(
    "Bio-Plasmic Scream",
    "When this creature enters, exile target player's graveyard.",
    ClauseKind::Triggered
)]
#[case::triggered_with_an_apostrophe(
    "Genestealer's Kiss",
    "Whenever ~ attacks, mill three cards.",
    ClauseKind::Triggered
)]
#[case::triggered_with_the_cards_own_name(
    "Blade of ~",
    "Whenever ~ attacks, mill three cards.",
    ClauseKind::Triggered
)]
#[case::triggered_with_commas(
    "Beeeeeeep, Beeeeeeep, Beeeeeeep",
    "When this creature enters, exile target player's graveyard.",
    ClauseKind::Triggered
)]
#[case::triggered_with_a_comma_and_five_words(
    "In You, All Things Are",
    "When this creature enters, exile target player's graveyard.",
    ClauseKind::Triggered
)]
#[case::triggered_with_a_question_mark(
    "Do You Like Squirrels?",
    "Whenever ~ attacks, mill three cards.",
    ClauseKind::Triggered
)]
#[case::triggered_with_an_exclamation_mark(
    "Bring it Down!",
    "Whenever ~ attacks, mill three cards.",
    ClauseKind::Triggered
)]
#[case::a_name_of_five_words(
    "A Thousand Souls Die Every Day",
    "When this creature enters, exile target player's graveyard.",
    ClauseKind::Triggered
)]
#[case::mana_ability(
    "Genomic Enhancement",
    "{T}: Add one mana of any color.",
    ClauseKind::Activated
)]
#[case::activated("Field Reprogramming", "{T}: Add {C}{C}.", ClauseKind::Activated)]
#[case::static_ability(
    "Unearthly Power",
    "Instant and sorcery spells you cast cost {1} less to cast for each creature token you control.",
    ClauseKind::Static
)]
#[case::static_ability_named_after_the_card(
    "Blade of ~",
    "Instant and sorcery spells you cast cost {1} less to cast for each creature token you control.",
    ClauseKind::Static
)]
fn a_flavour_name_is_left_out_of_the_reading(
    #[case] name: &str,
    #[case] sentence: &str,
    #[case] kind: ClauseKind,
) {
    let line = format!("{name} {DASH} {sentence}");
    assert_eq!(classify::classify(&line, false), kind, "{line}");
    let plain = golden(sentence, kind, false);
    assert_eq!(clause(&line, kind, false), Some(plain), "{line}");
}

#[rstest]
#[case::a_clause_with_a_comma(
    "At the beginning of your end step, create a 1/1 blue Tyranid Gargoyle creature token with flying"
)]
#[case::a_comma_name_with_lower_case_words("Gear Up, sponsored by Wizards of the Coast")]
#[case::a_comma_name_of_six_words("In You, All Things Are Possible Now")]
#[case::forty_one_characters("Abcdefghijklmnopqrstuvwxyzabcdefghijklmno")]
#[case::lower_case("heavy rock cutter")]
#[case::a_modal_lead("Choose one")]
#[case::a_sentence_with_a_period("I. AM. TALKING!")]
#[case::a_cost("Prototype {2}{R}")]
#[case::a_bullet("\u{2022} Disarm")]
#[case::a_number("Stage 1")]
fn a_text_that_is_no_flavour_name_is_no_ability_word(#[case] before_the_dash: &str) {
    let line = format!("{before_the_dash} {DASH} Draw a card.");
    assert_eq!(classify::ability_word_len(&line), None, "{line}");
}

#[rstest]
#[case::channel(
    "Channel \u{2014} {1}{G}, Discard this card: Draw a card.",
    ClauseKind::Activated
)]
#[case::boast("Boast \u{2014} {1}: Draw a card.", ClauseKind::Activated)]
#[case::a_name_before_nothing("Heavy Rock Cutter \u{2014} ", ClauseKind::Triggered)]
#[case::a_name_before_a_sentence_that_is_not_read(
    "Heavy Rock Cutter \u{2014} Whenever this creature attacks, wish for a card.",
    ClauseKind::Triggered
)]
#[case::a_name_before_a_cost_that_is_not_a_cost(
    "Heavy Rock Cutter \u{2014} Wish: Draw a card.",
    ClauseKind::Activated
)]
fn a_near_miss_stays_with_the_table(#[case] line: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(line, kind, false), None, "{line}");
}

#[rstest]
#[case::draw_and_lose(
    "When this creature enters from a graveyard, you draw two cards and you lose 2 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Graveyard","to":"Battlefield"}},"effects":[{"Draw":{"count":2}},{"LoseLife":{"amount":2}}]}}"#
)]
#[case::exile_a_permanent(
    "When this creature enters from a graveyard, exile up to one target nonland permanent.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Graveyard","to":"Battlefield"}},"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]},"bounds":{"min":0,"max":1}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#
)]
fn a_card_that_enters_from_a_graveyard_is_a_zone_change_from_it(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
    let named = format!("Dynastic Codes {DASH} {text}");
    let read = clause(&named, ClauseKind::Triggered, false);
    assert_eq!(read, Some(ability(json)), "{named}");
}

#[rstest]
#[case::your_graveyard("When this creature enters from your graveyard, draw a card.")]
#[case::exile("When this creature enters from exile, draw a card.")]
#[case::anywhere("When this creature enters from anywhere, draw a card.")]
#[case::the_zone_after_the_comma("When this creature enters, from a graveyard, draw a card.")]
fn an_entering_from_another_place_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

fn card(name: &str, type_line: &str, oracle_text: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
        name: name.into(),
        faces: vec![PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            oracle_text: oracle_text.into(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    }
}

#[rstest]
#[case::the_name_of_the_card_in_the_flavour_name(
    "Magnus the Red",
    "Legendary Creature \u{2014} Demon Primarch",
    "Flying\nUnearthly Power \u{2014} Instant and sorcery spells you cast cost {1} less to cast for each creature token you control.\nBlade of Magnus \u{2014} Whenever Magnus the Red deals combat damage to a player, create a 3/3 red Spawn creature token.",
    ImplStatus::Full
)]
#[case::enters_from_a_graveyard(
    "Triarch Praetorian",
    "Artifact Creature \u{2014} Necron",
    "Flying\nDynastic Codes \u{2014} When this creature enters from a graveyard, you draw two cards and you lose 2 life.",
    ImplStatus::Full
)]
#[case::a_name_before_a_sentence_that_is_not_read(
    "Biophagus",
    "Creature \u{2014} Human Tyranid Wizard",
    "Genomic Enhancement \u{2014} {T}: Add one mana of any color. If this mana is spent to cast a creature spell, that creature enters with an additional +1/+1 counter on it.",
    ImplStatus::Manual
)]
fn a_card_with_flavour_names_reads_whole_or_not_at_all(
    #[case] name: &str,
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] expected: ImplStatus,
) {
    let compiled = crate::compile::compile(&card(name, type_line, text));
    let status = compiled.card.as_ref().map(|card| card.impl_status);
    assert_eq!(status.unwrap_or(ImplStatus::Manual), expected, "{name}");
}
