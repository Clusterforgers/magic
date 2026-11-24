import { anchor } from '@/features/learn/anchors'
import {
  Children,
  Fragment,
  isValidElement,
  useContext,
  useDeferredValue,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from 'react'
import { createPortal } from 'react-dom'
import type { ReactNode, RefObject } from 'react'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { SideContext } from '../sideContext'
import { DECISION_LABEL } from '../turnText'
import type { DecisionName } from '../turnText'
import { GenericAnswers } from './GenericAnswers'
import { usePrompt } from './PromptContext'
import { useGenericOffer } from './useGenericOffer'
import { isClamped } from './promptBand'
import { AskerChip } from './detail/AskerChip'
import { detailKind } from './detail/asking'
import { useAsker } from './detail/useAsker'
import { useWindowHost } from './choiceOverlay'
import { KeyHint } from './KeyHint'
import { useHomeKeys } from './homeKeys'
import { isTyping, usePromptKeys } from './usePromptKeys'
import { clockKind } from './clockText'
import { askedByCard } from './home'
import type { Home } from './home'
import { ResponseClock } from './ResponseClock'
import { useDockForm, useDockWide } from '../dockLayout'
import { StopsDock } from '../stops/StopsDock'
import { useDecisionHome } from './useDecisionHome'
import { PickOnBoardContext, usePickOnBoardHost } from './pickOnBoard'
import { PickOnBoardButton } from './PickOnBoardButton'

/** The coach's anchors of a decision's home: its question and its primary answer. */
const QUESTION = anchor('ask.question')
const PRIMARY = anchor('ask.primary')['data-coach']

/** A polite live region whose words arrive after it is in the page, which is when screen readers announce them. */
function LiveText({
  children,
  className,
  id,
  textRef,
  coach = false,
}: {
  children: ReactNode
  className?: string
  id?: string
  textRef?: RefObject<HTMLParagraphElement | null>
  /** The question of a decision in its home: the coach's `ask.question` anchor. */
  coach?: boolean
}) {
  // The first render has no words; the deferred one that follows has them.
  const words = useDeferredValue(children, null)
  return (
    <p
      ref={textRef}
      id={id}
      data-slot="prompt-text"
      {...(coach ? QUESTION : {})}
      // The preview covers the sentence only where no place at full size leaves it clear (`fitClear`).
      data-keep-clear="text"
      aria-live="polite"
      aria-atomic="true"
      className={cn('text-sm font-medium', className)}
    >
      {words}
    </p>
  )
}
/** Follows whether `ref`'s text is cut by its clamp, as the band and the words change. */
function useClamped(ref: RefObject<HTMLElement | null>, open: boolean): boolean {
  const [clamped, setClamped] = useState(false)
  useLayoutEffect(() => {
    const box = ref.current
    if (box === null || open) return undefined
    const check = (): void => setClamped(isClamped(box))
    check()
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(check) : null
    observer?.observe(box)
    const words = typeof MutationObserver === 'function' ? new MutationObserver(check) : null
    words?.observe(box, { childList: true, characterData: true, subtree: true })
    return () => {
      observer?.disconnect()
      words?.disconnect()
    }
  }, [ref, open])
  return clamped
}

/**
 * The sentence of the band: one line (three beside choices, `promptBand.css`), cut with an ellipsis where it does not
 * fit, and a button that opens it whole. The live region holds every word, so a screen reader hears the whole sentence.
 */
function Sentence({
  text,
  quiet = false,
  capital = false,
  coach = false,
}: {
  text: ReactNode
  quiet?: boolean
  /** The words start after a name that is not drawn: the first letter in view is drawn as a capital. */
  capital?: boolean
  /** The question of a decision in its home: the coach's `ask.question` anchor. */
  coach?: boolean
}) {
  const [open, setOpen] = useState(false)
  const words = useRef<HTMLParagraphElement>(null)
  const clamped = useClamped(words, open)
  const id = useId()
  return (
    <div
      data-slot="prompt-sentence"
      data-open={open || undefined}
      data-quiet={quiet || undefined}
      className={cn('flex min-w-0 items-start gap-1', quiet && 'sr-only')}
    >
      <LiveText
        coach={coach}
        id={id}
        textRef={words}
        className={cn(
          'min-w-0 flex-1',
          open ? null : 'prompt-sentence-clamp',
          capital && 'first-letter:uppercase',
          // Read aloud and not drawn: the words themselves take no room either.
          quiet && 'sr-only',
        )}
      >
        {text}
      </LiveText>
      {/* A sentence that is not drawn has nothing to open: a reader hears all of it. */}
      {(clamped || open) && !quiet ? (
        <button
          type="button"
          data-slot="prompt-expand"
          aria-expanded={open}
          aria-controls={id}
          aria-label={open ? 'Show less' : 'Show the whole sentence'}
          onClick={() => setOpen((was) => !was)}
          className="-my-0.5 shrink-0 rounded-sm p-0.5 text-muted-foreground hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"
        >
          {open ? (
            <ChevronUp aria-hidden="true" className="size-4" />
          ) : (
            <ChevronDown aria-hidden="true" className="size-4" />
          )}
        </button>
      ) : null}
    </div>
  )
}

/**
 * `text` without the asker's name at its start ("Grizzly Bears: Draw a card?"), which the chip beside it shows: the
 * name stays for a screen reader, and the sentence draws its first letter as a capital (`Sentence`'s `lead`).
 */
function withoutAsker(text: ReactNode, name: string): ReactNode {
  if (name === '') return text
  const lead = `${name}: `
  if (typeof text === 'string') {
    if (!text.startsWith(lead)) return text
    return (
      <>
        <span className="sr-only">{lead}</span>
        {text.slice(lead.length)}
      </>
    )
  }
  if (isValidElement<{ children?: ReactNode }>(text) && text.type === Fragment) {
    const [first, ...others] = Children.toArray(text.props.children)
    return (
      <>
        {withoutAsker(first, name)}
        {others}
      </>
    )
  }
  return text
}

/** Props of `PromptLayout`. */
export interface PromptLayoutProps {
  /** What the player may do now, in plain words; announced when it changes. */
  text: ReactNode
  /** The answers (buttons and their notes), the primary one first: the home draws them from its right end leftward. */
  children?: ReactNode
  /**
   * What the player chooses among (cards, modes, a list): under the question in the Stack panel, in a window over the
   * table where the dock or a phone's bar is the home (`home.ts`, `detailPlace`).
   */
  choices?: ReactNode
  /** The choices are short (a number, a few options): they stand beside the question in every home. */
  inline?: boolean
  /** A short reading between the question and the answers: a run's place, what a payment still owes. */
  meter?: ReactNode
  /** The meter says what the sentence says (a payment's symbols): the sentence is read aloud and not drawn. */
  quiet?: boolean
  /** What is being done with the asking card, over its name on the chip ("Casting", "Plainscycling"). */
  doing?: string | undefined
  /** The chip that leads the dock or bar for a decision no one card asks (the triggers to order): in place of the asker's. */
  lead?: ReactNode
  /** The choices are cards that open their own window where the home has no room for them (`CardChoices`). */
  folded?: boolean
  /** One row: the sentence and the controls side by side (the combat prompts). */
  compact?: boolean
  /** Drawn where it is called (the opening hand's centred panel), not in the home; the home holds its own answers. */
  here?: boolean
}

/** Where the choices of a prompt go in its home. */
type ChoicesAt = 'home' | 'window' | 'own' | 'sheet' | 'none'

/** Where `PromptLayout`'s choices stand for a decision of `name` in `home`. */
function choicesAt(name: string, home: Home | null, inline: boolean, folded: boolean): ChoicesAt {
  if (home === null || home === 'panel' || inline) return 'home'
  // Cards decide their own window (`CardChoices`); a payment's sources are pressed on the board, its list is the panel's.
  if (folded) return 'own'
  if (name === 'PayMana') return home === 'bar' ? 'sheet' : 'none'
  return 'window'
}

/**
 * The window a list stands in where its home has no room for it (the dock, a phone's bar): anchored over the dock, a
 * bottom sheet on a phone. Its foot holds the question (the answers stay in the home, one set), and View table (V) shows
 * the table through it and the window again.
 */
function DecisionWindow({
  children,
  text,
  name,
  home,
}: {
  children: ReactNode
  text: ReactNode
  name: string
  home: Home
}) {
  const { id } = usePrompt()
  const [peek, setPeek] = useState(false)
  usePromptKeys({ v: () => setPeek((was) => !was) })
  // Escape hides the window before it cancels: after a preview and a search, before the prompt's Cancel (`undo.ts`).
  useEffect(() => {
    if (peek) return undefined
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape' || event.repeat || event.defaultPrevented || isTyping(event.target)) return
      event.preventDefault()
      setPeek(true)
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [peek])
  return (
    <div
      data-slot="decision-window"
      data-decision={name}
      data-decision-id={id}
      data-home={home}
      data-peek={peek || undefined}
      className={cn(
        'plaque flex max-h-full flex-col gap-2 overflow-hidden rounded-xl border-2 border-brass p-3 shadow-e3',
        home === 'bar' ? 'w-full self-end rounded-b-none' : 'ms-auto w-[min(36rem,100%)] self-end',
      )}
    >
      <div
        data-slot="prompt-choices"
        className={cn('fade-scroll grid min-h-0 content-start gap-2 overflow-y-auto', peek && 'invisible')}
      >
        {children}
      </div>
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <p data-slot="window-question" aria-hidden="true" className="min-w-0 flex-1 text-sm font-semibold">
          {text}
        </p>
        <Button
          variant="ghost"
          size="sm"
          data-slot="view-table"
          aria-pressed={peek}
          aria-keyshortcuts="V"
          onClick={() => setPeek((was) => !was)}
        >
          {peek ? 'Show the window' : 'View table'}
          <KeyHint>V</KeyHint>
        </Button>
      </div>
    </div>
  )
}

/**
 * Puts the coach's `ask.primary` anchor on the home's primary answer (the first of its answers) and keeps it there as
 * the answers change; a ref callback, so it needs no state.
 */
function markPrimary(home: HTMLElement | null): (() => void) | undefined {
  if (home === null || typeof MutationObserver !== 'function') return undefined
  const mark = (): void => {
    const primary = home.querySelector('[data-slot="prompt-actions"] > button')
    for (const old of home.querySelectorAll(`[data-coach="${PRIMARY}"]`))
      if (old !== primary) old.removeAttribute('data-coach')
    if (primary !== null && !primary.hasAttribute('data-coach')) primary.setAttribute('data-coach', PRIMARY)
  }
  mark()
  const watch = new MutationObserver(mark)
  watch.observe(home, { childList: true, subtree: true })
  return () => watch.disconnect()
}

/** A phone's bar: the chip that opens the stack sheet, with how many items are on the stack. */
function BarStack({ count, onOpen }: { count: number; onOpen: () => void }) {
  return (
    <button
      type="button"
      data-slot="bar-stack"
      onClick={onOpen}
      className="inline-flex h-8 items-center gap-1 rounded-md border px-2 text-xs font-medium text-muted-foreground outline-primary focus-visible:outline-2 pointer-coarse:h-11"
    >
      Stack {count}
      <ChevronUp aria-hidden="true" className="size-3.5" />
    </button>
  )
}

/**
 * The shape every prompt shares: the decision's name for a screen reader, the asking chip, the live question, the
 * reading, the choices, the clock and the answers. At a table they are drawn into the decision's home (`useDecisionHome`:
 * the Stack panel under the asking card, the controls dock, a phone's bar), which keeps the prompt's state wherever it
 * stands; outside a table (a test of one prompt) they stand where they are called.
 */
export function PromptLayout({
  text,
  children,
  choices,
  inline = false,
  compact = false,
  meter,
  quiet = false,
  lead,
  doing,
  folded = false,
  here = false,
}: PromptLayoutProps) {
  const { id, name, decision } = usePrompt()
  const { home: found, element } = useDecisionHome()
  const dockForm = useDockForm()
  const dockWide = useDockWide()
  const home = here ? null : found
  const side = useContext(SideContext)
  const at = choicesAt(name, home, inline, folded)
  useHomeKeys(home !== null, askedByCard(name))
  const windowHost = useWindowHost(at === 'window' && hasContent(choices))
  // A card choice picked on the table hides its overlay; Show choices then stands with the answers (`pickOnBoard.ts`).
  const pick = usePickOnBoardHost()
  const asker = useAsker()
  const named = detailKind(name) && asker !== null && asker.shown !== ''
  // The chip leads the dock and the bar; in the panel the asking card stands above. Either way it names the asker.
  // A payment keeps its chip in the panel too: it says what is being done ("Casting", "Plainscycling").
  const chip = (home !== 'panel' || name === 'PayMana') && named
  const title = (
    <p data-slot="prompt-title" className="sr-only">
      Your decision: {Object.hasOwn(DECISION_LABEL, name) ? DECISION_LABEL[name as DecisionName] : name}
    </p>
  )
  const answers =
    children === undefined ? null : (
      <div
        data-slot="prompt-actions"
        data-keep-clear="answers"
        // Sticky to the foot of a home that scrolls (the panel), so the answers stay in view.
        className="prompt-answers sticky bottom-0 z-10 empty:hidden"
      >
        {children}
        {pick.value.hidden ? <PickOnBoardButton pick={pick.value} /> : null}
      </div>
    )
  const clock = <ResponseClock id={id} kind={clockKind(decision.kind)} compact />
  // A question other than priority takes the stops' row of the dock or bar: they stay one press away beside it.
  const stops =
    (home === 'dock' || home === 'bar') && name !== 'Priority' ? (
      <div data-slot="home-stops">
        {/* Only the widest dock has the line for the switches: elsewhere they wait in the button's popover. */}
        <StopsDock row={false} tuck={dockForm === 'full' && dockWide ? undefined : 'all'} />
      </div>
    ) : null
  const body = compact ? (
    <>
      {title}
      <div data-slot="combat-bar" className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
        <LiveText coach={home !== null} className="min-w-0 flex-1 basis-44">
          {text}
        </LiveText>
        {answers}
      </div>
      {clock}
      {stops}
    </>
  ) : (
    <>
      {title}
      {chip ? <AskerChip doing={doing} /> : home === 'panel' ? null : (lead ?? null)}
      {stops}
      {home === 'bar' && side !== null ? (
        <BarStack count={side.counts.stack} onOpen={() => side.activate('stack')} />
      ) : null}
      <Sentence
        text={named && home !== null ? withoutAsker(text, asker.shown) : text}
        quiet={quiet && side !== null}
        capital={named && home !== null}
        coach={home !== null}
      />
      {hasContent(meter) ? (
        <div data-slot="prompt-meter" className="flex min-w-0 items-center gap-2 empty:hidden">
          {meter}
        </div>
      ) : null}
      {!hasContent(choices) || at === 'none' ? null : at === 'sheet' ? (
        side === null || side.detail === null ? null : (
          createPortal(
            <div data-slot="prompt-choices" data-detail="" className="grid min-w-0 content-start gap-2">
              {choices}
            </div>,
            side.detail,
          )
        )
      ) : at === 'window' ? (
        windowHost === null ? null : (
          createPortal(
            <DecisionWindow text={text} name={name} home={home ?? 'dock'}>
              {choices}
            </DecisionWindow>,
            windowHost,
          )
        )
      ) : (
        <div
          data-slot="prompt-choices"
          data-detail={home === 'panel' && !inline ? '' : undefined}
          data-inline={inline || undefined}
          data-own={at === 'own' || undefined}
          className={cn(
            'min-w-0 empty:hidden',
            inline ? 'flex flex-wrap items-center gap-2' : 'grid content-start gap-2',
          )}
        >
          {choices}
        </div>
      )}
      {clock}
      {answers}
    </>
  )
  const provided = <PickOnBoardContext value={pick.value}>{body}</PickOnBoardContext>
  if (home === null) {
    return here ? (
      provided
    ) : (
      <div data-slot="decision-line" data-decision={name} data-decision-id={id} className="grid gap-2">
        {provided}
      </div>
    )
  }
  if (element === null) return null
  return createPortal(
    <div
      data-slot="decision-line"
      data-decision={name}
      data-decision-id={id}
      data-home={home}
      // The preview keeps off the home where it can; it covers the question and the answers last.
      data-keep-clear="soft"
      data-compact={compact || undefined}
      {...anchor('ask')}
      className="decision-home"
      ref={markPrimary}
    >
      {provided}
    </div>,
    element,
  )
}

/** Whether a prompt part draws anything. */
function hasContent(part: ReactNode): boolean {
  return part !== undefined && part !== null && part !== false
}

const UNSUPPORTED = 'This decision is not supported in the UI yet'

/**
 * The prompt of a decision kind the interface has no prompt for (and of a kind it does not know): it says so and
 * offers the answers any decision has (`genericChoices`), so a player is never left without a button.
 */
export function UnsupportedPrompt() {
  const offer = useGenericOffer()
  return (
    <PromptLayout
      text={offer.choices.length === 0 ? UNSUPPORTED : `${UNSUPPORTED}. Choose an answer to go on.`}
    >
      <GenericAnswers offer={offer} />
    </PromptLayout>
  )
}
