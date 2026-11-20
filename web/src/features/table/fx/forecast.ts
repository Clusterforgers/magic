/**
 * The stack forecast on screen (docs/web.md, Prompts: Forecast): the engine's `ObjectView.forecast` of the top stack item,
 * read per card and per seat. Each reader selects its own words, a string, so a batch redraws only the cards and pods
 * whose words changed. The client never works an outcome out; it only says the engine's lines.
 */
import { useCallback } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from '@/features/table/names'
import type { SeatNamer } from '@/features/table/names'
import type { PlayState } from '@/play/store'
import type { Change, ForecastChange, Forecasted, ObjectId, PlayerId } from '@/protocol/generated'
import { forecastWords } from './forecastWords'

const NONE: readonly ForecastChange[] = []

/** The forecast of the item on top of the stack; empty when there is none. */
export function topForecast(state: PlayState): readonly ForecastChange[] {
  const stack = state.view?.stack.cards ?? []
  const top = stack[stack.length - 1]
  if (top === undefined) return NONE
  return state.objectsById[top]?.forecast ?? NONE
}

/** Whether a line is about `subject`. */
export function isAbout(line: ForecastChange, subject: Forecasted): boolean {
  if ('Object' in line.subject) return 'Object' in subject && line.subject.Object === subject.Object
  return 'Player' in subject && line.subject.Player === subject.Player
}

/** The changes the lines name for `subject`, in the engine's order. */
export function changesOf(lines: readonly ForecastChange[], subject: Forecasted): Change[] {
  return lines.filter((line) => isAbout(line, subject)).map((line) => line.change)
}

/** The badge words of `subject` under the top stack item, or null when it changes nothing of it. */
export function stackForecastWords(state: PlayState, subject: Forecasted, names?: SeatNamer): string | null {
  return forecastWords(changesOf(topForecast(state), subject), names)
}

/**
 * The words of the engine's reply to the latest target preview for `subject` ("3 damage · dies"), as a target's tip
 * says them; null while there is no reply for the pending decision or it names nothing of `subject`.
 */
export function previewForecastWords(
  state: PlayState,
  subject: Forecasted,
  names?: SeatNamer,
): string | null {
  const reply = state.preview
  if (reply === null || state.pending?.id !== reply.decision) return null
  return forecastWords(changesOf(reply.changes ?? [], subject), names)
}

/** The badge words of one card under the top stack item. */
export function useCardForecast(id: ObjectId): string | null {
  const names = useSeatNamer()
  return usePlay(useCallback((state) => stackForecastWords(state, { Object: id }, names), [id, names]))
}

/** The badge words of one seat under the top stack item ("−3 → 17"). */
export function useSeatForecast(seat: PlayerId): string | null {
  const names = useSeatNamer()
  return usePlay(useCallback((state) => stackForecastWords(state, { Player: seat }, names), [seat, names]))
}
