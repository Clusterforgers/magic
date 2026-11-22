import type { AutoChoices, StackStops } from '@/protocol/generated'

/** The levels of `AutoChoices.stack_stops`, from asked the most to the least. */
export const STACK_STOP_LEVELS = ['All', 'Opponents', 'Aimed'] as const satisfies readonly StackStops[]

/** Whether `value` is a level. */
export function isStackStops(value: unknown): value is StackStops {
  return STACK_STOP_LEVELS.some((level) => level === value)
}

/** The level the choices hold; one left out is `All`. */
export function stackStopsOf(auto: AutoChoices): StackStops {
  return auto.stack_stops ?? 'All'
}

/** The choices with the level set; `All` is left out, as the server sends it. */
export function withStackStops(auto: AutoChoices, level: StackStops): AutoChoices {
  const rest = { ...auto }
  delete rest.stack_stops
  return level === 'All' ? rest : { ...rest, stack_stops: level }
}
