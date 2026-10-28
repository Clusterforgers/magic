//! Goal: the writers the sixth sample of the names G to L needed give the oracle wording of each whole ability of the
//! sample's proposals, of a card returned from the graveyard into an attack for a choice or a payment, and of a rule
//! that triggers abilities again for a draw or a death; another count, kind, step, zone, group, cost or duration never
//! reads back.
use super::*;
use rstest::rstest;

/// Whether some wording the checker accepts for the ability is the clause.
fn c6_gl_reads(ability: &Ability, clause: &str) -> bool {
    written_variants(ability)
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// The ability of the proposal whose wording is the clause.
fn c6_gl_proposal_ability(proposal: &str, clause: &str) -> Ability {
    let card: serde_json::Value = serde_json::from_str(proposal).expect("proposal JSON");
    card["faces"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|face| face["abilities"].as_array().cloned().unwrap_or_default())
        .map(|value| serde_json::from_value::<Ability>(value).expect("ability"))
        .find(|ability| c6_gl_reads(ability, clause))
        .unwrap_or_else(|| panic!("no ability of the proposal reads {clause}"))
}

macro_rules! c6_gl_proposal {
    ($slug:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/proposals/r2dc/",
            $slug,
            ".json"
        ))
    };
}

#[rstest]
#[case::spawn_for_cards_left(
    c6_gl_proposal!("glimpse_the_impossible"),
    "Exile the top three cards of your library. You may play those cards this turn. At the beginning of the next end step, if any of those cards remain exiled, put them into your graveyard, then create a 0/1 colorless Eldrazi Spawn creature token for each card put into your graveyard this way. Those tokens have \"Sacrifice this token: Add {C}.\"",
    r#""count":3"#,
    r#""count":4"#
)]
#[case::player_or_planeswalker(
    c6_gl_proposal!("grateful_apparition"),
    "Whenever this creature deals combat damage to a player or planeswalker, proliferate.",
    "Planeswalker",
    "Creature"
)]
#[case::two_copies(
    c6_gl_proposal!("gruff_triplets"),
    "When this creature enters, if it isn't a token, create two tokens that are copies of it.",
    r#""count":2"#,
    r#""count":3"#
)]
#[case::counters_on_the_named(
    c6_gl_proposal!("gruff_triplets"),
    "When this creature dies, put a number of +1/+1 counters equal to its power on each creature you control named ~.",
    "Power",
    "Toughness"
)]
#[case::deserts_from_the_graveyard(
    c6_gl_proposal!("hazezon_shaper_of_sand"),
    "You may play Desert lands from your graveyard.",
    "Desert",
    "Forest"
)]
#[case::sacrifice_then_damage(
    c6_gl_proposal!("heart_piercer_manticore"),
    "When this creature enters, you may sacrifice another creature. When you do, this creature deals damage equal to that creature's power to any target.",
    "Power",
    "Toughness"
)]
#[case::chosen_type_costs_less(
    c6_gl_proposal!("heralds_horn"),
    "Creature spells you cast of the chosen type cost {1} less to cast.",
    "{1}",
    "{2}"
)]
#[case::chosen_type_to_hand(
    c6_gl_proposal!("heralds_horn"),
    "At the beginning of your upkeep, look at the top card of your library. If it's a creature card of the chosen type, you may reveal it and put it into your hand.",
    r#""whose":"You""#,
    r#""whose":"Opponents""#
)]
#[case::free_from_a_graveyard(
    c6_gl_proposal!("ignite_the_future"),
    "Exile the top three cards of your library. Until the end of your next turn, you may play those cards. If this spell was cast from a graveyard, you may play cards this way without paying their mana costs.",
    "UntilEndOfYourNextTurn",
    "UntilEndOfTurn"
)]
#[case::twice_from_a_graveyard(
    c6_gl_proposal!("increasing_vengeance"),
    "Copy target instant or sorcery spell you control. If this spell was cast from a graveyard, copy that spell twice instead. You may choose new targets for the copies.",
    r#""count":2"#,
    r#""count":3"#
)]
#[case::any_number_if_kicked(
    c6_gl_proposal!("inscription_of_abundance"),
    "Choose one. If this spell was kicked, choose any number instead.\n• Put two +1/+1 counters on target creature.\n• Target player gains X life, where X is the greatest power among creatures they control.\n• Target creature you control fights target creature you don't control.",
    r#""all_if":"Kicked","#,
    ""
)]
#[case::paid_dinosaur_damage(
    c6_gl_proposal!("itzquinth_firstborn_of_gishath"),
    "When ~ enters, you may pay {2}. When you do, target Dinosaur you control deals damage equal to its power to another target creature.",
    "{2}",
    "{3}"
)]
#[case::two_of_three(
    c6_gl_proposal!("keskit_the_flesh_sculptor"),
    "{T}, Sacrifice three other artifacts and/or creatures: Look at the top three cards of your library. Put two of them into your hand and the other into your graveyard.",
    r#""to":"Graveyard""#,
    r#""to":"Hand""#
)]
#[case::damage_dealt_this_turn(
    c6_gl_proposal!("knollspine_dragon"),
    "When this creature enters, you may discard your hand and draw cards equal to the damage dealt to target opponent this turn.",
    "DamageTaken",
    "LifeLost"
)]
#[case::land_becomes_hydra(
    c6_gl_proposal!("lair_of_the_hydra"),
    "{X}{G}: Until end of turn, this land becomes an X/X green Hydra creature. It's still a land. X can't be 0.",
    "Hydra",
    "Wurm"
)]
#[case::distribute_life_gained(
    c6_gl_proposal!("lathiel_the_bounteous_dawn"),
    "At the beginning of each end step, if you gained life this turn, distribute up to that many +1/+1 counters among any number of other target creatures.",
    r#""step":"End""#,
    r#""step":"Upkeep""#
)]
#[case::goaded_for_the_game(
    c6_gl_proposal!("life_of_the_party"),
    "When this creature enters, if it's not a token, each opponent creates a token that's a copy of it. The tokens are goaded for the rest of the game.",
    "Forever",
    "UntilEndOfTurn"
)]
#[case::extra_draw_for_a_land(
    c6_gl_proposal!("lord_windgrace"),
    "+2: Discard a card, then draw a card. If a land card is discarded this way, draw an additional card.",
    "Land",
    "Creature"
)]
#[case::chosen_opponents(
    c6_gl_proposal!("intellectual_offering"),
    "Choose an opponent. You and that player each draw three cards.\nChoose an opponent. Untap all nonland permanents you control and all nonland permanents that player controls.",
    r#""count":3"#,
    r#""count":2"#
)]
#[case::graveyards_return(
    c6_gl_proposal!("living_death"),
    "Each player exiles all creature cards from their graveyard, then sacrifices all creatures they control, then puts all cards they exiled this way onto the battlefield.",
    r#""In":"Graveyard""#,
    r#""In":"Hand""#
)]
#[case::choose_two_of_four(
    c6_gl_proposal!("incendiary_command"),
    "Choose two —\n• ~ deals 4 damage to target player or planeswalker.\n• ~ deals 2 damage to each creature.\n• Destroy target nonbasic land.\n• Each player discards all the cards in their hand, then draws that many cards.",
    r#""amount":4"#,
    r#""amount":5"#
)]
#[case::steal_or_fling(
    c6_gl_proposal!("grab_the_reins"),
    "Choose one —\n• Until end of turn, you gain control of target creature and it gains haste.\n• Sacrifice a creature. ~ deals damage equal to that creature's power to any target.",
    "Haste",
    "Trample"
)]
fn c6_gl_wholes_read_back(
    #[case] proposal: &str,
    #[case] clause: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let ability = c6_gl_proposal_ability(proposal, clause);
    let json = serde_json::to_string(&ability).expect("JSON");
    assert!(json.contains(from), "{json}");
    let near = super::ability(&json.replacen(from, to, 1));
    assert!(
        !c6_gl_reads(&near, clause),
        "near-miss reads back: {clause}"
    );
}

const C6_GL_JOCASTA: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":["Commander",{"OwnedBy":"You"},{"ControlledBy":"You"}]}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{"tapped":true}}}},{"JoinAttack":{"what":"This"}}]}}]}}"#;
const C6_GL_INTERCEPTOR: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Supertype":"Legendary"},{"Type":"Creature"},{"ControlledBy":"You"}]}},"from":"Graveyard","effects":[{"Pay":{"cost":"{2}{B}","paid":[{"Move":{"what":"This","to":{"Battlefield":{"tapped":true}}}},{"JoinAttack":{"what":"This"}}]}}]}}"#;
const C6_GL_MAY: &str =
    "if this card is in your graveyard, you may return it to the battlefield tapped and attacking.";
const C6_GL_PAY: &str =
    "If you do, return this card from your graveyard to the battlefield tapped and attacking.";

#[rstest]
#[case::commander_may(C6_GL_JOCASTA, "", "", "your commander", C6_GL_MAY, true)]
#[case::legendary_pay(
    C6_GL_INTERCEPTOR,
    "",
    "",
    "one or more legendary creatures",
    C6_GL_PAY,
    true
)]
#[case::creatures_may(
    C6_GL_JOCASTA,
    r#"["Commander",{"OwnedBy":"You"},"#,
    r#"[{"Type":"Creature"},"#,
    "one or more creatures",
    C6_GL_MAY,
    true
)]
#[case::another_group(
    C6_GL_JOCASTA,
    r#""Commander""#,
    r#"{"Type":"Artifact"}"#,
    "your commander",
    C6_GL_MAY,
    false
)]
#[case::untapped(
    C6_GL_JOCASTA,
    r#""tapped":true"#,
    r#""tapped":false"#,
    "your commander",
    C6_GL_MAY,
    false
)]
#[case::not_attacking(
    C6_GL_INTERCEPTOR,
    r#",{"JoinAttack":{"what":"This"}}"#,
    "",
    "one or more legendary creatures",
    C6_GL_PAY,
    false
)]
#[case::from_the_battlefield(
    C6_GL_JOCASTA,
    r#""from":"Graveyard","#,
    "",
    "your commander",
    C6_GL_MAY,
    false
)]
#[case::other_mana(
    C6_GL_INTERCEPTOR,
    "{2}{B}",
    "{3}{B}",
    "one or more legendary creatures",
    C6_GL_PAY,
    false
)]
fn c6_gl_returns_into_an_attack(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] group: &str,
    #[case] tail: &str,
    #[case] reads: bool,
) {
    let changed = json.replacen(from, to, 1);
    let clause = if tail == C6_GL_PAY {
        format!("Whenever you attack with {group}, you may pay {{2}}{{B}}. {tail}")
    } else {
        format!("Whenever you attack with {group}, {tail}")
    };
    assert_eq!(
        c6_gl_reads(&super::ability(&changed), &clause),
        reads,
        "{clause}"
    );
}

#[rstest]
#[case::a_draw(r#"{"Drew":{}}"#, "a player drawing a card", true)]
#[case::a_death(
    r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}}"#,
    "a creature dying",
    true
)]
#[case::your_draw(r#"{"Drew":{"who":"You"}}"#, "a player drawing a card", false)]
#[case::an_exile(
    r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Exile"}}"#,
    "a creature dying",
    false
)]
fn c6_gl_trigger_again_causes(#[case] cause: &str, #[case] words: &str, #[case] reads: bool) {
    let json = format!(r#"{{"Rule":{{"rule":{{"TriggerAgain":{{"cause":{cause}}}}}}}}}"#);
    let clause = format!(
        "If {words} causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
    );
    assert_eq!(
        c6_gl_reads(&super::ability(&json), &clause),
        reads,
        "{clause}"
    );
}
