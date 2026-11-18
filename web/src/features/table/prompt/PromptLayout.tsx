import {
  Children,
  Fragment,
  isValidElement,
  useContext,
  useDeferredValue,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from 'react'
import { createPortal } from 'react-dom'
import type { ReactNode, RefObject } from 'react'
import { ChevronDown, ChevronUp } from 'lucide-react'
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

/** A polite live region whose words arrive after it is in the page, which is when screen readers announce them. */
function LiveText({
  children,
  className,
  id,
  textRef,
}: {
  children: ReactNode
  className?: string
  id?: string
  textRef?: RefObject<HTMLParagraphElement | null>
}) {
  // The first render has no words; the deferred one that follows has them.
  const words = useDeferredValue(children, null)
  return (
    <p
      ref={textRef}
      id={id}
      data-slot="prompt-text"
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
function Sentence({ text, quiet = false }: { text: ReactNode; quiet?: boolean }) {
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
        id={id}
        textRef={words}
        className={cn('min-w-0 flex-1', open ? null : 'prompt-sentence-clamp')}
      >
        {text}
      </LiveText>
      {clamped || open ? (
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
 * name stays for a screen reader and the question starts with a capital.
 */
function withoutAsker(text: ReactNode, name: string): ReactNode {
  if (name === '') return text
  const lead = `${name}: `
  if (typeof text === 'string') {
    if (!text.startsWith(lead)) return text
    const rest = text.slice(lead.length)
    return (
      <>
        <span className="sr-only">{lead}</span>
        {rest.charAt(0).toUpperCase() + rest.slice(1)}
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
  /** The answers (buttons and their notes), the primary one first: the band draws them from its right end leftward. */
  children?: ReactNode
  /**
   * What the player chooses among (cards, modes, a list): drawn in the Stack column, or in the sheet over the hand
   * where there is no column (`detail/DecisionDetail`); in place where neither exists (a prompt outside a table).
   */
  choices?: ReactNode
  /** The choices are short (a number, a few options): they stay in the band beside the sentence. */
  inline?: boolean
  /** A short reading in the band between the sentence and the answers: a run's place, what a payment still owes. */
  meter?: ReactNode
  /** The meter says what the sentence says (a payment's symbols): the sentence is read aloud and not drawn. */
  quiet?: boolean
  /** The chip at the band's left for a decision no one card asks (the triggers to order): in place of the asker's. */
  lead?: ReactNode
  /** The answer is a press on the table or in the centre overlay: the decision sheet over the hand starts folded. */
  folded?: boolean
  /** One row: the sentence and the controls side by side, so the sheet stays one line high (the combat prompts). */
  compact?: boolean
}

/**
 * The shape every prompt shares: the decision's name for a screen reader, the live sentence, the choices and the
 * answers. In the strip they are the cells of the prompt band (`prompt-band`, `promptBand.css`); elsewhere (the opening
 * hand's overlay) they stack.
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
  folded = false,
}: PromptLayoutProps) {
  const { name } = usePrompt()
  const side = useContext(SideContext)
  // At a table the detail of a decision is the column's or the sheet's; its element arrives a frame after the prompt.
  // Short choices stay in the band, except in a narrow window, whose band has two short lines: they go to the sheet too.
  const short = inline && side?.kind !== 'sheet'
  const aside = side !== null && !short && detailKind(name)
  const hint = side?.setSheetStart
  useLayoutEffect(() => {
    hint?.(folded)
  }, [hint, folded])
  const asker = useAsker()
  const chip = detailKind(name) && asker !== null && asker.shown !== ''
  const title = (
    <p data-slot="prompt-title" className="text-xs font-medium text-muted-foreground">
      Your decision: {Object.hasOwn(DECISION_LABEL, name) ? DECISION_LABEL[name as DecisionName] : name}
    </p>
  )
  if (compact) {
    return (
      <>
        {title}
        <div data-slot="combat-bar" className="flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-1">
          <LiveText className="min-w-0 flex-1 basis-44">{text}</LiveText>
          {children === undefined ? null : (
            <div
              data-slot="prompt-actions"
              data-keep-clear="answers"
              className="flex flex-wrap items-center gap-1.5 empty:hidden"
            >
              {children}
            </div>
          )}
        </div>
      </>
    )
  }
  return (
    <>
      {title}
      {chip ? <AskerChip /> : (lead ?? null)}
      <Sentence text={chip ? withoutAsker(text, asker.shown) : text} quiet={quiet && side !== null} />
      {meter === undefined || meter === null || meter === false ? null : (
        <div data-slot="prompt-meter" className="flex items-center gap-2 empty:hidden">
          {meter}
        </div>
      )}
      {choices === undefined || choices === null || choices === false ? null : aside ? (
        side.detail === null ? null : (
          createPortal(
            <div data-slot="prompt-choices" data-detail="" className="grid min-w-0 content-start gap-2 empty:hidden">
              {choices}
            </div>,
            side.detail,
          )
        )
      ) : (
        <div
          data-slot="prompt-choices"
          data-inline={short || undefined}
          className="flex min-w-0 flex-wrap items-center gap-2 empty:hidden"
        >
          {choices}
        </div>
      )}
      {children === undefined ? null : (
        <div
          data-slot="prompt-actions"
          data-keep-clear="answers"
          className="prompt-answers empty:hidden"
        >
          {children}
        </div>
      )}
    </>
  )
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
