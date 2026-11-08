//! Goal: the pack of w20 reads back as the cards print it: an Aura of an artifact that cannot attack or block, an equipped creature that deals combat damage, the top card exiled from the library of that player, the top X cards exiled by counters, a card that may be put onto the battlefield with a reward when not, and a historic card in any order of its parts; another amount, kind, count or target never reads back.
use super::*;
const COCOON_STATIC: &str =
    r#"{"Static":{"affected":"Attached","mods":["CantAttack","CantBlock"]}}"#;
const COCOON_ONLY_ATTACK: &str = r#"{"Static":{"affected":"Attached","mods":["CantAttack"]}}"#;
const COCOON_RULE: &str = r#"{"Rule":{"rule":{"Cant":{"who":"EachPlayer","action":{"ActivateKind":{"kind":"Every","sources":[{"Is":"Attached"}]}}}}}}"#;

#[rstest]
#[case::artifact(COCOON_STATIC, true)]
#[case::only_attack(COCOON_ONLY_ATTACK, false)]
fn w20_an_aura_of_an_artifact_that_cannot_attack_or_block_is_one_sentence(
    #[case] first: &str,
    #[case] reads: bool,
) {
    let clause =
        "Enchanted artifact can't attack or block, and its activated abilities can't be activated.";
    let found = crate::compile::read::static_pair_texts(&ability(first), &ability(COCOON_RULE))
        .iter()
        .any(|wording| same_text(wording, clause));
    assert_eq!(found, reads);
}

const FISHING_GEAR: &str = r#"{"Triggered":{"effects":[{"ExileTop":{"as":"exiled","who":"Triggering"}},{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}],"subject":{"Bound":"exiled"}}},"then":[{"May":{"effects":[{"Move":{"as":"moved2","to":{"Battlefield":{"under":"You"}},"what":{"Bound":"exiled"}}}]}}]}},{"If":{"condition":{"CountOf":{"op":"Eq","right":0,"selector":[],"subject":{"Bound":"moved2"}}},"then":[{"CreateToken":{"token":{"Custom":{"colors":["Blue"],"name":"Fish","power":1,"subtypes":["Fish"],"toughness":1,"types":["Creature"]}}}}]}}],"when":{"DealsDamage":{"combat":true,"source":[{"Type":"Creature"},{"Is":"Attached"}],"to":"EachPlayer"}}}}"#;
const FISHING_CLAUSE: &str = "Whenever equipped creature deals combat damage to a player, exile the top card of that player's library. If it's a permanent card, you may put it onto the battlefield under your control. If you don't, create a 1/1 blue Fish creature token.";

const ROTISSERIE: &str = r#"{"Triggered":{"effects":[{"PutCounters":{"counter":{"Other":"skewer"},"on":"This"}},{"May":{"effects":[{"Sacrifice":{"as":"sacrificed","filter":["This"]}}]}},{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[],"subject":{"Bound":"sacrificed"}}},"then":[{"ExileTop":{"as":"exiled2","count":{"Counters":{"counter":{"Other":"skewer"},"on":"This"}}}},{"Rule":{"duration":"UntilEndOfTurn","rule":{"MayCast":{"from":"Exile","only":{"Bound":"exiled2"},"play":true}}}}]}}],"when":{"DealsDamage":{"combat":true,"source":["This"],"to":"EachPlayer"}}}}"#;
const ROTISSERIE_CLAUSE: &str = "Whenever ~ deals combat damage to a player, put a skewer counter on ~. Then you may sacrifice it. If you do, exile the top X cards of your library, where X is the number of skewer counters on ~. You may play those cards this turn.";

/// The ability with the text replaced and whether some wording of it is the clause.
fn w20_reads(json: &str, from: &str, to: &str, clause: &str) -> bool {
    assert!(json.contains(from), "{from}");
    written_variants(&ability(&json.replace(from, to)))
        .iter()
        .any(|wording| same_text(wording, clause))
}

#[rstest]
#[case::as_printed(
    FISHING_GEAR,
    "\"combat\":true",
    "\"combat\":true",
    FISHING_CLAUSE,
    true
)]
#[case::rotisserie(
    ROTISSERIE,
    "\"combat\":true",
    "\"combat\":true",
    ROTISSERIE_CLAUSE,
    true
)]
#[case::fish_blow(FISHING_GEAR, "\"combat\":true,", "", FISHING_CLAUSE, false)]
#[case::another_fish(FISHING_GEAR, "\"power\":1", "\"power\":2", FISHING_CLAUSE, false)]
#[case::their_control(FISHING_GEAR, "{\"under\":\"You\"}", "{}", FISHING_CLAUSE, false)]
#[case::your_library(
    FISHING_GEAR,
    "\"who\":\"Triggering\"",
    "\"who\":\"You\"",
    FISHING_CLAUSE,
    false
)]
#[case::lands_only(
    FISHING_GEAR,
    "\"selector\":[{\"AnyOf\":[{\"Type\":\"Artifact\"},{\"Type\":\"Battle\"},{\"Type\":\"Creature\"},{\"Type\":\"Enchantment\"},{\"Type\":\"Land\"},{\"Type\":\"Planeswalker\"}]}]",
    "\"selector\":[{\"Type\":\"Land\"}]",
    FISHING_CLAUSE,
    false
)]
#[case::fish_when_it_was_put(
    FISHING_GEAR,
    "\"op\":\"Eq\",\"right\":0",
    "\"op\":\"Ge\",\"right\":1",
    FISHING_CLAUSE,
    false
)]
#[case::another_counter(
    ROTISSERIE,
    "\"Other\":\"skewer\"},\"on\":\"This\"}}}},{\"Rule",
    "\"Other\":\"charge\"},\"on\":\"This\"}}}},{\"Rule",
    ROTISSERIE_CLAUSE,
    false
)]
#[case::fixed_count(
    ROTISSERIE,
    "{\"Counters\":{\"counter\":{\"Other\":\"skewer\"},\"on\":\"This\"}}",
    "3",
    ROTISSERIE_CLAUSE,
    false
)]
#[case::this_turn_only_for_ever(ROTISSERIE, "UntilEndOfTurn", "Forever", ROTISSERIE_CLAUSE, false)]
#[case::without_the_sacrifice(
    ROTISSERIE,
    "\"op\":\"Ge\",\"right\":1",
    "\"op\":\"Eq\",\"right\":0",
    ROTISSERIE_CLAUSE,
    false
)]
fn w20_the_pack_reads_back_as_printed(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(w20_reads(json, from, to, clause), reads);
}

#[rstest]
#[case::exile(
    "Whenever ~ deals combat damage to a player, exile the top card of that player's library.",
    Some(
        "Whenever ~ deals combat damage to a player, that player exiles the top card of their library."
    )
)]
#[case::exile_your_own("Whenever ~ attacks, exile the top card of your library.", None)]
#[case::untap(
    "that player discards a card and untap each land you control.",
    Some("that player discards a card and you untap all lands you control.")
)]
#[case::untap_theirs("untap each land target player controls.", None)]
#[case::destroy(
    "destroy up to one target planeswalker, then destroy up to one target artifact.",
    Some("destroy up to one target planeswalker and up to one target artifact.")
)]
#[case::destroy_all(
    "destroy up to one target planeswalker, then destroy all artifacts.",
    None
)]
#[case::historic_permanent(
    "return target historic permanent card from your graveyard to your hand.",
    Some("return target historic card from your graveyard to your hand.")
)]
#[case::historic_kind(
    "return target historic non-Assassin card from your graveyard to your hand.",
    Some("return target non-Assassin historic card from your graveyard to your hand.")
)]
#[case::attacking_historic(
    "target historic attacking creature gets +1/+1.",
    Some("target attacking historic creature gets +1/+1.")
)]
#[case::legendary_permanent(
    "return target legendary permanent card from your graveyard to your hand.",
    None
)]
fn w20_the_card_prints_another_wording(#[case] text: &str, #[case] printed: Option<&str>) {
    let found = crate::compile::read::t18_words::w20_wordings(text);
    assert_eq!(found.first().map(String::as_str), printed, "{found:?}");
}

const WHAT_MUST_BE_DONE_MODE: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[{"Type":"Creature"}],"subject":{"Target":0}}},"otherwise":[{"Move":{"to":{"Battlefield":{}},"what":{"Target":0}}}],"then":[{"Move":{"to":{"Battlefield":{"counters":[["+1/+1",2]]}},"what":{"Target":0}}}]}}],"targets":[{"what":{"Object":[{"AnyOf":[{"Supertype":"Legendary"},{"Type":"Artifact"},{"Subtype":"Saga"}]},{"OwnedBy":"You"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"In":"Graveyard"}]}}]}}"#;
const WHAT_MUST_BE_DONE_CLAUSE: &str = "Return target historic permanent card from your graveyard to the battlefield. It enters with two additional +1/+1 counters on it if it's a creature.";

#[rstest]
#[case::as_printed("\"Supertype\":\"Legendary\"", "\"Supertype\":\"Legendary\"", true)]
#[case::saga_first(
    "{\"AnyOf\":[{\"Supertype\":\"Legendary\"},{\"Type\":\"Artifact\"},{\"Subtype\":\"Saga\"}]}",
    "{\"AnyOf\":[{\"Subtype\":\"Saga\"},{\"Type\":\"Artifact\"},{\"Supertype\":\"Legendary\"}]}",
    true
)]
#[case::not_a_saga("{\"Subtype\":\"Saga\"}", "{\"Subtype\":\"Aura\"}", false)]
#[case::not_legendary("{\"Supertype\":\"Legendary\"},", "", false)]
#[case::three_counters("[\"+1/+1\",2]", "[\"+1/+1\",3]", false)]
#[case::from_exile("{\"In\":\"Graveyard\"}", "{\"In\":\"Exile\"}", false)]
#[case::not_yours("{\"OwnedBy\":\"You\"}", "{\"OwnedBy\":\"Opponents\"}", false)]
fn w20_historic_in_any_order_is_historic(
    #[case] from: &str,
    #[case] to: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        w20_reads(WHAT_MUST_BE_DONE_MODE, from, to, WHAT_MUST_BE_DONE_CLAUSE),
        reads
    );
}
