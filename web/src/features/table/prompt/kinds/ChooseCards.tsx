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
import { awayOf, inLibrary } from '../lookText'
import { markCards, useCardMarks } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { UndoButton } from '../UndoButton'
import { useObjects } from '../useCombatNames'
import { useDefaultFocus } from '../usePromptKeys'
import { ScryCards } from './ScryCards'

const NONE: readonly ObjectId[] = []

/**
 * The pick of cards from a list (a discard, a sacrifice, a search). The cards on offer are drawn from the
 * views, in a grid that scrolls inside itself, and are chosen by a click, Enter or Space, or the number keys; the
 * cards of the table that are on offer are marked the way targets are. With room for one a press on another card
 * changes the choice, with room for several a full choice closes the rest, and Confirm waits until the number is
 * legal. A long list and a search of a library get a search box. The choice goes in the order it was made. The
 * server's answer at the deadline is the first `min` cards of the list, so focus starts on the first card, or on the
 * search box of a search of a library. Cards picked for the cost of a spell or ability being announced can be backed
 * out of with Undo or Escape, which pays nothing.
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

  const searching = from.length > 0 && from.every((id) => inLibrary(byId.get(id)?.zone))
  const fromSet = useMemo(() => new Set(from), [from])
  const chosen = useMemo(() => [...selection.ids].filter((id) => fromSet.has(id)), [selection.ids, fromSet])
  // What is chosen including presses the next render has not shown yet, so two quick presses cannot pass the most.
  const latest = useRef<readonly ObjectId[]>(chosen)
  useLayoutEffect(() => {
    latest.current = chosen
  })
  // "With different names" or "with different powers": a card of a name or group a chosen card has is closed until
  // that card is put back.
  const sameName = useMemo(() => {
    const shut = new Set<ObjectId>()
    // "With total mana value 6 or less": a card that would take the chosen ones over the total is closed.
    const budget = options.budget
    if (budget !== undefined) {
      const weightOf = (id: ObjectId): number => budget.weights[from.indexOf(id)] ?? 0
      const left = budget.most - chosen.reduce((sum, id) => sum + weightOf(id), 0)
      for (const id of from) {
        if (!chosen.includes(id) && weightOf(id) > left) shut.add(id)
      }
      return shut
    }
    const groups = options.groups ?? []
    if (groups.length > 0) {
      const groupOf = (id: ObjectId): number | undefined => groups[from.indexOf(id)]
      const taken = new Set(chosen.map(groupOf).filter((group) => group !== undefined))
      for (const id of from) {
        const group = groupOf(id)
        if (!chosen.includes(id) && group !== undefined && taken.has(group)) shut.add(id)
      }
      return shut
    }
    if (options.one_per_name !== true) return shut
    const taken = new Set(chosen.map((id) => byId.get(id)?.name).filter((name) => name !== undefined))
    for (const id of from) {
      const name = byId.get(id)?.name
      if (!chosen.includes(id) && name !== undefined && taken.has(name)) shut.add(id)
    }
    return shut
  }, [options.one_per_name, options.groups, options.budget, chosen, from, byId])
  const press = (id: ObjectId): void => {
    if (!fromSet.has(id) || sameName.has(id)) return
    const next = pressedCard(latest.current, id, hi)
    if (next === latest.current) return
    latest.current = next
    selection.set(next)
  }

  const closed = hi > 1 && chosen.length >= hi
  const open = useMemo(
    () => (hi === 0 || closed ? NONE : from.filter((id) => !sameName.has(id))),
    [hi, closed, from, sameName],
  )
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
  const budget = options.budget
  const weight =
    budget === undefined ? 0 : chosen.reduce((sum, id) => sum + (budget.weights[from.indexOf(id)] ?? 0), 0)
  const progress =
    (total > 0 && hi > 0 ? ` · ${chosen.length} chosen` : '') +
    (budget === undefined ? '' : ` · total ${weight} of ${budget.most}`)

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
      {options.can_cancel === true ? (
        <UndoButton zone={source?.zone} className="h-9 px-4" onUndo={() => void answer('Undo')} />
      ) : null}
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
        shut={sameName}
        labels={labels}
        objects={byId}
        onPress={press}
        label="Cards to choose from"
        keys
        focusFirst={first}
        search={searching}
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
