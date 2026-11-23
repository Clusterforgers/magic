/**
 * The tip of a legal target on the table (`docs/ui-design.md`, Forecast): hovering or focusing it asks the engine for a
 * preview of that answer, and its foot says what the press does and what the engine forecasts ("Click to target ·
 * 3 damage · dies"). The client only says the engine's words (`previewForecastWords`).
 */
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { DecisionId, ObjectId, Target } from '@/protocol/generated'
import { previewForecastWords } from '../fx/forecast'
import { useSeatNamer } from '../names'
import { NO_NOTES, useCardNotes } from './marks'
import type { CardNotes } from './marks'

/** A legal card of the table under the pointer or the focus, for `event`'s target. */
export function pointedTarget(target: EventTarget | null): ObjectId | null {
  if (!(target instanceof Element)) return null
  const card = target.closest('[data-area] button[data-object-id][data-highlight="legal"]')
  const id = Number(card?.getAttribute('data-object-id') ?? Number.NaN)
  return Number.isFinite(id) ? id : null
}

/** The words of a target's tip: what the press does, and the engine's forecast when it gave one. */
export function tipWords(forecast: string | null): string {
  return forecast === null ? 'Click to target' : `Click to target · ${forecast}`
}

/**
 * Asks a preview for the legal object target pointed at, and writes its tip on its foot, over the prompt's own `base`
 * notes (a target the cost takes).
 */
export function useTargetTips(
  decision: DecisionId,
  offered: readonly Target[],
  base: CardNotes = NO_NOTES,
): void {
  const { session } = useGame()
  const names = useSeatNamer()
  const [pointed, setPointed] = useState<ObjectId | null>(null)
  useEffect(() => {
    const on = (event: Event): void => setPointed(pointedTarget(event.target))
    document.addEventListener('pointerover', on)
    document.addEventListener('focusin', on)
    return () => {
      document.removeEventListener('pointerover', on)
      document.removeEventListener('focusin', on)
    }
  }, [])
  const target = useMemo(
    () =>
      pointed === null ? undefined : offered.find((one) => 'Object' in one && one.Object.id === pointed),
    [offered, pointed],
  )
  useEffect(() => {
    if (target !== undefined) session.preview(decision, { ChooseTargets: { Targets: [target] } })
  }, [session, decision, target])
  const forecast = usePlay(
    useCallback(
      (state) =>
        pointed === null || target === undefined
          ? null
          : previewForecastWords(state, { Object: pointed }, names),
      [pointed, target, names],
    ),
  )
  const notes = useMemo<CardNotes>(() => {
    if (pointed === null || target === undefined) return base
    const tip = tipWords(forecast)
    const own = base.get(pointed)
    const merged = new Map(base)
    merged.set(pointed, { ...own, words: own === undefined ? tip : `${own.words}. ${tip}`, hint: tip })
    return merged
  }, [pointed, target, forecast, base])
  useCardNotes(notes)
}
