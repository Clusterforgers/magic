/** The words of a team's shared combat in a player's attack prompt: what the teammates chose and why a choice was refused. */
import type { AttackTarget, ObjectId, PartyAttack, PlayerId } from '@/protocol/generated'

/** What the teammates before the chooser chose, creature to what it attacks: drawn as chosen while the player chooses. */
export function teamAttacks(party: PartyAttack | undefined): ReadonlyMap<ObjectId, AttackTarget> {
  return new Map(
    (party?.declared ?? []).map(([, object, target]): [ObjectId, AttackTarget] => [object, target]),
  )
}

/**
 * "The team's attack was refused: …. Ann attacks with 2 creatures, Bo with 1." or "No teammate attacks yet.", ending
 * in a space so the prompt's own sentence follows; empty for a player who declares alone.
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
  const refused = party.refused === undefined ? '' : `The team's attack was refused: ${party.refused}. `
  return `${refused}${chosen} `
}
