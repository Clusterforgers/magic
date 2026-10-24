//! Goal: the writers of the proposal checker give the oracle wording of an event or subject the data states with one more predicate than the plain wording ("enters from a graveyard", "enters untapped", "casts a spell from a graveyard", "this creature and land creatures you control", "Each planeswalker you control enters with an additional loyalty counter on it", "outlaws"), of the dead card a trigger exiles ("exile it"), and a wrong zone, count, kind or group never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn trigger(when: &str, extra: &str, effects: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when}{extra},"effects":{effects}}}}}"#)
}

const FROM_GRAVEYARD: &str =
    r#"{"ZoneChange":{"what":["This"],"from":"Graveyard","to":"Battlefield"}}"#;
const DRAW_AND_LOSE: &str = r#"[{"Draw":{"count":2}},{"LoseLife":{"amount":2}}]"#;

#[rstest]
#[case::the_card_itself(
    trigger(FROM_GRAVEYARD, "", DRAW_AND_LOSE),
    "When this creature enters from a graveyard, you draw two cards and you lose 2 life."
)]
#[case::the_card_or_another_permanent(
    trigger(
        &format!(r#"{{"AnyOf":[{FROM_GRAVEYARD},{{"ZoneChange":{{"what":["Other"],"from":"Graveyard","to":"Battlefield"}}}}]}}"#),
        "",
        r#"[{"Draw":{}}]"#
    ),
    "Whenever this creature or another permanent enters from a graveyard, draw a card."
)]
#[case::a_spell_from_a_graveyard(
    trigger(
        r#"{"SpellCast":{"what":[{"CastFrom":"Graveyard"}],"by":"EachPlayer"}}"#,
        "",
        r#"[{"Draw":{}}]"#
    ),
    "Whenever a player casts a spell from a graveyard, draw a card."
)]
#[case::a_creature_spell_from_a_graveyard(
    trigger(
        r#"{"SpellCast":{"what":[{"Type":"Creature"},{"CastFrom":"Graveyard"}],"by":"You"}}"#,
        "",
        r#"[{"Draw":{}}]"#
    ),
    "Whenever you cast a creature spell from a graveyard, draw a card."
)]
#[case::a_land_untapped(
    trigger(
        r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#,
        r#","if":{"Matches":{"subject":"This","selector":["Untapped"]}}"#,
        r#"[{"Draw":{}}]"#
    ),
    "When this land enters untapped, draw a card."
)]
fn an_event_with_one_more_predicate_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::the_plain_entry("When this creature enters, you draw two cards and you lose 2 life.")]
#[case::your_graveyard(
    "When this creature enters from your graveyard, you draw two cards and you lose 2 life."
)]
#[case::exile("When this creature enters from exile, you draw two cards and you lose 2 life.")]
#[case::another_count(
    "When this creature enters from a graveyard, you draw three cards and you lose 2 life."
)]
#[case::another_loss(
    "When this creature enters from a graveyard, you draw two cards and you lose 3 life."
)]
#[case::dies("When this creature dies, you draw two cards and you lose 2 life.")]
fn a_card_that_enters_from_a_graveyard_does_not_read_as_another_entry(#[case] clause: &str) {
    assert!(
        !written(&trigger(FROM_GRAVEYARD, "", DRAW_AND_LOSE), clause),
        "{clause}"
    );
}

#[rstest]
#[case::from_your_graveyard("Whenever a player casts a spell from your graveyard, draw a card.")]
#[case::from_exile("Whenever a player casts a spell from exile, draw a card.")]
#[case::you_cast("Whenever you cast a spell from a graveyard, draw a card.")]
#[case::any_spell("Whenever a player casts a spell, draw a card.")]
#[case::a_creature_spell("Whenever a player casts a creature spell from a graveyard, draw a card.")]
#[case::enters_from_a_graveyard("Whenever a permanent enters from a graveyard, draw a card.")]
fn a_spell_cast_from_a_graveyard_does_not_read_as_another_spell(#[case] clause: &str) {
    let json = trigger(
        r#"{"SpellCast":{"what":[{"CastFrom":"Graveyard"}],"by":"EachPlayer"}}"#,
        "",
        r#"[{"Draw":{}}]"#,
    );
    assert!(!written(&json, clause), "{clause}");
}

#[rstest]
#[case::tapped("When this land enters tapped, draw a card.")]
#[case::plain("When this land enters, draw a card.")]
#[case::from_a_graveyard("When this land enters from a graveyard, draw a card.")]
fn a_land_that_enters_untapped_does_not_read_as_another_entry(#[case] clause: &str) {
    let json = trigger(
        r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#,
        r#","if":{"Matches":{"subject":"This","selector":["Untapped"]}}"#,
        r#"[{"Draw":{}}]"#,
    );
    assert!(!written(&json, clause), "{clause}");
}

fn static_group(group: &str, condition: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":{group}}},"mods":[{{"ModifyPT":{{"power":2,"toughness":2}}}}]{condition}}}}}"#
    )
}

const LANDS_TOO: &str =
    r#"[{"AnyOf":["This",{"Type":"Land"}]},{"Type":"Creature"},{"ControlledBy":"You"}]"#;
const SIX_LANDS: &str = r#","condition":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Ge","right":6}}"#;

#[rstest]
#[case::with_a_condition(
    static_group(LANDS_TOO, SIX_LANDS),
    "As long as you control six or more lands, this creature and land creatures you control get +2/+2."
)]
#[case::always(
    static_group(LANDS_TOO, ""),
    "This creature and land creatures you control get +2/+2."
)]
fn a_group_that_includes_the_card_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::without_the_card(
    "As long as you control six or more lands, land creatures you control get +2/+2."
)]
#[case::another_group(
    "As long as you control six or more lands, this creature and artifact creatures you control get +2/+2."
)]
#[case::another_bonus(
    "As long as you control six or more lands, this creature and land creatures you control get +1/+1."
)]
#[case::another_count(
    "As long as you control five or more lands, this creature and land creatures you control get +2/+2."
)]
#[case::the_card_alone("As long as you control six or more lands, this creature gets +2/+2.")]
fn a_group_that_includes_the_card_does_not_read_as_another_group(#[case] clause: &str) {
    assert!(
        !written(&static_group(LANDS_TOO, SIX_LANDS), clause),
        "{clause}"
    );
}

fn enters_with(what: &str, counter: &str, count: u32) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":{what}}}}},"by":{{"WithCounters":{{"counter":"{counter}","count":{count}}}}}}}}}"#
    )
}

const PLANESWALKERS: &str = r#"[{"Type":"Planeswalker"},{"ControlledBy":"You"}]"#;

#[rstest]
#[case::planeswalkers(
    enters_with(PLANESWALKERS, "loyalty", 1),
    "Each planeswalker you control enters with an additional loyalty counter on it."
)]
#[case::creatures(
    enters_with(r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#, "+1/+1", 1),
    "Each creature you control enters with an additional +1/+1 counter on it."
)]
fn a_group_entering_with_one_more_counter_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::two_counters(
    "Each planeswalker you control enters with two additional loyalty counters on it."
)]
#[case::another_kind(
    "Each planeswalker you control enters with an additional +1/+1 counter on it."
)]
#[case::another_type("Each creature you control enters with an additional loyalty counter on it.")]
#[case::an_opponent_controls(
    "Each planeswalker an opponent controls enters with an additional loyalty counter on it."
)]
#[case::not_additional("Each planeswalker you control enters with a loyalty counter on it.")]
#[case::the_card_itself("~ enters with an additional loyalty counter on it.")]
fn a_group_entering_with_one_more_counter_does_not_read_as_another_entry(#[case] clause: &str) {
    assert!(
        !written(&enters_with(PLANESWALKERS, "loyalty", 1), clause),
        "{clause}"
    );
}

#[rstest]
#[case::outlaws(
    "Whenever one or more outlaws you control deal combat damage to a player, create a Treasure token."
)]
fn outlaws_are_written_as_one_word(#[case] clause: &str) {
    let outlaw = r#"{"AnyOf":[{"Subtype":"Assassin"},{"Subtype":"Mercenary"},{"Subtype":"Pirate"},{"Subtype":"Rogue"},{"Subtype":"Warlock"}]}"#;
    let json = trigger(
        &format!(
            r#"{{"Batch":{{"DealsDamage":{{"source":[{outlaw},{{"ControlledBy":"You"}}],"to":"EachPlayer","combat":true}}}}}}"#
        ),
        "",
        r#"[{"CreateToken":{"token":{"Named":"treasure"}}}]"#,
    );
    assert!(written(&json, clause), "{clause}");
    assert!(!written(&json, &clause.replace("outlaws", "Pirates")));
}

const DEAD_CARD: &str = r#"{"Each":[{"In":"Graveyard"},"This"]}"#;

fn dies_then_returns(count: u32, to: &str) -> String {
    trigger(
        r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#,
        "",
        &format!(
            r#"[{{"Exile":{{"what":{DEAD_CARD}}}}},{{"ChooseAtRandom":{{"from":{{"Each":[{{"In":"Graveyard"}},{{"OwnedBy":"You"}},{{"Type":"Creature"}}]}},"count":{count},"as":"card"}}}},{{"Move":{{"what":{{"Bound":"card"}},"to":{to}}}}}]"#
        ),
    )
}

#[test]
fn the_card_a_trigger_exiles_after_it_died_is_it() {
    let json = dies_then_returns(2, r#"{"Battlefield":{}}"#);
    assert!(written(
        &json,
        "When this creature dies, exile it, then return two creature cards at random from your graveyard to the battlefield."
    ));
}

#[rstest]
#[case::three_cards(
    "When this creature dies, exile it, then return three creature cards at random from your graveyard to the battlefield."
)]
#[case::to_the_hand(
    "When this creature dies, exile it, then return two creature cards at random from your graveyard to your hand."
)]
#[case::another_card_type(
    "When this creature dies, exile it, then return two artifact cards at random from your graveyard to the battlefield."
)]
#[case::no_exile(
    "When this creature dies, return two creature cards at random from your graveyard to the battlefield."
)]
fn the_card_a_trigger_exiles_after_it_died_does_not_read_as_another_move(#[case] clause: &str) {
    let json = dies_then_returns(2, r#"{"Battlefield":{}}"#);
    assert!(!written(&json, clause), "{clause}");
}
