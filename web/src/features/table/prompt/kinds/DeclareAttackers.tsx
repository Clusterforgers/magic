import { useCallback, useMemo, useState } from 'react'
import { Check, Eraser, Swords, UsersRound, X } from 'lucide-react'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import type { AttackerOption, AttackTarget, ObjectId, PlayerId } from '@/protocol/generated'
import { useDefenderStore, usePublishDefenders } from '../../combat/defender'
import { attacksAnswer, useBlockPreview } from '../../combat/forecast'
import { usePublishPending } from '../../combat/pending'
import { usePreviewRequest } from '../../combat/usePreviewRequest'
import { usePreviewTarget } from '../../preview'
import { attackSummary, distinctTargets, sameTarget, targetKey } from '../combatText'
import { BarButton } from '../CombatBar'
import { markCards, useCardMarks, useCardNotes } from '../marks'
import type { CardNotes } from '../marks'
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
 * written on the card. With several defenders the one chosen now (a click on a pod, or the list in the bar) is where
 * the next creature goes, and the last choice is where the next attack starts. Confirm sends the attack, No attack an
 * empty one.
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
  const names = useTargetNames(defenders)
  const several = defenders.length > 1

  // The defender chosen now: the one picked, else the one picked last time, else the first.
  const [aimKey, setAimKey] = useState<string | null>(null)
  const wanted = aimKey ?? defenderStore.remembered()
  const aim = defenders.find((target) => targetKey(target) === wanted) ?? defenders[0]
  const [targets, setTargets] = useState<ReadonlyMap<ObjectId, AttackTarget>>(new Map())
  const pickAim = useCallback(
    (key: string): void => {
      setAimKey(key)
      defenderStore.remember(key)
    },
    [defenderStore],
  )
  const seats = useMemo(
    () =>
      several ? defenders.flatMap((target): PlayerId[] => ('Player' in target ? [target.Player] : [])) : [],
    [several, defenders],
  )
  const pickSeat = useCallback((seat: PlayerId): void => pickAim(targetKey({ Player: seat })), [pickAim])
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

  const marks = useMemo(() => markCards(own, legal, declaredIds), [own, legal, declaredIds])
  const notes = useMemo<CardNotes>(
    () =>
      new Map(declared.map(({ object, target }) => [object, { words: `attacking ${names.long(target)}` }])),
    [declared, names],
  )

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
    if (!offered.has(id)) return
    if (!selection.has(id)) aimAll([id], aim)
    selection.toggle(id)
  }
  useCardMarks(marks, choose, { toggles: true })
  useCardNotes(notes)

  const attackWithAll = (): void => {
    const fresh = legal.filter((id) => !selection.has(id))
    aimAll(fresh, aim)
    selection.set([...selection.ids, ...fresh])
  }
  const allAt = (): void => aimAll(declaredIds, aim)
  const elsewhere = aim !== undefined && declared.some(({ target }) => !sameTarget(target, aim))
  const confirm = (): void => {
    if (declared.length > 0)
      answer({ DeclareAttackers: declared.map(({ object, target }) => [object, target]) })
  }
  // With nothing focused, Enter confirms the attack that is chosen; Space is left alone.
  usePrimaryKey({ space: () => undefined, enter: confirm }, declared.length > 0)

  const text =
    legal.length === 0
      ? 'None of your creatures can attack. Choose No attack to go on.'
      : cannotAim !== undefined
        ? cannotAim
        : declared.length > 0
          ? `${attackSummary(
              declared.map((entry) => entry.target),
              names,
            )}. Confirm the attack, or keep choosing.`
          : several && aim !== undefined
            ? `Click the creatures that attack ${names.short(aim)}, or another player to change who, or choose No attack.`
            : `Choose the creatures that attack${aim === undefined ? '' : ` ${names.short(aim)}`}, or choose No attack.`

  return (
    <PromptLayout compact text={text}>
      {options.most === undefined ? null : (
        <p data-slot="declaration-limit" className="basis-full text-xs text-muted-foreground">
          No more than {options.most} {options.most === 1 ? 'creature' : 'creatures'} can attack this combat.
        </p>
      )}
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
        disabled={declared.length === 0}
        onClick={selection.clear}
      />
      <BarButton
        icon={Check}
        label="Confirm attack"
        word="Attack"
        keepWord
        disabled={declared.length === 0}
        onClick={confirm}
      />
      <BarButton
        variant="outline"
        icon={X}
        label="No attack"
        word="No attack"
        onClick={() => answer({ DeclareAttackers: [] })}
      />
      {declared.length === 0 ? null : (
        <div className="sr-only">
          <PreviewOutcome
            slot="attack-outcome"
            heading="If you attack like this"
            forecast={preview}
            status={previewStatus}
          />
        </div>
      )}
    </PromptLayout>
  )
}
