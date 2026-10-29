import { useMemo, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectView } from '@/protocol/generated'
import { commanderDestination, returnDetail, returnNoLabel, returnQuestion } from '../../commander'
import { playerOf } from '../../selectors'
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
 * as running out of time. The question reads aloud with the keys; the table's cards are left as they are. The
 * question about one of the asker's own commanders is the return choice (`CommanderReturn`).
 */
export const YesNoPrompt: PromptComponent<'YesNo'> = ({ options }) => {
  const { decision } = usePrompt()
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const commander = usePlay(
    (state) =>
      playerOf(state, decision.player)?.commanders?.some((entry) => entry.object === options.source) === true,
  )
  return commander && source !== undefined ? (
    <CommanderReturn source={source} text={options.text ?? ''} />
  ) : (
    <PlainYesNo name={source?.name} text={options.text ?? ''} />
  )
}

function PlainYesNo({ name, text }: { name: string | undefined; text: string }) {
  const { answer } = usePrompt()
  const no = useRef<HTMLButtonElement>(null)
  useDefaultFocus(no)
  const say = (yes: boolean) => () => void answer({ YesNo: yes })
  usePromptKeys({ y: say(true), n: say(false) })

  const sentence = yesNoSentence(askerWords(name), text)
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

/**
 * The return choice of a commander that would go to a graveyard, exile, a hand or a library: "Return Atraxa to the
 * command zone?", with the place a no leaves it in. Returning is what a player almost always wants, so it is the
 * primary button and has the focus; Y and N answer as for any question.
 */
function CommanderReturn({ source, text }: { source: ObjectView; text: string }) {
  const { answer } = usePrompt()
  const yes = useRef<HTMLButtonElement>(null)
  useDefaultFocus(yes)
  const say = (choice: boolean) => () => void answer({ YesNo: choice })
  usePromptKeys({ y: say(true), n: say(false) })
  const destination = commanderDestination(text, source.zone)
  const noLabel = returnNoLabel(destination)
  return (
    <PromptLayout
      text={
        <span data-slot="commander-return">
          {returnQuestion(source.name)}{' '}
          <span className="text-muted-foreground">{returnDetail(destination)}</span>
          <span className="sr-only"> {YES_NO_KEYS}</span>
        </span>
      }
    >
      <Button
        ref={yes}
        className="h-10 min-w-24 px-4"
        aria-label="Return to the command zone"
        aria-keyshortcuts="Y"
        onClick={say(true)}
      >
        Return
        <KeyHint>Y</KeyHint>
      </Button>
      <Button
        variant="outline"
        className="h-10 min-w-24 px-4"
        aria-label={noLabel}
        aria-keyshortcuts="N"
        onClick={say(false)}
      >
        {noLabel}
        <KeyHint>N</KeyHint>
      </Button>
    </PromptLayout>
  )
}
