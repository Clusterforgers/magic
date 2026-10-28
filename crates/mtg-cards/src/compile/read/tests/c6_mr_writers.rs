//! Goal: the writers of the proposal checker give the oracle wording of the sixth sample of the names M to R (an attack of you, your first spell in an opponent's turn, a look at the top X cards, a free cast from among them, the other card into your graveyard, a free play of an exiled card, counters on the source in an activation condition, an exiled card copied and cast, "If you do" with two steps, a target player's return, a land from your hand or graveyard, damage equal to an amount and a spell that chooses three modes), and a wrong player, number, zone, cost or step never reads back.
use rstest::rstest;

use super::*;

/// Whether some wording the checker accepts for the rules is the clause, with the wordings for a failed assertion.
fn c6_mr_written(json: &str, clause: &str) -> (bool, Vec<String>) {
    let wordings = written_variants(&ability(json));
    let found = wordings.iter().any(|wording| same_text(wording, clause));
    (found, wordings)
}

/// "Whenever a creature with power 4 or greater attacks you, its controller loses 2 life and you gain 2 life."
const C6_MR_MACCREADY: &str = r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"},{"Compare":{"op":"Ge","to":4,"stat":"Power"}}],"at":"You"}},"effects":[{"LoseLife":{"who":{"ControllerOf":"Triggering"},"amount":2}},{"GainLife":{"amount":2}}]}}"#;

/// "Whenever you cast your first spell during each opponent's turn, look at the top two cards of your library. Put one of those cards into your hand and the other into your graveyard."
const C6_MR_NYMRIS: &str = r#"{"Triggered":{"when":{"Nth":{"nth":1,"event":{"SpellCast":{"by":"You"}}}},"if":{"Not":"YourTurn"},"effects":[{"LookAtTop":{"count":2,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"card","rest_as":"rest"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":"Graveyard"}}]}}"#;

/// "{3}{U}, {T}: Look at the top X cards of your library, where X is the greatest mana value among artifacts you control. You may put an artifact card from among them onto the battlefield. Put the rest on the bottom of your library in any order."
const C6_MR_MUZZIO: &str = r#"{"Activated":{"cost":[{"Mana":"{3}{U}"},"Tap"],"effects":[{"LookAtTop":{"count":{"Greatest":{"of":[{"Type":"Artifact"},{"ControlledBy":"You"}],"stat":"ManaValue"}},"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Artifact"}]},"count":{"min":0,"max":1},"as":"kept"}},{"Move":{"what":{"Bound":"kept"},"to":{"Battlefield":{}}}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}"#;

/// "{3}, {T}: Look at the top X cards of your library, where X is the number of Bobbleheads you control. You may cast a spell with mana value 3 or less from among them without paying its mana cost. Put the rest on the bottom of your library in a random order."
const C6_MR_BOBBLEHEAD: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"effects":[{"LookAtTop":{"count":{"Count":[{"Subtype":"Bobblehead"},{"ControlledBy":"You"}]},"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}]},"count":{"min":0,"max":1},"as":"kept"}},{"CastFromZone":{"what":{"Bound":"kept"},"free":true}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#;

/// "{T}: Exile the top card of your library. Until end of turn, you may play that card without paying its mana cost. Activate only if there are three or more brick counters on this artifact."
const C6_MR_ORACLES_VAULT: &str = r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"brick"}}},"op":"Ge","right":3}},"effects":[{"ExileTop":{"as":"impulse"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true,"free":true}},"duration":"UntilEndOfTurn"}}]}}"#;

/// "At the beginning of your upkeep, you may sacrifice another creature. If you do, put a +1/+1 counter on this creature and exile the top card of your library. You may play that card this turn."
const C6_MR_REDCAP: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"May":{"effects":[{"Sacrifice":{"filter":["Other",{"Type":"Creature"}]}}]}},{"If":{"condition":"Performed","then":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"ExileTop":{"as":"impulse"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true}},"duration":"UntilEndOfTurn"}}]}}]}}"#;

/// "Whenever this creature attacks, exile up to one target instant or sorcery card from your graveyard and copy it. You may cast the copy without paying its mana cost."
const C6_MR_RENEGADE_BULL: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"CopyCard":{"what":{"Bound":"exiled"},"as":"copy"}},{"CastFromZone":{"what":{"Bound":"copy"},"free":true}}]}}"#;

/// "When this creature enters, it deals damage equal to the number of spells you've cast this turn to any target."
const C6_MR_NOISE_MARINE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Tallied":{"who":"You","what":"SpellsCast"}}}}]}}"#;

/// The three modes of Riveteers Confluence.
const C6_MR_RIVETEERS: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":3,"max":3},"repeat":true,"modes":[{"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]},{"effects":[{"DealDamage":{"to":{"Each":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"Not":{"ControlledBy":"You"}}]},"amount":1}}]},{"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"AnyOf":[{"In":"Hand"},{"In":"Graveyard"}]},{"OwnedBy":"You"},{"Type":"Land"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}]}}]}]}}]}}"#;

/// The oracle text of Riveteers Confluence.
const C6_MR_RIVETEERS_TEXT: &str = "Choose three. You may choose the same mode more than once.\n• You draw a card and you lose 1 life.\n• ~ deals 1 damage to each creature and planeswalker you don't control.\n• You may put a land card from your hand or graveyard onto the battlefield tapped.";

/// The three modes of Obscura Confluence.
const C6_MR_OBSCURA: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":3,"max":3},"repeat":true,"modes":[{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["LoseAbilities",{"SetPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]},{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Connive":{"what":{"Target":0}}}]},{"targets":[{"what":"Player"}],"effects":[{"Choose":{"who":{"Target":0},"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}},{"Type":"Creature"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}}]}]}}]}}"#;

/// The oracle text of Obscura Confluence.
const C6_MR_OBSCURA_TEXT: &str = "Choose three. You may choose the same mode more than once.\n• Until end of turn, target creature loses all abilities and has base power and toughness 1/1.\n• Target creature connives.\n• Target player returns a creature card from their graveyard to their hand.";

#[rstest]
#[case::attacks_you(
    C6_MR_MACCREADY,
    "Whenever a creature with power 4 or greater attacks you, its controller loses 2 life and you gain 2 life."
)]
#[case::first_spell(
    C6_MR_NYMRIS,
    "Whenever you cast your first spell during each opponent's turn, look at the top two cards of your library. Put one of those cards into your hand and the other into your graveyard."
)]
#[case::top_x(
    C6_MR_MUZZIO,
    "{3}{U}, {T}: Look at the top X cards of your library, where X is the greatest mana value among artifacts you control. You may put an artifact card from among them onto the battlefield. Put the rest on the bottom of your library in any order."
)]
#[case::cast_among(
    C6_MR_BOBBLEHEAD,
    "{3}, {T}: Look at the top X cards of your library, where X is the number of Bobbleheads you control. You may cast a spell with mana value 3 or less from among them without paying its mana cost. Put the rest on the bottom of your library in a random order."
)]
#[case::free_play(
    C6_MR_ORACLES_VAULT,
    "{T}: Exile the top card of your library. Until end of turn, you may play that card without paying its mana cost. Activate only if there are three or more brick counters on this artifact."
)]
#[case::if_you_do(
    C6_MR_REDCAP,
    "At the beginning of your upkeep, you may sacrifice another creature. If you do, put a +1/+1 counter on this creature and exile the top card of your library. You may play that card this turn."
)]
#[case::copy_cast(
    C6_MR_RENEGADE_BULL,
    "Whenever this creature attacks, exile up to one target instant or sorcery card from your graveyard and copy it. You may cast the copy without paying its mana cost."
)]
#[case::equal_to(
    C6_MR_NOISE_MARINE,
    "When this creature enters, it deals damage equal to the number of spells you've cast this turn to any target."
)]
#[case::modes(C6_MR_RIVETEERS, C6_MR_RIVETEERS_TEXT)]
#[case::their_card(C6_MR_OBSCURA, C6_MR_OBSCURA_TEXT)]
fn c6_mr_wordings_read_back(#[case] json: &str, #[case] clause: &str) {
    let (found, wordings) = c6_mr_written(json, clause);
    assert!(found, "{clause}: {wordings:#?}");
}

#[rstest]
#[case::other_player(C6_MR_MACCREADY, r#""at":"You""#, r#""at":"Opponents""#)]
#[case::any_turn(C6_MR_NYMRIS, r#","if":{"Not":"YourTurn"}"#, "")]
#[case::second_spell(C6_MR_NYMRIS, r#""nth":1"#, r#""nth":2"#)]
#[case::other_to_hand(
    C6_MR_NYMRIS,
    r#"{"Bound":"rest"},"to":"Graveyard""#,
    r#"{"Bound":"rest"},"to":"Hand""#
)]
#[case::other_count(C6_MR_MUZZIO, r#""stat":"ManaValue""#, r#""stat":"Power""#)]
#[case::paid_cast(C6_MR_BOBBLEHEAD, r#""free":true"#, r#""free":false"#)]
#[case::paid_play(C6_MR_ORACLES_VAULT, r#""play":true,"free":true"#, r#""play":true"#)]
#[case::fewer_counters(C6_MR_ORACLES_VAULT, r#""right":3"#, r#""right":2"#)]
#[case::if_not_done(
    C6_MR_REDCAP,
    r#""condition":"Performed""#,
    r#""condition":{"Not":"Performed"}"#
)]
#[case::paid_copy(C6_MR_RENEGADE_BULL, r#""free":true"#, r#""free":false"#)]
#[case::other_tally(C6_MR_NOISE_MARINE, r#""SpellsCast""#, r#""Drawn""#)]
#[case::two_modes(C6_MR_RIVETEERS, r#""min":3,"max":3"#, r#""min":2,"max":2"#)]
#[case::hand_only(
    C6_MR_RIVETEERS,
    r#"{"AnyOf":[{"In":"Hand"},{"In":"Graveyard"}]}"#,
    r#"{"In":"Hand"}"#
)]
#[case::once_each(C6_MR_RIVETEERS, r#""repeat":true"#, r#""repeat":false"#)]
#[case::their_battlefield(
    C6_MR_OBSCURA,
    r#"{"Bound":"card"},"to":"Hand""#,
    r#"{"Bound":"card"},"to":{"Battlefield":{}}"#
)]
fn c6_mr_near_misses_do_not_read_back(#[case] json: &str, #[case] from: &str, #[case] to: &str) {
    assert!(json.contains(from), "{from}");
    let missed = json.replacen(from, to, 1);
    let clause = match json {
        C6_MR_MACCREADY => {
            "Whenever a creature with power 4 or greater attacks you, its controller loses 2 life and you gain 2 life."
        }
        C6_MR_NYMRIS => {
            "Whenever you cast your first spell during each opponent's turn, look at the top two cards of your library. Put one of those cards into your hand and the other into your graveyard."
        }
        C6_MR_MUZZIO => {
            "{3}{U}, {T}: Look at the top X cards of your library, where X is the greatest mana value among artifacts you control. You may put an artifact card from among them onto the battlefield. Put the rest on the bottom of your library in any order."
        }
        C6_MR_BOBBLEHEAD => {
            "{3}, {T}: Look at the top X cards of your library, where X is the number of Bobbleheads you control. You may cast a spell with mana value 3 or less from among them without paying its mana cost. Put the rest on the bottom of your library in a random order."
        }
        C6_MR_ORACLES_VAULT => {
            "{T}: Exile the top card of your library. Until end of turn, you may play that card without paying its mana cost. Activate only if there are three or more brick counters on this artifact."
        }
        C6_MR_REDCAP => {
            "At the beginning of your upkeep, you may sacrifice another creature. If you do, put a +1/+1 counter on this creature and exile the top card of your library. You may play that card this turn."
        }
        C6_MR_RENEGADE_BULL => {
            "Whenever this creature attacks, exile up to one target instant or sorcery card from your graveyard and copy it. You may cast the copy without paying its mana cost."
        }
        C6_MR_NOISE_MARINE => {
            "When this creature enters, it deals damage equal to the number of spells you've cast this turn to any target."
        }
        C6_MR_RIVETEERS => C6_MR_RIVETEERS_TEXT,
        _ => C6_MR_OBSCURA_TEXT,
    };
    let (found, wordings) = c6_mr_written(&missed, clause);
    assert!(!found, "{missed} reads as {clause}: {wordings:#?}");
}
