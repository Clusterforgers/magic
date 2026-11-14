/** The campaign content a fight against a boss reads its rules from, and the boss's abilities from it and the table. */
import { createContext, useContext, useEffect, useState } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { delvesApi } from '@/net/delves'
import type { DelveContent, PlayerId } from '@/protocol/generated'
import { useSeatNamer } from '../names'
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
  passive: 'Always',
  phase: 'Phase now',
  'next-phase': 'Next phase',
  scheme: 'Scheme now',
  omen: 'Omen',
}

/** The abilities of the boss at `seat`, read from the table and the content. */
export function useBossAbilities(seat: PlayerId): BossAbility[] {
  const name = useSeatNamer()(seat)
  const content = useDelveContent(true)
  return usePlay(
    useStable((state) => {
      const player = playerOf(state, seat)
      return player === undefined ? [] : bossAbilities(player, name, state.objectsById, content)
    }),
  )
}
