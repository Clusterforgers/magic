//! Goal: the mill and rad leftovers read into their data and write back as the card prints them: a per-player batch of mills, target bounds from the count of a mill batch, "Do this only once each turn" after a lone may, "you may get" counters, "that was milled this turn", entering tapped for counters, "for each rad counter you have", "loses all rad counters" and a mode name with an inner capital; every near-miss stays unread.
use super::*;

const MILLED_BATCH: &str = r#"{"Batch":{"Milled":{"what":[{"Not":{"Type":"Land"}}]}}}"#;

#[rstest]
#[case::per_player_batch(
    "Whenever a player mills one or more creature cards, you create a 1/1 black Horror creature token.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Batch":{"Milled":{"what":[{"Type":"Creature"}],"per_player":true}}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Horror","colors":["Black"],"types":["Creature"],"subtypes":["Horror"],"power":1,"toughness":1}}}}]}}"#.to_owned()
)]
#[case::an_opponent_per_player(
    "Whenever an opponent mills one or more nonland cards, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Batch":{"Milled":{"what":[{"Not":{"Type":"Land"}}],"by":"Opponents","per_player":true}}},"effects":[{"Draw":{}}]}}"#.to_owned()
)]
#[case::bounds_from_the_count(
    "Whenever one or more nonland cards are milled, put a +1/+1 counter on each of up to X target creatures, where X is the number of nonland cards milled this way.",
    ClauseKind::Triggered,
    format!(r#"{{"Triggered":{{"when":{MILLED_BATCH},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}},"bounds":{{"min":0,"max":"Triggering"}}}}],"effects":[{{"PutCounters":{{"on":{{"Target":0}},"counter":"+1/+1"}}}}]}}}}"#)
)]
#[case::do_this_once(
    "Whenever one or more nonland cards are milled, you may draw a card. Do this only once each turn.",
    ClauseKind::Triggered,
    format!(r#"{{"Triggered":{{"when":{MILLED_BATCH},"limit":"OncePerTurn","effects":[{{"May":{{"effects":[{{"Draw":{{}}}}]}}}}]}}}}"#)
)]
#[case::may_get(
    "Whenever a land you control enters, you may get two rad counters.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"PutCounters":{"on":"You","counter":{"Other":"rad"},"count":2}}]}}]}}"#.to_owned()
)]
#[case::milled_this_turn(
    "{T}: Put target land card in a graveyard that was milled this turn onto the battlefield tapped under your control.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Land"},{"In":"Graveyard"},"MilledThisTurn"]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true,"under":"You"}}}}]}}"#.to_owned()
)]
#[case::tapped_for_counters(
    "You may have this land enter tapped. If you do, you get two rad counters.",
    ClauseKind::Static,
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"TappedForCounters":{"counter":{"Other":"rad"},"count":2}}}}"#.to_owned()
)]
#[case::less_for_each_rad_counter(
    "{5}, {T}: Draw a card. This ability costs {1} less to activate for each rad counter you have.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap",{"Changed":{"change":"Reduce","mana":"{1}","per":{"Counters":{"on":"You","counter":{"Other":"rad"}}}}}],"effects":[{"Draw":{}}]}}"#.to_owned()
)]
#[case::loses_all(
    "Target player loses all rad counters.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"RemoveCounters":{"from":{"Target":0},"counter":{"Other":"rad"},"count":{"Counters":{"on":{"Target":0},"counter":{"Other":"rad"}}}}}]}}"#.to_owned()
)]
#[case::inner_capital_mode_name(
    "{1}, {T}: Choose one that hasn't been chosen \u{2014}\n\u{2022} Stimpak \u{2014} Draw a card.\n\u{2022} RadAway \u{2014} Target player loses all rad counters. Sacrifice this artifact.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"Modal":{"once_ever":true,"modes":[{"label":"Stimpak","effects":[{"Draw":{}}]},{"label":"RadAway","targets":[{"what":"Player"}],"effects":[{"RemoveCounters":{"from":{"Target":0},"counter":{"Other":"rad"},"count":{"Counters":{"on":{"Target":0},"counter":{"Other":"rad"}}}}},{"Sacrifice":{"filter":["This"]}}]}]}}]}}"#.to_owned()
)]
fn w13b_reads_and_writes_back(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: String) {
    reads_as(text, kind, false, &json);
}

#[rstest]
#[case::bounds_from_other_cards(
    "Whenever one or more nonland cards are milled, put a +1/+1 counter on each of up to X target creatures, where X is the number of creature cards milled this way.",
    ClauseKind::Triggered
)]
#[case::bounds_after_another_event(
    "Whenever a creature dies, put a +1/+1 counter on each of up to X target creatures, where X is the number of nonland cards milled this way.",
    ClauseKind::Triggered
)]
#[case::do_this_once_after_no_may(
    "Whenever one or more nonland cards are milled, draw a card. Do this only once each turn.",
    ClauseKind::Triggered
)]
#[case::may_get_energy_by_word(
    "Whenever a land you control enters, you may get two shield counters.",
    ClauseKind::Triggered
)]
#[case::tapped_for_object_counters(
    "You may have this land enter tapped. If you do, you get two +1/+1 counters.",
    ClauseKind::Static
)]
#[case::less_for_object_counters_you_have(
    "{5}, {T}: Draw a card. This ability costs {1} less to activate for each +1/+1 counter you have.",
    ClauseKind::Activated
)]
#[case::loses_all_object_counters(
    "Target player loses all +1/+1 counters.",
    ClauseKind::Spell
)]
fn w13b_near_misses_stay_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[test]
fn w13b_a_camel_case_word_is_a_name_where_a_sentence_opens() {
    assert_eq!(
        tokenize("RadAway \u{2014} Draw.")[0],
        Token::Name("RadAway".into())
    );
    assert_eq!(tokenize("Draw a card.")[0], Token::Word("draw".into()));
    assert!(!matches!(tokenize("Non-Elf creatures get -2/-2.")[0], Token::Name(_)));
}
