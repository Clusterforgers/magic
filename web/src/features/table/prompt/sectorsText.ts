/** The sectors of the space sculptor prompt (`P170aAssignSectors`): the spread every creature starts in, and the words. */
import type { ObjectId, Sector } from '@/protocol/generated'

/** The three sectors, in order. */
export const SECTORS: readonly Sector[] = ['Alpha', 'Beta', 'Gamma']

/** The creatures spread over the sectors in turn, as the response clock answers: alpha, beta, gamma, alpha, ... */
export function spreadSectors(creatures: readonly ObjectId[]): [ObjectId, Sector][] {
  return creatures.map((creature, at) => [creature, SECTORS[at % SECTORS.length] ?? 'Alpha'])
}

/** The assignments with `creature` moved to `sector`. */
export function moveToSector(
  sectors: readonly [ObjectId, Sector][],
  creature: ObjectId,
  sector: Sector,
): [ObjectId, Sector][] {
  return sectors.map(([id, was]) => [id, id === creature ? sector : was])
}

/** "Alpha: Bear, Ogre" or "Alpha: none". */
export function sectorWords(sector: Sector, names: readonly string[]): string {
  return `${sector}: ${names.length === 0 ? 'none' : names.join(', ')}`
}
