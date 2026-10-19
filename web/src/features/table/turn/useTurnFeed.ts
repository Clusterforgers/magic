import { useContext, useEffect, useRef } from 'react'
import { useGame } from '@/features/game/gameContext'
import { EFFECTS_BY_DEFAULT } from '../fx/useTableFx'
import { useSeatNamer } from '../names'
import { TableOptionsContext } from '../tableOptions'
import { feedBetween } from './feed'
import type { Feed } from './feed'

/** Whether the table draws the timed turn cues, and the factor the dev route's `?slow=` stretches their times by. */
export function useCueOptions(): { on: boolean; slow: number } {
  const options = useContext(TableOptionsContext)
  return { on: options.cues ?? EFFECTS_BY_DEFAULT, slow: options.effectsSlow ?? 1 }
}

/**
 * Calls `onFeed` with the feed of every batch the play store applies from now on, until the caller goes; nothing
 * is subscribed while the table draws no timed cues. It only reads the store: a cue never answers, waits for or
 * changes anything the game does.
 */
export function useTurnFeed(onFeed: (feed: Feed) => void): void {
  const { on } = useCueOptions()
  const { session } = useGame()
  const name = useSeatNamer()
  const latest = useRef({ onFeed, name })
  useEffect(() => {
    latest.current = { onFeed, name }
  })
  useEffect(() => {
    if (!on) return undefined
    return session.store.subscribe((next, prev) => {
      const feed = feedBetween(prev, next, latest.current.name)
      if (feed !== null) latest.current.onFeed(feed)
    })
  }, [on, session])
}
