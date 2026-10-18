import { useDeferredValue } from 'react'
import type { ReactNode } from 'react'
import { DECISION_LABEL } from '../turnText'
import { usePrompt } from './PromptContext'

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
        Your decision: {DECISION_LABEL[name]}
      </p>
      <LiveText>{text}</LiveText>
      {children === undefined ? null : (
        <div data-slot="prompt-actions" className="flex flex-wrap items-center gap-2">
          {children}
        </div>
      )}
    </>
  )
}

/** The prompt of a decision kind the interface cannot answer yet: it says so and offers no controls. */
export function UnsupportedPrompt() {
  return <PromptLayout text="This decision is not supported in the UI yet" />
}
