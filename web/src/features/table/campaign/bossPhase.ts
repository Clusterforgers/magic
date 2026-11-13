/**
 * The words of a boss seat's phases and the phase cue: pure over the public `BossView` of each seat, so every seat
 * reads the same cue from the same batch.
 */
import type { BossView, PlayerId, PlayerView } from '@/protocol/generated'

const ROMAN = ['I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII']

/** "II" for 2; the figure itself past the table. */
export const roman = (n: number): string => ROMAN[n - 1] ?? String(n)

/** One step of a boss's phase track. */
export interface PhaseStep {
  /** "Phase II". */
  label: string
  /** The phase's own words after the dash ("The Nest Stirs"); the opening phase has none. */
  title: string
  state: 'done' | 'current' | 'next'
}

/** The words of a phase name after its "Phase II — " lead, or the whole name when it has none. */
export function phaseTitle(name: string): string {
  const at = name.indexOf(' — ')
  return at < 0 ? name : name.slice(at + 3)
}

/** The track of a boss: the opening phase, then one step per phase card, the begun ones done and the newest current. */
export function phaseSteps(boss: BossView): PhaseStep[] {
  const names = ['', ...boss.phases]
  return names.map((name, i) => ({
    label: `Phase ${roman(i + 1)}`,
    title: name === '' ? '' : phaseTitle(name),
    state: i < boss.begun ? 'done' : i === boss.begun ? 'current' : 'next',
  }))
}

/** A phase that began in a batch. */
export interface PhaseCue {
  seat: PlayerId
  /** The boss's name. */
  who: string
  /** "Phase II". */
  label: string
  /** "The Nest Stirs". */
  title: string
  /** The sentence for the live region. */
  announcement: string
}

/**
 * The newest phase that began between two views: a boss seat whose `begun` grew. None for the first view (a
 * snapshot is no event), a seat new to the view, or a count that stayed or fell.
 */
export function phaseCueBetween(
  before: readonly PlayerView[] | undefined,
  after: readonly PlayerView[] | undefined,
  name: (seat: PlayerId) => string,
): PhaseCue | null {
  if (before === undefined || after === undefined) return null
  let cue: PhaseCue | null = null
  for (const player of after) {
    const now = player.boss
    const was = before.find((p) => p.seat === player.seat)?.boss
    if (now === undefined || was === undefined || now.begun <= was.begun) continue
    const label = `Phase ${roman(now.begun + 1)}`
    const title = phaseTitle(now.phases[now.begun - 1] ?? '')
    const who = name(player.seat)
    cue = {
      seat: player.seat,
      who,
      label,
      title,
      announcement: `${who} enters ${label}${title === '' ? '' : `: ${title}`}.`,
    }
  }
  return cue
}
