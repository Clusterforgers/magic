//! Goal: the mana wordings that count a tally of the turn (cards drawn, lands played) or name the permanents controlled read in an activated mana ability, each writes back as the card's text, and a near-miss stays with the table.
use super::*;

#[rstest]
#[case::cards_drawn(
    "{T}: Add {C} for each card you've drawn this turn.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Tallied":{"who":"You","what":"Drawn"}}}}]}}"#
)]
#[case::a_land_was_played(
    "{T}: Add {U}. If you played a land this turn, add {B} instead.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Blue"]},"when":{"Compare":{"left":{"Tallied":{"who":"You","what":"LandsPlayed"}},"op":"Ge","right":1}},"instead":{"Fixed":["Black"]}}}}}]}}"#
)]
#[case::two_named_lands(
    "{T}: Add {C}. If you control an Urza's Power-Plant and an Urza's Tower, add {C}{C} instead.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Colorless"]},"when":{"All":[{"Compare":{"left":{"Count":[{"Subtype":"Urza's Power-Plant"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Count":[{"Subtype":"Urza's Tower"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}]},"instead":{"Fixed":["Colorless","Colorless"]}}}}}]}}"#
)]
fn tallied_mana_and_named_lands_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::drawn_by_an_opponent("{T}: Add {C} for each card an opponent has drawn this turn.")]
#[case::lands_by_a_player("{T}: Add {U}. If a player played a land this turn, add {B} instead.")]
#[case::no_condition_word("{T}: Add {U}. You played a land this turn, add {B} instead.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_none(),
        "{text}"
    );
}

#[rstest]
#[case::the_number_of_cards_drawn(
    "Creatures you control get +X/+X, where X is the number of cards you've drawn this turn.",
    ClauseKind::Static,
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":{"Tallied":{"who":"You","what":"Drawn"}},"toughness":{"Tallied":{"who":"You","what":"Drawn"}}}}]}}"#
)]
#[case::a_bonus_for_each_card_drawn(
    "Draw a card. Until end of turn, target creature gains trample and gets +1/+0 for each card you've drawn this turn.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Trample"},{"ModifyPT":{"power":{"Tallied":{"who":"You","what":"Drawn"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn amounts_of_cards_drawn_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[test]
fn probe_cost() {
    let tokens = tokenize("for each card in target opponent's hand");
    eprintln!("PROBE tokens {tokens:?}");
    let mut reader = Reader::new(&tokens, false);
    let each = super::amounts::for_each(&mut reader);
    eprintln!(
        "PROBE each {each:?} end={} targets={:?}",
        reader.end(),
        reader.targets
    );
    if let Some(each) = &each {
        eprintln!("PROBE text {:?}", super::amounts::for_each_text(each));
    }
}

#[test]
fn probe() {
    for (text, kind) in [
        (
            "Whenever this creature or equipped creature attacks, you gain 1 life.",
            ClauseKind::Triggered,
        ),
        (
            "Whenever this creature or equipped creature attacks, add {G}.",
            ClauseKind::Triggered,
        ),
        (
            "Whenever this creature attacks, add an amount of {G} equal to its power.",
            ClauseKind::Triggered,
        ),
        (
            "Whenever equipped creature attacks, add an amount of {G} equal to its power.",
            ClauseKind::Triggered,
        ),
        (
            "Whenever this creature attacks, add an amount of {G} equal to the greatest power among creatures you control.",
            ClauseKind::Triggered,
        ),
        (
            "Whenever Neheb deals combat damage to a player or planeswalker, you may discard any number of cards. If you do, draw that many cards and add that much {R}.",
            ClauseKind::Triggered,
        ),
        (
            "Whenever ~ deals combat damage to a player, you may discard a card. If you do, draw a card.",
            ClauseKind::Triggered,
        ),
        ("{T}, Mill a card: Draw a card.", ClauseKind::Activated),
        ("{T}, Discard a card: Add {C}.", ClauseKind::Activated),
        ("{T}, Mill two cards: Add {C}.", ClauseKind::Activated),
        (
            "{T}: Target player adds one mana of any color.",
            ClauseKind::Activated,
        ),
        ("Add {R} for each card in their hand.", ClauseKind::Spell),
        (
            "Target opponent loses 1 life for each card in their hand.",
            ClauseKind::Spell,
        ),
        ("{T}, Mill a card: Add {C}.", ClauseKind::Activated),
        (
            "{T}, Mill a card: Target player adds one mana of any color.",
            ClauseKind::Activated,
        ),
        (
            "Add {R} for each card in target opponent's hand.",
            ClauseKind::Spell,
        ),
        (
            "Add {R} for each card in target opponent's hand. Until end of turn, you don't lose this mana as steps and phases end.",
            ClauseKind::Spell,
        ),
        (
            "Add {R} for each card in target opponent's hand. Until end of turn, you don't lose this mana as steps and phases end. Exile ~ with three time counters on it.",
            ClauseKind::Spell,
        ),
        (
            "{T}: Add {U} for each counter on ~. Spend this mana only to cast noncreature spells.",
            ClauseKind::Activated,
        ),
        (
            "{T}, Remove X +1/+1 counters from this creature: Add X mana of any one color.",
            ClauseKind::Activated,
        ),
        (
            "Devouring Monster — {T}, Remove X +1/+1 counters from this creature: Add X mana of any one color.",
            ClauseKind::Activated,
        ),
        (
            "Whenever this creature or equipped creature attacks, add an amount of {G} equal to its power. Until end of turn, you don't lose this mana as steps and phases end.",
            ClauseKind::Triggered,
        ),
    ] {
        let read = clause(text, kind, false);
        eprintln!(
            "PROBE {text}\n  => {}",
            read.map_or("NONE".into(), |a| serde_json::to_string(&a).unwrap())
        );
    }
}
