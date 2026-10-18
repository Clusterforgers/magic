/** The spec of `/dev/table?stops=`, such as `own:main1,opp:end:keep`, read into the stops it lights. */
import type { StopStep } from '@/play/stops'
import type { Stop, StopSide } from '@/protocol/generated'
import { withStop } from './devStops'

const SIDES = new Map<string, StopSide>([
  ['own', 'Own'],
  ['opp', 'Opponent'],
  ['opponent', 'Opponent'],
])

/** The names a step goes by in a spec: its own name in lower case, and a short one for the long names. */
const STEPS = new Map<string, StopStep>([
  ['upkeep', 'Upkeep'],
  ['draw', 'Draw'],
  ['main1', 'Main1'],
  ['begin', 'BeginCombat'],
  ['begincombat', 'BeginCombat'],
  ['attackers', 'DeclareAttackers'],
  ['declareattackers', 'DeclareAttackers'],
  ['blockers', 'DeclareBlockers'],
  ['declareblockers', 'DeclareBlockers'],
  ['firststrike', 'FirstStrikeDamage'],
  ['firststrikedamage', 'FirstStrikeDamage'],
  ['damage', 'CombatDamage'],
  ['combatdamage', 'CombatDamage'],
  ['endcombat', 'EndCombat'],
  ['main2', 'Main2'],
  ['end', 'End'],
  ['endstep', 'End'],
])

/**
 * Reads comma-separated `side:step` or `side:step:keep` items (`own` or `opp`, a step name, and `keep` for a lasting
 * stop) into the list the server would hold, own side first and each side in turn order. An item that is not one (an
 * unknown side or step, untap, cleanup, extra parts) is skipped; a stop named twice is held once, the last wins.
 */
export function parseStopSpec(spec: string): Stop[] {
  let stops: Stop[] = []
  for (const item of spec.split(',')) {
    const parts = item
      .toLowerCase()
      .split(':')
      .map((part) => part.trim())
    const [sideName = '', stepName = '', keepName, ...extra] = parts
    const side = SIDES.get(sideName)
    const step = STEPS.get(stepName)
    if (side === undefined || step === undefined || extra.length > 0) continue
    if (keepName !== undefined && keepName !== 'keep') continue
    stops = withStop(stops, side, step, true, keepName === 'keep')
  }
  return stops
}
