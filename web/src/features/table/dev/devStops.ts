/** What the dev table's stand-in for the server does with a stop request, so the ladder can be clicked without one. */
import { STOP_STEPS } from '@/play/stops'
import type { Step, Stop, StopSide } from '@/protocol/generated'

const SIDES: readonly StopSide[] = ['Own', 'Opponent']

const place = (stop: Stop): number =>
  SIDES.indexOf(stop.side) * STOP_STEPS.length + STOP_STEPS.findIndex((step) => step === stop.step)

/**
 * `stops` after the server has set (`on`) or cleared the stop at `side` and `step`: one stop per side and step, own
 * side first and each side in turn order, the way the server lists them.
 */
export function withStop(
  stops: readonly Stop[],
  side: StopSide,
  step: Step,
  on: boolean,
  keep: boolean,
): Stop[] {
  const others = stops.filter((stop) => stop.side !== side || stop.step !== step)
  return (on ? [...others, { side, step, keep }] : others).sort((a, b) => place(a) - place(b))
}
