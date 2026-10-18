import { useMemo, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { ManaOption, ManaType, ObjectId } from '@/protocol/generated'
import { ManaChoice } from '../ManaChoice'
import { markCards, useCardMarks } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { payText } from '../payText'
import { allowed, covers, fittingOptions, MANA_TYPES, owed, toPayment } from '../payment'
import type { Spend } from '../payment'
import { PaymentSummary, spendWords } from '../PaymentSummary'
import { manaByPermanent } from '../priorityText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'

const focusCard = (id: ObjectId): void =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)?.focus()

/** The payment so far, in order of choice, and the sentence about the last change to it. */
interface Chosen {
  spends: readonly Spend[]
  note: string
}
const START: Chosen = { spends: [], note: '' }

/**
 * PayMana: what the spell costs, the floating mana, and what is still owed. Untapped permanents with a mana ability
 * are legal; a click adds one (a permanent with several kinds of mana asks which), a click on a chosen one takes it
 * back. Mana that would be wasted is not offered, so a payment is complete when nothing is owed. Confirm sends the
 * chosen payment, "Pay automatically" lets the game choose, Cancel gives up on casting the spell.
 */
export const PayManaPrompt: PromptComponent<'PayMana'> = ({ options }) => {
  const { answer, selection } = usePrompt()
  const { spell, cost, pool } = options
  const own = useOwnCardIds()
  const byPermanent = useMemo(() => manaByPermanent(options.sources), [options.sources])
  const named = useMemo(() => [spell, ...byPermanent.keys()], [spell, byPermanent])
  const names = usePlay(useShallow((state) => named.map((id) => state.objectsById[id]?.name)))
  const nameOf = (id: ObjectId): string =>
    names[named.indexOf(id)] ?? (id === spell ? 'the spell' : 'a permanent')

  const [{ spends, note }, setChosen] = useState(START)
  const [choosing] = selection.ids
  const fitting = (id: ObjectId, given: readonly Spend[] = spends): ManaOption[] =>
    fittingOptions(cost, pool, given, byPermanent.get(id) ?? [])
  const choices = choosing === undefined ? [] : fitting(choosing)
  const chooser = choices.length > 1 ? choosing : undefined

  const legal = useMemo(
    () =>
      [...byPermanent.keys()].filter(
        (id) => fittingOptions(cost, pool, spends, byPermanent.get(id) ?? []).length > 0,
      ),
    [byPermanent, cost, pool, spends],
  )
  const taken = useMemo(
    () => spends.flatMap((spend) => (spend.kind === 'tap' ? [spend.option.source.object] : [])),
    [spends],
  )
  const marks = useMemo(
    () => markCards(own, legal, chooser === undefined ? taken : [...taken, chooser]),
    [own, legal, taken, chooser],
  )

  const canSpend = (mana: ManaType): boolean => allowed(cost, pool, spends, { kind: 'pool', mana })
  const owedNow = owed(cost, spends)
  const covered = covers(cost, spends)
  const canAdd = legal.length > 0 || MANA_TYPES.some(canSpend)

  const add = (spend: Spend): void => {
    // Checked against the latest payment, so two quick clicks cannot add what no longer fits.
    setChosen((now) =>
      allowed(cost, pool, now.spends, spend)
        ? { spends: [...now.spends, spend], note: `Added ${spendWords(spend, nameOf)}.` }
        : now,
    )
    selection.clear()
  }
  const removeAt = (index: number): void => {
    setChosen((now) => {
      const gone = now.spends[index]
      return gone === undefined
        ? now
        : { spends: now.spends.filter((_, at) => at !== index), note: `Removed ${spendWords(gone, nameOf)}.` }
    })
    selection.clear()
  }
  const clear = (): void => {
    setChosen({ spends: [], note: 'Cleared the payment.' })
    selection.clear()
  }

  const act = (id: ObjectId): void => {
    const chosen = spends.findIndex((spend) => spend.kind === 'tap' && spend.option.source.object === id)
    if (chosen >= 0) {
      removeAt(chosen)
      return
    }
    if (chooser === id) {
      selection.clear()
      return
    }
    const here = fitting(id)
    const [only] = here
    if (only === undefined) return
    if (here.length === 1) add({ kind: 'tap', option: only })
    else selection.set([id])
  }
  useCardMarks(marks, act, { toggles: true })

  const pick = (option: ManaOption): void => {
    add({ kind: 'tap', option })
    focusCard(option.source.object)
  }
  const closeChooser = (): void => {
    selection.clear()
    if (chooser !== undefined) focusCard(chooser)
  }

  const text = payText({
    spell: nameOf(spell),
    cost,
    owed: owedNow,
    chosen: spends.length,
    covered,
    canAdd,
    auto: options.auto_available,
    note,
  })
  return (
    <PromptLayout text={text}>
      <PaymentSummary
        cost={cost}
        owed={owedNow}
        covered={covered}
        pool={pool}
        spends={spends}
        nameOf={nameOf}
        canSpend={canSpend}
        onSpendPool={(mana) => add({ kind: 'pool', mana })}
        onRemove={removeAt}
        onClear={clear}
      />
      {chooser !== undefined ? (
        <ManaChoice card={chooser} options={choices} onPick={pick} onCancel={closeChooser} />
      ) : null}
      <Button
        className="h-10 px-4"
        data-slot="pay-confirm"
        disabled={!covered}
        onClick={() => answer({ PayMana: { Manual: toPayment(spends) } })}
      >
        Confirm payment
      </Button>
      {options.auto_available ? (
        <Button
          className="h-10 px-4"
          variant="secondary"
          data-slot="pay-auto"
          onClick={() => answer({ PayMana: 'Auto' })}
        >
          Pay automatically
        </Button>
      ) : null}
      <Button
        className="h-10 px-4"
        variant="ghost"
        data-slot="pay-cancel"
        onClick={() => answer({ PayMana: 'Cancel' })}
      >
        Cancel casting
      </Button>
    </PromptLayout>
  )
}
