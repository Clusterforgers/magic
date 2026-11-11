//! Goal: the wordings of a player controlled during their next turn (Mindslaver, Worst Fears, Emrakul, the Promised End), of opponents controlled while they search (Opposition Agent), of a spell taken (Aethersnatch, Commandeer), of a restart of the game (Karn Liberated) and of a text change (Sleight of Mind, Magical Hack, Mind Bend, Crystal Spray, Trait Doctoring, Artificial Evolution) read into the DSL, write back as the card's own text, and leave the near-misses to the table.
use super::*;

const PLAYER: &str = r#""targets":[{"what":"Player"}]"#;

fn text_change(target: &str, from: &str, to: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{target}}}],"effects":[{{"ChooseValue":{{"kind":{from},"as":"from"}}}},{{"ChooseValue":{{"kind":{to},"as":"to"}}}},{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"ChangeTextChosen":{{"from":"from","to":"to"}}}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

const SPELL_OR_PERMANENT: &str = r#"{"Object":[{"AnyOf":["Spell",{"In":"Battlefield"}]}]}"#;
const PERMANENT: &str = r#"{"Object":[]}"#;
const LIKE: &str = r#"{"LikeChosen":"from"}"#;

#[rstest]
#[case::worst_fears(
    "You control target player during that player's next turn.",
    &format!(r#"{{"Spell":{{{PLAYER},"effects":[{{"ControlPlayer":{{"who":{{"Target":0}}}}}}]}}}}"#)
)]
fn a_player_is_controlled_during_their_next_turn(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn mindslaver_controls_a_player_and_emrakul_adds_the_extra_turn() {
    let slaver = golden(
        "{4}, {T}, Sacrifice ~: You control target player during that player's next turn.",
        ClauseKind::Activated,
        false,
    );
    assert!(
        serde_json::to_string(&slaver)
            .unwrap()
            .contains(r#"{"ControlPlayer":{"who":{"Target":0}}}"#)
    );
    let emrakul = golden(
        "When you cast this spell, you gain control of target opponent during that player's next turn. After that turn, that player takes an extra turn.",
        ClauseKind::Triggered,
        false,
    );
    let json = serde_json::to_string(&emrakul).unwrap();
    assert!(
        json.contains(r#"{"ControlPlayer":{"who":{"Target":0},"extra_turn":true}}"#),
        "{json}"
    );
    assert!(
        json.contains(r#""targets":[{"what":"Opponent"}]"#),
        "{json}"
    );
}

#[test]
fn karn_restarts_the_game_keeping_what_it_exiled() {
    let read = golden(
        "Restart the game, leaving in exile all non-Aura permanent cards exiled with ~. Then put those cards onto the battlefield under your control.",
        ClauseKind::Spell,
        false,
    );
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(r#"{"RestartGame":{"keep":["#), "{json}");
    assert!(json.contains(r#"{"ExiledWith":"This"}"#), "{json}");
    assert!(json.contains(r#""Aura""#), "{json}");
}

#[test]
fn opponents_are_controlled_while_they_search() {
    reads_as(
        "You control your opponents while they're searching their libraries.",
        ClauseKind::Static,
        false,
        r#"{"Rule":{"rule":{"ControlSearches":"Opponents"}}}"#,
    );
}

#[rstest]
#[case::sleight_of_mind(
    "Change the text of target spell or permanent by replacing all instances of one color word with another.",
    text_change(SPELL_OR_PERMANENT, r#""Color""#, LIKE, "Forever")
)]
#[case::magical_hack(
    "Change the text of target spell or permanent by replacing all instances of one basic land type with another.",
    text_change(SPELL_OR_PERMANENT, r#""BasicLandType""#, LIKE, "Forever")
)]
#[case::mind_bend(
    "Change the text of target permanent by replacing all instances of one color word with another or one basic land type with another.",
    text_change(PERMANENT, r#""ColorOrBasicLandType""#, LIKE, "Forever")
)]
#[case::trait_doctoring(
    "Change the text of target permanent by replacing all instances of one color word with another or one basic land type with another until end of turn.",
    text_change(PERMANENT, r#""ColorOrBasicLandType""#, LIKE, "UntilEndOfTurn")
)]
#[case::crystal_spray(
    "Change the text of target spell or permanent by replacing all instances of one color word with another or one basic land type with another until end of turn.",
    text_change(
        SPELL_OR_PERMANENT,
        r#""ColorOrBasicLandType""#,
        LIKE,
        "UntilEndOfTurn"
    )
)]
#[case::artificial_evolution(
    "Change the text of target spell or permanent by replacing all instances of one creature type with another. The new creature type can't be Wall.",
    text_change(
        SPELL_OR_PERMANENT,
        r#""CreatureType""#,
        r#"{"CreatureTypeExcept":["Wall"]}"#,
        "Forever"
    )
)]
fn a_text_change_reads_its_words(#[case] text: &str, #[case] json: String) {
    reads_as(text, ClauseKind::Spell, false, &json);
}

#[rstest]
#[case::any_spell("Gain control of target spell. You may choose new targets for it.")]
#[case::a_noncreature_spell(
    "Gain control of target noncreature spell. You may choose new targets for it."
)]
fn a_spell_is_taken(#[case] text: &str) {
    let read = golden(text, ClauseKind::Spell, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(r#"{"SetController":"You"}"#), "{json}");
}

#[rstest]
#[case::their_turn("You control target player during their next turn.")]
#[case::this_turn("You control target player during that player's turn.")]
#[case::two_extra_turns(
    "When you cast this spell, you gain control of target opponent during that player's next turn. After that turn, that player takes two extra turns."
)]
#[case::gain_control_without_the_extra_turn(
    "You gain control of target player during that player's next turn."
)]
#[case::a_creature_type_or_color(
    "Change the text of target permanent by replacing all instances of one creature type with another or one basic land type with another."
)]
#[case::all_permanents(
    "Change the text of all permanents by replacing all instances of one color word with another."
)]
#[case::some_instances(
    "Change the text of target permanent by replacing one instance of one color word with another."
)]
#[case::a_restart_keeping_nothing("Restart the game.")]
#[case::a_restart_keeping_cards_not_exiled_with_it(
    "Restart the game, leaving in exile all non-Aura permanent cards. Then put those cards onto the battlefield under your control."
)]
#[case::a_restart_keeping_them_in_exile(
    "Restart the game, leaving in exile all non-Aura permanent cards exiled with ~."
)]
#[case::the_new_type_may_be_a_wall(
    "Change the text of target spell or permanent by replacing all instances of one creature type with another. The new creature type can be Wall."
)]
fn near_misses_stay_unread(#[case] text: &str) {
    let kind = if text.starts_with("When") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    };
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[test]
fn a_near_miss_of_the_search_rule_stays_unread() {
    assert_eq!(
        clause(
            "You control your opponents while they're searching their graveyards.",
            ClauseKind::Static,
            false
        ),
        None
    );
}

/// The abilities a whole card compiles to, as JSON.
fn compiled(name: &str, type_line: &str, mana_cost: &str, text: &str) -> Vec<String> {
    let printed = crate::support::PrintedCard {
        layout: String::new(),
        oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
        name: name.into(),
        keywords: vec![],
        faces: vec![crate::support::PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            mana_cost: mana_cost.into(),
            oracle_text: text.into(),
            ..crate::support::PrintedFace::default()
        }],
    };
    let card = crate::compile::compile(&printed).card.expect("a card");
    card.faces[0]
        .abilities
        .iter()
        .map(|ability| serde_json::to_string(ability).unwrap())
        .collect()
}

const CLEAVE: &str = " (You may cast this spell for its cleave cost. If you do, remove the words in square brackets.)";

#[rstest]
#[case::path_of_peril(
    "Path of Peril",
    "Sorcery",
    "{1}{W}{B}",
    "Cleave {4}{W}{B}",
    "Destroy all creatures [with mana value 2 or less].",
    r#""spell":{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]}}}]}}"#
)]
#[case::fierce_retribution(
    "Fierce Retribution",
    "Instant",
    "{1}{W}",
    "Cleave {5}{W}",
    "Destroy target [attacking] creature.",
    r#""spell":{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]"#
)]
#[case::dig_up(
    "Dig Up",
    "Sorcery",
    "{G}",
    "Cleave {1}{B}{B}{G}",
    "Search your library for a [basic land] card, [reveal it,] put it into your hand, then shuffle.",
    r#""Search":{"filter":[]"#
)]
#[case::parasitic_grasp(
    "Parasitic Grasp",
    "Instant",
    "{1}{B}",
    "Cleave {1}{B}{B}",
    "Parasitic Grasp deals 3 damage to target [Human] creature. You gain 3 life.",
    r#""spell":{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]"#
)]
fn cleave_reads_the_spell_without_the_bracketed_words(
    #[case] name: &str,
    #[case] type_line: &str,
    #[case] mana_cost: &str,
    #[case] keyword: &str,
    #[case] spell: &str,
    #[case] cleaved: &str,
) {
    let abilities = compiled(
        name,
        type_line,
        mana_cost,
        &format!("{keyword}{CLEAVE}\n{spell}"),
    );
    assert_eq!(abilities.len(), 2, "{abilities:?}");
    assert!(
        abilities[0].starts_with(r#"{"Keyword":{"Cleave":"#),
        "{abilities:?}"
    );
    assert!(abilities[0].contains(cleaved), "{abilities:?}");
    // The printed spell keeps the words inside the brackets.
    assert!(abilities[1].starts_with(r#"{"Spell":"#), "{abilities:?}");
    assert!(!abilities[0].contains(&abilities[1]), "{abilities:?}");
    assert!(
        !abilities.iter().any(|ability| ability.contains("Manual")),
        "{abilities:?}"
    );
}

#[test]
fn brackets_outside_a_cleave_card_stay_text() {
    let abilities = compiled(
        "Odd Spell",
        "Instant",
        "{1}",
        "Destroy target [attacking] creature.",
    );
    assert!(
        abilities.iter().all(|ability| ability.contains("Manual")),
        "{abilities:?}"
    );
}
