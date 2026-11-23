import { TriangleAlert } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Button } from '@/components/ui/button'
import type { ClashWarning } from './costTarget'
import { KeyHint } from './KeyHint'
import { useDefaultFocus } from './usePromptKeys'

/** The warning in the prompt's sentence line, in place of the question while it stands. */
export function CostTargetText({ warning }: { warning: ClashWarning }) {
  return (
    <span role="alert" data-slot="cost-target-warning">
      {/* Inline, so the sentence keeps its one-line clamp and its button that opens it whole. */}
      <TriangleAlert aria-hidden="true" className="me-1 inline size-4 align-[-3px] text-warning" />
      {warning.text}
    </span>
  )
}

/** Props of `CostTargetAnswers`. */
export interface CostTargetAnswersProps {
  /** The words of the button that sends the answer as it is ("Pay anyway", "Target anyway"). */
  anyway: string
  onAnyway(): void
  /** Back to the choice, with nothing sent. */
  onBack(): void
}

/**
 * The two answers of the warning: send the answer anyway, or choose another. The warning never blocks: either is one
 * press. Focus starts on "Choose another", at the right end where the safe answer stands, and Escape presses it
 * before the prompt's Cancel would.
 */
export function CostTargetAnswers({ anyway, onAnyway, onBack }: CostTargetAnswersProps) {
  const back = useRef<HTMLButtonElement>(null)
  useDefaultFocus(back)
  const latest = useRef(onBack)
  useEffect(() => {
    latest.current = onBack
  })
  useEffect(() => {
    // Capture on the document runs before the window overlay's and the Cancel's Escape.
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape' || event.repeat || event.defaultPrevented) return
      event.preventDefault()
      event.stopPropagation()
      latest.current()
    }
    document.addEventListener('keydown', onKeyDown, true)
    return () => document.removeEventListener('keydown', onKeyDown, true)
  }, [])
  return (
    <>
      <Button className="h-10 px-4" variant="outline" data-slot="cost-target-anyway" onClick={onAnyway}>
        {anyway}
      </Button>
      <Button
        ref={back}
        className="h-10 px-4"
        data-slot="cost-target-back"
        aria-keyshortcuts="Escape"
        onClick={onBack}
      >
        Choose another
        <KeyHint>Esc</KeyHint>
      </Button>
    </>
  )
}
