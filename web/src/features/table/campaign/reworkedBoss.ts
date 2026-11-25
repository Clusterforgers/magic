/**
 * The words of a reworked boss: its innate with its level, the fight's modifier, its relics, its two phases by round
 * with the countdown to the next, and the marks of its Satchel. Pure over the public `BossView` (in the game) or
 * `DelveFightView` (on the intro), whose cards carry their own rules text.
 */
import type {
  BossPartView,
  BossSatchelView,
  BossView,
  InnateView,
  RoundPhaseView,
} from '@/protocol/generated'
import { summaryOf } from './bossAbilities'
import type { BossAbility } from './bossAbilities'
import { roman } from './bossPhase'

/** The parts a reworked boss shows, the same on the intro and at the table. */
export interface ReworkedParts {
  innate?: InnateView
  modifier?: BossPartView
  round_phases?: RoundPhaseView[]
  relics?: BossPartView[]
}

/** Whether a boss view is a reworked boss's: it has an innate or phases by round. */
export function isReworked(parts: ReworkedParts | null | undefined): boolean {
  return parts?.innate !== undefined || (parts?.round_phases?.length ?? 0) > 0
}

/** "Phase 1", by place from 0. */
export const phaseLabel = (at: number): string => `Phase ${at + 1}`

/** "Phase 1 at round 8": when a phase by round begins, as the intro names it. */
export function phaseAt(at: number, phase: RoundPhaseView): string {
  return `${phaseLabel(at)} at round ${phase.round}`
}

/** "Level III". */
export const levelWords = (level: number): string => `Level ${roman(level)}`

/** The countdown to a phase: "in 3 rounds", "in 1 round"; 1 is the current round, so "at the end of this round". */
export function roundsWords(rounds: number): string {
  if (rounds <= 1) return 'at the end of this round'
  return `in ${rounds} rounds`
}

/** What the bar's phase pill says. */
export interface PhaseCountdown {
  /** The words in view: "Phase 1 in 3 rounds" before it begins, the phase's name once it has. */
  label: string
  /** Whether a phase is in force. */
  begun: boolean
  /** The countdown to the next phase while one is in force ("Phase 2 in 4 rounds"); none otherwise. */
  next?: string
  /** The sentence for a screen reader and the tooltip. */
  title: string
  /** The words where room is short: the next phase and its rounds ("P1 in 3", "P2 in 4"), else the one in force ("P2"). */
  short: string
  /** The place of the phase the pill names: the one to come before any began, else the one in force. */
  named: number
}

/** "P1 in 3": a countdown where room is short. */
function shortCountdown(at: number, rounds: number): string {
  return `P${at + 1} in ${Math.max(1, rounds)}`
}

/** The phase pill of a reworked boss: the countdown to Phase 1, then the phase in force with the countdown to the next. */
export function phaseCountdown(view: BossView): PhaseCountdown | null {
  const phases = view.round_phases ?? []
  if (phases.length === 0) return null
  const upcoming = phases.findIndex((phase) => !phase.begun)
  const current = upcoming === -1 ? phases.length - 1 : upcoming - 1
  const countdown =
    upcoming === -1 || view.phase_in === undefined
      ? undefined
      : `${phaseLabel(upcoming)} ${roundsWords(view.phase_in)}`
  if (current < 0) {
    const label = countdown ?? phaseLabel(0)
    const short = view.phase_in === undefined ? 'P1' : shortCountdown(0, view.phase_in)
    return { label, begun: false, title: `${label}.`, short, named: 0 }
  }
  const now = phases[current]
  const label = now?.name ?? phaseLabel(current)
  const title = `${phaseLabel(current)}: ${label}${countdown === undefined ? '' : `. ${countdown}`}.`
  const short =
    countdown === undefined || view.phase_in === undefined
      ? `P${current + 1}`
      : shortCountdown(upcoming, view.phase_in)
  return {
    label,
    begun: true,
    ...(countdown === undefined ? {} : { next: countdown }),
    title,
    short,
    named: current,
  }
}

/** The mark of a Satchel card: "returns · +{2}" for a reusable one with its tax now, "once" for the others. */
export function satchelMark(card: BossSatchelView): string {
  return card.reusable ? `returns · +{${card.tax}}` : 'once'
}

/** What a Satchel mark means, for its tooltip. */
export function satchelMarkTitle(card: BossSatchelView): string {
  return card.reusable
    ? `Returns to the Satchel after each cast; the next cast costs {${card.tax}} more.`
    : 'Cast once this fight.'
}

function part(
  key: string,
  kind: BossAbility['kind'],
  name: string,
  text: string,
  when?: string,
  card: string = name,
): BossAbility {
  return { key, kind, name, text, summary: summaryOf(text), card, ...(when === undefined ? {} : { when }) }
}

/**
 * The abilities of a reworked boss in reading order: its innate with its level, the fight's modifier, its relics (a
 * copy counted, "×2"), then its phases: the begun ones in force, the others to come with the round they begin at.
 */
export function reworkedAbilities(parts: ReworkedParts): BossAbility[] {
  const out: BossAbility[] = []
  const innate = parts.innate
  if (innate !== undefined)
    out.push(part(`innate-${innate.name}`, 'innate', innate.name, innate.text, levelWords(innate.level)))
  const modifier = parts.modifier
  if (modifier !== undefined)
    out.push(part(`modifier-${modifier.name}`, 'modifier', modifier.name, modifier.text))
  const counts = new Map<string, { relic: BossPartView; count: number }>()
  for (const relic of parts.relics ?? []) {
    const seen = counts.get(relic.name)
    counts.set(relic.name, { relic, count: (seen?.count ?? 0) + 1 })
  }
  for (const { relic, count } of counts.values()) {
    const name = count > 1 ? `${relic.name} ×${count}` : relic.name
    out.push(part(`relic-${relic.name}`, 'relic', name, relic.text, undefined, relic.name))
  }
  ;(parts.round_phases ?? []).forEach((phase, at) => {
    out.push(
      phase.begun
        ? part(`phase-${phase.name}`, 'phase', phase.name, phase.text, phaseLabel(at))
        : part(`phase-${phase.name}`, 'next-phase', phase.name, phase.text, phaseAt(at, phase)),
    )
  })
  return out
}
