# Bots

How the bot brains (`crates/mtg-testkit/src/brain`) decide, what the design draws on, and how strong each level measures.

## What works for Magic and card-game AI

- **Forge** (open-source MTG engine, `forge-ai`): hand-written policy per effect (`*Ai` classes such as `CounterAi`, `DestroyAi`, `PumpAi`) gated by `AiPlayDecision`; card value from `ComputerUtilCard.evaluateCreature` (a base for a non-token, power and toughness weighted, keyword bonuses, minus drawbacks); `AiAttackController` sorts attackers and declares only attacks whose blocks it can survive, with an aggression level from the race (turns to kill each way); `AiBlockController` blocks to survive first, then good trades, then chumps only when lethal is coming. Counter decisions read AI-profile properties (`MIN_SPELL_CMC_TO_COUNTER`, `ALWAYS_COUNTER_REMOVAL_SPELLS`, `ALWAYS_COUNTER_OTHER_COUNTERSPELLS`, chances to hold combat tricks and reserve mana for the second main phase). Strength comes from many specific rules; weakness is that nothing looks ahead, so it is beaten by sequencing and bluffs.
- **Monte Carlo search on Magic**: Ward and Cowling, "Monte Carlo search applied to card selection in Magic: The Gathering" (IEEE CIG 2009) show rollouts over a rule-based player beat the rules alone for card choice; Cowling, Ward and Powley, "Ensemble determinization in Monte Carlo tree search for the imperfect information card game Magic: The Gathering" (IEEE TCIAIG 2012) show many shallow determinized trees beat one deep tree and that a rollout policy with domain knowledge matters more than iterations; Cowling, Powley and Whitehouse, "Information Set Monte Carlo Tree Search" (IEEE TCIAIG 2012) share one tree across determinizations and avoid strategy fusion.
- **Hearthstone and other CCGs**: Santos, Santos and Melo, "Monte Carlo tree search experiments in Hearthstone" (IEEE CIG 2017) and Zhang and Buro, "Improving Hearthstone AI by learning high-level rollout policies and bucketing chance node events" (CIG 2017) find the evaluation at the rollout cut-off and a sensible rollout policy dominate; Świechowski, Tajmajer and Janusz, "Improving Hearthstone AI by combining MCTS and supervised learning algorithms" (CIG 2018) learn the cut-off evaluation from game records; Kowalski and Miernik, "Evolutionary approach to collectible card game arena deckbuilding" and the Legends of Code and Magic contests (2018 to 2020) show linear feature evaluations tuned by self-play are strong under tight time limits.
- **Learned evaluation**: Silver et al., "A general reinforcement learning algorithm that masters chess, shogi, and Go through self-play" (Science 2018) learn value and policy by self-play; under a 5 s budget on a CPU without a network runtime, the useful part is a small linear evaluation over hand-picked features tuned by self-play (as in TD-Gammon, Tesauro 1995), kept deterministic.
- **Multiplayer search and politics**: Sturtevant and Korf, "On pruning techniques for multi-player games" (AAAI 2000) contrast max^n with the paranoid assumption; Schadd and Winands, "Best reply search for multiplayer games" (IEEE TCIAIG 2011) let only the strongest opponent reply. For Commander the practical rules are archenemy targeting (pressure the leader, not the weakest), not overextending into a single wipe, and avoiding kingmaking (do not cripple the player who is the only check on the leader).
- **Strong Commander players**: hold interaction with mana up and spend it at the end of an opponent's turn when unused; counter or remove by threat, not by whatever came first (engines, commanders, combo pieces and bombs over mana dorks); judge threat by board power, evasion, card-advantage engines, commander and commander damage, mana available, cards in hand and life; sequence so mana is used fully; play around open mana and known tricks; chump only to survive; save wipes for when the opponents' boards are worth more than their own.

## Approach

The 5 s server deadline rules out deep full-game search per decision; the levels share one evaluation and one threat model, and add search:

- **Threat model** (`brain/threat.rs`): a value per object (body, evasion, keywords, engines that draw or make mana each turn, commanders, tutor and combo features by card text, mana value) and per player (board power, evasion, engines, commander damage dealt, mana, hand, life). It ranks targets, counters, attacks and blocks and names the archenemy in multiplayer.
- **Interaction policy** (`brain/priority.rs`): a held counterspell, flash or instant is worth mana kept open; a counter or removal is spent only on a threat above a threshold (scaled by what the opponents can still show), on removal aimed at the seat's key creature, or at the end of an opponent's turn when the mana would go unused.
- **Search** (Normal, Hard): plays, attacks, blocks and targets are simulated on engine copies with quick policies for every seat and scored by the shared evaluation, which counts the threat model; opponents with open mana are assumed to hold interaction.
- **ISMCTS** (Very Hard): one tree over redealt worlds per decision with the heuristic answer first, the same evaluation at the cut-off.

## Decision-quality suite

`crates/mtg-testkit/src/brain/suite.rs` holds hand-made positions with their acceptable answers; `cargo run --release -p mtg-testkit --example bot_suite [level]` prints the score per level and per theme.

## Measured

Suite scores (66 cases, untimed, seed 7) before the threat model: Easy 38%, Normal 76%, Hard 71%, Very Hard 73%.
