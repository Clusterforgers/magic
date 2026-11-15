// Goal: a player's attack prompt in a team's shared combat (Two-Headed Giant, shared team turns, Delves) names what each teammate before them chose, counting each teammate's attackers once, and leads with the reason when the team's last choice was refused; a player who declares alone sees nothing extra.
import { describe, expect, it } from 'vitest'
import type { PartyAttack } from '@/protocol/generated'
import { partyWords } from './partyAttack'

const seat = (player: number): string => ['Ann', 'Bo', 'Cy'][player] ?? `Seat ${player}`
const at = { Player: 3 }

describe('partyWords', () => {
  it.each<[string, PartyAttack | undefined, string]>([
    ['a player who declares alone', undefined, ''],
    ['the first player', { declared: [] }, 'No teammate attacks yet. '],
    [
      'a player after two teammates',
      {
        declared: [
          [0, 10, at],
          [0, 11, at],
          [1, 12, at],
        ],
      },
      'Ann attacks with 2 creatures, Bo with 1 creature. ',
    ],
    [
      'a player asked again',
      { declared: [], refused: 'Flunky can’t attack alone' },
      "The team's attack was refused: Flunky can’t attack alone. No teammate attacks yet. ",
    ],
  ])('words %s', (_, party, words) => {
    expect(partyWords(party, seat)).toBe(words)
  })
})
