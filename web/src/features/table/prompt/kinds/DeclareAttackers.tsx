import { useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Check, Eraser, Swords, UsersRound, X, Zap } from 'lucide-react'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import type { AttackerOption, AttackTarget, ObjectId, PlayerId } from '@/protocol/generated'
import { useDefenderStore, usePublishDefenders } from '../../combat/defender'
import { attacksAnswer, useBlockPreview } from '../../combat/forecast'
import { usePublishPending } from '../../combat/pending'
import { usePreviewRequest } from '../../combat/usePreviewRequest'
import { usePreviewTarget } from '../../preview'
import { symbolText } from '../../manaSymbols'
import {
  attackLifeOf,
  attackSummary,
  attackTaxOf,
  costLine,
  costPhrase,
  distinctTargets,
  sameTarget,
  targetKey,
} from '../combatText'
import { BarButton } from '../CombatBar'
import { markCards, useCardMarks, useCardNotes } from '../marks'
import type { CardMarks, CardNote, CardNotes } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { PreviewOutcome } from '../PreviewOutcome'
import type { PromptComponent } from '../types'
import { usePrimaryKey } from '../usePrimaryKey'
import { useObjects, useTargetNames } from '../useCombatNames'

/** The attacker's defender that is `aim`, else its first defender; undefined for an attacker with none. */
function aimedAt(option: AttackerOption, aim: AttackTarget | undefined): AttackTarget | undefined {
  return (
    option.defenders.find((target) => aim !== undefined && sameTarget(target, aim)) ?? option.defenders[0]
  )
}

/**
 * DeclareAttackers: creatures that may attack are legal, the rest of the hand and battlefield dimmed. A click, Enter
 * or Space puts a creature in the attack or takes it out; its arrow to the defender says it attacks, no label is
 * written on the card. With several defenders the creature clicked in last is held: a click on a defender (a seat's
 * pod, tile or board, or the card of a planeswalker or battle) sends it there, and that defender is where the next
 * creatures go, as is the one chosen in the list of the bar; the last choice is where the next attack starts. A
 * creature a rule requires to attack starts in the attack, reads "Must attack" and cannot be taken out. Confirm sends the attack, No attack an empty one.
 */
export const DeclareAttackersPrompt: PromptComponent<'DeclareAttackers'> = ({ options }) => {
  const { id, answer, selection } = usePrompt()
  const own = useOwnCardIds()
  const defenderStore = useDefenderStore()
  // An attacker with no defender cannot attack; a creature listed twice counts once.
  const offered = useMemo(() => {
    const byId = new Map<ObjectId, AttackerOption>()
    for (const option of options.attackers) {
      if (option.defenders.length > 0 && !byId.has(option.object)) byId.set(option.object, option)
    }
    return byId
  }, [options.attackers])
  const legal = useMemo(() => [...offered.keys()], [offered])
  const defenders = useMemo(() => distinctTargets([...offered.values()]), [offered])
  // The planeswalkers and battles on offer, whose cards a click attacks.
  const walkers = useMemo(
    () => defenders.flatMap((target): ObjectId[] => ('Planeswalker' in target ? [target.Planeswalker] : [])),
    [defenders],
  )
  const required = useMemo(
    () => (options.required ?? []).filter((object) => offered.has(object)),
    [options.required, offered],
  )
  const names = useTargetNames(defenders)
  const several = defenders.length > 1

  // The defender chosen now: the one picked, else the one picked last time, else the first.
  const [aimKey, setAimKey] = useState<string | null>(null)
  const wanted = aimKey ?? defenderStore.remembered()
  const aim = defenders.find((target) => targetKey(target) === wanted) ?? defenders[0]
  const [targets, setTargets] = useState<ReadonlyMap<ObjectId, AttackTarget>>(new Map())
  // The creature clicked into the attack last, which a click on a defender sends there.
  const [held, setHeld] = useState<ObjectId | null>(null)
  const pickAim = useCallback(
    (key: string): void => {
      setAimKey(key)
      defenderStore.remember(key)
    },
    [defenderStore],
  )
  // A click on a defender sends the held creature there when it may attack it, and aims the next creatures there.
  const pickTarget = (target: AttackTarget): void => {
    const option = held === null ? undefined : offered.get(held)
    if (held !== null && option?.defenders.some((to) => sameTarget(to, target)) === true) {
      setTargets((current) => new Map(current).set(held, target))
    }
    setHeld(null)
    pickAim(targetKey(target))
  }
  const latestPick = useRef(pickTarget)
  useLayoutEffect(() => {
    latestPick.current = pickTarget
  })
  const seats = useMemo(
    () =>
      several ? defenders.flatMap((target): PlayerId[] => ('Player' in target ? [target.Player] : [])) : [],
    [several, defenders],
  )
  const pickSeat = useCallback((seat: PlayerId): void => latestPick.current({ Player: seat }), [])
  usePublishDefenders(seats, aim !== undefined && 'Player' in aim ? aim.Player : null, pickSeat)

  // The attack as declared so far: creatures in the order they were chosen, each with its defender.
  const declared = useMemo(
    () =>
      [...selection.ids].flatMap((object) => {
        const option = offered.get(object)
        const target = option === undefined ? undefined : (targets.get(object) ?? aimedAt(option, undefined))
        return target === undefined ? [] : [{ object, target }]
      }),
    [selection.ids, offered, targets],
  )
  const declaredIds = useMemo(() => declared.map((entry) => entry.object), [declared])
  // The attack as chosen so far is drawn as lines from each attacker to what it attacks, before anything is sent.
  const drawn = useMemo(() => new Map(declared.map(({ object, target }) => [object, target])), [declared])
  usePublishPending({ attacks: drawn })
  // The engine is asked what the attack would do each time it changes (nobody is blocking yet); its forecast comes
  // back through the seam and is drawn on the seats it would hit.
  const attackMade = useMemo(() => attacksAnswer(drawn), [drawn])
  const previewStatus = usePreviewRequest(id, attackMade)
  const preview = useBlockPreview()

  // The creature under the pointer or the focus: when it cannot attack the defender chosen, the bar says so.
  const shown = usePreviewTarget()?.id
  const [shownCreature] = useObjects(shown === undefined ? [] : [shown])
  const shownOption = shown === undefined ? undefined : offered.get(shown)
  const [instead] = shownOption?.defenders ?? []
  const cannotAim =
    aim === undefined ||
    shownOption === undefined ||
    instead === undefined ||
    shownOption.defenders.some((target) => sameTarget(target, aim))
      ? undefined
      : `${shownCreature?.name ?? 'This creature'} cannot attack ${names.short(aim)}; it attacks ${names.short(instead)} instead.`

  const aimedWalker = aim !== undefined && 'Planeswalker' in aim ? aim.Planeswalker : undefined
  const marks = useMemo<CardMarks>(() => {
    const marked = new Map(markCards(own, legal, declaredIds))
    // With several defenders a planeswalker or battle on offer is clicked to attack it; the one chosen reads pressed.
    if (several) {
      for (const walker of walkers) {
        if (!offered.has(walker)) marked.set(walker, walker === aimedWalker ? 'selected' : 'legal')
      }
    }
    return marked
  }, [own, legal, declaredIds, several, walkers, offered, aimedWalker])
  // What each chosen attacker costs, and the cost line of the bar; nothing while no effect taxes the attack.
  const costs = useMemo(
    () =>
      declared.map(({ object, target }) => {
        const option = offered.get(object)
        return {
          object,
          cost: option === undefined ? [] : attackTaxOf(option, target),
          life: option === undefined ? 0 : attackLifeOf(option, target),
        }
      }),
    [declared, offered],
  )
  const taxed = useMemo(
    () =>
      [...offered.values()].some((option) => (option.tax?.length ?? 0) > 0 || (option.life?.length ?? 0) > 0),
    [offered],
  )
  const notes = useMemo<CardNotes>(() => {
    const map = new Map<ObjectId, CardNote>()
    // A creature that is not in the attack yet says what attacking would cost.
    for (const [object, option] of offered) {
      const price = aim === undefined ? '' : costPhrase(attackTaxOf(option, aim), attackLifeOf(option, aim))
      if (price !== '' && !selection.ids.has(object))
        map.set(object, { words: `can attack; attacking ${price}` })
    }
    for (const { object, target } of declared) {
      const price = costs.find((entry) => entry.object === object)
      const pays = price === undefined ? '' : costPhrase(price.cost, price.life)
      const label =
        pays === '' || price === undefined ? undefined : `Pays ${price.cost.map(symbolText).join('')}`
      const owed = pays === '' ? '' : `; attacking ${pays}`
      map.set(
        object,
        object === held
          ? {
              words: `attacking ${names.long(target)}${owed}; click a player, planeswalker or battle to send it there`,
              cue: 'active' as const,
            }
          : required.includes(object)
            ? { label: 'Must attack', words: `must attack ${names.long(target)} if able${owed}` }
            : { ...(label === undefined ? {} : { label }), words: `attacking ${names.long(target)}${owed}` },
      )
    }
    return map
  }, [declared, names, held, required, offered, aim, selection.ids, costs])

  const aimAll = (ids: readonly ObjectId[], target: AttackTarget | undefined): void => {
    setTargets((current) => {
      const next = new Map(current)
      for (const id of ids) {
        const option = offered.get(id)
        const to = option === undefined ? undefined : aimedAt(option, target)
        if (to !== undefined) next.set(id, to)
      }
      return next
    })
  }
  const choose = (id: ObjectId): void => {
    if (!offered.has(id)) {
      if (walkers.includes(id)) pickTarget({ Planeswalker: id })
      return
    }
    // A creature a rule requires to attack stays in the attack.
    if (required.includes(id)) return
    if (selection.has(id)) {
      if (held === id) setHeld(null)
    } else {
      aimAll([id], aim)
      if (several) setHeld(id)
    }
    selection.toggle(id)
  }
  useCardMarks(marks, choose, { toggles: true })
  useCardNotes(notes)

  // The creatures a rule requires to attack start in the attack, once, at the defender chosen when they may.
  const started = useRef(false)
  useLayoutEffect(() => {
    if (started.current || required.length === 0) return
    started.current = true
    aimAll(required, aim)
    selection.set(required)
  })

  const attackWithAll = (): void => {
    const fresh = legal.filter((id) => !selection.has(id))
    aimAll(fresh, aim)
    selection.set([...selection.ids, ...fresh])
  }
  const allAt = (): void => aimAll(declaredIds, aim)
  const clear = (): void => {
    setHeld(null)
    aimAll(required, aim)
    selection.set(required)
  }
  // The required creatures left out of the attack, which the bar names.
  const missing = useMemo(() => required.filter((object) => !selection.has(object)), [required, selection])
  const missingCreatures = useObjects(missing)
  // The declared attackers that may be exerted, each with a toggle in the bar; an exert of a creature taken out of the attack is dropped.
  const [exertedSet, setExerted] = useState<ReadonlySet<ObjectId>>(new Set())
  const exertable = useMemo(
    () => declaredIds.filter((object) => offered.get(object)?.exert === true),
    [declaredIds, offered],
  )
  const exertableCreatures = useObjects(exertable)
  const exerting = exertable.filter((object) => exertedSet.has(object))
  const toggleExert = (object: ObjectId): void =>
    setExerted((current) => {
      const next = new Set(current)
      if (!next.delete(object)) next.add(object)
      return next
    })
  const elsewhere = aim !== undefined && declared.some(({ target }) => !sameTarget(target, aim))
  const confirm = (): void => {
    if (declared.length === 0) return
    const attackers = declared.map(({ object, target }): [ObjectId, AttackTarget] => [object, target])
    answer(
      exerting.length > 0
        ? { DeclareExerting: { attackers, exert: exerting } }
        : { DeclareAttackers: attackers },
    )
  }
  // With nothing focused, Enter confirms the attack that is chosen; Space is left alone.
  usePrimaryKey({ space: () => undefined, enter: confirm }, declared.length > 0)

  const heldTarget = declared.find((entry) => entry.object === held)?.target
  const [heldCreature] = useObjects(held === null ? [] : [held])
  const text =
    legal.length === 0
      ? 'None of your creatures can attack. Choose No attack to go on.'
      : cannotAim !== undefined
        ? cannotAim
        : missing.length > 0
          ? `${missingCreatures.map((object) => object?.name ?? 'A creature').join(', ')} must attack if able.`
          : declared.length > 0
            ? `${attackSummary(
                declared.map((entry) => entry.target),
                names,
              )}.${several && heldTarget !== undefined ? ` Click a player, planeswalker or battle to send ${heldCreature?.name ?? 'it'} there.` : ''}`
            : several && aim !== undefined
              ? `Click a creature, then the player, planeswalker or battle it attacks (${names.short(aim)} until then).`
              : `Choose the creatures that attack${aim === undefined ? '' : ` ${names.short(aim)}`}, or choose No attack.`
  const cleared = selection.ids.size === required.length && missing.length === 0

  return (
    <PromptLayout compact text={text}>
      {options.most === undefined ? null : (
        <p data-slot="declaration-limit" className="basis-full text-xs text-muted-foreground">
          No more than {options.most} {options.most === 1 ? 'creature' : 'creatures'} can attack this combat.
        </p>
      )}
      {taxed ? (
        <p data-slot="attack-cost" className="basis-full text-xs text-muted-foreground">
          {costLine(costs, true, options.mana)}
        </p>
      ) : null}
      {(options.most_at ?? []).map(([target, most]) => (
        <p
          key={JSON.stringify(target)}
          data-slot="defender-limit"
          className="basis-full text-xs text-muted-foreground"
        >
          No more than {most} {most === 1 ? 'creature' : 'creatures'} can attack {names.short(target)} this
          combat.
        </p>
      ))}
      {several && aim !== undefined ? (
        <NativeSelect
          size="sm"
          className="max-w-44"
          aria-label="Attack at"
          value={targetKey(aim)}
          onChange={(event) => pickAim(event.target.value)}
        >
          {defenders.map((target) => (
            <NativeSelectOption key={targetKey(target)} value={targetKey(target)}>
              {names.long(target)}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      ) : null}
      <BarButton
        variant="outline"
        icon={Swords}
        label="Attack with all"
        word="All"
        disabled={legal.length === 0}
        onClick={attackWithAll}
      />
      {several && aim !== undefined && elsewhere ? (
        <BarButton
          variant="outline"
          icon={UsersRound}
          label={`All at ${names.short(aim)}`}
          word={`All at ${names.short(aim)}`}
          onClick={allAt}
        />
      ) : null}
      <BarButton
        variant="outline"
        icon={Eraser}
        label="Clear"
        word="Clear"
        disabled={cleared}
        onClick={clear}
      />
      {exertable.map((object, index) => {
        const name = exertableCreatures[index]?.name ?? 'creature'
        const on = exertedSet.has(object)
        return (
          <BarButton
            key={object}
            variant={on ? 'secondary' : 'outline'}
            icon={Zap}
            label={`Exert ${name}`}
            word={on ? 'Exerting' : 'Exert'}
            aria-pressed={on}
            onClick={() => toggleExert(object)}
          />
        )
      })}
      <BarButton
        icon={Check}
        label="Confirm attack"
        word="Attack"
        keepWord
        disabled={declared.length === 0}
        onClick={confirm}
      />
      {/* A rule that requires an attacker leaves no empty attack to choose. */}
      {required.length > 0 ? null : (
        <BarButton
          variant="outline"
          icon={X}
          label="No attack"
          word="No attack"
          onClick={() => answer({ DeclareAttackers: [] })}
        />
      )}
      {declared.length === 0 ? null : (
        <PreviewOutcome
          slot="attack-outcome"
          heading="If you attack like this"
          forecast={preview}
          status={previewStatus}
        />
      )}
    </PromptLayout>
  )
}
