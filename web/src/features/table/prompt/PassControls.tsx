import { usePassHeld } from '@/features/learn/useLessonHold'
import { anchor } from '@/features/learn/anchors'
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
import { cn } from '@/lib/utils'
import type { PlayState } from '@/play/store'
import type { ObjectId, ObjectView, YieldUntil } from '@/protocol/generated'
import { InControlsDock } from '../ControlsDock'
import { useDockForm } from '../dockLayout'
import { usePrompt } from './PromptContext'
import {
  endTurnShown,
  enterYields,
  KEY_LEGEND,
  lastWindow,
  passSituation,
  primaryDescription,
  primaryLabel,
  resolveAllShown,
  YIELD_WORDS,
} from './passWords'
import { usePrimaryKey } from './usePrimaryKey'

/** What "Always yield to" does, for a screen reader. */
const ALWAYS_YIELD_DESCRIPTION =
  'Lets this ability, and every later one with the same name and words, resolve without asking you for the rest of the game. Full Control still asks.'

/** The ability on top of the stack when every player can read it, which the seat can always yield to; else undefined. */
export function topYieldable(state: PlayState): ObjectView | undefined {
  const id = state.view?.stack.cards?.at(-1)
  const top = id === undefined ? undefined : state.objectsById[id]
  return top?.kind === 'Ability' && top.name !== '' ? top : undefined
}

/** A key as a small cap on a button, drawn where the dock has the room (`full`) and always hidden from a screen reader. */
// A touch screen has no keys to press, so the caps give their room to the label there.
const KEY_CAP = 'ml-1 inline-block rounded border px-1 text-xs leading-4 font-normal pointer-coarse:hidden'

/** End turn carries one anchor, in the dock or in the menu where the dock is narrow. */
const END_TURN = anchor('pass.end-turn')

/**
 * The ways to pass priority, for a Priority prompt: a primary button that passes once and says what that does now
 * ("Next: Combat", "Resolve Lightning Bolt", "Continue"), "End turn" beside it to pass until the turn ends, and a small
 * menu with "Resolve all", "Always yield to <ability>" (over an ability every player can read) and "Pass until my
 * next turn". Over a stack of two or more objects "Resolve all" (MTG Arena's) stands beside the primary button instead,
 * off a phone, and "End turn" moves into the menu, so the dock keeps five controls; Space and Enter keep their meaning.
 * Resolve and End turn are never one button: in the last window of the viewer's turn the primary button is not drawn and "End turn" alone moves on, so a Resolve pressed twice never ends the turn. The
 * yields are the server's: it passes for the seat and says so, and the prompt goes with its first pass. With nothing
 * holding the keyboard Space passes once and Enter ends the turn (the "End turn" button); under Full Control, which the
 * server does not let a yield run beside, Enter passes once.
 * On the table the buttons are drawn in the controls dock at the bottom right (`InControlsDock`), not in the prompt;
 * the dock's form (`useDockForm`) says whether they carry key hints and the spell's name or keep to short words.
 */
export function PassControls({ canPass }: { canPass: boolean }) {
  const { session } = useGame()
  // A lesson holds passing back while its step asks for something else: the button looks off and a press says why.
  const held = usePassHeld()
  const { answer } = usePrompt()
  const situation = usePlay(useShallow(passSituation))
  const stackCount = usePlay((state) => state.view?.stack.count ?? 0)
  const stacked = stackCount > 0
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
  const always = (id: ObjectId): void => {
    if (!free) return
    if (session.autoYield(id)) setAsked(true)
  }
  const topAbility = usePlay(topYieldable)
  const yieldKey = enterYields(!canYield)
  const endTurn = yieldKey ? () => yieldTo('EndOfTurn') : pass
  const form = useDockForm()
  const last = lastWindow(situation)
  // The dock of a phone has no room for "End turn" beside the primary button, so the menu holds it; in the last
  // window there is no primary button and "End turn" stands in the dock.
  const compact = form === 'compact'
  const full = form === 'full'
  // Over a stack of two or more "Resolve all" stands beside the primary button and "End turn" waits in the menu, so the
  // dock keeps its five controls; Enter still ends the turn. A phone has no room for it: its menu holds it.
  const resolveAll = resolveAllShown(stackCount, !canYield)
  const resolveAllInDock = resolveAll && !compact
  const endTurnInDock = endTurnShown(situation, !canYield) && (!compact || last) && !resolveAllInDock
  usePrimaryKey({ space: pass, enter: endTurn }, free)

  const label = primaryLabel(situation, form)
  // Where the dock has no room for key hints the buttons are as wide as their words need: the keys are in the tooltips and the menu.
  const padding = full ? 'px-4' : 'px-3'
  return (
    <InControlsDock>
      {endTurnInDock ? (
        <Button
          data-slot="secondary-action"
          {...END_TURN}
          variant="outline"
          className={cn('h-11', padding)}
          disabled={!free}
          title="Enter"
          aria-keyshortcuts="Enter"
          aria-description={YIELD_WORDS.EndOfTurn.description}
          onClick={endTurn}
        >
          {YIELD_WORDS.EndOfTurn.action}
          {full ? (
            <kbd aria-hidden="true" data-slot="key-cap" className={`${KEY_CAP} border-foreground/30`}>
              Enter
            </kbd>
          ) : null}
        </Button>
      ) : null}
      {canYield ? (
        <DropdownMenu>
          <DropdownMenuTrigger
            data-slot="more-passing"
            {...anchor('pass.more')}
            aria-label="More ways to pass"
            disabled={!free}
            render={<Button variant="outline" size="icon" className="size-11" />}
          >
            <EllipsisVertical aria-hidden="true" />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            {!endTurnInDock ? (
              <DropdownMenuItem
                data-yield="EndOfTurn"
                {...END_TURN}
                aria-description={YIELD_WORDS.EndOfTurn.description}
                onClick={() => yieldTo('EndOfTurn')}
              >
                {YIELD_WORDS.EndOfTurn.action}
              </DropdownMenuItem>
            ) : null}
            {stacked && !resolveAllInDock ? (
              <DropdownMenuItem
                data-yield="Resolve"
                aria-description={YIELD_WORDS.Resolve.description}
                onClick={() => yieldTo('Resolve')}
              >
                {YIELD_WORDS.Resolve.action}
              </DropdownMenuItem>
            ) : null}
            {topAbility ? (
              <DropdownMenuItem
                data-yield="Always"
                aria-description={ALWAYS_YIELD_DESCRIPTION}
                onClick={() => always(topAbility.id)}
              >
                Always yield to {topAbility.name}
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
      {resolveAllInDock ? (
        <Button
          data-slot="resolve-all"
          data-yield="Resolve"
          variant="outline"
          className={cn('h-11', padding)}
          disabled={!free}
          aria-description={YIELD_WORDS.Resolve.description}
          onClick={() => yieldTo('Resolve')}
        >
          {YIELD_WORDS.Resolve.action}
        </Button>
      ) : null}
      {/* Last, so the primary button is the one in the corner of the dock and the last stop of its row. */}
      {label === null ? null : (
        <Button
          data-slot="primary-action"
          {...anchor('pass.primary')}
          data-label={label}
          data-held={held || undefined}
          aria-disabled={held || undefined}
          className={cn(
            'action-glow h-11 min-w-0 font-semibold max-sm:flex-1',
            padding,
            held && 'opacity-50 saturate-50',
          )}
          disabled={!free}
          title={held ? 'Not in this step of the lesson' : yieldKey ? 'Space' : 'Space or Enter'}
          aria-keyshortcuts={yieldKey ? 'Space' : 'Space Enter'}
          aria-description={primaryDescription(situation)}
          onClick={pass}
        >
          <span className="truncate">{label}</span>
          {full ? (
            <kbd aria-hidden="true" data-slot="key-cap" className={`${KEY_CAP} border-primary-foreground/40`}>
              {yieldKey ? 'Space' : 'Space · Enter'}
            </kbd>
          ) : null}
        </Button>
      )}
    </InControlsDock>
  )
}
