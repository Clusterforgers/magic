import { useMemo, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { askerWords, YES_NO_KEYS, yesNoSentence } from '../choiceText'
import { KeyHint } from '../KeyHint'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/**
 * YesNo: a "may" or a "pay or not", in the words of the card. Yes and No are big buttons, Y and N answer from the
 * keyboard, and focus starts on No, the answer the response clock gives, so a press made without looking is the same
 * as running out of time. The question reads aloud with the keys; the table's cards are left as they are.
 */
export const YesNoPrompt: PromptComponent<'YesNo'> = ({ options }) => {
  const { answer } = usePrompt()
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const no = useRef<HTMLButtonElement>(null)
  useDefaultFocus(no)
  const say = (yes: boolean) => () => void answer({ YesNo: yes })
  usePromptKeys({ y: say(true), n: say(false) })

  const sentence = yesNoSentence(askerWords(source?.name), options.text ?? '')
  return (
    <PromptLayout
      text={
        <>
          {sentence}
          <span className="sr-only"> {YES_NO_KEYS}</span>
        </>
      }
    >
      <Button
        variant="outline"
        className="h-10 min-w-24 px-4"
        aria-label="Yes"
        aria-keyshortcuts="Y"
        onClick={say(true)}
      >
        Yes
        <KeyHint>Y</KeyHint>
      </Button>
      <Button
        ref={no}
        variant="outline"
        className="h-10 min-w-24 px-4"
        aria-label="No"
        aria-keyshortcuts="N"
        onClick={say(false)}
      >
        No
        <KeyHint>N</KeyHint>
      </Button>
    </PromptLayout>
  )
}
