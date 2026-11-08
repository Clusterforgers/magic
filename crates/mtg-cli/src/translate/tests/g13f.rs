//! Goal: round 13 pack g13f reads a card put onto the battlefield attacking by a dig, one pick of each kind (`ChooseEach$`), a
//! choice of the player who picks among the cards dug, a cost cut by a count, each player taking back the permanents they
//! own, and a dig that may be declined; other shapes of the same keys stay a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line, script lines and Oracle text, as compact JSON, and its gaps.
fn card(types: &str, lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\nPT:2/2\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::dig_attacking(
    "Creature",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ Whenever CARDNAME attacks, look at the top five cards of your library. You may put a creature card from among them onto the battlefield tapped and attacking. Put the rest on the bottom of your library in a random order.\nSVar:D:DB$ Dig | DigNum$ 5 | ChangeNum$ 1 | ChangeValid$ Creature | DestinationZone$ Battlefield | Tapped$ True | Attacking$ True | DestinationZone2$ Library | LibraryPosition$ -1 | RestRandomOrder$ True | Optional$ True",
    "Whenever Subject attacks, look at the top five cards of your library. You may put a creature card from among them onto the battlefield tapped and attacking. Put the rest on the bottom of your library in a random order.",
    r#""JoinAttack":{"what":{"Bound":"moved"#
)]
#[case::dig_attacking_joins_the_card_that_entered(
    "Creature",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ Whenever CARDNAME attacks, look at the top six cards of your library. You may put a creature card from among them onto the battlefield tapped and attacking. It gains indestructible until end of turn.\nSVar:D:DB$ Dig | DigNum$ 6 | ChangeNum$ 1 | Optional$ True | ChangeValid$ Creature | DestinationZone$ Battlefield | Tapped$ True | Attacking$ True | RememberChanged$ True | SubAbility$ P\nSVar:P:DB$ Pump | Defined$ Remembered | KW$ Indestructible",
    "Whenever Subject attacks, look at the top six cards of your library. You may put a creature card from among them onto the battlefield tapped and attacking. It gains indestructible until end of turn.",
    r#""affected":{"Bound":"moved"#
)]
#[case::dig_attacking_reveal_the_text_does_not_print(
    "Creature",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ Whenever CARDNAME attacks, look at the top six cards of your library. You may put a creature card from among them onto the battlefield tapped and attacking.\nSVar:D:DB$ Dig | DigNum$ 6 | ChangeNum$ 1 | Optional$ True | Reveal$ True | ChangeValid$ Creature | DestinationZone$ Battlefield | Tapped$ True | Attacking$ True",
    "Whenever Subject attacks, look at the top six cards of your library. You may put a creature card from among them onto the battlefield tapped and attacking.",
    r#""LookAtTop":{"as":"top","count":6}},{"Choose""#
)]
#[case::one_pick_of_each_kind(
    "Sorcery",
    "A:SP$ ChooseCard | Defined$ Player | Choices$ Permanent | ChooseEach$ Artifact & Creature & Enchantment & Land | ControlledByPlayer$ Chooser | Mandatory$ True | Reveal$ True | SubAbility$ S | SpellDescription$ Each player chooses from among the permanents they control an artifact, a creature, an enchantment, and a land, then sacrifices the rest.\nSVar:S:DB$ SacrificeAll | ValidCards$ Permanent.nonChosenCard",
    "Each player chooses from among the permanents they control an artifact, a creature, an enchantment, and a land, then sacrifices the rest.",
    r#"{"Not":{"Is":{"Bound":"card3"}}}"#
)]
#[case::a_lock_in_text_flag_says_nothing(
    "Sorcery",
    "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Hand | TgtPrompt$ Choose up to one target white card in your graveyard | ValidTgts$ Card.White+YouOwn | TargetMin$ 0 | TargetMax$ 1 | LockInText$ True",
    "Return up to one target white card from your graveyard to your hand.",
    r#""Color":"White""#
)]
#[case::each_player_takes_back_what_they_own(
    "Enchantment",
    "T:Mode$ Phase | Phase$ End of Turn | TriggerZones$ Battlefield | Execute$ G | TriggerDescription$ At the beginning of each end step, each player gains control of all nontoken permanents they own.\nSVar:G:DB$ GainControlVariant | AllValid$ Permanent.!token | ChangeController$ CardOwner",
    "At the beginning of each end step, each player gains control of all nontoken permanents they own.",
    r#""mods":[{"SetController":{"Bound":"owner"#
)]
#[case::a_cost_cut_by_the_cards_drawn(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4\nS:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ Count$YouDrewThisTurn | EffectZone$ All | Description$ This spell costs {1} less to cast for each card you've drawn this turn.",
    "This spell costs {1} less to cast for each card you've drawn this turn.\nSubject deals 4 damage to target creature.",
    r#""per":{"Tallied":{"what":"Drawn","who":"You"}}"#
)]
#[case::the_one_card_looked_at_may_go_to_the_battlefield(
    "Artifact Vehicle",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ Whenever CARDNAME attacks, look at the top card of your library. If it's a land card, you may put it onto the battlefield tapped.\nSVar:D:DB$ Dig | DigNum$ 1 | ChangeNum$ All | ForceRevealToController$ True | Optional$ True | PromptToSkipOptionalAbility$ True | ChangeValid$ Land | DestinationZone$ Battlefield | Tapped$ True | LibraryPosition2$ 0",
    "Whenever Subject attacks, look at the top card of your library. If it's a land card, you may put it onto the battlefield tapped.",
    r#""count":{"max":1,"min":0}"#
)]
#[case::piles_of_the_top_cards_without_a_look_flag(
    "Sorcery",
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | LibraryPosition2$ 0 | SkipReorder$ True | SubAbility$ D2 | NoLooking$ True\nSVar:D2:DB$ Dig | DigNum$ 2 | ChangeNum$ 1 | DestinationZone$ Graveyard | NoLooking$ True",
    "Look at the top three cards of your library. Put one of those cards into your hand, one into your graveyard, and one on the bottom of your library.",
    r#""to":"Graveyard""#
)]
#[case::an_opponent_chooses_among_the_cards_dug(
    "Sorcery",
    "A:SP$ Dig | DigNum$ 3 | Reveal$ True | ChangeNum$ 1 | ChangeValid$ Card | DestinationZone$ Graveyard | DestinationZone2$ Hand | Choser$ Player.Opponent",
    "Reveal the top three cards of your library. An opponent chooses one of them. Put that card into your graveyard and the rest into your hand.",
    r#""from":"Opponents""#
)]
#[case::the_targeted_player_chooses(
    "Artifact",
    "A:AB$ Dig | Cost$ 3 T | ValidTgts$ Player | DigNum$ 3 | ChangeNum$ 1 | Choser$ Targeted | DestinationZone$ Library | LibraryPosition$ 0 | DestinationZone2$ Exile",
    "{3}, {T}: Target player looks at the top three cards of their library, puts one of them back on top of their library, then exiles the rest.",
    r#""rest_as":"rest3","who":{"Target":0}"#
)]
#[case::reveal_until_a_creature_attacking(
    "Creature",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ Whenever CARDNAME attacks, reveal cards from the top of your library until you reveal a creature card. Put that card onto the battlefield tapped and attacking and the rest on the bottom of your library in a random order.\nSVar:D:DB$ DigUntil | Valid$ Creature | FoundDestination$ Battlefield | Tapped$ True | Attacking$ True | RevealedDestination$ Library | RevealedLibraryPosition$ -1 | RevealRandomOrder$ True",
    "Whenever Subject attacks, reveal cards from the top of your library until you reveal a creature card. Put that card onto the battlefield tapped and attacking and the rest on the bottom of your library in a random order.",
    r#""JoinAttack""#
)]
#[case::reveal_until_may_be_declined(
    "Creature",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ When CARDNAME enters, you may reveal cards from the top of your library until you reveal a creature card. If you do, put that card onto the battlefield and shuffle all other cards revealed this way into your library.\nSVar:D:DB$ DigUntil | Valid$ Creature | FoundDestination$ Battlefield | Optional$ True | RevealedDestination$ Library | Shuffle$ True",
    "When Subject enters, you may reveal cards from the top of your library until you reveal a creature card. If you do, put that card onto the battlefield and shuffle all other cards revealed this way into your library.",
    r#""May":{"effects":[{"RevealUntil""#
)]
#[case::the_one_card_an_event_names_moves_once(
    "Artifact",
    "T:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ S | TriggerDescription$ When CARDNAME is put into a graveyard from the battlefield, its owner shuffles it into their library.\nSVar:S:DB$ ChangeZone | Origin$ Graveyard | Destination$ Library | Defined$ TriggeredNewCardLKICopy | ChangeNum$ 1 | Shuffle$ True | Mandatory$ True",
    "When Subject is put into a graveyard from the battlefield, its owner shuffles it into their library.",
    r#""place":"Shuffled""#
)]
fn reads(#[case] types: &str, #[case] lines: &str, #[case] oracle: &str, #[case] needle: &str) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    assert!(json.contains(needle), "{needle} not in {json}");
}

#[rstest]
#[case::dig_attacking_for_several(
    "Creature",
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ Whenever CARDNAME attacks, look at the top five cards of your library. You may put two creature cards from among them onto the battlefield tapped and attacking.\nSVar:D:DB$ Dig | DigNum$ 5 | ChangeNum$ 2 | ChangeValid$ Creature | DestinationZone$ Battlefield | Tapped$ True | Attacking$ True | Optional$ True",
    "Whenever Subject attacks, look at the top five cards of your library. You may put two creature cards from among them onto the battlefield tapped and attacking.",
    "key:Dig.Attacking-shape"
)]
#[case::a_party_is_not_a_kind_of_permanent(
    "Sorcery",
    "A:SP$ ChooseCard | Defined$ Player | Choices$ Creature | ChooseEach$ Party | Reveal$ True | SpellDescription$ Each player chooses a party from among creatures they control.",
    "Each player chooses a party from among creatures they control.",
    "valid-base:Party"
)]
#[case::a_chooser_that_is_nobody_known(
    "Sorcery",
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | ChangeValid$ Card | DestinationZone$ Hand | Choser$ ChosenPlayer",
    "Look at the top three cards of your library. Put one of them into your hand.",
    "key:Dig.Choser"
)]
#[case::owners_is_the_only_new_controller_read(
    "Enchantment",
    "T:Mode$ Phase | Phase$ End of Turn | TriggerZones$ Battlefield | Execute$ G | TriggerDescription$ At the beginning of each end step, each player gains control of a nonland permanent of your choice controlled by the player to their right.\nSVar:G:DB$ GainControlVariant | AllValid$ Permanent.nonLand | ChangeController$ ChooseFromPlayerToTheirRight",
    "At the beginning of each end step, each player gains control of a nonland permanent of your choice controlled by the player to their right.",
    "key:GainControlVariant.ChangeController"
)]
#[case::a_change_type_other_than_card_is_a_filter_not_read(
    "Creature",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ D | TriggerDescription$ When CARDNAME enters, look at the top two cards of your library.\nSVar:D:DB$ Dig | DigNum$ 2 | ChangeNum$ 1 | ChangeType$ Land | DestinationZone2$ Graveyard",
    "When Subject enters, look at the top two cards of your library.",
    "key:Dig.ChangeType"
)]
fn stays_a_gap(#[case] types: &str, #[case] lines: &str, #[case] oracle: &str, #[case] gap: &str) {
    let (_, gaps) = card(types, lines, oracle);
    assert_eq!(gaps, [gap]);
}
