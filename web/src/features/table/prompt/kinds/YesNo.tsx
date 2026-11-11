import { useMemo, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { AutoYield, ObjectView } from '@/protocol/generated'
import { answerKeyOf } from '../../autoSkips'
import { commanderDestination, returnDetail, returnNoLabel, returnQuestion } from '../../commander'
import { askerWords, NO_KEY, YES_NO_KEYS, yesNoSentence } from '../choiceText'
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
 * question the engine marks as a commander's return is the return choice (`CommanderReturn`); any other question
 * about a commander (its own "you may") is a plain one. A question of the seat's own ability on top of the stack also
 * offers "Always Yes" and "Always No": the server keeps the rule for the rest of the game and answers it from then on.
 */
export const YesNoPrompt: PromptComponent<'YesNo'> = ({ options }) => {
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  const ability = useOwnAbility(options.source)
  return options.commander_return === true && source !== undefined ? (
    <CommanderReturn source={source} text={options.text ?? ''} />
  ) : (
    <PlainYesNo
      name={source?.name}
      text={options.text ?? ''}
      onlyNo={options.only_no === true}
      ability={ability}
    />
  )
}

/** With `onlyNo` (the payment cannot be made) Yes is disabled and Y does nothing, so no refused answer is sent. */
function PlainYesNo({
  name,
  text,
  onlyNo,
  ability,
}: {
  name: string | undefined
  text: string
  onlyNo: boolean
  ability: AutoYield | null
}) {
  const { answer } = usePrompt()
  const { session } = useGame()
  const always = (yes: boolean) => () => {
    if (ability === null) return
    session.autoSkip({ kind: 'Answer', ability, yes })
    answer({ YesNo: yes })
  }
  const no = useRef<HTMLButtonElement>(null)
  useDefaultFocus(no)
  const say = (yes: boolean) => () => void answer({ YesNo: yes })
  usePromptKeys(onlyNo ? { n: say(false) } : { y: say(true), n: say(false) })

  const sentence = yesNoSentence(askerWords(name), text)
  return (
    <PromptLayout
      text={
        <>
          {sentence}
          <span className="sr-only"> {onlyNo ? NO_KEY : YES_NO_KEYS}</span>
        </>
      }
    >
      <Button
        variant="outline"
        className="h-10 min-w-24 px-4"
        aria-label="Yes"
        aria-keyshortcuts={onlyNo ? undefined : 'Y'}
        disabled={onlyNo}
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
      {ability === null ? null : (
        <span data-slot="always-answer" className="flex gap-1">
          <Button
            size="sm"
            variant="ghost"
            aria-label={`Always answer Yes for ${ability.name}`}
            disabled={onlyNo}
            onClick={always(true)}
          >
            Always Yes
          </Button>
          <Button
            size="sm"
            variant="ghost"
            aria-label={`Always answer No for ${ability.name}`}
            onClick={always(false)}
          >
            Always No
          </Button>
        </span>
      )}
    </PromptLayout>
  )
}

/** The key of this seat's own ability on top of the stack when it is what asks about `source`, else null. */
function useOwnAbility(source: ObjectView['id']): AutoYield | null {
  const seat = usePlay((state) => state.seat)
  const top = usePlay((state) => {
    const id = state.view?.table.stack.cards?.at(-1)
    return id === undefined ? undefined : state.objectsById[id]
  })
  return useMemo(() => answerKeyOf(top, seat, source), [top, seat, source])
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
