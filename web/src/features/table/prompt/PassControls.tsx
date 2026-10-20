import { EllipsisVertical } from 'lucide-react'
import { useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { YieldUntil } from '@/protocol/generated'
import { InControlsDock } from '../ControlsDock'
import { useCompactDock } from '../dockLayout'
import { usePrompt } from './PromptContext'
import {
  enterYields,
  KEY_LEGEND,
  passSituation,
  primaryDescription,
  primaryLabel,
  YIELD_WORDS,
} from './passWords'
import { usePrimaryKey } from './usePrimaryKey'

/** A key as a small cap on a button; hidden on a narrow screen, which has no keyboard, and from a screen reader. */
const KEY_CAP = 'ml-1 hidden rounded border px-1 text-[0.6875rem] leading-4 font-normal sm:inline-block'

/**
 * The ways to pass priority, for a Priority prompt: a primary button that says what passing does now ("Next: Combat",
 * "Resolve Lightning Bolt", "End turn", "Continue"), "End turn" beside it to pass until the turn ends, and a small menu
 * with "Resolve everything on the stack" and "Pass until my next turn". The yields are the server's: it passes for the
 * seat and says so, and the prompt goes with its first pass. With nothing holding the keyboard Space passes once and
 * Enter ends the turn (the "End turn" button), or passes once where there is no such button: under Full Control, which
 * the server does not let a yield run beside, and in the last window of the turn, where the primary button ends it.
 * On the table the buttons are drawn in the controls dock at the bottom right (`InControlsDock`), not in the prompt.
 */
export function PassControls({ canPass }: { canPass: boolean }) {
  const { session } = useGame()
  const { answer } = usePrompt()
  const situation = usePlay(useShallow(passSituation))
  const stacked = usePlay((state) => (state.view?.stack.count ?? 0) > 0)
  // The server refuses a yield while Full Control is on, so there is nothing to offer then.
  const canYield = usePlay((state) => state.fullControl === 'Off')
  // One request per prompt: the server answers it with a new decision or a notice, and either replaces this prompt.
  const [asked, setAsked] = useState(false)
  const free = canPass && !asked

  const pass = (): void => {
    answer({ Priority: 'Pass' })
  }
  const yieldTo = (until: YieldUntil): void => {
    if (!free) return
    if (session.yieldUntil(until)) setAsked(true)
  }
  const yieldKey = enterYields(situation, !canYield)
  // The dock of a phone has no room for "End turn" beside the primary button, so the menu holds it.
  const compact = useCompactDock()
  usePrimaryKey({ space: pass, enter: yieldKey ? () => yieldTo('EndOfTurn') : pass }, free)

  const label = primaryLabel(situation)
  return (
    <InControlsDock>
      {yieldKey && !compact ? (
        <Button
          data-slot="secondary-action"
          variant="outline"
          className="h-11 px-4"
          disabled={!free}
          title="Enter"
          aria-keyshortcuts="Enter"
          aria-description={YIELD_WORDS.EndOfTurn.description}
          onClick={() => yieldTo('EndOfTurn')}
        >
          {YIELD_WORDS.EndOfTurn.action}
          <kbd aria-hidden="true" className={`${KEY_CAP} border-foreground/30`}>
            Enter
          </kbd>
        </Button>
      ) : null}
      {canYield ? (
        <DropdownMenu>
          <DropdownMenuTrigger
            data-slot="more-passing"
            aria-label="More ways to pass"
            disabled={!free}
            render={<Button variant="outline" size="icon" className="size-11" />}
          >
            <EllipsisVertical aria-hidden="true" />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            {yieldKey && compact ? (
              <DropdownMenuItem
                data-yield="EndOfTurn"
                aria-description={YIELD_WORDS.EndOfTurn.description}
                onClick={() => yieldTo('EndOfTurn')}
              >
                {YIELD_WORDS.EndOfTurn.action}
              </DropdownMenuItem>
            ) : null}
            {stacked ? (
              <DropdownMenuItem
                data-yield="Resolve"
                aria-description={YIELD_WORDS.Resolve.description}
                onClick={() => yieldTo('Resolve')}
              >
                {YIELD_WORDS.Resolve.action}
              </DropdownMenuItem>
            ) : null}
            <DropdownMenuItem
              data-yield="MyTurn"
              aria-description={YIELD_WORDS.MyTurn.description}
              onClick={() => yieldTo('MyTurn')}
            >
              {YIELD_WORDS.MyTurn.action}
            </DropdownMenuItem>
            <p
              data-slot="key-legend"
              aria-hidden="true"
              className="mt-1 border-t px-2 pt-1.5 pb-1 text-xs text-muted-foreground max-sm:hidden"
            >
              {KEY_LEGEND}
            </p>
          </DropdownMenuContent>
        </DropdownMenu>
      ) : null}
      {/* Last, so the primary button is the one in the corner of the dock and the last stop of its row. */}
      <Button
        data-slot="primary-action"
        className="h-11 min-w-0 px-4 max-sm:basis-full"
        disabled={!free}
        title={yieldKey ? 'Space' : 'Space or Enter'}
        aria-keyshortcuts={yieldKey ? 'Space' : 'Space Enter'}
        aria-description={primaryDescription(situation)}
        onClick={pass}
      >
        <span className="truncate">{label}</span>
        <kbd aria-hidden="true" className={`${KEY_CAP} border-primary-foreground/40`}>
          {yieldKey ? 'Space' : 'Space · Enter'}
        </kbd>
      </Button>
    </InControlsDock>
  )
}
