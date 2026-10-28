//! Goal: each whole ability of the stored proposals of the names A to L that the single-wording tail needed reads back
//! as the clause its card prints; another count, kind, player, zone or duration in the same ability never does.
use super::*;
use rstest::rstest;

/// The JSON swaps that make another ability: another event, player, kind, zone or duration.
const C11F_SWAPS: [(&str, &str); 10] = [
    ("\"Attacks\"", "\"Blocks\""),
    ("\"Spirit\"", "\"Zombie\""),
    ("\"You\"", "\"Opponents\""),
    ("\"Opponents\"", "\"You\""),
    ("\"Creature\"", "\"Artifact\""),
    ("\"Artifact\"", "\"Enchantment\""),
    ("\"Land\"", "\"Creature\""),
    ("\"Graveyard\"", "\"Exile\""),
    ("\"Hand\"", "\"Graveyard\""),
    ("\"UntilEndOfTurn\"", "\"UntilYourNextTurn\""),
];

/// Whether some wording the checker accepts for the ability is the clause.
fn c11f_reads(ability: &Ability, clause: &str) -> bool {
    written_variants(ability)
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// The ability at the index of the first face of the proposal, as compact JSON.
fn c11f_ability_json(proposal: &str, index: usize) -> String {
    let card: serde_json::Value = serde_json::from_str(proposal).expect("proposal JSON");
    card["faces"][0]["abilities"][index].to_string()
}

/// The JSON with its first number after a key raised by one: another count.
fn c11f_raised(json: &str) -> Option<String> {
    let (start, len) = json.match_indices("\":").find_map(|(at, _)| {
        let digits = json[at + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .count();
        (digits > 0).then_some((at + 2, digits))
    })?;
    let number: u64 = json[start..start + len].parse().ok()?;
    Some(format!(
        "{}{}{}",
        &json[..start],
        number + 1,
        &json[start + len..]
    ))
}

macro_rules! c11f_proposal {
    ($dir:literal, $slug:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/proposals/",
            $dir,
            "/",
            $slug,
            ".json"
        ))
    };
}

#[rstest]
#[case::calculating_lich_1(c11f_proposal!("r2af", "calculating_lich"), 1, "Whenever a creature attacks one of your opponents, that player loses 1 life.")]
#[case::crimson_wisps_0(c11f_proposal!("r2af", "crimson_wisps"), 0, "Target creature becomes red and gains haste until end of turn.\nDraw a card.")]
#[case::crypt_incursion_0(c11f_proposal!("r2af", "crypt_incursion"), 0, "Exile all creature cards from target player's graveyard. You gain 3 life for each card exiled this way.")]
#[case::diabolic_vision_0(c11f_proposal!("r2af", "diabolic_vision"), 0, "Look at the top five cards of your library. Put one of them into your hand and the rest on top of your library in any order.")]
#[case::dispatch_0(c11f_proposal!("r2af", "dispatch"), 0, "Tap target creature.\nIf you control three or more artifacts, exile that creature.")]
#[case::eldrazi_displacer_1(c11f_proposal!("r2af", "eldrazi_displacer"), 1, "{2}{C}: Exile another target creature, then return it to the battlefield tapped under its owner's control.")]
#[case::fleetfoot_panther_1(c11f_proposal!("r2af", "fleetfoot_panther"), 1, "When this creature enters, return a green or white creature you control to its owner's hand.")]
#[case::forgotten_creation_1(c11f_proposal!("r2af", "forgotten_creation"), 1, "At the beginning of your upkeep, you may discard all the cards in your hand. If you do, draw that many cards.")]
#[case::gigantoplasm_0(c11f_proposal!("r2ag", "gigantoplasm"), 0, "You may have this creature enter as a copy of any creature on the battlefield, except it has \"{X}: This creature has base power and toughness X/X.\"")]
#[case::goblin_kaboomist_0(c11f_proposal!("r2ag", "goblin_kaboomist"), 0, "At the beginning of your upkeep, create a colorless artifact token named Land Mine with \"{R}, Sacrifice this token: This token deals 2 damage to target attacking creature without flying.\" Then flip a coin. If you lose the flip, this creature deals 2 damage to itself.")]
#[case::grave_peril_0(c11f_proposal!("r2ag", "grave_peril"), 0, "When a nonblack creature enters, sacrifice this enchantment. If you do, destroy that creature.")]
#[case::hazel_of_the_rootbloom_1(c11f_proposal!("r2ag", "hazel_of_the_rootbloom"), 1, "At the beginning of your end step, create a token that's a copy of target token you control. If that token is a Squirrel, instead create two tokens that are copies of it.")]
#[case::ingenious_prodigy_2(c11f_proposal!("r2ag", "ingenious_prodigy"), 2, "At the beginning of your upkeep, if this creature has one or more +1/+1 counters on it, you may remove a +1/+1 counter from it. If you do, draw a card.")]
#[case::isolated_watchtower_1(c11f_proposal!("r2ag", "isolated_watchtower"), 1, "{2}, {T}: Scry 1, then you may reveal the top card of your library. If a basic land card is revealed this way, put it onto the battlefield tapped. Activate only if an opponent controls at least two more lands than you.")]
#[case::jolted_awake_0(c11f_proposal!("r2ag", "jolted_awake"), 0, "Choose up to one target artifact or creature card in your graveyard. You get {E}{E}. Then you may pay an amount of {E} equal to that card's mana value. If you do, return it from your graveyard to the battlefield.")]
#[case::kaldra_compleat_2(c11f_proposal!("r2ag", "kaldra_compleat"), 2, "Equipped creature gets +5/+5 and has first strike, trample, indestructible, haste, and \"Whenever this creature deals combat damage to a creature, exile that creature.\"")]
#[case::kirtars_wrath_0(c11f_proposal!("r2ag", "kirtars_wrath"), 0, "Destroy all creatures. They can't be regenerated.\nIf there are seven or more cards in your graveyard, instead destroy all creatures, then create two 1/1 white Spirit creature tokens with flying. Creatures destroyed this way can't be regenerated.")]
#[case::knickknack_ouphe_1(c11f_proposal!("r2ag", "knickknack_ouphe"), 1, "When this creature enters, reveal the top X cards of your library. You may put any number of Aura cards with mana value X or less from among them onto the battlefield. Then put all cards revealed this way that weren't put onto the battlefield on the bottom of your library in a random order.")]
#[case::acolyte_hybrid_0(c11f_proposal!("r2ba", "acolyte_hybrid"), 0, "Whenever this creature attacks, destroy up to one target artifact. If an artifact is destroyed this way, its controller draws a card.")]
#[case::avacyn_angel_of_horror_2(c11f_proposal!("r2ba", "avacyn_angel_of_horror"), 2, "Whenever ~ or another nontoken creature you control dies, return that card to the battlefield under your control at the beginning of the next end step.")]
#[case::canoptek_spyder_1(c11f_proposal!("r2ba", "canoptek_spyder"), 1, "Whenever another nontoken artifact creature or Vehicle you control enters, draw a card.")]
#[case::captain_america_team_leader_0(c11f_proposal!("r2ba", "captain_america_team_leader"), 0, "Whenever another Hero you control enters, it gains vigilance and haste until end of turn. Put a +1/+1 counter on that Hero and a +1/+1 counter on ~.")]
#[case::chocobo_knights_0(c11f_proposal!("r2ba", "chocobo_knights"), 0, "Whenever you attack, creatures you control with counters on them gain double strike until end of turn.")]
#[case::dire_tactics_0(c11f_proposal!("r2ba", "dire_tactics"), 0, "Exile target creature. If you don't control a Human, you lose life equal to that creature's toughness.")]
#[case::drana_kalastria_bloodchief_1(c11f_proposal!("r2ba", "drana_kalastria_bloodchief"), 1, "{X}{B}{B}: Target creature gets -0/-X until end of turn and ~ gets +X/+0 until end of turn.")]
#[case::falcon_and_redwing_1(c11f_proposal!("r2ba", "falcon_and_redwing"), 1, "Whenever ~ deal combat damage to a player, create that many 1/1 white Bird creature tokens with flying, then put a +1/+1 counter on ~.")]
#[case::fools_demise_1(c11f_proposal!("r2ba", "fools_demise"), 1, "When enchanted creature dies, return that card to the battlefield under your control.")]
#[case::geode_rager_1(c11f_proposal!("r2bb", "geode_rager"), 1, "Whenever a land you control enters, goad each creature target player controls.")]
#[case::greenwarden_of_murasa_1(c11f_proposal!("r2bb", "greenwarden_of_murasa"), 1, "When this creature dies, you may exile it. If you do, return target card from your graveyard to your hand.")]
#[case::hammer_of_nazahn_0(c11f_proposal!("r2bb", "hammer_of_nazahn"), 0, "Whenever ~ or another Equipment you control enters, you may attach that Equipment to target creature you control.")]
#[case::jedit_ojanen_of_efrava_1(c11f_proposal!("r2bb", "jedit_ojanen_of_efrava"), 1, "Whenever Jedit Ojanen attacks or blocks, create a 2/2 green Cat Warrior creature token with forestwalk.")]
#[case::kaseto_orochi_archmage_0(c11f_proposal!("r2bb", "kaseto_orochi_archmage"), 0, "{G}{U}: Target creature can't be blocked this turn. If that creature is a Snake, it gets +2/+2 until end of turn.")]
#[case::lifeblood_hydra_2(c11f_proposal!("r2bb", "lifeblood_hydra"), 2, "When this creature dies, you gain life and draw cards equal to its power.")]
#[case::lotleth_giant_0(c11f_proposal!("r2bb", "lotleth_giant"), 0, "When this creature enters, it deals 1 damage to target opponent for each creature card in your graveyard.")]
#[case::aetherflux_reservoir_0(c11f_proposal!("r2ca", "aetherflux_reservoir"), 0, "Whenever you cast a spell, you gain 1 life for each spell you've cast this turn.")]
#[case::akoum_hellkite_1(c11f_proposal!("r2ca", "akoum_hellkite"), 1, "Whenever a land you control enters, this creature deals 1 damage to any target. If that land is a Mountain, this creature deals 2 damage instead.")]
#[case::arden_angel_1(c11f_proposal!("r2ca", "arden_angel"), 1, "At the beginning of your upkeep, if ~ is in your graveyard, roll a four-sided die. If the result is 1, return ~ from your graveyard to the battlefield.")]
#[case::arwen_weaver_of_hope_0(c11f_proposal!("r2ca", "arwen_weaver_of_hope"), 0, "Each other creature you control enters with a number of additional +1/+1 counters on it equal to ~'s toughness.")]
#[case::bident_of_thassa_1(c11f_proposal!("r2ca", "bident_of_thassa"), 1, "{1}{U}, {T}: Creatures your opponents control attack this turn if able.")]
#[case::bramble_sovereign_0(c11f_proposal!("r2ca", "bramble_sovereign"), 0, "Whenever another nontoken creature enters, you may pay {1}{G}. If you do, that creature's controller creates a token that's a copy of that creature.")]
#[case::breath_of_the_sleepless_0(c11f_proposal!("r2ca", "breath_of_the_sleepless"), 0, "You may cast Spirit spells as though they had flash.")]
#[case::breath_of_the_sleepless_1(c11f_proposal!("r2ca", "breath_of_the_sleepless"), 1, "Whenever you cast a creature spell during an opponent's turn, tap up to one target creature.")]
#[case::brokers_charm_0(c11f_proposal!("r2ca", "brokers_charm"), 0, "Choose one —\n• Target creature you control gets +1/+0 until end of turn. It deals damage equal to its power to target creature or planeswalker an opponent controls.\n• Destroy target enchantment.\n• Draw two cards.")]
#[case::cruel_ultimatum_0(c11f_proposal!("r2ca", "cruel_ultimatum"), 0, "Target opponent sacrifices a creature of their choice, discards three cards, then loses 5 life. You return a creature card from your graveyard to your hand, draw three cards, then gain 5 life.")]
#[case::curse_of_clinging_webs_1(c11f_proposal!("r2ca", "curse_of_clinging_webs"), 1, "Whenever a nontoken creature enchanted player controls dies, exile it and you create a 1/2 green Spider creature token with reach.")]
#[case::cyberman_patrol_0(c11f_proposal!("r2ca", "cyberman_patrol"), 0, "Artifact creatures you control have afflict 3.")]
#[case::dark_depths_2(c11f_proposal!("r2ca", "dark_depths"), 2, "When ~ has no ice counters on it, sacrifice it. If you do, create Marit Lage, a legendary 20/20 black Avatar creature token with flying and indestructible.")]
#[case::demanding_dragon_1(c11f_proposal!("r2ca", "demanding_dragon"), 1, "When this creature enters, it deals 5 damage to target opponent unless that player sacrifices a creature of their choice.")]
#[case::drakuseth_maw_of_flames_1(c11f_proposal!("r2ca", "drakuseth_maw_of_flames"), 1, "Whenever ~ attacks, ~ deals 4 damage to any target and 3 damage to each of up to two other targets.")]
#[case::enchanters_bane_0(c11f_proposal!("r2ca", "enchanters_bane"), 0, "At the beginning of your end step, target enchantment deals damage equal to its mana value to its controller unless that player sacrifices it.")]
#[case::extract_from_darkness_0(c11f_proposal!("r2ca", "extract_from_darkness"), 0, "Each player mills two cards. Then you put a creature card from a graveyard onto the battlefield under your control.")]
#[case::fathom_fleet_captain_1(c11f_proposal!("r2ca", "fathom_fleet_captain"), 1, "Whenever this creature attacks, if you control another nontoken Pirate, you may pay {2}. If you do, create a 2/2 black Pirate creature token with menace.")]
#[case::flameshadow_conjuring_0(c11f_proposal!("r2ca", "flameshadow_conjuring"), 0, "Whenever a nontoken creature you control enters, you may pay {R}. If you do, create a token that's a copy of that creature. That token gains haste. Exile it at the beginning of the next end step.")]
#[case::folk_hero_0(c11f_proposal!("r2ca", "folk_hero"), 0, "Commander creatures you own have \"Whenever you cast a spell that shares a creature type with this creature, draw a card. This ability triggers only once each turn.\"")]
#[case::gamekeeper_0(c11f_proposal!("r2cb", "gamekeeper"), 0, "When this creature dies, you may exile it. If you do, reveal cards from the top of your library until you reveal a creature card. Put that card onto the battlefield and put all other cards revealed this way into your graveyard.")]
#[case::gary_clone_1(c11f_proposal!("r2cb", "gary_clone"), 1, "Whenever this creature attacks, each creature you control named ~ gets +1/+0 until end of turn.")]
#[case::genestealer_locus_0(c11f_proposal!("r2cb", "genestealer_locus"), 0, "Whenever a creature attacks you, it gets -1/-0 until end of turn.")]
#[case::geode_golem_1(c11f_proposal!("r2cb", "geode_golem"), 1, "Whenever this creature deals combat damage to a player, you may cast your commander from the command zone without paying its mana cost.")]
#[case::giggling_skitterspike_1(c11f_proposal!("r2cb", "giggling_skitterspike"), 1, "Whenever this creature attacks, blocks, or becomes the target of a spell, it deals damage equal to its power to each opponent.")]
#[case::gix_yawgmoth_praetor_0(c11f_proposal!("r2cb", "gix_yawgmoth_praetor"), 0, "Whenever a creature deals combat damage to one of your opponents, its controller may pay 1 life. If they do, they draw a card.")]
#[case::glimmer_lens_1(c11f_proposal!("r2cb", "glimmer_lens"), 1, "Whenever equipped creature and at least one other creature attack, draw a card.")]
#[case::goblin_spymaster_1(c11f_proposal!("r2cb", "goblin_spymaster"), 1, "At the beginning of each opponent's end step, that player creates a 1/1 red Goblin creature token with \"Creatures you control attack each combat if able.\"")]
#[case::grave_upheaval_0(c11f_proposal!("r2cb", "grave_upheaval"), 0, "Put target creature card from a graveyard onto the battlefield under your control. It gains haste.")]
#[case::hall_of_oracles_2(c11f_proposal!("r2cb", "hall_of_oracles"), 2, "{T}: Put a +1/+1 counter on target creature. Activate only as a sorcery and only if you've cast an instant or sorcery spell this turn.")]
#[case::helix_pinnacle_2(c11f_proposal!("r2cb", "helix_pinnacle"), 2, "At the beginning of your upkeep, if there are 100 or more tower counters on this enchantment, you win the game.")]
#[case::hellkite_tyrant_2(c11f_proposal!("r2cb", "hellkite_tyrant"), 2, "Whenever this creature deals combat damage to a player, gain control of all artifacts that player controls.")]
#[case::hindervines_0(c11f_proposal!("r2cb", "hindervines"), 0, "Prevent all combat damage that would be dealt this turn by creatures with no +1/+1 counters on them.")]
#[case::hullbreaker_horror_2(c11f_proposal!("r2cb", "hullbreaker_horror"), 2, "Whenever you cast a spell, choose up to one —\n• Return target spell you don't control to its owner's hand.\n• Return target nonland permanent to its owner's hand.")]
#[case::hydroid_krasis_0(c11f_proposal!("r2cb", "hydroid_krasis"), 0, "When you cast this spell, you gain half X life and draw half X cards. Round down each time.")]
#[case::indulgent_tormentor_1(c11f_proposal!("r2cb", "indulgent_tormentor"), 1, "At the beginning of your upkeep, draw a card unless target opponent sacrifices a creature of their choice or pays 3 life.")]
#[case::infested_thrinax_1(c11f_proposal!("r2cb", "infested_thrinax"), 1, "When this creature enters, until end of turn, whenever a nontoken creature you control dies, create a number of 1/1 green Saproling creature tokens equal to that creature's power.")]
#[case::in_too_deep_1(c11f_proposal!("r2cb", "in_too_deep"), 1, "Enchant creature, planeswalker, or Clue")]
#[case::ironsoul_enforcer_0(c11f_proposal!("r2cb", "ironsoul_enforcer"), 0, "Whenever this creature or a commander you control attacks alone, return target artifact card from your graveyard to the battlefield.")]
#[case::it_that_betrays_1(c11f_proposal!("r2cb", "it_that_betrays"), 1, "Whenever an opponent sacrifices a nontoken permanent, put that card onto the battlefield under your control.")]
#[case::kari_zev_skyship_raider_2(c11f_proposal!("r2cb", "kari_zev_skyship_raider"), 2, "Whenever ~ attacks, create Ragavan, a legendary 2/1 red Monkey creature token. Ragavan enters tapped and attacking. Exile that token at end of combat.")]
#[case::kasla_the_broken_halo_4(c11f_proposal!("r2cb", "kasla_the_broken_halo"), 4, "Whenever you cast another spell that has convoke, scry 2, then draw a card.")]
#[case::keeper_of_the_accord_0(c11f_proposal!("r2cb", "keeper_of_the_accord"), 0, "At the beginning of each opponent's end step, if that player controls more creatures than you, create a 1/1 white Soldier creature token.")]
#[case::keeper_of_the_accord_1(c11f_proposal!("r2cb", "keeper_of_the_accord"), 1, "At the beginning of each opponent's end step, if that player controls more lands than you, you may search your library for a basic Plains card, put it onto the battlefield tapped, then shuffle.")]
#[case::leitmotif_composer_2(c11f_proposal!("r2cb", "leitmotif_composer"), 2, "{2}{U}: Creatures named ~ can't be blocked this turn.")]
#[case::lokis_scepter_0(c11f_proposal!("r2cb", "lokis_scepter"), 0, "When ~ enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it becomes a Villain in addition to its other types and gains haste.")]
#[case::bretagard_stronghold_2(c11f_proposal!("r2ck", "bretagard_stronghold"), 2, "{G}{W}{W}, {T}, Sacrifice this land: Put a +1/+1 counter on each of up to two target creatures you control. They gain vigilance and lifelink until end of turn. Activate only as a sorcery.")]
#[case::chrome_courier_1(c11f_proposal!("r2ck", "chrome_courier"), 1, "When this creature enters, reveal the top two cards of your library. Put one of them into your hand and the other into your graveyard. If you put an artifact card into your hand this way, you gain 3 life.")]
#[case::decree_of_justice_2(c11f_proposal!("r2ck", "decree_of_justice"), 2, "When you cycle this card, you may pay {X}. If you do, create X 1/1 white Soldier creature tokens.")]
#[case::dulcet_sirens_0(c11f_proposal!("r2ck", "dulcet_sirens"), 0, "{U}, {T}: Target creature attacks target opponent this turn if able.")]
#[case::elenda_and_azor_2(c11f_proposal!("r2ck", "elenda_and_azor"), 2, "Whenever ~ attacks, you may pay {X}{W}{U}{B}. If you do, draw X cards.")]
#[case::feral_appetite_1(c11f_proposal!("r2ck", "feral_appetite"), 1, "{1}{G}: Exile target card from a graveyard. If a creature card is exiled this way, create a 1/1 black and green Pest creature token with \"When this token dies, you gain 1 life.\"")]
#[case::filigree_vector_0(c11f_proposal!("r2ck", "filigree_vector"), 0, "When this creature enters, put a +1/+1 counter on each of any number of target creatures and a charge counter on each of any number of target artifacts.")]
#[case::flameblast_dragon_1(c11f_proposal!("r2ck", "flameblast_dragon"), 1, "Whenever this creature attacks, you may pay {X}{R}. If you do, it deals X damage to any target.")]
#[case::horde_of_notions_3(c11f_proposal!("r2cl", "horde_of_notions"), 3, "{W}{U}{B}{R}{G}: You may play target Elemental card from your graveyard without paying its mana cost.")]
#[case::journey_to_eternity_atzal_1(c11f_proposal!("r2cl", "journey_to_eternity_atzal_cave_of_eternity"), 1, "When enchanted creature dies, return it to the battlefield under your control, then return this card to the battlefield transformed under your control.")]
#[case::curse_of_unbinding_1(c11f_proposal!("r2cy", "curse_of_unbinding"), 1, "At the beginning of enchanted player's upkeep, that player reveals cards from the top of their library until they reveal a creature card. Put that card onto the battlefield under your control. That player puts the rest of the revealed cards into their graveyard.")]
#[case::embodiment_of_insight_2(c11f_proposal!("r2cy", "embodiment_of_insight"), 2, "Whenever a land you control enters, you may have target land you control become a 3/3 Elemental creature with haste until end of turn. It's still a land.")]
#[case::acquired_mutation_2(c11f_proposal!("r2o", "acquired_mutation"), 2, "Whenever enchanted creature attacks, defending player gets two rad counters.")]
#[case::fathom_seer_0(c11f_proposal!("r2o", "fathom_seer"), 0, "Morph—Return two Islands you control to their owner's hand.")]
#[case::familys_favor_0(c11f_proposal!("r2s", "familys_favor"), 0, "Whenever you attack, put a shield counter on target attacking creature. Until end of turn, it gains \"Whenever this creature deals combat damage to a player, remove a shield counter from it. If you do, draw a card.\"")]
#[case::biovisionary_0(c11f_proposal!("r2y", "biovisionary"), 0, "At the beginning of the end step, if you control four or more creatures named ~, you win the game.")]
#[case::blood_curdle_0(c11f_proposal!("r2y", "blood_curdle"), 0, "Destroy target creature. Put a menace counter on a creature you control.")]
#[case::confiscation_coup_0(c11f_proposal!("r2y", "confiscation_coup"), 0, "Choose target artifact or creature. You get {E}{E}{E}{E}, then you may pay an amount of {E} equal to that permanent's mana value. If you do, gain control of it.")]
fn c11f_whole_reads_and_its_near_misses_do_not(
    #[case] proposal: &str,
    #[case] index: usize,
    #[case] clause: &str,
) {
    assert!(!clause.trim().is_empty(), "an empty clause");
    let json = c11f_ability_json(proposal, index);
    let ability: Ability = serde_json::from_str(&json).expect("ability");
    assert!(
        c11f_reads(&ability, clause),
        "{json}\ndoes not read {clause}"
    );
    let swapped = C11F_SWAPS
        .iter()
        .filter(|(from, _)| json.contains(from))
        .map(|(from, to)| json.replacen(from, to, 1));
    let mut misses = 0;
    for other in swapped.chain(c11f_raised(&json)) {
        if let Ok(other) = serde_json::from_str::<Ability>(&other) {
            assert!(!c11f_reads(&other, clause), "{other:?}\nreads {clause}");
            misses += 1;
        }
    }
    assert!(misses > 0, "no near-miss of {json}");
}
