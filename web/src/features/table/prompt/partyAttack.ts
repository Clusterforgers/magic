/** The words of a party's shared combat in a hero's attack prompt: what the teammates chose and why a choice was refused. */
import type { PartyAttack, PlayerId } from '@/protocol/generated'

/**
 * "The party's attack was refused: …. Ann attacks with 2 creatures, Bo with 1." or "No teammate attacks yet.", ending
 * in a space so the prompt's own sentence follows; empty for a hero who declares alone.
 */
export function partyWords(party: PartyAttack | undefined, seat: (player: PlayerId) => string): string {
  if (party === undefined) return ''
  const counts = new Map<PlayerId, number>()
  for (const [player] of party.declared) counts.set(player, (counts.get(player) ?? 0) + 1)
  const parts = [...counts].map(
    ([player, count], at) =>
      `${seat(player)} ${at === 0 ? 'attacks ' : ''}with ${count} ${count === 1 ? 'creature' : 'creatures'}`,
  )
  const chosen = parts.length === 0 ? 'No teammate attacks yet.' : `${parts.join(', ')}.`
  const refused = party.refused === undefined ? '' : `The party's attack was refused: ${party.refused}. `
  return `${refused}${chosen} `
}
