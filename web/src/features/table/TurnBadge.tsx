import { anchor } from '@/features/learn/anchors'
import { useShallow } from 'zustand/react/shallow'
import { Pill } from '@/components/Pill'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { useSeatNamer } from './names'
import { isYourTurn, turnLeft, turnParty, turnTeammate } from './selectors'
import { PhaseTag } from './turn/PhaseTag'
import { usePhase } from './turn/usePhase'
import { badgeParts, badgeText, badgeTitle } from './turnText'

// Whole class names, so Tailwind can see them. The phase's colour fills the badge on your turn and tints it on another's.
const ON_PHASE = {
  you: 'w-auto rounded-l-none border-2 border-phase border-l-on-phase/30 bg-phase leading-6 text-on-phase',
  them: 'w-auto rounded-l-none border-2 border-phase border-l-phase/40 bg-phase-soft leading-6 text-foreground',
}

/** The party's seats from their joined form, which a shallow selector compares as one string. */
const partySeats = (joined: string): number[] => (joined === '' ? [] : joined.split(',').map(Number))

/**
 * Whose turn it is and in which step, always in the top bar: "Your turn · Round 3 · Precombat main", "Bo's turn · Round 3 ·
 * Declare blockers" (the turn number is in its tooltip and in `data-turn`), and what the game is doing before and after
 * its turns ("Opening hands"). During the turns the phase's icon and name stand at its left in the phase's colour (`PhaseTag`): filled for your own turn, tinted for another's.
 * The heading of the table's moment, plain text and not a live region; `data-slot="banner"` is where scripts read it.
 */
export function TurnBadge() {
  const name = useSeatNamer()
  const input = usePlay(
    useShallow((state) => ({
      stage: state.view?.stage ?? 'PreGame',
      turn: state.view?.turn.number ?? 0,
      round: state.view?.turn.round ?? 0,
      active: state.view?.turn.active ?? 0,
      step: state.view?.turn.step ?? 'Untap',
      left: turnLeft(state),
      controlledBy: state.view?.turn.controlled_by,
      teammate: turnTeammate(state),
      party: turnParty(state).join(','),
    })),
  )
  const seat = usePlay((state) => state.seat)
  const yours = usePlay(isYourTurn)
  const phase = usePhase()
  const { who, rest } = badgeParts(badgeText({ ...input, party: partySeats(input.party) }, seat, name))
  return (
    // It takes whose turn it is first and grows into the free room for the rest, up to all its words, before the room
    // of the turn banner (`TurnBanner`) does; with no `min-w-0`
    // it never shrinks below whose turn it is, so the phase pill and the round and step give way first.
    <div
      data-slot="turn-badge"
      {...anchor('turn.badge')}
      className="flex h-7 max-w-max grow-[999] basis-0 items-center"
    >
      {phase === null ? null : <PhaseTag phase={phase} yours={yours} />}
      <h2
        data-slot="banner"
        data-your-turn={yours || undefined}
        data-left={input.left || undefined}
        data-turn={input.turn}
        data-round={input.round}
      >
        <Pill
          data-themed={phase === null ? undefined : ''}
          title={badgeTitle({ ...input, party: partySeats(input.party) })}
          className={cn(
            // A grid whose second column may fall to nothing: a squeeze takes the round and step, never whose turn it is.
            'grid h-7 max-w-full grid-cols-[minmax(min-content,auto)_minmax(0,max-content)] justify-start px-3 text-sm max-sm:px-2',
            phase !== null && ON_PHASE[yours ? 'you' : 'them'],
          )}
        >
          <span data-slot="turn-who" className="max-w-48 truncate max-sm:max-w-32">
            {who}
          </span>
          {rest === '' ? null : <span className="min-w-0 truncate">{rest}</span>}
        </Pill>
      </h2>
    </div>
  )
}
