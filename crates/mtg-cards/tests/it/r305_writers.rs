//! Goal: the r30-5 wordings read back exactly the shapes they name and nothing near them: a creature held back after it attacked during your last turn, power and toughness counted from counters or from creatures on the battlefield, a cast ban during combat, spells cast from a graveyard or exile that cost more or less, a bonus for each player who has lost, all colors, attacking as though it had haste unless it entered this turn, and the draft rules that note what was drafted.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

fn wordings(json: &str) -> Vec<String> {
    written_variants(&serde_json::from_str::<Ability>(json).expect("an ability"))
}

fn held(mod_name: &str, pred: &str, affected: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{affected},"mods":["{mod_name}"],"condition":{{"Matches":{{"subject":"This","selector":["{pred}"]}}}}}}}}"#
    )
}

fn counters(power: &str, toughness: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"SetPT":{{"power":{power},"toughness":{toughness}}}}}],"defining":true}}}}"#
    )
}

const ON_YOURS: &str = r#"{"Counters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}}"#;
const ON_THEIRS: &str = r#"{"Counters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"counter":"+1/+1"}}"#;

fn counted(stat: &str, color: &str, defining: bool) -> String {
    let defining = if defining { r#","defining":true"# } else { "" };
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"{stat}":{{"Count":[{{"Type":"Creature"}},{{"Color":"{color}"}}]}}}}]{defining}}}}}"#
    )
}

fn cast_ban(who: &str, unless: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"Cant":{{"who":"{who}","action":{{"Cast":{{"unless":{unless}}}}}}}}}}}}}"#
    )
}

fn taxed(who: &str, zones: &str, change: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["Spell",{{"ControlledBy":"{who}"}},{zones}],"change":"{change}","mana":"{{2}}"}}}}"#
    )
}

const ELSEWHERE: &str = r#"{"AnyOf":[{"CastFrom":"Graveyard"},{"CastFrom":"Exile"}]}"#;
const GRAVEYARD: &str = r#"{"CastFrom":"Graveyard"}"#;

fn per(amount: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"ModifyPT":{{"power":{{"Times":{{"by":10,"of":{amount}}}}},"toughness":{{"Times":{{"by":10,"of":{amount}}}}}}}}}]}}}}"#
    )
}

fn colors(colors: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"SetColors":{colors}}}],"defining":true}}}}"#
    )
}

fn haste(condition: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"Combat":{{"who":"This","quirk":"AttackAsThoughHaste"}}}},"condition":{condition}}}}}"#
    )
}

fn draft(rule: &str) -> String {
    format!(r#"{{"Rule":{{"rule":{{"Draft":"{rule}"}}}}}}"#)
}

#[rstest]
#[case::cant_attack(
    held("CantAttack", "AttackedLastTurn", r#""This""#),
    "~ can't attack if it attacked during your last turn.",
    true
)]
#[case::doesnt_untap(
    held("DoesntUntap", "AttackedLastTurn", r#""This""#),
    "~ doesn't untap during your untap step if it attacked during your last turn.",
    true
)]
#[case::this_turn_is_not_last_turn(
    held("CantAttack", "AttackedThisTurn", r#""This""#),
    "~ can't attack if it attacked during your last turn.",
    false
)]
#[case::an_aura_is_not_the_creature(
    held("CantAttack", "AttackedLastTurn", r#""Attached""#),
    "~ can't attack if it attacked during your last turn.",
    false
)]
#[case::counters_on_yours(
    counters(ON_YOURS, ON_YOURS),
    "~'s power and toughness are each equal to the number of +1/+1 counters on creatures you control.",
    true
)]
#[case::counters_on_theirs(
    counters(ON_THEIRS, ON_THEIRS),
    "~'s power and toughness are each equal to the number of +1/+1 counters on creatures you control.",
    false
)]
#[case::counters_power_only(
    counters(ON_YOURS, "1"),
    "~'s power and toughness are each equal to the number of +1/+1 counters on creatures you control.",
    false
)]
#[case::red_power(
    counted("SetPower", "Red", true),
    "~'s power is equal to the number of red creatures on the battlefield.",
    true
)]
#[case::green_toughness(
    counted("SetToughness", "Green", true),
    "~'s toughness is equal to the number of green creatures on the battlefield.",
    true
)]
#[case::green_is_not_red(
    counted("SetPower", "Green", true),
    "~'s power is equal to the number of red creatures on the battlefield.",
    false
)]
#[case::not_defining(
    counted("SetPower", "Red", false),
    "~'s power is equal to the number of red creatures on the battlefield.",
    false
)]
#[case::opponents_in_combat(
    cast_ban("Opponents", r#"{"Not":"InCombat"}"#),
    "Your opponents can't cast spells during combat.",
    true
)]
#[case::players_in_combat(
    cast_ban("EachPlayer", r#"{"Not":"InCombat"}"#),
    "Players can't cast spells during combat.",
    true
)]
#[case::outside_combat_is_other(
    cast_ban("Opponents", r#""InCombat""#),
    "Your opponents can't cast spells during combat.",
    false
)]
#[case::opponents_pay_more(
    taxed("Opponents", ELSEWHERE, "Increase"),
    "Spells your opponents cast from graveyards or from exile cost {2} more to cast.",
    true
)]
#[case::you_pay_less(
    taxed("You", ELSEWHERE, "Reduce"),
    "Spells you cast from your graveyard or from exile cost {2} less to cast.",
    true
)]
#[case::less_is_not_more(
    taxed("Opponents", ELSEWHERE, "Reduce"),
    "Spells your opponents cast from graveyards or from exile cost {2} more to cast.",
    false
)]
#[case::graveyard_alone(
    taxed("Opponents", GRAVEYARD, "Increase"),
    "Spells your opponents cast from graveyards or from exile cost {2} more to cast.",
    false
)]
#[case::lost_players(
    per(r#""LostPlayers""#),
    "~ gets +10/+10 for each player who has lost the game.",
    true
)]
#[case::turns_are_not_players(
    per(r#""TurnNumber""#),
    "~ gets +10/+10 for each player who has lost the game.",
    false
)]
#[case::all_colors(
    colors(r#"["White","Blue","Black","Red","Green"]"#),
    "~ is all colors. This ability doesn't affect its color identity.",
    true
)]
#[case::all_colors_plain(
    colors(r#"["White","Blue","Black","Red","Green"]"#),
    "~ is all colors.",
    true
)]
#[case::four_colors(colors(r#"["White","Blue","Black","Red"]"#), "~ is all colors.", false)]
#[case::haste_unless_entered(
    haste(r#"{"Not":{"Matches":{"subject":"This","selector":["EnteredThisTurn"]}}}"#),
    "~ can attack as though it had haste unless it entered this turn.",
    true
)]
#[case::haste_only_if_entered(
    haste(r#"{"Matches":{"subject":"This","selector":["EnteredThisTurn"]}}"#),
    "~ can attack as though it had haste unless it entered this turn.",
    false
)]
#[case::booster_pack(
    draft("AddBoosterPack"),
    "Reveal ~ as you draft it. After you draft ~, you may add a booster pack to the draft.",
    true
)]
#[case::drafted_count(
    draft("NoteDraftedCount"),
    "Reveal ~ as you draft it and note how many cards you've drafted this draft round, including ~.",
    true
)]
#[case::passer(
    draft("NotePasser"),
    "Reveal ~ as you draft it and note the player who passed it to you.",
    true
)]
#[case::next_name(
    draft("NoteNextName"),
    "Reveal ~ as you draft it. Reveal the next card you draft and note its name.",
    true
)]
#[case::guess(
    draft("GuessNextPick"),
    "Reveal ~ as you draft it. The next time a player drafts a card from this booster pack, guess that card's name. Then that player reveals the drafted card.",
    true
)]
#[case::passer_is_no_pack(
    draft("NotePasser"),
    "Reveal ~ as you draft it. After you draft ~, you may add a booster pack to the draft.",
    false
)]
fn r305_wordings_read_back_their_shape(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let said = wordings(&json);
    assert_eq!(said.iter().any(|w| w == text), reads, "{json}\n{said:#?}");
}
