import { useLayoutEffect, useMemo, useRef, useState } from 'react'
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
import { cardsTotalWords, fitsCardsTotal, overTotal } from '../cardsTotal'
import { askerWords } from '../choiceText'
import { uniqueNames } from '../combatText'
import { awayOf, inLibrary } from '../lookText'
import { markCards, useCardMarks, useCardNotes } from '../marks'
import type { CardNotes } from '../marks'
import { clashWarning, TARGETED_BADGE } from '../costTarget'
import type { ClashWarning } from '../costTarget'
import { CostTargetAnswers, CostTargetText } from '../CostTargetWarning'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { UndoButton } from '../UndoButton'
import { useObjects } from '../useCombatNames'
import { QuickSelect } from '../QuickSelect'
import { useAllies } from '../useAllies'
import { QUICK_ROW_REM } from '../cardPrompts'
import { quickButtons } from '../quickSelect'
import { usePlay } from '@/features/game/gameContext'
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
 * out of with Undo or Escape, which pays nothing. With room for several the quick selection buttons (`QuickSelect`) add
 * every card, the viewer's own or their teammates', or take all back.
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
  // that card is put back; with a total, a card that would take the chosen ones over it is closed.
  const cardsTotal = options.total
  const sameName = useMemo(() => {
    const shut = overTotal(from, cardsTotal, chosen)
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
  }, [options.one_per_name, options.groups, chosen, from, byId, cardsTotal])
  const [warning, setWarning] = useState<ClashWarning | null>(null)
  const press = (id: ObjectId): void => {
    if (!fromSet.has(id) || sameName.has(id)) return
    setWarning(null)
    const next = pressedCard(latest.current, id, hi)
    if (next === latest.current) return
    latest.current = next
    selection.set(next)
  }

  // A whole quick choice keeps to the total, and to one card per name or group.
  const fitsQuick = (next: readonly ObjectId[]): boolean => {
    if (!fitsCardsTotal(from, cardsTotal, next)) return false
    const groups = options.groups ?? []
    const keyOf = (id: ObjectId): string | number | undefined =>
      groups.length > 0
        ? groups[from.indexOf(id)]
        : options.one_per_name === true
          ? byId.get(id)?.name
          : undefined
    const keys = next.map(keyOf).filter((key) => key !== undefined)
    return new Set(keys).size === keys.length
  }
  const me = usePlay((state) => state.seat)
  const allies = useAllies()
  const quick = quickButtons<ObjectId>({
    legal: from,
    chosen,
    max: hi,
    same: (a, b) => a === b,
    seatOf: (id) => byId.get(id)?.controller ?? null,
    me,
    allies,
    breaks: (next) => (fitsQuick(next) ? undefined : 'Together they break the rule of this choice'),
  })
  const setQuick = (next: readonly ObjectId[]): void => {
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

  const legal = countFits(chosen.length, bounds) && fitsCardsTotal(from, cardsTotal, chosen)
  // A pick that takes a target of the same spell or ability is warned of once, never refused (`costTarget.ts`).
  const targeted = options.targeted ?? NONE
  const badges = useMemo(() => new Map(targeted.map((id) => [id, TARGETED_BADGE] as const)), [targeted])
  const notes = useMemo<CardNotes>(
    () => new Map(targeted.map((id) => [id, { label: TARGETED_BADGE, words: TARGETED_BADGE }] as const)),
    [targeted],
  )
  useCardNotes(notes)
  const confirm = (): void => {
    if (!legal) return
    const clash = clashWarning({
      chosen,
      clashing: targeted,
      source: source?.name,
      objects: (id) => byId.get(id),
      all: targeted.every((id) => chosen.includes(id)),
    })
    if (clash === null) answer({ ChooseCards: [...chosen] })
    else setWarning(clash)
  }
  const asker = askerWords(source?.name)
  const words = options.text ?? ''
  const text =
    warning !== null ? (
      <CostTargetText warning={warning} />
    ) : chosen.length === 0 ? (
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
  const totalWords = cardsTotalWords(from, cardsTotal, chosen)
  const progress =
    (total > 0 && hi > 0 ? ` · ${chosen.length} chosen` : '') + (totalWords === '' ? '' : ` · ${totalWords}`)

  return (
    <PromptLayout
      text={text}
      // The cards are in the centre overlay where there is no column: a sheet over the hand keeps out of the way.
      folded
      choices={
        <>
          <CardChoices
            quick={<QuickSelect buttons={quick} onSet={setQuick} />}
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
            rest={quick.length > 0 ? QUICK_ROW_REM : 0}
            badges={badges}
          />
        </>
      }
    >
      {warning !== null ? (
        <CostTargetAnswers
          anyway="Pay anyway"
          onAnyway={() => answer({ ChooseCards: [...chosen] })}
          onBack={() => setWarning(null)}
        />
      ) : (
        <>
          <Button className="h-9 px-4" disabled={!legal} onClick={confirm}>
            Confirm
          </Button>
          {options.can_cancel === true ? (
            <UndoButton zone={source?.zone} className="h-9 px-4" onUndo={() => void answer('Undo')} />
          ) : null}
        </>
      )}
      <p
        data-slot="cards-count"
        aria-hidden="true"
        className="text-xs font-medium text-muted-foreground tabular-nums"
      >
        {cardsRule(bounds, total)}
        {progress}
      </p>
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
