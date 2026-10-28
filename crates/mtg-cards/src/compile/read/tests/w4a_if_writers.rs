//! Goal: a conditional step reads back as the oracle words it ("If X, Y.", "Y if X.", "Otherwise, Z.", "If you can't, Z.", "Then if X, Y.", "Z. If X, create N of those tokens instead."); another condition, number, kind, player or branch never does.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn w4a_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const W4A_MONUMENT: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#;
const W4A_PASSAGE: &str = r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Ge","right":4}},"then":[{"Untap":{"what":{"Bound":"land"}}}]}}]}}"#;
const W4A_HIGH_SCORE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}},"op":"Ge","right":{"Greatest":{"of":[{"Type":"Creature"}],"stat":"Power"}}}},"then":[{"Draw":{}}]}}]}}"#;
const W4A_DEVOTION: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Matches":{"subject":"This","selector":[{"CastFrom":"Graveyard"}]}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Human","colors":["White"],"types":["Creature"],"subtypes":["Human"],"power":1,"toughness":1}},"count":10}}],"otherwise":[{"CreateToken":{"token":{"Custom":{"name":"Human","colors":["White"],"types":["Creature"],"subtypes":["Human"],"power":1,"toughness":1}},"count":5}}]}}]}}"#;
const W4A_TELLER: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}}]},"targets":[{"what":{"Object":[{"In":"Graveyard"}]}}],"effects":[{"Exile":{"what":{"Target":0}}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Type":"Creature"}]}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Rogue","colors":["Black"],"types":["Creature"],"subtypes":["Rogue"],"power":2,"toughness":2}}}}],"otherwise":[{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Type":"Land"}]}},"then":[{"CreateToken":{"token":{"Named":"treasure"}}}],"otherwise":[{"GainLife":{"amount":3}}]}}]}}]}}"#;
const W4A_EMPATHY: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}},"op":"Ge","right":{"Greatest":{"of":[{"Type":"Creature"}],"stat":"Power"}}}},"then":[{"Draw":{}}],"otherwise":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"creature"}},{"PutCounters":{"on":{"Bound":"creature"},"counter":"+1/+1"}}]}}]}}"#;
const W4A_REVELRY: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"HighestLife":"Opponents"},"op":"Gt","right":{"Life":"You"}}},"then":[{"GainLife":{"amount":4}}]}},{"If":{"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"op":"Gt","right":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Human","colors":["White"],"types":["Creature"],"subtypes":["Human"],"power":1,"toughness":1}},"count":2}}]}},{"If":{"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"In":"Hand"},{"OwnedBy":"You"}]}},"op":"Gt","right":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},"then":[{"Draw":{}}]}}]}}"#;
const W4A_EXPANSION: &str = r#"{"Spell":{"effects":[{"Draw":{"count":"X"}},{"If":{"condition":{"Compare":{"left":"X","op":"Ge","right":5}},"then":[{"CastFromZone":{"what":{"Each":[{"Subtype":"Hero"},{"In":"Hand"},{"OwnedBy":"You"}]},"free":true}}]}}]}}"#;
const W4A_ESTATES: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"Type":"Land"},{"ControlledBy":"You"}]}},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}},"then":[{"Search":{"filter":[{"Subtype":"Plains"}],"count":{"min":0,"max":3},"reveal":true,"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":"Hand"}},{"Shuffle":{}}]}}]}}"#;
const W4A_AJANI: &str = r#"{"Loyalty":{"cost":"Zero","effects":[{"If":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":{"Sum":[{"StartingLife":"You"},15]}}},"then":[{"Exile":{"what":"This"}},{"Exile":{"what":{"Each":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"ControlledBy":"Opponents"}]}}}]}}]}}"#;
const W4A_OOZE: &str = r#"{"Activated":{"cost":[{"Mana":"{G}"}],"targets":[{"what":{"Object":[{"In":"Graveyard"}]}}],"effects":[{"Exile":{"what":{"Target":0}}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Type":"Creature"}]}},"then":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"GainLife":{"amount":1}}]}}]}}"#;

const W4A_MONUMENT_TEXT: &str =
    "At the beginning of your upkeep, sacrifice a creature. If you can't, sacrifice ~.";
const W4A_PASSAGE_TEXT: &str = "{T}, Sacrifice ~: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle. Then if you control four or more lands, untap that land.";
const W4A_HIGH_SCORE_TEXT: &str = "At the beginning of your end step, draw a card if you control a creature with the greatest power among creatures on the battlefield.";
const W4A_DEVOTION_TEXT: &str = "Create five 1/1 white Human creature tokens. If this spell was cast from a graveyard, create ten of those tokens instead.";
const W4A_TELLER_TEXT: &str = "Whenever ~ enters or deals combat damage to a player, exile target card from a graveyard. If it was a creature card, create a 2/2 black Rogue creature token. If it was a land card, create a Treasure token. Otherwise, you gain 3 life.";
const W4A_EMPATHY_TEXT: &str = "At the beginning of your upkeep, draw a card if you control a creature with the greatest power among creatures on the battlefield. Otherwise, put a +1/+1 counter on a creature you control.";
const W4A_REVELRY_TEXT: &str = "If an opponent has more life than you, you gain 4 life. If an opponent controls more creatures than you, create two 1/1 white Human creature tokens. If an opponent has more cards in hand than you, draw a card.";
const W4A_EXPANSION_TEXT: &str = "Draw X cards. If X is 5 or more, you may cast a Hero spell from your hand without paying its mana cost.";
const W4A_ESTATES_TEXT: &str = "If an opponent controls more lands than you, search your library for up to three Plains cards, reveal them, put them into your hand, then shuffle.";
const W4A_AJANI_TEXT: &str = "0: If you have at least 15 life more than your starting life total, exile ~ and each artifact and creature your opponents control.";
const W4A_OOZE_TEXT: &str = "{G}: Exile target card from a graveyard. If it was a creature card, put a +1/+1 counter on ~ and you gain 1 life.";

#[rstest]
#[case::if_you_cant(W4A_MONUMENT, W4A_MONUMENT_TEXT)]
#[case::then_if(W4A_PASSAGE, W4A_PASSAGE_TEXT)]
#[case::step_if(W4A_HIGH_SCORE, W4A_HIGH_SCORE_TEXT)]
#[case::instead(W4A_DEVOTION, W4A_DEVOTION_TEXT)]
#[case::chain(W4A_TELLER, W4A_TELLER_TEXT)]
#[case::step_if_otherwise(W4A_EMPATHY, W4A_EMPATHY_TEXT)]
#[case::three_conditions(W4A_REVELRY, W4A_REVELRY_TEXT)]
#[case::free_cast(W4A_EXPANSION, W4A_EXPANSION_TEXT)]
#[case::search(W4A_ESTATES, W4A_ESTATES_TEXT)]
#[case::loyalty(W4A_AJANI, W4A_AJANI_TEXT)]
#[case::past_tense(W4A_OOZE, W4A_OOZE_TEXT)]
fn w4a_the_oracle_wording_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        w4a_written(json, clause),
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::monument_if_you_do(&W4A_MONUMENT.replace(r#"{"Not":"Performed"}"#, r#""Performed""#), W4A_MONUMENT_TEXT)]
#[case::passage_five_lands(&W4A_PASSAGE.replace(r#""right":4"#, r#""right":5"#), W4A_PASSAGE_TEXT)]
#[case::passage_a_creature(&W4A_PASSAGE.replace(r#"{"Type":"Land"}],"as""#, r#"{"Type":"Creature"}],"as""#), W4A_PASSAGE_TEXT)]
#[case::passage_untaps_unconditionally(&W4A_PASSAGE.replace(r#"{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Ge","right":4}},"then":[{"Untap":{"what":{"Bound":"land"}}}]}}"#, r#"{"Untap":{"what":{"Bound":"land"}}}"#), W4A_PASSAGE_TEXT)]
#[case::high_score_toughness(&W4A_HIGH_SCORE.replace("Power", "Toughness"), W4A_HIGH_SCORE_TEXT)]
#[case::high_score_strictly(&W4A_HIGH_SCORE.replace(r#""op":"Ge""#, r#""op":"Gt""#), W4A_HIGH_SCORE_TEXT)]
#[case::high_score_an_opponent(&W4A_HIGH_SCORE.replace(r#"{"ControlledBy":"You"}"#, r#"{"ControlledBy":"Opponents"}"#), W4A_HIGH_SCORE_TEXT)]
#[case::devotion_nine(&W4A_DEVOTION.replace(r#""count":10"#, r#""count":9"#), W4A_DEVOTION_TEXT)]
#[case::devotion_from_exile(&W4A_DEVOTION.replace(r#""CastFrom":"Graveyard""#, r#""CastFrom":"Exile""#), W4A_DEVOTION_TEXT)]
#[case::devotion_branches_swapped(&W4A_DEVOTION.replace(r#""count":10"#, r#""count":0"#).replace(r#""count":5"#, r#""count":10"#).replace(r#""count":0"#, r#""count":5"#), W4A_DEVOTION_TEXT)]
#[case::teller_artifact(&W4A_TELLER.replace(r#""Land""#, r#""Artifact""#), W4A_TELLER_TEXT)]
#[case::teller_flat(&W4A_TELLER.replace(r#""otherwise":[{"If""#, r#""otherwise":[]}},{"If""#).replace(r#"{"GainLife":{"amount":3}}]}}]}}]}}"#, r#"{"GainLife":{"amount":3}}]}}]}}"#), W4A_TELLER_TEXT)]
#[case::empathy_minus_counter(&W4A_EMPATHY.replace(r#""counter":"+1/+1""#, r#""counter":"-1/-1""#), W4A_EMPATHY_TEXT)]
#[case::empathy_their_creature(&W4A_EMPATHY.replace(r#""from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#, r#""from":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#), W4A_EMPATHY_TEXT)]
#[case::revelry_at_least(&W4A_REVELRY.replacen(r#""op":"Gt""#, r#""op":"Ge""#, 1), W4A_REVELRY_TEXT)]
#[case::revelry_graveyard(&W4A_REVELRY.replace(r#"{"In":"Hand"}"#, r#"{"In":"Graveyard"}"#), W4A_REVELRY_TEXT)]
#[case::revelry_five_life(&W4A_REVELRY.replace(r#""amount":4"#, r#""amount":5"#), W4A_REVELRY_TEXT)]
#[case::expansion_six(&W4A_EXPANSION.replace(r#""right":5"#, r#""right":6"#), W4A_EXPANSION_TEXT)]
#[case::expansion_wizard(&W4A_EXPANSION.replace("Hero", "Wizard"), W4A_EXPANSION_TEXT)]
#[case::expansion_paid(&W4A_EXPANSION.replace(r#","free":true"#, ""), W4A_EXPANSION_TEXT)]
#[case::estates_creatures(&W4A_ESTATES.replace(r#"{"Type":"Land"}"#, r#"{"Type":"Creature"}"#), W4A_ESTATES_TEXT)]
#[case::ajani_twenty(&W4A_AJANI.replace("15", "20"), W4A_AJANI_TEXT)]
#[case::ajani_your_own(&W4A_AJANI.replace(r#""ControlledBy":"Opponents""#, r#""ControlledBy":"You""#), W4A_AJANI_TEXT)]
#[case::ooze_land(&W4A_OOZE.replace(r#"[{"Type":"Creature"}]"#, r#"[{"Type":"Land"}]"#), W4A_OOZE_TEXT)]
fn w4a_a_near_miss_never_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!w4a_written(json, clause), "{clause}");
}
