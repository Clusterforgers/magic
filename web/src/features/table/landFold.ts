/** Whether an opponent's lands lie folded into its pod's lands chip, shared by the seat's pod and its board. */
import { createContext } from 'react'
import type { PlayState } from '@/play/store'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { battlefieldStacks } from './rowStacks'

/** The fold of one seat's lands: laid out or not, and the press that turns it. */
export interface LandFold {
  open: boolean
  toggle(): void
}

/** Provided by `SeatArea` for an opponent whose pod is drawn; a board without it lays its lands out. */
export const LandFoldContext = createContext<LandFold | null>(null)

/** What the chip says of a seat's lands: how many, how many tapped, and the names with their counts, most first. */
export interface LandTally {
  count: number
  tapped: number
  names: readonly { name: string; count: number }[]
}

const NONE: readonly ObjectId[] = []

/** The lands a seat controls, in battlefield order; wrap in `useShallow`. */
export function landIdsOf(state: PlayState, seat: PlayerId): readonly ObjectId[] {
  const lands = battlefieldStacks(state, seat).plain.lands
  return lands.length === 0 ? NONE : lands.flatMap((stack) => stack.ids)
}

/** The tally of these lands. */
export function landTally(ids: readonly ObjectId[], objects: PlayState['objectsById']): LandTally {
  const byName = new Map<string, number>()
  let tapped = 0
  for (const id of ids) {
    const object = objects[id]
    if (object === undefined) continue
    if (object.permanent?.tapped === true) tapped += 1
    byName.set(object.name, (byName.get(object.name) ?? 0) + 1)
  }
  const names = [...byName].map(([name, count]) => ({ name, count }))
  names.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
  return { count: names.reduce((sum, entry) => sum + entry.count, 0), tapped, names }
}

/** The tally in words: "14 lands, 5 tapped". */
export function landWords({ count, tapped }: Pick<LandTally, 'count' | 'tapped'>): string {
  const lands = `${count} ${count === 1 ? 'land' : 'lands'}`
  return tapped === 0 ? lands : `${lands}, ${tapped} tapped`
}

/** The names for the hover: "Island ×4, Command Tower". */
export function landNames(names: LandTally['names']): string {
  return names.map(({ name, count }) => (count > 1 ? `${name} ×${count}` : name)).join(', ')
}
