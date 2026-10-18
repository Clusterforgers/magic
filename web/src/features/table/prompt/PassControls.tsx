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
import { usePrompt } from './PromptContext'
import { passSituation, primaryDescription, primaryEndsTurn, primaryLabel, YIELD_WORDS } from './passWords'
import { usePrimaryKey } from './usePrimaryKey'

/** What the keyboard hint on the primary button promises, spoken and shown. */
const SHORTCUT_HINT = 'Space or Enter'

/**
 * The ways to pass priority, for a Priority prompt: a primary button that says what passing does now ("Next: Combat",
 * "Resolve Lightning Bolt", "End turn", "Continue"), "End turn" beside it to pass until the turn ends, and a small menu
 * with "Resolve everything on the stack" and "Pass until my next turn". The yields are the server's: it passes for the
 * seat and says so, and the prompt goes with its first pass. Space or Enter presses the primary button when nothing
 * has the keyboard.
 */
export function PassControls({ canPass }: { canPass: boolean }) {
  const { session } = useGame()
  const { answer } = usePrompt()
  const situation = usePlay(useShallow(passSituation))
  const stacked = usePlay((state) => (state.view?.stack.count ?? 0) > 0)
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
  usePrimaryKey(pass, free)

  const label = primaryLabel(situation)
  return (
    <>
      <Button
        data-slot="primary-action"
        className="h-10 px-4 max-sm:basis-full"
        disabled={!free}
        title={SHORTCUT_HINT}
        aria-keyshortcuts="Space Enter"
        aria-description={primaryDescription(situation)}
        onClick={pass}
      >
        <span className="truncate">{label}</span>
        <kbd
          aria-hidden="true"
          className="ml-1 hidden rounded border border-primary-foreground/40 px-1 text-[0.65rem] leading-4 font-normal sm:inline-block"
        >
          Space
        </kbd>
      </Button>
      {primaryEndsTurn(situation) ? null : (
        <Button
          data-slot="secondary-action"
          variant="outline"
          className="h-10 px-4"
          disabled={!free}
          aria-description={YIELD_WORDS.EndOfTurn.description}
          onClick={() => yieldTo('EndOfTurn')}
        >
          {YIELD_WORDS.EndOfTurn.action}
        </Button>
      )}
      <DropdownMenu>
        <DropdownMenuTrigger
          data-slot="more-passing"
          aria-label="More ways to pass"
          disabled={!free}
          render={<Button variant="outline" size="icon" className="size-10" />}
        >
          <EllipsisVertical aria-hidden="true" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
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
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  )
}
