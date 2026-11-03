/** What a card's picture needs drawn over its stat box: the current power/toughness or loyalty where it differs. */
import type { ObjectView } from '@/protocol/generated'

/** The printed stats of the face a permanent shows, as `GET /api/cards/{oracle_id}` gives them. */
export interface PrintedStats {
  power: string | null
  toughness: string | null
  loyalty: string | null
}

/** Printed stats, still being asked for, unknown for good (no record or no server), or none to compare (a token). */
export type Printed = PrintedStats | 'loading' | 'unavailable' | 'none'

/** Which way the current value moved from the printed one; null when it is not known. */
export type Modified = 'up' | 'down' | 'changed' | null

/** The badge on the stat box: its text and how it differs. */
export interface StatBadge {
  kind: 'pt' | 'loyalty'
  text: string
  modified: Modified
}

const numberOf = (text: string | null): number | null => {
  if (text === null || !/^-?\d+$/.test(text.trim())) return null
  return Number(text)
}

// Up when no value fell and one rose, down the other way round, changed when one rose and another fell.
function direction(pairs: readonly (readonly [number, number])[]): Modified {
  const up = pairs.some(([now, was]) => now > was)
  const down = pairs.some(([now, was]) => now < was)
  if (up && down) return 'changed'
  if (up) return 'up'
  return down ? 'down' : null
}

/**
 * The badge a picture needs: the current power/toughness (or a planeswalker's loyalty) when it differs from the
 * printed one, marked up or down; a plain one when the printed value is a star or cannot be known; a token always, tinted
 * against the numbers it was created with (plain when it carries none); none while it is asked for or when a creature's picture shows the
 * right number. A planeswalker's loyalty always shows, as the picture only has its starting loyalty.
 */
export function statBadge(object: ObjectView, printed: Printed, loyalty: number | null): StatBadge | null {
  if (printed === 'loading') return null
  const { power, toughness } = object
  // A face-down creature has no printed card to compare with, so its real numbers show plainly.
  const plain =
    power !== null && toughness !== null
      ? ({ kind: 'pt', text: `${power}/${toughness}`, modified: null } as const)
      : null
  if (object.face_down === true) return plain
  if (printed === 'none') {
    if (object.kind !== 'Token') return null
    const { printed_power: was_power, printed_toughness: was_toughness } = object
    if (power === null || toughness === null || was_power == null || was_toughness == null) return plain
    const modified = direction([
      [power, was_power],
      [toughness, was_toughness],
    ])
    return { kind: 'pt', text: `${power}/${toughness}`, modified }
  }
  if (power !== null && toughness !== null) {
    const text = `${power}/${toughness}`
    if (printed === 'unavailable') return { kind: 'pt', text, modified: null }
    const was = [numberOf(printed.power), numberOf(printed.toughness)] as const
    if (was[0] === null || was[1] === null) return { kind: 'pt', text, modified: null }
    const modified = direction([
      [power, was[0]],
      [toughness, was[1]],
    ])
    return modified === null ? null : { kind: 'pt', text, modified }
  }
  if (loyalty === null) return null
  const text = String(loyalty)
  if (printed === 'unavailable') return { kind: 'loyalty', text, modified: null }
  const was = numberOf(printed.loyalty)
  if (was === null) return { kind: 'loyalty', text, modified: null }
  const modified = direction([[loyalty, was]])
  return { kind: 'loyalty', text, modified }
}
