/** The campaign content a fight against a boss reads its rules from, and the boss's abilities from it and the table. */
import { createContext, useContext, useEffect, useState } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { delvesApi } from '@/net/delves'
import type { DelveContent, PlayerId } from '@/protocol/generated'
import { playerOf } from '../selectors'
import { useStable } from '../useStable'
import { bossAbilities } from './bossAbilities'
import type { BossAbility, BossAbilityKind } from './bossAbilities'

/** The campaign content the table reads boss rules from; undefined to fetch it, null while it is not known. */
export const DelveContentContext = createContext<DelveContent | null | undefined>(undefined)

/** True inside a boss's side whose abilities bar is shown: the boss's pod then leaves out the chips the bar repeats. */
export const BossBarContext = createContext(false)

let fetched: Promise<DelveContent> | null = null

/** The campaign content: the context's, else fetched once per page load; null until it arrives or if it fails. */
export function useDelveContent(enabled: boolean): DelveContent | null {
  const given = useContext(DelveContentContext)
  const [content, setContent] = useState<DelveContent | null>(null)
  useEffect(() => {
    if (given !== undefined || !enabled) return
    let live = true
    fetched ??= delvesApi.content()
    fetched
      .then((answer) => live && setContent(answer))
      .catch(() => {
        fetched = null
      })
    return () => {
      live = false
    }
  }, [given, enabled])
  return given === undefined ? content : given
}

/** What each kind is called on its chip and in the full text. */
export const KIND_WORDS: Record<BossAbilityKind, string> = {
  innate: 'Innate',
  phase: 'Phase now',
  'next-phase': 'Next phase',
  modifier: 'This fight',
  relic: 'Boss relic',
  personality: 'Personality',
}

/** The abilities of the boss at `seat`, read from the table. */
export function useBossAbilities(seat: PlayerId): BossAbility[] {
  return usePlay(
    useStable((state) => {
      const player = playerOf(state, seat)
      return player === undefined ? [] : bossAbilities(player)
    }),
  )
}

/** The campaign content at a table: fetched once a seat is a boss (a Delves fight); null at any other table. */
export function useTableContent(): DelveContent | null {
  const fight = usePlay((state) => state.view?.players.some((player) => player.boss != null) ?? false)
  return useDelveContent(fight)
}
