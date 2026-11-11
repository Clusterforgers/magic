//! Goal: every card of the rmt1 pack (but Lich's Mastery, whose life-loss clause is by hand, so bots that never do it cannot lose) plays in bot games at two and four seats without breaking an invariant, erroring, looping or ending badly: the smoke gate passes it, or the games simply never met it.
use mtg_testkit::corpus::corpus;
use mtg_testkit::smoke::{DEFAULT_SEEDS, SmokeFailure, smoke_card};
use rstest::rstest;

#[rstest]
#[case("Aang, Swift Savior // Aang and La, Ocean's Fury")]
#[case("Abdel Adrian, Gorion's Ward")]
#[case("Adagia, Windswept Bastion")]
#[case("Alliance of Arms")]
#[case("Aminatou, the Fateshifter")]
#[case("Arid Archway")]
#[case("Aurelia's Vindicator")]
#[case("Bridge from Below")]
#[case("Bumi, Unleashed")]
#[case("Burning-Rune Demon")]
#[case("Clandestine Meddler")]
#[case("Collective Voyage")]
#[case("Colossal Grave-Reaver")]
#[case("Diffusion Sliver")]
#[case("Donal, Herald of Wings")]
#[case("Durkwood Tracker")]
#[case("Energybending")]
#[case("Gifts Ungiven")]
#[case("Gluntch, the Bestower")]
#[case("Gravestorm")]
#[case("Harald Unites the Elves")]
#[case("Huatli, Poet of Unity // Roar of the Fifth People")]
#[case("Intuition")]
#[case("Invasion of Arcavios // Invocation of the Founders")]
#[case("J. Jonah Jameson")]
#[case("Jace, Mirror Mage")]
#[case("Kirol, Attentive First-Year")]
#[case("Koth, Fire of Resistance")]
#[case("Maddening Hex")]
#[case("Mindlink Mech")]
#[case("Minds Aglow")]
#[case("Mirror Sheen")]
#[case("Mischievous Quanar")]
#[case("Mizzix, Replica Rider")]
#[case("Nalfeshnee")]
#[case("Nethroi, Apex of Death")]
#[case("Nihil Spellbomb")]
#[case("Peter Parker's Camera")]
#[case("Professor Onyx")]
#[case("Ratadrabik of Urborg")]
#[case("Reality Smasher")]
#[case("Reunion of the House")]
#[case("Saheeli, Sublime Artificer")]
#[case("Schema Thief")]
#[case("Scheming Symmetry")]
#[case("Secret Invasion")]
#[case("Shared Trauma")]
#[case("Sigil Tracer")]
#[case("Silent Gravestone")]
#[case("Start the TARDIS")]
#[case("Summon: Primal Odin")]
#[case("Sword of Hearth and Home")]
#[case("Tezzeret, Cruel Machinist")]
#[case("The Jolly Balloon Man")]
#[case("The Phasing of Zhalfir")]
#[case("The Pride of Hull Clade")]
#[case("The Rise of Sozin // Fire Lord Sozin")]
#[case("Triumph of Saint Katherine")]
#[case("Ulalek, Fused Atrocity")]
#[case("Ursine Champion")]
fn rmt1_pack_card_plays_in_bot_games(#[case] name: &str) {
    let corpus = corpus().unwrap_or_else(|e| panic!("{e}"));
    let card = corpus.card(name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let report = smoke_card(&card, &DEFAULT_SEEDS);
    assert!(
        matches!(report.failure, None | Some(SmokeFailure::NeverSeen { .. })),
        "{name}: {:?}",
        report.failure
    );
}
