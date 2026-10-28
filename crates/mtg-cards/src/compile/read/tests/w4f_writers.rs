//! Goal: each rewording of the A to L mismatch proposals (sentences joined or ordered as printed, "for each" counts, a
//! token named before its quoted ability, a person's pronouns) reads back as the printed clause; another count, kind,
//! color, player, zone, event or duration in the same ability never does.
use super::*;
use rstest::rstest;

/// The JSON swaps that make another ability: another event, player, kind, color, zone, counter or duration.
const W4F_SWAPS: [(&str, &str); 16] = [
    ("\"Attacks\"", "\"Blocks\""),
    ("\"You\"", "\"Opponents\""),
    ("\"Creature\"", "\"Artifact\""),
    ("\"Land\"", "\"Creature\""),
    ("\"Graveyard\"", "\"Exile\""),
    ("\"UntilEndOfTurn\"", "\"UntilYourNextTurn\""),
    ("\"Red\"", "\"Blue\""),
    ("\"Black\"", "\"Red\""),
    ("\"White\"", "\"Black\""),
    ("\"Zombie\"", "\"Spirit\""),
    ("\"Detective\"", "\"Wizard\""),
    ("\"Sculpture\"", "\"Wizard\""),
    ("\"time\"", "\"fade\""),
    ("\"charge\"", "\"oil\""),
    ("\"+1/+1\"", "\"-1/-1\""),
    ("\"Indestructible\"", "\"Hexproof\""),
];

/// Inspirit's mode choice without the station condition its block line works under.
const W4F_INSPIRIT: &str = r#"{"Triggered":{"effects":[{"Modal":{"modes":[{"effects":[{"PutCounters":{"counter":"+1/+1","on":{"Target":0}}}],"targets":[{"bounds":{"max":1,"min":0},"what":{"Object":[{"Type":"Artifact"},"Other"]}}]},{"effects":[{"PutCounters":{"count":2,"counter":"charge","on":{"Target":0}}}],"targets":[{"bounds":{"max":1,"min":0},"what":{"Object":[{"Type":"Artifact"},"Other"]}}]}]}}],"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}}}}"#;

/// Whether some wording the checker accepts for the ability is the clause.
fn w4f_reads(ability: &Ability, clause: &str) -> bool {
    written_variants(ability)
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// The ability at the index of the first face of the proposal, as compact JSON.
fn w4f_ability_json(proposal: &str, index: usize) -> String {
    let card: serde_json::Value = serde_json::from_str(proposal).expect("proposal JSON");
    card["faces"][0]["abilities"][index].to_string()
}

/// The JSON with its first number after a key raised by one: another count.
fn w4f_raised(json: &str) -> Option<String> {
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

macro_rules! w4f_proposal {
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

/// Asserts that the ability reads the clause and that none of its near-misses does.
fn w4f_check(json: &str, clause: &str) {
    let ability: Ability = serde_json::from_str(json).expect("ability");
    assert!(
        w4f_reads(&ability, clause),
        "{json}\ndoes not read {clause}"
    );
    let swapped = W4F_SWAPS
        .iter()
        .filter(|(from, _)| json.contains(from))
        .map(|(from, to)| json.replacen(from, to, 1));
    let mut misses = 0;
    for other in swapped.chain(w4f_raised(json)) {
        if let Ok(other) = serde_json::from_str::<Ability>(&other) {
            assert!(!w4f_reads(&other, clause), "{other:?}\nreads {clause}");
            misses += 1;
        }
    }
    assert!(misses > 0, "no near-miss of {json}");
}

#[rstest]
#[case::limit_before_modes(w4f_proposal!("r3a", "aetheric_amplifier"), 1, "{4}, {T}: Choose one. Activate only as a sorcery.\n• Double the number of each kind of counter on target permanent.\n• Double the number of each kind of counter you have.")]
#[case::proliferate_twice(w4f_proposal!("r3a", "agent_frank_horrigan"), 2, "Whenever ~ enters or attacks, proliferate twice.")]
#[case::token_named_first(w4f_proposal!("r3a", "ajani_strength_of_the_pride"), 1, "−2: Create a 2/2 white Cat Soldier creature token named Ajani's Pridemate with \"Whenever you gain life, put a +1/+1 counter on this token.\"")]
#[case::he_was_kicked(w4f_proposal!("r3a", "batroc_the_leaper"), 1, "~ enters with a +1/+1 counter on him for each time he was kicked.")]
#[case::for_each_first(w4f_proposal!("r3a", "blood_money"), 0, "Destroy all creatures. For each nontoken creature destroyed this way, you create a tapped Treasure token.")]
#[case::chosen_color(w4f_proposal!("r3a", "brave_the_elements"), 0, "Choose a color. White creatures you control gain protection from the chosen color until end of turn.")]
#[case::for_each_last(w4f_proposal!("r3a", "ceaseless_conflict"), 0, "Destroy all creatures. Then create a 3/2 red and white Spirit creature token for each nontoken creature you controlled that was destroyed this way.")]
#[case::exiled_with_counters(w4f_proposal!("r3a", "charnel_serenade"), 0, "Surveil 3, then return a creature card from your graveyard to the battlefield with a finality counter on it. Exile ~ with three time counters on it.")]
#[case::this_token(w4f_proposal!("r3a", "circle_of_power"), 0, "You draw two cards and you lose 2 life. Create a 0/1 black Wizard creature token with \"Whenever you cast a noncreature spell, this token deals 1 damage to each opponent.\"\nWizards you control get +1/+0 and gain lifelink until end of turn.")]
#[case::first_time(w4f_proposal!("r3a", "crawling_sensation"), 1, "Whenever one or more land cards are put into your graveyard from anywhere for the first time each turn, create a 1/1 green Insect creature token.")]
#[case::mana_for_each(w4f_proposal!("r3a", "culling_ritual"), 0, "Destroy each nonland permanent with mana value 2 or less. Add {B} or {G} for each permanent destroyed this way.")]
#[case::subtype_after_condition(w4f_proposal!("r3b", "detective_of_the_month"), 1, "As long as you have the city's blessing, Detectives you control can't be blocked.")]
#[case::and_unblockable(w4f_proposal!("r3b", "dimir_keyrune"), 1, "{U}{B}: This artifact becomes a 2/2 blue and black Horror artifact creature until end of turn and can't be blocked this turn.")]
#[case::unsized_token(w4f_proposal!("r3b", "doomed_artisan"), 1, "At the beginning of your end step, create a colorless Sculpture artifact creature token with \"This token's power and toughness are each equal to the number of Sculptures you control.\"")]
#[case::and_amass(w4f_proposal!("r3b", "dreadhorde_invasion"), 0, "At the beginning of your upkeep, you lose 1 life and amass Zombies 1.")]
#[case::spent_both(w4f_proposal!("r3b", "firespout"), 0, "~ deals 3 damage to each creature without flying if {R} was spent to cast this spell and 3 damage to each creature with flying if {G} was spent to cast this spell.")]
#[case::him_he(w4f_proposal!("r3c", "hercules_olympian_hero"), 0, "Whenever ~ attacks, put a +1/+1 counter on him. He gains indestructible until end of turn.")]
#[case::on_him(w4f_proposal!("r3c", "hercules_olympian_hero"), 1, "Whenever ~ is dealt damage for the first time each turn, put that many +1/+1 counters on him.")]
#[case::exile_it_cost(w4f_proposal!("r3c", "hourglass_of_the_lost"), 1, "{T}, Remove X time counters from this artifact and exile it: Return each nonland permanent card with mana value X from your graveyard to the battlefield. Activate only as a sorcery.")]
#[case::that_permanent_produced(w4f_proposal!("r3d", "kinnan_bonder_prodigy"), 0, "Whenever you tap a nonland permanent for mana, add one mana of any type that permanent produced.")]
fn w4f_rewording_reads_and_its_near_misses_do_not(
    #[case] proposal: &str,
    #[case] index: usize,
    #[case] clause: &str,
) {
    w4f_check(&w4f_ability_json(proposal, index), clause);
}

#[test]
fn w4f_choice_of_counters_reads_and_its_near_misses_do_not() {
    w4f_check(
        W4F_INSPIRIT,
        "At the beginning of combat on your turn, put your choice of a +1/+1 counter or two charge counters on up to one other target artifact.",
    );
}
