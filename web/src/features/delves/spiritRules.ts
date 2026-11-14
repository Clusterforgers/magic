/** How Spirits work and the ways a fallen hero comes back, in the words of the Delves rules (docs/delves.md). */

/** How a Spirit works, one line each. */
export const SPIRIT_RULES: readonly string[] = [
  'A hero at 0 life becomes a Restless Spirit while a teammate lives; the party loses only when every hero has fallen.',
  "Enemies can't attack a Spirit or target it.",
  'A Spirit starts with 1 ectoplasm and gains 1 each of its turns, and spends it on haunt actions: Wail, Ward, Whisper and Hinder cost 1, Howl costs 3.',
  'With 5 ectoplasm a Spirit can Rise Again in its main phase: back in the fight with 10 life and five fresh cards.',
]

/** The ways a fallen hero comes back, one line each. */
export const REVIVAL_WAYS: readonly string[] = [
  'Rise Again: a Spirit with 5 ectoplasm returns with 10 life and five cards.',
  'Phoenix Feather: a relic whose holder pays 6 life, once each fight, to return every fallen teammate with 8 life.',
  'Win the fight: every Spirit comes back at 10 life for the next one, and a Campfire rest heals 20.',
]
