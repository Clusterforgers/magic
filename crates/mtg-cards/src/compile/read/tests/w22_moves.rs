//! Goal: the w22 writers give the oracle wording of cards returned that were put into a graveyard from the battlefield this turn, of the card with the ability or the card it enchants put into a library or a hand, and of a card that returns itself with a counter; another zone, place, count or owner never reads back as the clause.
use super::*;

fn reads(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn step_end(effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{effect}],"when":{{"StepBegins":{{"step":"End","whose":"You"}}}}}}}}"#
    )
}

const RETURN_GRAVED: &str = r#"{"Move":{"to":"Hand","what":{"Each":[{"Type":"Creature"},{"OwnedBy":"You"},{"ArrivedFrom":"Battlefield"},{"In":"Graveyard"}]}}}"#;

#[rstest]
#[case::as_printed(
    RETURN_GRAVED,
    "At the beginning of your end step, return to your hand all creature cards in your graveyard that were put there from the battlefield this turn.",
    true
)]
#[case::not_from_the_battlefield(
    &RETURN_GRAVED.replace("Battlefield", "Library"),
    "At the beginning of your end step, return to your hand all creature cards in your graveyard that were put there from the battlefield this turn.",
    false
)]
#[case::other_graveyard(
    &RETURN_GRAVED.replace(r#"{"OwnedBy":"You"},"#, ""),
    "At the beginning of your end step, return to your hand all creature cards in your graveyard that were put there from the battlefield this turn.",
    false
)]
#[case::other_card_type(
    &RETURN_GRAVED.replace("Creature", "Artifact"),
    "At the beginning of your end step, return to your hand all creature cards in your graveyard that were put there from the battlefield this turn.",
    false
)]
#[case::to_the_battlefield(
    &RETURN_GRAVED.replace(r#""to":"Hand""#, r#""to":{"Battlefield":{}}"#),
    "At the beginning of your end step, return to your hand all creature cards in your graveyard that were put there from the battlefield this turn.",
    false
)]
fn w22_cards_put_there_this_turn_are_returned(
    #[case] effect: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(&step_end(effect), clause), same, "{clause}");
}

#[rstest]
#[case::chosen_and_put(
    r#"{"Spell":{"effects":[{"Move":{"to":{"Battlefield":{"under":"You"}},"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ArrivedFrom":"Battlefield"},{"In":"Graveyard"}]}}]}}"#,
    "Choose target creature card in a graveyard that was put there from the battlefield this turn. Put that card onto the battlefield under your control.",
    true
)]
#[case::put_directly(
    r#"{"Spell":{"effects":[{"Move":{"to":{"Battlefield":{"under":"You"}},"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ArrivedFrom":"Battlefield"},{"In":"Graveyard"}]}}]}}"#,
    "Put target creature card in a graveyard that was put there from the battlefield this turn onto the battlefield under your control.",
    true
)]
#[case::from_the_library(
    r#"{"Spell":{"effects":[{"Move":{"to":{"Battlefield":{"under":"You"}},"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ArrivedFrom":"Library"},{"In":"Graveyard"}]}}]}}"#,
    "Choose target creature card in a graveyard that was put there from the battlefield this turn. Put that card onto the battlefield under your control.",
    false
)]
#[case::under_its_owner(
    r#"{"Spell":{"effects":[{"Move":{"to":{"Battlefield":{}},"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ArrivedFrom":"Battlefield"},{"In":"Graveyard"}]}}]}}"#,
    "Choose target creature card in a graveyard that was put there from the battlefield this turn. Put that card onto the battlefield under your control.",
    false
)]
fn w22_a_chosen_card_put_there_this_turn(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

#[rstest]
#[case::top(
    r#"{"Activated":{"cost":"{U}","effects":[{"Move":{"to":{"Library":{"place":"Top"}},"what":"This"}}]}}"#,
    "{U}: Put this creature on top of its owner's library.",
    true
)]
#[case::bottom_for_top(
    r#"{"Activated":{"cost":"{U}","effects":[{"Move":{"to":{"Library":{"place":"Bottom"}},"what":"This"}}]}}"#,
    "{U}: Put this creature on top of its owner's library.",
    false
)]
#[case::enchanted_third(
    r#"{"Activated":{"cost":"{3}{U}{U}","effects":[{"Move":{"to":{"Library":{"place":{"Below":2}}},"what":"Attached"}}]}}"#,
    "{3}{U}{U}: Put enchanted creature into its owner's library third from the top.",
    true
)]
#[case::enchanted_second_for_third(
    r#"{"Activated":{"cost":"{3}{U}{U}","effects":[{"Move":{"to":{"Library":{"place":{"Below":1}}},"what":"Attached"}}]}}"#,
    "{3}{U}{U}: Put enchanted creature into its owner's library third from the top.",
    false
)]
#[case::enchanted_hand(
    r#"{"Activated":{"cost":"{W}","effects":[{"Move":{"to":"Hand","what":"Attached"}}]}}"#,
    "{W}: Return enchanted creature to its owner's hand.",
    true
)]
#[case::enchanted_graveyard_for_hand(
    r#"{"Activated":{"cost":"{W}","effects":[{"Move":{"to":"Graveyard","what":"Attached"}}]}}"#,
    "{W}: Return enchanted creature to its owner's hand.",
    false
)]
#[case::from_the_graveyard(
    r#"{"Activated":{"cost":"{2}{G}","effects":[{"Move":{"to":{"Library":{"place":{"Below":2}}},"what":"This"}}],"from":"Graveyard"}}"#,
    "{2}{G}: Put this card from your graveyard into your library third from the top.",
    true
)]
#[case::top_from_the_graveyard(
    r#"{"Activated":{"cost":"{5}{B}{B}","effects":[{"Move":{"to":{"Library":{"place":"Top"}},"what":"This"}}],"from":"Graveyard"}}"#,
    "{5}{B}{B}: Put this creature on top of your library from your graveyard.",
    true
)]
#[case::top_from_the_battlefield_for_the_graveyard(
    r#"{"Activated":{"cost":"{5}{B}{B}","effects":[{"Move":{"to":{"Library":{"place":"Top"}},"what":"This"}}]}}"#,
    "{5}{B}{B}: Put this creature on top of your library from your graveyard.",
    false
)]
#[case::from_the_battlefield_not_the_graveyard(
    r#"{"Activated":{"cost":"{2}{G}","effects":[{"Move":{"to":{"Library":{"place":{"Below":2}}},"what":"This"}}]}}"#,
    "{2}{G}: Put this card from your graveyard into your library third from the top.",
    false
)]
fn w22_a_card_put_into_a_library_or_a_hand(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

#[rstest]
#[case::graveyards(
    r#"{"Spell":{"effects":[{"Move":{"to":{"Library":{"place":"Shuffled"}},"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"EachPlayer"}]}}}]}}"#,
    "Each player shuffles their graveyard into their library.",
    true
)]
#[case::hands_for_graveyards(
    r#"{"Spell":{"effects":[{"Move":{"to":{"Library":{"place":"Shuffled"}},"what":{"Each":[{"In":"Hand"},{"OwnedBy":"EachPlayer"}]}}}]}}"#,
    "Each player shuffles their graveyard into their library.",
    false
)]
#[case::on_top_for_shuffled(
    r#"{"Spell":{"effects":[{"Move":{"to":{"Library":{"place":"Top"}},"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"EachPlayer"}]}}}]}}"#,
    "Each player shuffles their graveyard into their library.",
    false
)]
fn w22_every_graveyard_shuffled_in(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

#[rstest]
#[case::finality(
    r#"[["Other","finality"],1]"#,
    "{B}{R}: Return this card from your graveyard to the battlefield with a finality counter on it.",
    true
)]
#[case::two_counters(
    r#"[["Other","finality"],2]"#,
    "{B}{R}: Return this card from your graveyard to the battlefield with a finality counter on it.",
    false
)]
fn w22_a_card_returns_itself_with_a_counter(
    #[case] counter: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    let counter = counter.replacen(r#"["Other","finality"]"#, r#"{"Other":"finality"}"#, 1);
    let json = format!(
        r#"{{"Activated":{{"cost":"{{B}}{{R}}","effects":[{{"Move":{{"to":{{"Battlefield":{{"counters":[{counter}]}}}},"what":"This"}}}}],"from":"Graveyard"}}}}"#
    );
    assert_eq!(reads(&json, clause), same, "{clause}");
}
