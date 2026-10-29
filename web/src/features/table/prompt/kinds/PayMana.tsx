import { useId, useMemo, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { ManaOption, ManaType, ObjectId } from '@/protocol/generated'
import { LIFE_PER_SYMBOL, manaWords } from '../../manaSymbols'
import { ManaChoice } from '../ManaChoice'
import { ManaSources } from '../ManaSources'
import { markCards, useCardMarks } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { payText, spendWords } from '../payText'
import {
  allowed,
  completable,
  covers,
  fittingOptions,
  lifeChoices,
  lifeOf,
  MANA_TYPES,
  owed,
  toPayment,
  withoutSpend,
} from '../payment'
import type { LifeSymbols, Reach, Spend } from '../payment'
import { PaymentSummary } from '../PaymentSummary'
import { manaByPermanent } from '../priorityText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'

const focusCard = (id: ObjectId): void =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)?.focus()

/** The payment so far, in order of choice, the symbols life pays, and the sentence about the last change to it. */
interface Chosen {
  spends: readonly Spend[]
  life: LifeSymbols
  note: string
}
const START: Chosen = { spends: [], life: [], note: '' }

/** Why "Pay automatically" is off: the game found no payment (or only one that takes the last life). */
export const AUTO_REASON = 'The game finds no way to pay this cost with what you have.'

/**
 * PayMana: what the spell costs, the floating mana, and what is still owed. Untapped permanents with a mana ability
 * are legal; a click adds one (a permanent with several kinds of mana asks which), a click on a chosen one takes it
 * back. Mana that would be wasted is not offered, so a payment is complete when nothing is owed. A Phyrexian symbol
 * has a switch that pays 2 life for it instead of mana (within the life total and what the options allow). Confirm
 * sends the chosen payment; with nothing chosen yet "Pay automatically" lets the game choose, and after some picks
 * "Pay the rest for me" keeps them and lets the game pay what is owed; Cancel (only when the options allow it)
 * gives up on the cast.
 */
export const PayManaPrompt: PromptComponent<'PayMana'> = ({ options }) => {
  const { answer, selection } = usePrompt()
  const { spell, cost, pool } = options
  const own = useOwnCardIds()
  const byPermanent = useMemo(() => manaByPermanent(options.sources), [options.sources])
  const reach = useMemo<Reach>(
    () => ({ sources: options.sources, maxLife: options.max_life }),
    [options.sources, options.max_life],
  )
  const named = useMemo(() => [spell, ...byPermanent.keys()], [spell, byPermanent])
  const names = usePlay(useShallow((state) => named.map((id) => state.objectsById[id]?.name)))
  const nameOf = (id: ObjectId): string =>
    names[named.indexOf(id)] ?? (id === spell ? 'the spell' : 'a permanent')

  const [{ spends, life, note }, setChosen] = useState(START)
  const autoReasonId = useId()
  const [choosing] = selection.ids
  const fitting = (id: ObjectId, given: readonly Spend[] = spends): ManaOption[] =>
    fittingOptions(cost, pool, given, byPermanent.get(id) ?? [], life, reach)
  const choices = choosing === undefined ? [] : fitting(choosing)
  const chooser = choices.length > 1 ? choosing : undefined

  const legal = useMemo(
    () =>
      [...byPermanent.keys()].filter(
        (id) => fittingOptions(cost, pool, spends, byPermanent.get(id) ?? [], life, reach).length > 0,
      ),
    [byPermanent, cost, pool, spends, life, reach],
  )
  const taken = useMemo(
    () => spends.flatMap((spend) => (spend.kind === 'tap' ? [spend.option.source.object] : [])),
    [spends],
  )
  const marks = useMemo(
    () => markCards(own, legal, chooser === undefined ? taken : [...taken, chooser]),
    [own, legal, taken, chooser],
  )

  const canSpend = (mana: ManaType): boolean =>
    allowed(cost, pool, spends, { kind: 'pool', mana }, life, reach)
  const owedNow = owed(cost, spends, life)
  const covered = covers(cost, spends, life)
  const lifeSwitches = lifeChoices(cost, spends, life, options.max_life)
  const lifeCanPay = lifeSwitches.some((choice) => !choice.paid && choice.available)
  const canAdd = legal.length > 0 || MANA_TYPES.some(canSpend) || lifeCanPay
  const paidLife = lifeOf(life)
  const wordsOf = (index: number): string => {
    const symbol = cost[index]
    return symbol === undefined ? 'mana' : manaWords(symbol)
  }

  const add = (spend: Spend): void => {
    // Checked against the latest payment, so two quick clicks cannot add what no longer fits.
    setChosen((now) =>
      allowed(cost, pool, now.spends, spend, now.life, reach)
        ? { ...now, spends: [...now.spends, spend], note: `Added ${spendWords(spend, nameOf)}.` }
        : now,
    )
    selection.clear()
  }
  const removeAt = (index: number): void => {
    setChosen((now) => {
      const gone = now.spends[index]
      return gone === undefined
        ? now
        : {
            ...now,
            spends: withoutSpend(now.spends, index),
            note: `Removed ${spendWords(gone, nameOf)}.`,
          }
    })
    selection.clear()
  }
  const clear = (): void => {
    setChosen({ spends: [], life: [], note: 'Cleared the payment.' })
    selection.clear()
  }
  const toggleLife = (index: number): void => {
    setChosen((now) => {
      const paying = !now.life.includes(index)
      const next = paying ? [...now.life, index] : now.life.filter((other) => other !== index)
      // Checked against the latest payment too: life only changes to a payment the engine could still accept.
      const flip = lifeChoices(cost, now.spends, now.life, options.max_life).find(
        (choice) => choice.index === index,
      )
      if (flip?.available !== true) return now
      const word = wordsOf(index)
      return {
        ...now,
        life: next,
        note: paying
          ? `Paying ${LIFE_PER_SYMBOL} life instead of ${word}.`
          : `No longer paying ${LIFE_PER_SYMBOL} life instead of ${word}.`,
      }
    })
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

  const hasLife = lifeSwitches.length > 0
  const text = payText({
    spell: nameOf(spell),
    cost,
    owed: owedNow,
    chosen: spends.length + life.length,
    covered,
    canAdd,
    auto: options.auto_available,
    cancel: options.can_cancel,
    note,
    life: hasLife ? { paid: paidLife, after: options.life - paidLife, may: lifeCanPay } : undefined,
  })
  const nothingChosen = spends.length === 0 && life.length === 0
  const restCanBePaid = completable(cost, pool, spends, life, reach)
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
        sources={
          <ManaSources sources={byPermanent} nameOf={nameOf} legal={legal} taken={taken} onAct={act} />
        }
        life={
          hasLife
            ? {
                total: options.life,
                paid: paidLife,
                choices: lifeSwitches,
                words: wordsOf,
                onToggle: toggleLife,
              }
            : undefined
        }
      />
      {chooser !== undefined ? (
        <ManaChoice card={chooser} options={choices} onPick={pick} onCancel={closeChooser} />
      ) : null}
      <Button
        className="h-10 px-4"
        data-slot="pay-confirm"
        disabled={!covered}
        onClick={() => answer({ PayMana: { Manual: toPayment(spends, life) } })}
      >
        Confirm payment
      </Button>
      {nothingChosen ? (
        <Button
          className="h-10 px-4"
          variant="outline"
          data-slot="pay-auto"
          disabled={!options.auto_available}
          aria-describedby={options.auto_available ? undefined : autoReasonId}
          onClick={() => answer({ PayMana: 'Auto' })}
        >
          Pay automatically
        </Button>
      ) : null}
      {nothingChosen && !options.auto_available ? (
        <p id={autoReasonId} data-slot="pay-auto-reason" className="text-muted-foreground text-sm">
          {AUTO_REASON}
        </p>
      ) : null}
      {options.auto_available && !nothingChosen && !covered && restCanBePaid ? (
        <Button
          className="h-10 px-4"
          variant="outline"
          data-slot="pay-rest"
          onClick={() => answer({ PayMana: { Complete: toPayment(spends, life) } })}
        >
          Pay the rest for me
        </Button>
      ) : null}
      {options.can_cancel ? (
        <Button
          className="h-10 px-4"
          variant="outline"
          data-slot="pay-cancel"
          onClick={() => answer({ PayMana: 'Cancel' })}
        >
          Cancel casting
        </Button>
      ) : null}
    </PromptLayout>
  )
}
