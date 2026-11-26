/** The piles of the pile prompt (`R509BlockPiles`): toggling a creature in a pile within its limit, and the words. */
import type { ObjectId } from '@/protocol/generated'

/** The piles with `creature` put into pile `at`, or taken out of it; a creature already in `most` piles joins no more. */
export function togglePile(piles: readonly ObjectId[][], at: number, creature: ObjectId, most: number): ObjectId[][] {
  const inPile = piles[at]?.includes(creature) ?? false
  const count = piles.filter((pile) => pile.includes(creature)).length
  if (!inPile && count >= most) return piles.map((pile) => [...pile])
  return piles.map((pile, place) => {
    if (place !== at) return [...pile]
    return inPile ? pile.filter((id) => id !== creature) : [...pile, creature]
  })
}

/** Every pile empty, one for each attacker. */
export function emptyPiles(attackers: number): ObjectId[][] {
  return Array.from({ length: attackers }, () => [])
}

/** "Pile 2: Wall, Ogre" or "Pile 2: empty". */
export function pileWords(at: number, names: readonly string[]): string {
  return `Pile ${at + 1}: ${names.length === 0 ? 'empty' : names.join(', ')}`
}

/** The confirming button: how many creatures the piles hold. */
export function confirmPilesLabel(piles: readonly ObjectId[][]): string {
  const creatures = new Set(piles.flat()).size
  if (creatures === 0) return 'Confirm empty piles'
  return `Confirm piles (${creatures} ${creatures === 1 ? 'creature' : 'creatures'})`
}
