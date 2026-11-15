import { useDeferredValue, useId, useLayoutEffect, useRef, useState } from 'react'
import type { ReactNode, RefObject } from 'react'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { cn } from '@/lib/utils'
import { DECISION_LABEL } from '../turnText'
import type { DecisionName } from '../turnText'
import { GenericAnswers } from './GenericAnswers'
import { usePrompt } from './PromptContext'
import { useGenericOffer } from './useGenericOffer'
import { isClamped } from './promptBand'

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
function Sentence({ text }: { text: ReactNode }) {
  const [open, setOpen] = useState(false)
  const words = useRef<HTMLParagraphElement>(null)
  const clamped = useClamped(words, open)
  const id = useId()
  return (
    <div data-slot="prompt-sentence" data-open={open || undefined} className="flex min-w-0 items-start gap-1">
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

/** Props of `PromptLayout`. */
export interface PromptLayoutProps {
  /** What the player may do now, in plain words; announced when it changes. */
  text: ReactNode
  /** The answers (buttons and their notes), the primary one first: the band draws them from its right end leftward. */
  children?: ReactNode
  /** What the player chooses among (cards, modes, a number, a list): the band's middle, beside the sentence. */
  choices?: ReactNode
  /** The choices are short (a number, a few options): they stay beside the sentence at any band width but a phone's. */
  inline?: boolean
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
}: PromptLayoutProps) {
  const { name } = usePrompt()
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
            <div data-slot="prompt-actions" className="flex flex-wrap items-center gap-1.5 empty:hidden">
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
      <Sentence text={text} />
      {choices === undefined || choices === null || choices === false ? null : (
        <div
          data-slot="prompt-choices"
          data-inline={inline || undefined}
          className="flex min-w-0 flex-wrap items-center gap-2 empty:hidden"
        >
          {choices}
        </div>
      )}
      {children === undefined ? null : (
        <div
          data-slot="prompt-actions"
          // Sticky to the sheet's edges: the answers stay in view while a tall band scrolls inside the sheet.
          className="prompt-answers sticky top-0 bottom-0 z-10 empty:hidden"
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
