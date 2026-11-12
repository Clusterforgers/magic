//! Goal: the whole-text secret choices read as their fixed effects, write back as the printed text, and a near miss of
//! the text or of the rules reads or writes as something else.
use super::*;

const OGRE: &str = "When this creature enters, each player secretly chooses a number. Then those numbers are revealed. Each player with the highest number loses that much life. If you are one of those players, put two +1/+1 counters on this creature.";

const OGRE_RULES: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"ChooseNumbers":{"secret":true,"as":"number","highest":"highest","lowest":"lowest"}},{"LoseLife":{"who":{"Bound":"highest"},"amount":{"ChosenAs":"number"}}},{"If":{"condition":{"Among":{"Bound":"highest"}},"then":[{"PutCounters":{"on":"This","counter":"+1/+1","count":2}}]}}]}}"#;

#[test]
fn menacing_ogre_reads_and_writes_back() {
    reads_as(OGRE, ClauseKind::Triggered, false, OGRE_RULES);
}

#[rstest]
#[case::open("each player secretly", "each player")]
#[case::lowest("the highest number loses", "the lowest number loses")]
#[case::gains("loses that much life", "gains that much life")]
#[case::not_one("If you are one", "If you aren't one")]
#[case::three_counters("put two", "put three")]
fn a_near_miss_of_the_text_stays_unread(#[case] from: &str, #[case] to: &str) {
    let text = OGRE.replace(from, to);
    assert_ne!(
        clause(&text, ClauseKind::Triggered, false),
        Some(ability(OGRE_RULES)),
        "{text}"
    );
}

#[rstest]
#[case::open(r#""secret":true,"#, "")]
#[case::lowest(r#""who":{"Bound":"highest"}"#, r#""who":{"Bound":"lowest"}"#)]
#[case::not_among(
    r#""condition":{"Among":{"Bound":"highest"}}"#,
    r#""condition":{"Not":{"Among":{"Bound":"highest"}}}"#
)]
#[case::three_counters(r#""count":2"#, r#""count":3"#)]
#[case::from_one(r#""secret":true,"#, r#""secret":true,"min":1,"#)]
fn a_near_miss_of_the_rules_is_not_written_as_the_text(#[case] from: &str, #[case] to: &str) {
    let json = OGRE_RULES.replacen(from, to, 1);
    let written = render_variants(&ability(&json));
    assert!(
        !written.iter().any(|said| same_text(said, OGRE)),
        "{json}: {written:?}"
    );
}

const CIRDAN: &str = "Whenever ~ enters or attacks, each player secretly votes for a player, then those votes are revealed. Each player draws a card for each vote they received. Each player who received no votes may put a permanent card from their hand onto the battlefield.";

const CIRDAN_RULES: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"effects":[{"Vote":{"options":{"Of":"EachPlayer"},"secret":true,"tally":"votes"}},{"RepeatEach":{"over":"EachPlayer","as":"voter","effects":[{"Draw":{"who":{"Bound":"voter"},"count":{"Players":{"PlayersWhere":{"who":{"Bound":"votes"},"test":{"Among":{"Bound":"voter"}}}}}}}]}},{"RepeatEach":{"over":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":{"Bound":"votes"}}}}},"as":"unvoted","effects":[{"Choose":{"who":{"Bound":"unvoted"},"from":{"Each":[{"OwnedBy":{"Bound":"unvoted"}},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"In":"Hand"}]},"count":{"min":0,"max":1},"as":"picked"}},{"Move":{"what":{"Bound":"picked"},"to":{"Battlefield":{}}}}]}}]}}"#;

const MOB: &str = "Each player secretly votes for another player, then those votes are revealed. For each vote an opponent received, ~ deals 2 damage to that player and each creature that player controls. For each vote you received, draw a card.";

const MOB_RULES: &str = r#"{"Spell":{"effects":[{"Vote":{"options":"AnotherPlayer","secret":true,"tally":"votes"}},{"RepeatEach":{"over":"Opponents","as":"voter","effects":[{"DealDamage":{"to":{"Union":[{"Bound":"voter"},{"Each":[{"Type":"Creature"},{"ControlledBy":{"Bound":"voter"}}]}]},"amount":{"Times":{"by":2,"of":{"Players":{"PlayersWhere":{"who":{"Bound":"votes"},"test":{"Among":{"Bound":"voter"}}}}}}}}}]}},{"Draw":{"count":{"Players":{"PlayersWhere":{"who":{"Bound":"votes"},"test":{"Among":"Asker"}}}}}}]}}"#;

const DILEMMA: &str = "Each opponent secretly chooses silence or snitch, then the choices are revealed. If each opponent chose silence, ~ deals 4 damage to each of them. If each opponent chose snitch, ~ deals 8 damage to each of them. Otherwise, ~ deals 12 damage to each opponent who chose silence.";

const DILEMMA_RULES: &str = r#"{"Spell":{"effects":[{"ChooseWords":{"who":"Opponents","words":["silence","snitch"],"secret":true}},{"If":{"condition":{"Compare":{"left":{"Players":{"Bound":"silence"}},"op":"Eq","right":{"Players":"Opponents"}}},"then":[{"DealDamage":{"to":"Opponents","amount":4}}],"otherwise":[{"If":{"condition":{"Compare":{"left":{"Players":{"Bound":"snitch"}},"op":"Eq","right":{"Players":"Opponents"}}},"then":[{"DealDamage":{"to":"Opponents","amount":8}}],"otherwise":[{"DealDamage":{"to":{"Bound":"silence"},"amount":12}}]}}]}}]}}"#;

#[rstest]
#[case::cirdan(CIRDAN, ClauseKind::Triggered, CIRDAN_RULES)]
#[case::mob_verdict(MOB, ClauseKind::Spell, MOB_RULES)]
#[case::prisoners_dilemma(DILEMMA, ClauseKind::Spell, DILEMMA_RULES)]
fn the_secret_votes_and_choices_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::open_vote(MOB, "secretly votes", "votes")]
#[case::any_player(MOB, "for another player", "for a player")]
#[case::three_damage(MOB, "deals 2 damage", "deals 3 damage")]
#[case::no_creatures(MOB, " and each creature that player controls", "")]
#[case::cirdan_two_cards(CIRDAN, "draws a card for each", "draws two cards for each")]
#[case::cirdan_some_votes(CIRDAN, "received no votes", "received votes")]
#[case::dilemma_each_player(DILEMMA, "Each opponent secretly", "Each player secretly")]
#[case::dilemma_open(DILEMMA, "secretly chooses", "chooses")]
#[case::dilemma_snitches_hit(DILEMMA, "who chose silence.", "who chose snitch.")]
#[case::dilemma_nine(DILEMMA, "deals 8 damage", "deals 9 damage")]
fn a_near_miss_of_a_text_is_read_as_something_else(
    #[case] text: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let kind = if text == CIRDAN {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    };
    let changed = text.replace(from, to);
    assert_ne!(changed, text);
    let read = clause(&changed, kind, false);
    for json in [CIRDAN_RULES, MOB_RULES, DILEMMA_RULES] {
        assert_ne!(read, Some(ability(json)), "{changed}");
    }
}

#[rstest]
#[case::open_vote(MOB_RULES, r#""secret":true,"#, "", MOB)]
#[case::any_player(
    MOB_RULES,
    r#""options":"AnotherPlayer""#,
    r#""options":{"Of":"EachPlayer"}"#,
    MOB
)]
#[case::three_damage(MOB_RULES, r#""by":2"#, r#""by":3"#, MOB)]
#[case::cirdan_opponents(
    CIRDAN_RULES,
    r#""over":"EachPlayer""#,
    r#""over":"Opponents""#,
    CIRDAN
)]
#[case::dilemma_open(DILEMMA_RULES, r#""secret":true"#, r#""secret":false"#, DILEMMA)]
#[case::dilemma_twelve_to_snitches(
    DILEMMA_RULES,
    r#""to":{"Bound":"silence"}"#,
    r#""to":{"Bound":"snitch"}"#,
    DILEMMA
)]
#[case::dilemma_nine(DILEMMA_RULES, r#""amount":8"#, r#""amount":9"#, DILEMMA)]
fn a_near_miss_of_the_rules_is_not_written_as_its_text(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] text: &str,
) {
    let changed = json.replacen(from, to, 1);
    assert_ne!(changed, json);
    let written = render_variants(&ability(&changed));
    assert!(
        !written.iter().any(|said| same_text(said, text)),
        "{changed}: {written:?}"
    );
}

const GATEWAY: &str = "Tap two untapped creatures you control: You may put a creature card from your hand that shares a creature type with each creature tapped this way onto the battlefield.";

const GATEWAY_RULES: &str = r#"{"Activated":{"cost":[{"TapPermanents":{"filter":[{"Type":"Creature"}],"count":2}}],"effects":[{"Choose":{"from":{"Each":[{"OwnedBy":"You"},{"Type":"Creature"},{"In":"Hand"},{"SharesCreatureTypeWithEach":"Paid"}]},"count":{"min":0,"max":1},"as":"picked"}},{"Move":{"what":{"Bound":"picked"},"to":{"Battlefield":{}}}}]}}"#;

#[test]
fn cryptic_gateway_reads_and_writes_back() {
    reads_as(GATEWAY, ClauseKind::Activated, false, GATEWAY_RULES);
}

#[rstest]
#[case::one_of(
    r#"{"SharesCreatureTypeWithEach":"Paid"}"#,
    r#"{"SharesSubtype":"Paid"}"#
)]
#[case::must(r#""count":{"min":0,"max":1}"#, r#""count":{"min":1,"max":1}"#)]
#[case::any_card(r#"{"Type":"Creature"},"#, "")]
fn a_near_miss_of_the_gateway_is_not_written_as_its_text(#[case] from: &str, #[case] to: &str) {
    let changed = GATEWAY_RULES.replacen(from, to, 1);
    assert_ne!(changed, GATEWAY_RULES);
    let written = render_variants(&ability(&changed));
    assert!(
        !written.iter().any(|said| same_text(said, GATEWAY)),
        "{changed}: {written:?}"
    );
}

#[rstest]
#[case::one_of("with each creature", "with a creature")]
#[case::land("a creature card from", "a land card from")]
fn a_near_miss_of_the_gateway_text_is_read_as_something_else(#[case] from: &str, #[case] to: &str) {
    let changed = GATEWAY.replace(from, to);
    assert_ne!(
        clause(&changed, ClauseKind::Activated, false),
        Some(ability(GATEWAY_RULES))
    );
}

const TEYO: &str = "\u{2212}2: Choose left or right. Until your next turn, each player may attack only the nearest opponent in the last chosen direction and planeswalkers controlled by that opponent.";

const TEYO_RULES: &str = r#"{"Loyalty":{"cost":{"Minus":2},"effects":[{"ChooseValue":{"kind":{"Options":["left","right"]},"as":"direction"}},{"Rule":{"rule":{"AttackOnlyNearest":{"ChosenAs":"direction"}},"duration":"UntilYourNextTurn"}}]}}"#;

const PRAMIKON: &str = "Each player may attack only the nearest opponent in the chosen direction and planeswalkers controlled by that opponent.";

const PRAMIKON_RULES: &str = r#"{"Rule":{"rule":{"AttackOnlyNearest":"Entered"}}}"#;

#[test]
fn the_nearest_opponent_reads_and_writes_back() {
    reads_as(TEYO, ClauseKind::Loyalty, false, TEYO_RULES);
    reads_as(PRAMIKON, ClauseKind::Static, false, PRAMIKON_RULES);
    reads_as(
        "As ~ enters, choose left or right.",
        ClauseKind::Static,
        false,
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"ChooseWord":["left","right"]}}}"#,
    );
}

#[test]
fn another_pair_of_words_is_not_written_as_left_or_right() {
    let written = crate::compile::read::render::written_variants(&ability(
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"ChooseWord":["up","down"]}}}"#,
    ));
    assert!(
        !written.iter().any(|said| said.contains("left or right")),
        "{written:?}"
    );
}

#[rstest]
#[case::for_the_turn(TEYO_RULES, "UntilYourNextTurn", "UntilEndOfTurn", TEYO)]
#[case::entered(TEYO_RULES, r#"{"ChosenAs":"direction"}"#, r#""Entered""#, TEYO)]
#[case::up_or_down(TEYO_RULES, r#"["left","right"]"#, r#"["up","down"]"#, TEYO)]
#[case::by_resolution(
    PRAMIKON_RULES,
    r#""Entered""#,
    r#"{"ChosenAs":"direction"}"#,
    PRAMIKON
)]
fn a_near_miss_of_the_nearest_opponent_is_not_written_as_its_text(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] text: &str,
) {
    let changed = json.replacen(from, to, 1);
    assert_ne!(changed, json);
    let written = crate::compile::read::render::written_variants(&ability(&changed));
    assert!(
        !written.iter().any(|said| same_text(said, text)),
        "{changed}: {written:?}"
    );
}

#[rstest]
#[case::farthest("the nearest opponent", "the farthest opponent")]
#[case::creatures("and planeswalkers controlled", "and creatures controlled")]
fn a_near_miss_of_the_static_text_stays_unread(#[case] from: &str, #[case] to: &str) {
    let text = PRAMIKON.replace(from, to);
    assert_ne!(
        clause(&text, ClauseKind::Static, false),
        Some(ability(PRAMIKON_RULES)),
        "{text}"
    );
}

const HAMA: &str = "Room abilities of dungeons you own trigger an additional time.";

const HAMA_RULES: &str = r#"{"Rule":{"rule":{"TriggerAgain":{"source":[{"In":"Command"},"Dungeon",{"OwnedBy":"You"}]}}}}"#;

#[test]
fn room_abilities_triggering_again_read_and_write_back() {
    reads_as(HAMA, ClauseKind::Static, false, HAMA_RULES);
}

#[rstest]
#[case::any_owner(r#",{"OwnedBy":"You"}"#, "")]
#[case::not_a_dungeon(r#""Dungeon","#, "")]
#[case::on_the_battlefield(r#"{"In":"Command"},"#, "")]
fn a_near_miss_of_the_rooms_is_not_written_as_its_text(#[case] from: &str, #[case] to: &str) {
    let changed = HAMA_RULES.replacen(from, to, 1);
    assert_ne!(changed, HAMA_RULES);
    let written = crate::compile::read::render::written_variants(&ability(&changed));
    assert!(
        !written.iter().any(|said| same_text(said, HAMA)),
        "{changed}: {written:?}"
    );
}
