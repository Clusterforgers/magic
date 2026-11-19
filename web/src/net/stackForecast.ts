/** Guards for the stack forecast on the wire (`ObjectView.forecast`, `Forecast.changes`), checked down to the leaves. */
import type { ForecastChange } from '@/protocol/generated'
import { isCount, isInt, isListOf, isRecord, isText } from './check'

const FATES: readonly string[] = [
  'Destroyed',
  'Dies',
  'Sacrificed',
  'Exiled',
  'Hand',
  'Library',
  'Graveyard',
  'Countered',
]

/** The one key of an object such as `{ Damage: {...} }`, or null when it has not exactly one. */
function only(value: Record<string, unknown>): string | null {
  const keys = Object.keys(value)
  return keys.length === 1 ? (keys[0] ?? null) : null
}

const isPair = (value: unknown): boolean => Array.isArray(value) && value.length === 2 && value.every(isInt)

const isCountPair = (value: unknown): boolean =>
  Array.isArray(value) && value.length === 2 && value.every(isCount)

const isCounterKind = (value: unknown): boolean =>
  isText(value) || (isRecord(value) && only(value) === 'Other' && isText(value.Other))

function isSubject(value: unknown): boolean {
  if (!isRecord(value)) return false
  const tag = only(value)
  return (tag === 'Player' || tag === 'Object') && isCount(value[tag])
}

function isChange(value: unknown): boolean {
  if (value === 'Tapped' || value === 'Untapped') return true
  if (!isRecord(value)) return false
  const tag = only(value)
  const body = tag === null ? undefined : value[tag]
  if (!isRecord(body)) return false
  switch (tag) {
    case 'Damage':
      return isCount(body.amount)
    case 'Stats':
      return isPair(body.power) && isPair(body.toughness)
    case 'Counters':
      return isCounterKind(body.kind) && isCountPair(body.count)
    case 'Control':
      return isCount(body.controller)
    case 'Leaves':
      return isText(body.fate) && FATES.includes(body.fate)
    case 'Life':
      return isInt(body.life) && isInt(body.life_after)
    default:
      return false
  }
}

/** True for one line of a stack forecast: a subject and a change the client knows. */
export function isForecastChange(value: unknown): value is ForecastChange {
  return isRecord(value) && isSubject(value.subject) && isChange(value.change)
}

/** True for a stack forecast the server may leave out: absent, or a list of lines. */
export function isOptionalForecast(value: unknown): value is ForecastChange[] | undefined {
  return value === undefined || isListOf(value, isForecastChange)
}
