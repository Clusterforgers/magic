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
  const byRound = boss.round_phases ?? []
  // A reworked boss has no opening phase: its track is Phase 1 and Phase 2 by round, the newest begun one current.
  if (byRound.length > 0)
    return byRound.map((phase, i) => ({
      label: `Phase ${i + 1}`,
      title: phase.name,
      state: !phase.begun ? 'next' : byRound[i + 1]?.begun === true ? 'done' : 'current',
    }))
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
    const byRound = (now.round_phases?.length ?? 0) > 0
    // A reworked boss numbers its phases from 1 by round; an older boss counts its opening phase as Phase I.
    const label = byRound ? `Phase ${now.begun}` : `Phase ${roman(now.begun + 1)}`
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

/** The name of the phase card that caps attackers for a big party; it begins with the boss's last phase. */
export const BULWARK_NAME = 'Last Bulwark'

/** What the boss's commander card says in a list of its abilities. */
export const COMMANDER_TEXT =
  'As its last phase begins, the boss casts this card from its command zone without paying its mana cost.'

/**
 * The label of the boss's commander card, "Phase III: Akroma, Angel of Wrath": the phase it comes with is the boss's
 * last own phase (the Last Bulwark beside it does not count), numbered after the opening phase.
 */
export function commanderLabel(phases: readonly string[], commander: string): string {
  const own = phases.filter((name) => name !== BULWARK_NAME).length
  return `Phase ${roman(own + 1)}: ${commander}`
}
