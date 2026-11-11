//! Goal: the rmt1 writers give the oracle wording of whole abilities of the rmt1 pack; another count, player, zone, duration, filter or step in the same ability never reads back.
use super::*;

/// Whether some wording the checker accepts for the ability is the clause.
fn rmt1_reads(ability: &Ability, clause: &str) -> bool {
    written_variants(ability)
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// The ability of the rules file whose wording is the clause.
fn rmt1_card_ability(file: &str, clause: &str) -> Ability {
    let card: serde_json::Value = serde_json::from_str(file).expect("card JSON");
    card["faces"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|face| face["abilities"].as_array().cloned().unwrap_or_default())
        .map(|value| serde_json::from_value::<Ability>(value).expect("ability"))
        .find(|ability| rmt1_reads(ability, clause))
        .unwrap_or_else(|| panic!("no ability of the card reads {clause}"))
}

macro_rules! rmt1_card {
    ($slug:literal) => {
        include_str!(concat!("../../../../cards/", $slug, ".json"))
    };
}

#[rstest]
#[case::abdel_adrian_gorions_ward_0(
    rmt1_card!("abdel_adrian_gorions_ward"),
    "When ~ enters, exile any number of other nonland permanents you control until ~ leaves the battlefield. Create a 1/1 white Soldier creature token for each permanent exiled this way.",
    "\"power\":1",
    "\"power\":2"
)]
#[case::aminatou_the_fateshifter_0(
    rmt1_card!("aminatou_the_fateshifter"),
    "−1: Exile another target permanent you own, then return it to the battlefield under your control.",
    "\"Other\",",
    ""
)]
#[case::arid_archway_0(
    rmt1_card!("arid_archway"),
    "When this land enters, return a land you control to its owner's hand. If another Desert was returned this way, surveil 1.",
    "\"Desert\"",
    "\"Island\""
)]
#[case::burning_rune_demon_0(
    rmt1_card!("burning_rune_demon"),
    "When this creature enters, you may search your library for exactly two cards not named ~ that have different names. If you do, reveal those cards. An opponent chooses one of them. Put the chosen card into your hand and the other into your graveyard, then shuffle.",
    "\"min\":2",
    "\"min\":1"
)]
#[case::colossal_grave_reaver_0(
    rmt1_card!("colossal_grave_reaver"),
    "Whenever one or more creature cards are put into your graveyard from your library, put one of them onto the battlefield.",
    "\"from\":\"Library\"",
    "\"from\":\"Hand\""
)]
#[case::gifts_ungiven_0(
    rmt1_card!("gifts_ungiven"),
    "Search your library for up to four cards with different names and reveal them. Target opponent chooses two of those cards. Put the chosen cards into your graveyard and the rest into your hand. Then shuffle.",
    "\"max\":4",
    "\"max\":3"
)]
#[case::harald_unites_the_elves_0(
    rmt1_card!("harald_unites_the_elves"),
    "I — Mill three cards. You may put an Elf or Tyvar card from your graveyard onto the battlefield.",
    "\"Tyvar\"",
    "\"Goblin\""
)]
#[case::intuition_0(
    rmt1_card!("intuition"),
    "Search your library for three cards and reveal them. Target opponent chooses one. Put that card into your hand and the rest into your graveyard. Then shuffle.",
    "\"to\":\"Hand\"",
    "\"to\":{\"Battlefield\":{}}"
)]
#[case::lichs_mastery_0(
    rmt1_card!("lichs_mastery"),
    "When ~ leaves the battlefield, you lose the game.",
    "\"from\":\"Battlefield\"",
    "\"from\":\"Graveyard\""
)]
#[case::mischievous_quanar_0(
    rmt1_card!("mischievous_quanar"),
    "When this creature is turned face up, copy target instant or sorcery spell. You may choose new targets for that copy.",
    "\"new_targets\":true",
    "\"new_targets\":false"
)]
#[case::reality_smasher_0(
    rmt1_card!("reality_smasher"),
    "Whenever this creature becomes the target of a spell an opponent controls, counter that spell unless its controller discards a card.",
    "\"Spell\",",
    ""
)]
#[case::saheeli_sublime_artificer_0(
    rmt1_card!("saheeli_sublime_artificer"),
    "−2: Target artifact you control becomes a copy of another target artifact or creature you control until end of turn, except it's an artifact in addition to its other types.",
    "\"UntilEndOfTurn\"",
    "\"UntilYourNextTurn\""
)]
#[case::silent_gravestone_0(
    rmt1_card!("silent_gravestone"),
    "{4}, {T}: Exile this artifact and all cards from all graveyards. Draw a card.",
    "\"{4}\"",
    "\"{3}\""
)]
#[case::sword_of_hearth_and_home_0(
    rmt1_card!("sword_of_hearth_and_home"),
    "Whenever equipped creature deals combat damage to a player, exile up to one target creature you own, then search your library for a basic land card. Put both cards onto the battlefield under your control, then shuffle.",
    "\"Basic\"",
    "\"Snow\""
)]
#[case::tezzeret_cruel_machinist_0(
    rmt1_card!("tezzeret_cruel_machinist"),
    "0: Until your next turn, target artifact you control becomes a 5/5 creature in addition to its other types.",
    "\"power\":5",
    "\"power\":4"
)]
#[case::alliance_of_arms_0(
    rmt1_card!("alliance_of_arms"),
    "Starting with you, each player may pay any amount of mana. Each player creates X 1/1 white Soldier creature tokens, where X is the total amount of mana paid this way.",
    "\"power\":1",
    "\"power\":2"
)]
#[case::collective_voyage_0(
    rmt1_card!("collective_voyage"),
    "Starting with you, each player may pay any amount of mana. Each player searches their library for up to X basic land cards, where X is the total amount of mana paid this way, puts them onto the battlefield tapped, then shuffles.",
    "\"tapped\":true",
    "\"tapped\":false"
)]
#[case::minds_aglow_0(
    rmt1_card!("minds_aglow"),
    "Starting with you, each player may pay any amount of mana. Each player draws X cards, where X is the total amount of mana paid this way.",
    "\"Draw\":{\"who\":\"EachPlayer\"",
    "\"Draw\":{\"who\":\"Opponents\""
)]
#[case::shared_trauma_0(
    rmt1_card!("shared_trauma"),
    "Starting with you, each player may pay any amount of mana. Each player mills X cards, where X is the total amount of mana paid this way.",
    "\"cost\":\"{X}\"",
    "\"cost\":\"{X}{X}\""
)]
fn rmt1_wholes_read_back_and_near_misses_do_not(
    #[case] file: &str,
    #[case] clause: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let ability = rmt1_card_ability(file, clause);
    let json = serde_json::to_string(&ability).expect("JSON");
    assert!(json.contains(from), "{json}");
    let near = super::ability(&json.replacen(from, to, 1));
    assert!(!rmt1_reads(&near, clause), "near-miss reads back: {clause}");
}
