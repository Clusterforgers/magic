/** The quick selection buttons of a prompt that picks several objects: what each adds, and when it cannot. */

/** The buttons, in the order they are drawn. */
export type QuickKind = 'all' | 'owned' | 'allied' | 'none'

/** One button: its words, the choice it makes, and why it is off (undefined when it is on). */
export interface QuickButton<T> {
  kind: QuickKind
  label: string
  next: readonly T[]
  reason: string | undefined
}

/** What the buttons are worked out from. */
export interface QuickInput<T> {
  /** Every legal choice, in the order the prompt offers them. */
  legal: readonly T[]
  /** The choice so far, in the order it was made. */
  chosen: readonly T[]
  /** The most the prompt takes. */
  max: number
  /** Whether two choices are the same one. */
  same(a: T, b: T): boolean
  /** The seat that controls a choice (a player stands for themself), or null for none. */
  seatOf(choice: T): number | null
  /** The viewer's seat. */
  me: number | null
  /** The viewer's teammates, without the viewer; empty outside a team mode. */
  allies: readonly number[]
  /** Why a whole choice breaks a rule of the prompt besides its most (a total, one per name), or undefined. */
  breaks?: (next: readonly T[]) => string | undefined
}

/** The words of each button. */
export const QUICK_LABELS: Record<QuickKind, string> = {
  all: 'Select all',
  owned: 'Select all owned',
  allied: 'Select all allied',
  none: 'Unselect all',
}

/** `chosen` with every choice of `group` it lacks added at the end, in the order of `group`. */
function added<T>(chosen: readonly T[], group: readonly T[], same: (a: T, b: T) => boolean): readonly T[] {
  const extra = group.filter((choice) => !chosen.some((picked) => same(picked, choice)))
  return extra.length === 0 ? chosen : [...chosen, ...extra]
}

/**
 * The quick buttons of a prompt, or none when it takes at most one or offers fewer than two. "Select all owned" shows
 * when the viewer controls a legal choice, "Select all allied" only in a team mode when a teammate does; each adds its
 * group to the choice so far and is off, with a reason, when that would pass the most or a rule of the prompt or adds
 * nothing. "Unselect all" empties the choice.
 */
export function quickButtons<T>(input: QuickInput<T>): QuickButton<T>[] {
  const { legal, chosen, max, same, seatOf, me, allies, breaks } = input
  if (max < 2 || legal.length < 2) return []
  const groups: [QuickKind, readonly T[]][] = [['all', legal]]
  const owned = legal.filter((choice) => me !== null && seatOf(choice) === me)
  if (owned.length > 0) groups.push(['owned', owned])
  const allied = legal.filter((choice) => {
    const seat = seatOf(choice)
    return seat !== null && seat !== me && allies.includes(seat)
  })
  if (allies.length > 0 && allied.length > 0) groups.push(['allied', allied])
  const buttons = groups.map(([kind, group]): QuickButton<T> => {
    const next = added(chosen, group, same)
    const reason =
      next.length > max
        ? `Only ${max} can be chosen`
        : next === chosen
          ? 'All of them are chosen'
          : breaks?.(next)
    return { kind, label: QUICK_LABELS[kind], next, reason }
  })
  buttons.push({
    kind: 'none',
    label: QUICK_LABELS.none,
    next: [],
    reason: chosen.length === 0 ? 'Nothing is chosen' : undefined,
  })
  return buttons
}
