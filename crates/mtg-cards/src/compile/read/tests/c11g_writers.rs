//! Goal: the c11g writers give the oracle wording of whole abilities of the stored proposals of the names M to Z; another count, kind, player, zone, step, condition or keyword in the same ability never reads back.
use super::*;

/// Whether some wording the checker accepts for the ability is the clause.
fn c11g_reads(ability: &Ability, clause: &str) -> bool {
    written_variants(ability)
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// The ability of the proposal whose wording is the clause.
fn c11g_proposal_ability(proposal: &str, clause: &str) -> Ability {
    let card: serde_json::Value = serde_json::from_str(proposal).expect("proposal JSON");
    card["faces"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|face| face["abilities"].as_array().cloned().unwrap_or_default())
        .map(|value| serde_json::from_value::<Ability>(value).expect("ability"))
        .find(|ability| c11g_reads(ability, clause))
        .unwrap_or_else(|| panic!("no ability of the proposal reads {clause}"))
}

macro_rules! c11g_proposal {
    ($path:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/proposals/",
            $path,
            ".json"
        ))
    };
}

#[rstest]
#[case::memory_worm_0(
    c11g_proposal!("r2ah/memory_worm"),
    "Paradox — Whenever you cast a spell from anywhere other than your hand, this creature deals 2 damage to target player. That player discards a card, then draws a card. Put a +1/+1 counter on this creature.",
    "\"amount\":2",
    "\"amount\":3"
)]
#[case::oni_cult_anvil_0(
    c11g_proposal!("r2ah/oni_cult_anvil"),
    "Whenever one or more artifacts you control leave the battlefield during your turn, create a 1/1 colorless Construct artifact creature token. This ability triggers only once each turn.",
    "\"power\":1",
    "\"power\":2"
)]
#[case::rekindling_phoenix_1(
    c11g_proposal!("r2ah/rekindling_phoenix"),
    "When this creature dies, create a 0/1 red Elemental creature token with \"At the beginning of your upkeep, sacrifice this token and return target card named ~ from your graveyard to the battlefield. It gains haste until end of turn.\"",
    "\"Upkeep\"",
    "\"Draw\""
)]
#[case::renegade_silent_0(
    c11g_proposal!("r2ah/renegade_silent"),
    "At the beginning of your end step, goad up to one target creature you don't control and put a +1/+1 counter on this creature. This creature phases out.",
    "\"step\":\"End\"",
    "\"step\":\"Upkeep\""
)]
#[case::salt_road_ambushers_0(
    c11g_proposal!("r2ai/salt_road_ambushers"),
    "Whenever another permanent you control is turned face up, if it's a creature, put two +1/+1 counters on it.",
    "\"count\":2",
    "\"count\":3"
)]
#[case::martial_impetus_2(
    c11g_proposal!("r2cc/martial_impetus"),
    "Whenever enchanted creature attacks, each other creature that's attacking one of your opponents gets +1/+1 until end of turn.",
    "\"Opponents\"",
    "\"You\""
)]
#[case::nether_traitor_2(
    c11g_proposal!("r2cc/nether_traitor"),
    "Whenever another creature is put into your graveyard from the battlefield, you may pay {B}. If you do, return this card from your graveyard to the battlefield.",
    "\"{B}\"",
    "\"{2}\""
)]
#[case::mirror_of_the_forebears_1(
    c11g_proposal!("r2cm/mirror_of_the_forebears"),
    "{1}: Until end of turn, this artifact becomes a copy of target creature you control of the chosen type, except it's an artifact in addition to its other types.",
    "\"Artifact\"",
    "\"Enchantment\""
)]
#[case::obuun_mul_daya_ancestor_0(
    c11g_proposal!("r2cm/obuun_mul_daya_ancestor"),
    "At the beginning of combat on your turn, up to one target land you control becomes an X/X Elemental creature with trample and haste until end of turn, where X is ~'s power. It's still a land.",
    "\"Trample\"",
    "\"Flying\""
)]
#[case::rundvelt_hordemaster_1(
    c11g_proposal!("r2cm/rundvelt_hordemaster"),
    "Whenever this creature or another Goblin you control dies, exile the top card of your library. If it's a Goblin creature card, you may cast that card until the end of your next turn.",
    "\"Goblin\"",
    "\"Elf\""
)]
#[case::okoye_mighty_and_adored_1(
    c11g_proposal!("r2dd/okoye_mighty_and_adored"),
    "At the beginning of combat on your turn, put a +1/+1 counter on target creature. Whenever that creature attacks the monarch this turn, it gains double strike and trample until end of turn.",
    "\"DoubleStrike\"",
    "\"FirstStrike\""
)]
#[case::slurrk_all_ingesting_1(
    c11g_proposal!("r2p/slurrk_all_ingesting"),
    "Whenever ~ or another creature you control dies, if it had a +1/+1 counter on it, put a +1/+1 counter on each creature you control that has a +1/+1 counter on it.",
    "\"+1/+1\"",
    "\"-1/-1\""
)]
#[case::malicious_affliction_0(
    c11g_proposal!("r2cm/malicious_affliction"),
    "Morbid — When you cast this spell, if a creature died this turn, you may copy ~ and may choose a new target for the copy.",
    "\"Ge\"",
    "\"Gt\""
)]
#[case::mob_rule_0(
    c11g_proposal!("r2cc/mob_rule"),
    "Choose one —\n• Gain control of all creatures with power 4 or greater until end of turn. Untap those creatures. They gain haste until end of turn.\n• Gain control of all creatures with power 3 or less until end of turn. Untap those creatures. They gain haste until end of turn.",
    "\"to\":4",
    "\"to\":5"
)]
#[case::noxious_assault_0(
    c11g_proposal!("r2cc/noxious_assault"),
    "Creatures you control get +2/+2 until end of turn. Whenever a creature blocks this turn, its controller gets a poison counter.",
    "\"poison\"",
    "\"+1/+1\""
)]
#[case::necromancers_covenant_0(
    c11g_proposal!("r2ah/necromancers_covenant"),
    "When this enchantment enters, exile all creature cards from target player's graveyard, then create a 2/2 black Zombie creature token for each card exiled this way.",
    "\"power\":2",
    "\"power\":3"
)]
#[case::prismari_charm_0(
    c11g_proposal!("r2ah/prismari_charm"),
    "Choose one —\n• Surveil 2, then draw a card.\n• ~ deals 1 damage to each of one or two targets.\n• Return target nonland permanent to its owner's hand.",
    "\"count\":2",
    "\"count\":3"
)]
#[case::rakdos_charm_0(
    c11g_proposal!("r2ah/rakdos_charm"),
    "Choose one —\n• Exile target player's graveyard.\n• Destroy target artifact.\n• Each creature deals 1 damage to its controller.",
    "\"Artifact\"",
    "\"Enchantment\""
)]
#[case::rin_and_seri_inseparable_2(
    c11g_proposal!("r2ah/rin_and_seri_inseparable"),
    "{R}{G}{W}, {T}: ~ deals damage to any target equal to the number of Dogs you control. You gain life equal to the number of Cats you control.",
    "\"Cat\"",
    "\"Dog\""
)]
#[case::run_for_your_life_0(
    c11g_proposal!("r2ah/run_for_your_life"),
    "One or two target creatures each gain haste until end of turn. They can't be blocked this turn except by creatures with haste.",
    "\"Haste\"",
    "\"Flying\""
)]
#[case::verdurous_gearhulk_1(
    c11g_proposal!("r2ai/verdurous_gearhulk"),
    "When this creature enters, distribute four +1/+1 counters among any number of target creatures you control.",
    "\"divide\":4",
    "\"divide\":3"
)]
#[case::wall_of_reverence_2(
    c11g_proposal!("r2ai/wall_of_reverence"),
    "At the beginning of your end step, you may gain life equal to the power of target creature you control.",
    "\"Power\"",
    "\"Toughness\""
)]
#[case::yavimaya_dryad_1(
    c11g_proposal!("r2ai/yavimaya_dryad"),
    "When this creature enters, you may search your library for a Forest card, put it onto the battlefield tapped under target player's control, then shuffle.",
    "\"Forest\"",
    "\"Island\""
)]
#[case::zedruu_the_greathearted_0(
    c11g_proposal!("r2ai/zedruu_the_greathearted"),
    "At the beginning of your upkeep, you gain X life and draw X cards, where X is the number of permanents you own that your opponents control.",
    "\"Opponents\"",
    "\"You\""
)]
#[case::zedruu_the_greathearted_1(
    c11g_proposal!("r2ai/zedruu_the_greathearted"),
    "{U}{R}{W}: Target opponent gains control of target permanent you control.",
    "\"Opponent\"",
    "\"Player\""
)]
#[case::planar_chaos_1(
    c11g_proposal!("r2cc/planar_chaos"),
    "Whenever a player casts a spell, that player flips a coin. If they lose the flip, counter that spell.",
    "\"Counter\"",
    "\"Destroy\""
)]
#[case::magda_brazen_outlaw_2(
    c11g_proposal!("r2cm/magda_brazen_outlaw"),
    "Sacrifice five Treasures: Search your library for an artifact or Dragon card, put that card onto the battlefield, then shuffle.",
    "\"count\":5",
    "\"count\":4"
)]
#[case::metallurgic_summonings_1(
    c11g_proposal!("r2cm/metallurgic_summonings"),
    "{3}{U}{U}, Exile this enchantment: Return all instant and sorcery cards from your graveyard to your hand. Activate only if you control six or more artifacts.",
    "\"right\":6",
    "\"right\":5"
)]
#[case::refuse_cooperate_0(
    c11g_proposal!("r2cm/refuse_cooperate"),
    "~ deals damage to target spell's controller equal to that spell's mana value.",
    "\"ManaValue\"",
    "\"Power\""
)]
#[case::reinterpret_0(
    c11g_proposal!("r2cm/reinterpret"),
    "Counter target spell. You may cast a spell with equal or lesser mana value from your hand without paying its mana cost.",
    "\"Le\"",
    "\"Lt\""
)]
#[case::rooftop_storm_0(
    c11g_proposal!("r2cm/rooftop_storm"),
    "You may pay {0} rather than pay the mana cost for Zombie creature spells you cast.",
    "\"Zombie\"",
    "\"Vampire\""
)]
#[case::nimble_obstructionist_3(
    c11g_proposal!("r2w/nimble_obstructionist"),
    "When you cycle this card, counter target activated or triggered ability you don't control.",
    "\"Graveyard\"",
    "\"Hand\""
)]
#[case::ravener_2(
    c11g_proposal!("r2w/ravener"),
    "When this creature enters, target creature attacks target opponent this turn if able.",
    "\"Opponent\"",
    "\"Player\""
)]
#[case::tetsuko_umezawa_fugitive_0(
    c11g_proposal!("r2x/tetsuko_umezawa_fugitive"),
    "Creatures you control with power or toughness 1 or less can't be blocked.",
    "\"Le\"",
    "\"Ge\""
)]
#[case::obscura_storefront_0(
    c11g_proposal!("r2w/obscura_storefront"),
    "When this land enters, sacrifice it. When you do, search your library for a basic Plains, Island, or Swamp card, put it onto the battlefield tapped, then shuffle and you gain 1 life.",
    "\"Plains\"",
    "\"Forest\""
)]
#[case::maestros_theater_0(
    c11g_proposal!("r2w/maestros_theater"),
    "When this land enters, sacrifice it. When you do, search your library for a basic Island, Swamp, or Mountain card, put it onto the battlefield tapped, then shuffle and you gain 1 life.",
    "\"tapped\":true",
    "\"tapped\":false"
)]
#[case::reef_worm_0(
    c11g_proposal!("r2cc/reef_worm"),
    "When this creature dies, create a 3/3 blue Fish creature token with \"When this token dies, create a 6/6 blue Whale creature token with 'When this token dies, create a 9/9 blue Kraken creature token.'\"",
    "\"power\":9",
    "\"power\":8"
)]
#[case::nesting_dragon_1(
    c11g_proposal!("r2cc/nesting_dragon"),
    "Landfall — Whenever a land you control enters, create a 0/2 red Dragon Egg creature token with defender and \"When this token dies, create a 2/2 red Dragon creature token with flying and '{R}: This token gets +1/+0 until end of turn.'\"",
    "\"Defender\"",
    "\"Reach\""
)]
#[case::megatons_fate_0(
    c11g_proposal!("r2bc/megatons_fate"),
    "Choose one —\n• Disarm — Destroy target artifact. Create four Treasure tokens.\n• Detonate — ~ deals 8 damage to each creature. Each player gets four rad counters.",
    "\"count\":4",
    "\"count\":3"
)]
fn c11g_wholes_read_back_and_near_misses_do_not(
    #[case] proposal: &str,
    #[case] clause: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let ability = c11g_proposal_ability(proposal, clause);
    let json = serde_json::to_string(&ability).expect("JSON");
    assert!(json.contains(from), "{json}");
    let near = super::ability(&json.replacen(from, to, 1));
    assert!(!c11g_reads(&near, clause), "near-miss reads back: {clause}");
}
