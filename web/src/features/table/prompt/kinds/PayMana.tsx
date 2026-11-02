import { useId, useMemo, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { ManaOption, ManaType, ObjectId } from '@/protocol/generated'
import { LIFE_PER_SYMBOL, manaWords } from '../../manaSymbols'
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
  optionKey,
  owed,
  toPayment,
  withoutSpend,
} from '../payment'
import type { LifeSymbols, Reach, Spend } from '../payment'
import { PaymentSummary } from '../PaymentSummary'
import type { RestrictedEntry } from '../PaymentSummary'
import { offersOf } from '../offers'
import { lastTapOf, swapOptions, withSwap } from '../paySwap'
import { spendAllows, usesWords } from '../spendRules'
import { manaByPermanent } from '../priorityText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { UndoButton } from '../UndoButton'

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
 * "Pay the rest for me" keeps them and lets the game pay what is owed; Undo or Escape (only when the options allow
 * it) gives up on the cast, and the permanents picked stay untapped since nothing is paid until Confirm.
 */
export const PayManaPrompt: PromptComponent<'PayMana'> = ({ options }) => {
  const { answer, selection } = usePrompt()
  const { spell, cost, pool } = options
  const own = useOwnCardIds()
  const byPermanent = useMemo(() => manaByPermanent(options.sources), [options.sources])
  const inPool = useMemo<Spend[]>(
    () =>
      (pool.restricted ?? []).flatMap((restricted, index) =>
        spendAllows(options.spend, restricted.only) ? [{ kind: 'restricted', index, restricted }] : [],
      ),
    [pool.restricted, options.spend],
  )
  const pieces = useMemo<Spend[]>(
    () => [
      ...(options.convoke ?? []).flatMap((one): Spend[] => [
        { kind: 'convoke', creature: one.creature, pays: null },
        ...one.colors.map((color): Spend => ({ kind: 'convoke', creature: one.creature, pays: color })),
      ]),
      ...(options.improvise ?? []).map((artifact): Spend => ({ kind: 'improvise', artifact })),
      ...(options.delve ?? []).map((card): Spend => ({ kind: 'delve', card })),
    ],
    [options.convoke, options.improvise, options.delve],
  )
  const extras = useMemo(() => [...inPool, ...pieces], [inPool, pieces])
  const reach = useMemo<Reach>(
    () => ({ sources: options.sources, maxLife: options.max_life, spend: options.spend, extras }),
    [options.sources, options.max_life, options.spend, extras],
  )
  const named = useMemo(
    () => [
      spell,
      ...byPermanent.keys(),
      ...pieces.flatMap((one) =>
        one.kind === 'convoke'
          ? [one.creature]
          : one.kind === 'improvise'
            ? [one.artifact]
            : one.kind === 'delve'
              ? [one.card]
              : [],
      ),
    ],
    [spell, byPermanent, pieces],
  )
  const names = usePlay(useShallow((state) => named.map((id) => state.objectsById[id]?.name)))
  const nameOf = (id: ObjectId): string =>
    names[named.indexOf(id)] ?? (id === spell ? 'the spell' : 'a permanent')
  const spellZone = usePlay((state) => state.objectsById[spell]?.zone)

  const [{ spends, life, note }, setChosen] = useState(START)
  // The chosen permanent whose colour switch is open.
  const [switching, setSwitching] = useState<ObjectId | undefined>(undefined)
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

  const heldRestricted: RestrictedEntry[] = (pool.restricted ?? []).map((restricted, index) => ({
    index,
    mana: restricted.mana,
    words: `only for ${usesWords(restricted.only)}`,
    usable: inPool.some(
      (spend) =>
        spend.kind === 'restricted' &&
        spend.index === index &&
        allowed(cost, pool, spends, spend, life, reach),
    ),
  }))
  const offerRows = offersOf(pieces, spends, nameOf, (spend) =>
    allowed(cost, pool, spends, spend, life, reach),
  )
  const canSpend = (mana: ManaType): boolean =>
    allowed(cost, pool, spends, { kind: 'pool', mana }, life, reach)
  const owedNow = owed(cost, spends, life)
  const covered = covers(cost, spends, life)
  const lifeSwitches = lifeChoices(cost, spends, life, options.max_life)
  const lifeCanPay = lifeSwitches.some((choice) => !choice.paid && choice.available)
  const canAdd =
    legal.length > 0 ||
    MANA_TYPES.some(canSpend) ||
    heldRestricted.some((one) => one.usable) ||
    offerRows.some((row) => row.offers.some((offer) => offer.choices.some((choice) => choice.usable))) ||
    lifeCanPay
  const paidLife = lifeOf(life)
  const wordsOf = (index: number): string => {
    const symbol = cost[index]
    return symbol === undefined ? 'mana' : manaWords(symbol)
  }

  const add = (spend: Spend): void => {
    setSwitching(undefined)
    // Checked against the latest payment, so two quick clicks cannot add what no longer fits.
    setChosen((now) =>
      allowed(cost, pool, now.spends, spend, now.life, reach)
        ? { ...now, spends: [...now.spends, spend], note: `Added ${spendWords(spend, nameOf)}.` }
        : now,
    )
    selection.clear()
  }
  const removeAt = (index: number): void => {
    setSwitching(undefined)
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
    setSwitching(undefined)
    setChosen({ spends: [], life: [], note: 'Cleared the payment.' })
    selection.clear()
  }
  const toggleLife = (index: number): void => {
    setSwitching(undefined)
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
    else {
      setSwitching(undefined)
      selection.set([id])
    }
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

  const swaps = useMemo(
    () =>
      new Map(
        taken.flatMap((id): [ObjectId, ManaOption[]][] => {
          const abilities = byPermanent.get(id) ?? []
          if (abilities.length < 2) return []
          const found = swapOptions(cost, pool, spends, lastTapOf(spends, id), abilities, life, reach)
          return found.length > 0 ? [[id, found]] : []
        }),
      ),
    [taken, byPermanent, cost, pool, spends, life, reach],
  )
  const switchOptions = switching === undefined ? undefined : swaps.get(switching)
  const openSwitch =
    chooser !== undefined
      ? { card: chooser, options: choices, swap: false }
      : switching !== undefined && switchOptions !== undefined
        ? { card: switching, options: switchOptions, swap: true }
        : undefined
  const swap = (option: ManaOption): void => {
    const id = option.source.object
    setChosen((now) => {
      const at = lastTapOf(now.spends, id)
      const fits = swapOptions(cost, pool, now.spends, at, byPermanent.get(id) ?? [], now.life, reach).some(
        (other) => optionKey(other) === optionKey(option),
      )
      return fits
        ? {
            ...now,
            spends: withSwap(now.spends, at, option),
            note: `Now ${spendWords({ kind: 'tap', option }, nameOf)}.`,
          }
        : now
    })
    setSwitching(undefined)
    focusCard(id)
  }
  const openSwitchOn = (id: ObjectId): void => {
    selection.clear()
    setSwitching(id)
  }
  const closeSwitch = (): void => {
    setSwitching(undefined)
    if (switching !== undefined) focusCard(switching)
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
        restricted={heldRestricted}
        offerRows={offerRows}
        onOffer={add}
        onSpendRestricted={(index) => {
          const spend = inPool.find((one) => one.kind === 'restricted' && one.index === index)
          if (spend !== undefined) add(spend)
        }}
        onRemove={removeAt}
        onClear={clear}
        sources={
          <ManaSources
            sources={byPermanent}
            nameOf={nameOf}
            legal={legal}
            taken={taken}
            onAct={act}
            open={openSwitch}
            onPick={openSwitch?.swap === true ? swap : pick}
            onClose={openSwitch?.swap === true ? closeSwitch : closeChooser}
            swappable={[...swaps.keys()]}
            onSwitch={openSwitchOn}
          />
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
        <UndoButton zone={spellZone} onUndo={() => void answer({ PayMana: 'Cancel' })} />
      ) : null}
    </PromptLayout>
  )
}
