import { useDeferredValue } from 'react'
import type { ReactNode } from 'react'
import { DECISION_LABEL } from '../turnText'
import type { DecisionName } from '../turnText'
import { GenericAnswers } from './GenericAnswers'
import { usePrompt } from './PromptContext'
import { useGenericOffer } from './useGenericOffer'

/** A polite live region whose words arrive after it is in the page, which is when screen readers announce them. */
function LiveText({ children }: { children: ReactNode }) {
  // The first render has no words; the deferred one that follows has them.
  const words = useDeferredValue(children, null)
  return (
    <p data-slot="prompt-text" aria-live="polite" aria-atomic="true" className="text-sm font-medium">
      {words}
    </p>
  )
}

/** Props of `PromptLayout`. */
export interface PromptLayoutProps {
  /** What the player may do now, in plain words; announced when it changes. */
  text: ReactNode
  /** The controls (buttons, choices); left out when there are none. */
  children?: ReactNode
}

/** The shape every prompt shares: the decision's name, one live sentence, then a row of controls. */
export function PromptLayout({ text, children }: PromptLayoutProps) {
  const { name } = usePrompt()
  return (
    <>
      <p data-slot="prompt-title" className="text-xs font-medium text-muted-foreground">
        Your decision: {Object.hasOwn(DECISION_LABEL, name) ? DECISION_LABEL[name as DecisionName] : name}
      </p>
      <LiveText>{text}</LiveText>
      {children === undefined ? null : (
        <div data-slot="prompt-actions" className="flex flex-wrap items-center gap-2 empty:hidden">
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
