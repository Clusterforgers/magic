/** Narrowing guards for JSON read off the wire: every `unknown` becomes a typed value through one of these. */

/** True for a JSON object (not null, not an array). */
export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** True for a whole number of at least zero: ids, seats, counters and sequence numbers. */
export function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

/** True for any whole number; life totals and power can be negative. */
export function isInt(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value)
}

/** True for a string. */
export function isText(value: unknown): value is string {
  return typeof value === 'string'
}

/** True for a boolean. */
export function isFlag(value: unknown): value is boolean {
  return typeof value === 'boolean'
}

/** True for an array whose items all pass `guard`. */
export function isListOf<T>(value: unknown, guard: (item: unknown) => item is T): value is T[] {
  return Array.isArray(value) && value.every((item) => guard(item))
}
