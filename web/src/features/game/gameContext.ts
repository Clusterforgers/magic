import { createContext, useContext } from 'react'
import { useStore } from 'zustand'
import type { Session } from '@/play/session'
import type { PlayState } from '@/play/store'
import type { GameLookup } from './useGameInfo'

/** What every screen under a `GamePage` shares. */
export interface GameContextValue {
  /** The game's id. */
  game: string
  /** This seat's play session: read `session.store`, call `session.answer(...)` and the other actions. */
  session: Session
  /** Seat names and who is connected, from REST; see `useGameInfo`. */
  lookup: GameLookup
}

/** Provided by `GamePage`; read it through `useGame`. */
export const GameContext = createContext<GameContextValue | null>(null)

/** The game this screen belongs to; throws outside a `GamePage`. */
export function useGame(): GameContextValue {
  const value = useContext(GameContext)
  if (value === null) throw new Error('useGame must be used inside a GamePage')
  return value
}

/** Reads a slice of the live game state; re-renders only when the slice changes. Pass a stable selector. */
export function usePlay<T>(selector: (state: PlayState) => T): T {
  return useStore(useGame().session.store, selector)
}
