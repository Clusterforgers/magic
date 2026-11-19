import { anchor } from '@/features/learn/anchors'
import { useContext, useEffect, useId, useMemo, useRef, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { AbilityRef, ManaOption, ManaType, ObjectId } from '@/protocol/generated'
import { abilityKey, abilityLine, keywordDoing, spoken } from '../activations'
import { LIFE_PER_SYMBOL, manaWords } from '../../manaSymbols'
import { SideContext } from '../../sideContext'
import { markCards, useCardMarks } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { PayColumn, PayMeter } from '../PayColumn'
import { paySlots } from '../paySlots'
import { payText, proposalNote, spendWords } from '../payText'
import {
  allowed,
  completable,
  completion,
  covers,
  fittingOptions,
  fromPayment,
  lifeChoices,
  lifeOf,
  MANA_TYPES,
  optionKey,
  owed,
  toPayment,
  withoutSpend,
} from '../payment'
import type { LifeSymbols, Reach, Spend, Whole } from '../payment'
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

/** Why "Pay for me" is off: the game found no payment (or only one that takes the last life). */
export const AUTO_REASON = 'The game finds no way to pay this cost with what you have.'

/**
 * PayMana: what the spell costs, the floating mana, and what is still owed. Untapped permanents with a mana ability
 * are legal; a click adds one (a permanent with several kinds of mana asks which), a click on a chosen one takes it
 * back. Mana that would be wasted is not offered, so a payment is complete when nothing is owed. A Phyrexian symbol
 * has a switch that pays 2 life for it instead of mana (within the life total and what the options allow). Confirm
 * sends the chosen payment; with nothing chosen yet "Pay for me" lays out the game's payment (`options.proposal`) as
 * picks to confirm or change, and after some picks "Pay the rest for me" adds what is owed the same way; with "Always
 * pay automatically" on, both pay at once as the game chooses; Undo or Escape (only when the options allow
 * it) gives up on the cast, and the permanents picked stay untapped since nothing is paid until Confirm. A mana
 * ability that does more than add mana (Vexing Puzzlebox's roll, `options.announced`) is a button and a legal card:
 * it is activated at once and the payment is asked again with its mana floating.
 */
export const PayManaPrompt: PromptComponent<'PayMana'> = ({ options }) => {
  const { answer, selection } = usePrompt()
  const { spell, cost, pool } = options
  const announced = useMemo(() => options.announced ?? [], [options.announced])
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
      ...announced.map((ref) => ref.object),
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
    [spell, byPermanent, pieces, announced],
  )
  const names = usePlay(useShallow((state) => named.map((id) => state.objectsById[id]?.name)))
  const nameOf = (id: ObjectId): string =>
    names[named.indexOf(id)] ?? (id === spell ? 'the spell' : 'a permanent')
  const spellZone = usePlay((state) => state.objectsById[spell]?.zone)
  // What the payment is for, named in the band: the keyword of the ability activated ("Plainscycling"), or a cast.
  const activating = options.ability
  const keyword = usePlay((state) =>
    activating === undefined ? undefined : keywordDoing(activating, state.objectsById[activating.object]),
  )
  const isAbility = activating !== undefined || options.spend?.kind === 'Ability'
  const doing = isAbility
    ? (keyword ?? 'Activating')
    : options.spend?.kind === 'Spell'
      ? 'Casting'
      : undefined

  const [{ spends, life, note }, setChosen] = useState(START)
  // "Always pay automatically" makes "Pay for me" pay at once; without it the game's payment is proposed for a Confirm.
  const payAtOnce = usePlay((state) => state.autoChoices.auto_pay === true)
  const confirmRef = useRef<HTMLButtonElement>(null)
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
  // A source whose mana ability does more than add mana (a roll, a discard) is activated at once, apart from the
  // payment, which is asked again with its mana in the pool.
  const activatable = useMemo(
    () => [...new Set(announced.map((ref) => ref.object))].filter((id) => !byPermanent.has(id)),
    [announced, byPermanent],
  )
  const lines = usePlay(
    useShallow((state) => announced.map((ref) => abilityLine(ref, state.objectsById[ref.object]))),
  )
  const activate = (ref: AbilityRef): void => void answer({ PayMana: { Activate: ref } })
  const marks = useMemo(
    () => markCards(own, [...legal, ...activatable], chooser === undefined ? taken : [...taken, chooser]),
    [own, legal, activatable, taken, chooser],
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
  // The game's payment laid out as if picked by hand: the sources read pressed and the circles fill, and Confirm pays it.
  const propose = (whole: Whole): void => {
    setChosen({ spends: whole.spends, life: whole.life, note: proposalNote(whole, nameOf) })
    selection.clear()
    requestAnimationFrame(() => confirmRef.current?.focus())
  }
  const payForMe = (): void => {
    const whole =
      payAtOnce || options.proposal === undefined
        ? undefined
        : fromPayment(options.proposal, cost, pool, options.sources)
    if (whole === undefined) answer({ PayMana: 'Auto' })
    else propose(whole)
  }
  const payTheRest = (): void => {
    const preferred = new Set((options.proposal?.taps ?? []).map((tap) => `${tap.object}:${tap.ability}`))
    const whole = payAtOnce
      ? undefined
      : completion(cost, pool, spends, life, reach, (option) =>
          preferred.has(`${option.source.object}:${option.source.ability}`),
        )
    if (whole === undefined) answer({ PayMana: { Complete: toPayment(spends, life) } })
    else propose(whole)
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
    const announcedRef = activatable.includes(id) ? announced.find((ref) => ref.object === id) : undefined
    if (announcedRef !== undefined) {
      activate(announcedRef)
      return
    }
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

  // The colour of a card that was pressed on the board: the keyboard goes back to the card.
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
    focusCard(id)
  }
  // A card of the board that waits for its colour is answered in its row: a folded sheet opens for it.
  const side = useContext(SideContext)
  const openSheet = side?.setSheetFolded
  useEffect(() => {
    if (chooser !== undefined) openSheet?.(false)
  }, [chooser, openSheet])

  const hasLife = lifeSwitches.length > 0
  const text = payText({
    spell: nameOf(spell),
    ability: isAbility ? (keyword ?? 'an ability') : undefined,
    cost,
    owed: owedNow,
    chosen: spends.length + life.length,
    covered,
    canAdd,
    auto: options.auto_available,
    cancel: options.can_cancel,
    attack: options.attack === true,
    block: options.block === true,
    note,
    life: hasLife ? { paid: paidLife, after: options.life - paidLife, may: lifeCanPay } : undefined,
  })
  const nothingChosen = spends.length === 0 && life.length === 0
  const restCanBePaid = completable(cost, pool, spends, life, reach)
  const slots = paySlots(cost, spends, life)
  return (
    <PromptLayout
      text={text}
      quiet
      doing={doing}
      meter={<PayMeter slots={slots} />}
      choices={
        <PayColumn
          name={keyword === undefined ? nameOf(spell) : `${keyword} of ${nameOf(spell)}`}
          slots={slots}
          spends={spends}
          covered={covered}
          note={note}
          nameOf={nameOf}
          sources={byPermanent}
          fitting={(id) => fitting(id)}
          taken={taken}
          asking={chooser}
          swaps={swaps}
          onTap={(option) => (option.source.object === chooser ? pick(option) : add({ kind: 'tap', option }))}
          onCancelAsk={closeChooser}
          onSwap={swap}
          onRemove={removeAt}
          onClear={clear}
        >
          {nothingChosen && !options.auto_available ? (
            <p id={autoReasonId} data-slot="pay-auto-reason" className="text-muted-foreground text-sm">
              {AUTO_REASON}
            </p>
          ) : null}
          <PaymentSummary
            aside
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
          {announced.length === 0 ? null : (
            <div data-slot="pay-group" className="grid gap-1.5">
              <p className="mt-1 text-xs leading-4 font-semibold text-muted-foreground">
                Activates at once, then the cost is asked again
              </p>
              <ul className="grid list-none gap-1.5">
                {announced.map((ref, at) => {
                  const line = lines[at] ?? 'its mana ability'
                  const name = nameOf(ref.object)
                  return (
                    <li
                      key={`${ref.object}-${abilityKey(ref)}`}
                      data-interactive=""
                      className="detail-row grid-cols-[minmax(0,1fr)_auto] py-1 pr-1"
                    >
                      <span className="grid min-w-0">
                        <span className="truncate text-[0.8125rem] leading-[1.125rem] font-semibold">
                          {name}
                        </span>
                        <span className="truncate text-xs leading-4 text-muted-foreground" title={line}>
                          {line}
                        </span>
                      </span>
                      <Button
                        size="sm"
                        variant="outline"
                        data-slot="pay-activate"
                        title={`${name}: ${line}`}
                        aria-label={`Activate ${name}: ${spoken(line)}`}
                        onClick={() => activate(ref)}
                      >
                        Activate
                      </Button>
                    </li>
                  )
                })}
              </ul>
            </div>
          )}
        </PayColumn>
      }
    >
      <Button
        ref={confirmRef}
        className="h-10 px-4"
        data-slot="pay-confirm"
        {...anchor('pay.confirm')}
        aria-label="Confirm payment"
        disabled={!covered}
        onClick={() => answer({ PayMana: { Manual: toPayment(spends, life) } })}
      >
        Confirm
      </Button>
      {nothingChosen ? (
        <Button
          className="h-10 px-4"
          variant="outline"
          data-slot="pay-auto"
          {...anchor('pay.auto')}
          disabled={!options.auto_available}
          aria-describedby={options.auto_available ? undefined : autoReasonId}
          onClick={payForMe}
        >
          Pay for me
        </Button>
      ) : null}
      {options.auto_available && !nothingChosen && !covered && restCanBePaid ? (
        <Button className="h-10 px-4" variant="outline" data-slot="pay-rest" onClick={payTheRest}>
          Pay the rest for me
        </Button>
      ) : null}
      <Button
        className="h-10 px-3"
        variant="ghost"
        data-slot="pay-undo"
        aria-label="Undo the last choice"
        disabled={spends.length === 0}
        onClick={() => removeAt(spends.length - 1)}
      >
        Undo
      </Button>
      {options.can_cancel ? (
        options.attack === true || options.block === true ? (
          <Button
            className="h-10 px-4"
            variant="outline"
            data-slot="pay-cancel"
            onClick={() => answer({ PayMana: 'Cancel' })}
          >
            {options.attack === true ? 'Back to attackers' : 'Back to blockers'}
          </Button>
        ) : (
          <UndoButton
            zone={spellZone}
            ability={isAbility ? true : options.spend?.kind === 'Spell' ? false : undefined}
            onUndo={() => void answer('Undo')}
          />
        )
      ) : null}
    </PromptLayout>
  )
}
