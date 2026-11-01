import { useLayoutEffect, useMemo, useRef } from 'react'
import { Button } from '@/components/ui/button'
import type { ObjectId } from '@/protocol/generated'
import {
  boundsOf,
  cardKeysWords,
  cardsAsk,
  cardsProgress,
  cardsRule,
  cardsSpoken,
  cardsTitle,
  countFits,
  distinctCards,
  pressedCard,
} from '../cardsText'
import { CardChoices } from '../CardChoices'
import { askerWords } from '../choiceText'
import { uniqueNames } from '../combatText'
import { awayOf } from '../lookText'
import { markCards, useCardMarks } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus } from '../usePromptKeys'
import { ScryCards } from './ScryCards'

const NONE: readonly ObjectId[] = []

/**
 * The pick of cards from a list (a discard, a sacrifice, a search). The cards on offer are drawn from the
 * views, in a grid that scrolls inside itself, and are chosen by a click, Enter or Space, or the number keys; the
 * cards of the table that are on offer are marked the way targets are. With room for one a press on another card
 * changes the choice, with room for several a full choice closes the rest, and Confirm waits until the number is
 * legal. A long list (a search) gets a filter box. The choice goes in the order it was made. The server's answer at
 * the deadline is the first `min` cards of the list, so focus starts on the first card.
 */
const PickCards: PromptComponent<'ChooseCards'> = ({ options }) => {
  const { answer, selection } = usePrompt()
  const own = useOwnCardIds()
  const from = useMemo(() => distinctCards(options.from), [options.from])
  const total = from.length
  const { min, max } = options
  const bounds = useMemo(() => boundsOf(min, max, total), [min, max, total])
  const { hi } = bounds

  const known = useObjects(useMemo(() => [options.source, ...from], [options.source, from]))
  const source = known[0]
  const byId = useMemo(() => new Map(from.map((id, place) => [id, known[place + 1]] as const)), [from, known])
  const labels = useMemo(
    () => uniqueNames(from.map((id) => ({ id, name: byId.get(id)?.name ?? 'A card' }))),
    [from, byId],
  )

  const fromSet = useMemo(() => new Set(from), [from])
  const chosen = useMemo(() => [...selection.ids].filter((id) => fromSet.has(id)), [selection.ids, fromSet])
  // What is chosen including presses the next render has not shown yet, so two quick presses cannot pass the most.
  const latest = useRef<readonly ObjectId[]>(chosen)
  useLayoutEffect(() => {
    latest.current = chosen
  })
  const press = (id: ObjectId): void => {
    if (!fromSet.has(id)) return
    const next = pressedCard(latest.current, id, hi)
    if (next === latest.current) return
    latest.current = next
    selection.set(next)
  }

  const closed = hi > 1 && chosen.length >= hi
  const open = hi === 0 || closed ? NONE : from
  const marks = useMemo(() => markCards(own, open, chosen), [own, open, chosen])
  useCardMarks(marks, press, { toggles: true })

  const first = useRef<HTMLButtonElement>(null)
  useDefaultFocus(first)

  const legal = countFits(chosen.length, bounds)
  const asker = askerWords(source?.name)
  const words = options.text ?? ''
  const text =
    chosen.length === 0 ? (
      <>
        {cardsAsk(asker, words === '' ? cardsTitle(bounds, total) : words)}
        <span className="sr-only">
          {' '}
          {cardsSpoken(words === '' ? null : cardsRule(bounds, total), cardKeysWords(total))}
        </span>
      </>
    ) : (
      cardsProgress(asker, chosen.length, bounds)
    )
  const progress = total > 0 && hi > 0 ? ` · ${chosen.length} chosen` : ''

  return (
    <PromptLayout text={text}>
      <Button
        className="h-9 px-4"
        disabled={!legal}
        onClick={() => {
          if (legal) answer({ ChooseCards: [...chosen] })
        }}
      >
        Confirm
      </Button>
      <p
        data-slot="cards-count"
        aria-hidden="true"
        className="text-xs font-medium text-muted-foreground tabular-nums"
      >
        {cardsRule(bounds, total)}
        {progress}
      </p>
      <CardChoices
        cards={from}
        chosen={chosen}
        closed={closed}
        labels={labels}
        objects={byId}
        onPress={press}
        label="Cards to choose from"
        keys
        focusFirst={first}
      />
    </PromptLayout>
  )
}

/**
 * ChooseCards: a pick from a list, or, when the cards are the top of a library and the question is the engine's scry
 * or surveil ("Put any number of the cards on the bottom of your library."), the split of those cards into the pile
 * that stays on top and the pile that goes away (`ScryCards`). Both answer with the cards chosen.
 */
export const ChooseCardsPrompt: PromptComponent<'ChooseCards'> = ({ options }) => {
  const from = useMemo(() => distinctCards(options.from), [options.from])
  const zones = useObjects(from).map((object) => object?.zone)
  const kind = awayOf(options, from.length, zones)
  return kind === null ? <PickCards options={options} /> : <ScryCards options={options} kind={kind} />
}
