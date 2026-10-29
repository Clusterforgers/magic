import { useMemo, useRef, useState } from 'react'
import { Check } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { OptionalCost } from '@/protocol/generated'
import { costPips } from '../../cardModel'
import { SIZE } from '../../cardSize'
import { ManaPips } from '../../ManaPips'
import { askerWords, numberKey } from '../choiceText'
import {
  costKeysWords,
  costsAnswer,
  costsSentence,
  extraCost,
  extraCostWords,
  labelPieces,
  refusal,
} from '../costsText'
import { KeyHint } from '../KeyHint'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus, usePromptKeys } from '../usePromptKeys'

/** A cost as the card prints it, its mana symbols drawn as symbols; a screen reader gets the plain label from the button. */
function CostLabel({ label }: { label: string }) {
  return (
    <span aria-hidden="true" className="inline-flex flex-wrap items-center gap-x-1">
      {labelPieces(label).map((piece, at) =>
        'symbol' in piece ? (
          <ManaPips
            key={at}
            pips={costPips([piece.symbol])}
            sizeClass={SIZE.compact.pip}
            label={piece.inner}
          />
        ) : (
          <span key={at}>{piece.text}</span>
        ),
      )}
    </span>
  )
}

/**
 * ChooseCosts: the optional costs of a spell or ability (kicker, buyback, a gift) the player can pay for, each a
 * toggle that shows the cost as the card prints it and a check when it is taken. The line under them adds up what the
 * choice costs extra, and Confirm costs sends the chosen set, none included. The engine accepts one gift: a second is
 * refused with the reason in words until the first is taken back. The number keys toggle the first ten. A cast can be
 * taken back with Cancel. Focus starts on Confirm, so a press made without looking pays nothing extra, the answer the
 * response clock gives where the cast cannot be taken back.
 */
export const ChooseCostsPrompt: PromptComponent<'ChooseCosts'> = ({ options }) => {
  const { answer } = usePrompt()
  const { optional } = options
  const [source] = useObjects(useMemo(() => [options.source], [options.source]))
  // The indices of the chosen costs in the order they were chosen.
  const [chosen, setChosen] = useState<readonly number[]>([])
  const [problem, setProblem] = useState<string | undefined>(undefined)
  const confirm = useRef<HTMLButtonElement>(null)
  useDefaultFocus(confirm)

  const press = (cost: OptionalCost): void => {
    const why = refusal(cost, chosen, optional)
    setProblem(why)
    if (why !== undefined) return
    setChosen((current) =>
      current.includes(cost.index)
        ? current.filter((index) => index !== cost.index)
        : [...current, cost.index],
    )
  }
  usePromptKeys(
    Object.fromEntries(
      optional.slice(0, 10).flatMap((cost, place) => {
        const key = numberKey(place)
        return key === undefined ? [] : [[key, () => press(cost)] as const]
      }),
    ),
  )

  const extra = extraCost(optional.filter((cost) => chosen.includes(cost.index)).map((cost) => cost.label))
  const none = extra.mana.length === 0 && extra.others.length === 0
  const cancelWords = source?.zone === 'Battlefield' ? 'Cancel the ability' : 'Cancel the spell'

  return (
    <PromptLayout
      text={
        <>
          {costsSentence(askerWords(source?.name), chosen.length)}
          <span className="sr-only"> {costKeysWords(optional.length)}</span>
        </>
      }
    >
      <Button
        ref={confirm}
        className="h-10 px-4"
        data-slot="costs-confirm"
        onClick={() => answer({ ChooseCosts: { Costs: costsAnswer(chosen) } })}
      >
        Confirm costs
      </Button>
      {options.can_cancel === true ? (
        <Button variant="outline" className="h-10 px-4" onClick={() => answer({ ChooseCosts: 'Cancel' })}>
          {cancelWords}
        </Button>
      ) : null}
      <p data-slot="costs-total" className="flex items-center gap-1.5 text-sm">
        {none ? (
          'No extra cost'
        ) : (
          <>
            Extra cost
            {extra.mana.length === 0 ? null : (
              <ManaPips pips={costPips(extra.mana)} sizeClass={SIZE.compact.pip} label="Extra mana" />
            )}
            {extra.others.length === 0 ? null : <span>{extra.others.join(', ')}</span>}
            <span className="sr-only">: {extraCostWords(extra)}</span>
          </>
        )}
      </p>
      <div
        role="group"
        aria-label="Optional costs"
        data-slot="cost-list"
        className="flex max-h-56 basis-full flex-col gap-1 overflow-y-auto p-0.5"
      >
        {optional.map((cost, place) => {
          const key = numberKey(place)
          const picked = chosen.includes(cost.index)
          const refused = refusal(cost, chosen, optional) !== undefined
          return (
            <Button
              key={cost.index}
              variant={picked ? 'default' : 'outline'}
              className="h-auto min-h-9 justify-start whitespace-normal px-3 py-0.5 text-left text-sm leading-snug aria-disabled:opacity-50"
              aria-label={cost.label}
              aria-keyshortcuts={key}
              aria-pressed={picked}
              aria-disabled={refused || undefined}
              data-cost={cost.index}
              onClick={() => press(cost)}
            >
              {key === undefined ? null : <KeyHint>{key}</KeyHint>}
              <span className="ml-2 min-w-0">
                <CostLabel label={cost.label} />
              </span>
              {picked ? <Check aria-hidden="true" className="ml-auto size-4 shrink-0" /> : null}
            </Button>
          )
        })}
      </div>
      {problem === undefined ? null : (
        <p role="status" data-slot="costs-problem" className="basis-full text-sm text-muted-foreground">
          {problem}
        </p>
      )}
    </PromptLayout>
  )
}
