/** What the parts of the table that draw decoration read: the table's `Fx`, and the entries and ghosts in it. */
import { createContext, useContext } from 'react'
import { useStore } from 'zustand'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import type { RowKind } from '../cardModel'
import { NO_ENTRIES, NO_GHOSTS, rowKey } from './model'
import type { FxEntry, Ghost } from './model'
import { NO_FX } from './store'
import type { Fx } from './store'

/** The decoration layer of the table around the caller; outside a table it plays nothing. */
export const FxContext = createContext<Fx>(NO_FX)

/** The decoration layer of the table around the caller. */
export function useFx(): Fx {
  return useContext(FxContext)
}

/** What plays on one card now. The same list while nothing about this card changes, so it redraws only then. */
export function useCardFx(id: ObjectId): readonly FxEntry[] {
  return useStore(useFx().store, (state) => state.objects.get(id) ?? NO_ENTRIES)
}

/** What plays on one seat's life total now. */
export function useSeatFx(seat: PlayerId): readonly FxEntry[] {
  return useStore(useFx().store, (state) => state.seats.get(seat) ?? NO_ENTRIES)
}

/** The ghosts standing in one row of a seat's battlefield now. */
export function useGhosts(seat: PlayerId, row: RowKind): readonly Ghost[] {
  const key = rowKey(seat, row)
  return useStore(useFx().store, (state) => state.ghosts.get(key) ?? NO_GHOSTS)
}
