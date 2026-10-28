import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import type { AttackerOption, AttackTarget, ObjectId } from '@/protocol/generated'
import { attacksAnswer, useBlockPreview } from '../../combat/forecast'
import { usePublishPending } from '../../combat/pending'
import { usePreviewRequest } from '../../combat/usePreviewRequest'
import { attackSummary, distinctTargets, sameTarget, targetKey, uniqueNames } from '../combatText'
import { markCards, useCardMarks, useCardNotes } from '../marks'
import type { CardNotes } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { PreviewOutcome } from '../PreviewOutcome'
import type { PromptComponent } from '../types'
import { useObjects, useTargetNames } from '../useCombatNames'

/** The attacker's defender that is `aim`, else its first defender; undefined for an attacker with none. */
function aimedAt(option: AttackerOption, aim: AttackTarget | undefined): AttackTarget | undefined {
  return (
    option.defenders.find((target) => aim !== undefined && sameTarget(target, aim)) ?? option.defenders[0]
  )
}

/**
 * DeclareAttackers: creatures that may attack are legal, the rest of the hand and battlefield dimmed. A click, Enter
 * or Space puts a creature in the attack or takes it out; with several defenders a creature goes where "Attack at"
 * says and can be re-aimed in the list under the buttons. Confirm sends the attack, No attack an empty one.
 */
export const DeclareAttackersPrompt: PromptComponent<'DeclareAttackers'> = ({ options }) => {
  const { id, answer, selection } = usePrompt()
  const own = useOwnCardIds()
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

  const [aimKey, setAimKey] = useState<string | null>(null)
  const aim = defenders.find((target) => targetKey(target) === aimKey) ?? defenders[0]
  const [targets, setTargets] = useState<ReadonlyMap<ObjectId, AttackTarget>>(new Map())

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
  const creatures = useObjects(declaredIds)
  const labels = useMemo(
    () => uniqueNames(declaredIds.map((id, index) => ({ id, name: creatures[index]?.name ?? 'a creature' }))),
    [declaredIds, creatures],
  )

  const marks = useMemo(() => markCards(own, legal, declaredIds), [own, legal, declaredIds])
  const notes = useMemo<CardNotes>(
    () =>
      new Map(
        declared.map(({ object, target }) => [
          object,
          { label: `At ${names.short(target)}`, words: `attacking ${names.long(target)}` },
        ]),
      ),
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
  const aimOne = (id: ObjectId, key: string): void => {
    const target = offered.get(id)?.defenders.find((candidate) => targetKey(candidate) === key)
    if (target !== undefined) setTargets((current) => new Map(current).set(id, target))
  }

  const text =
    legal.length === 0
      ? 'None of your creatures can attack. Choose No attack to go on.'
      : declared.length > 0
        ? `${attackSummary(
            declared.map((entry) => entry.target),
            names,
          )}. Confirm the attack, or keep choosing.`
        : several
          ? 'Choose the creatures that attack and who each one attacks, or choose No attack.'
          : `Choose the creatures that attack${aim === undefined ? '' : ` ${names.short(aim)}`}, or choose No attack.`

  return (
    <PromptLayout text={text}>
      {options.most === undefined ? null : (
        <p data-slot="declaration-limit" className="basis-full text-sm text-muted-foreground">
          No more than {options.most} {options.most === 1 ? 'creature' : 'creatures'} can attack this combat.
        </p>
      )}
      {(options.most_at ?? []).map(([target, most]) => (
        <p
          key={JSON.stringify(target)}
          data-slot="defender-limit"
          className="basis-full text-sm text-muted-foreground"
        >
          No more than {most} {most === 1 ? 'creature' : 'creatures'} can attack {names.short(target)} this
          combat.
        </p>
      ))}
      {several && aim !== undefined ? (
        <label className="flex items-center gap-2 text-sm">
          Attack at
          <NativeSelect
            aria-label="Attack at"
            value={targetKey(aim)}
            onChange={(event) => setAimKey(event.target.value)}
          >
            {defenders.map((target) => (
              <NativeSelectOption key={targetKey(target)} value={targetKey(target)}>
                {names.long(target)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </label>
      ) : null}
      <Button variant="outline" className="h-10 px-4" disabled={legal.length === 0} onClick={attackWithAll}>
        Attack with all
      </Button>
      {several && aim !== undefined && declared.length > 0 ? (
        <Button variant="outline" className="h-10 px-4" onClick={allAt}>
          All at {names.short(aim)}
        </Button>
      ) : null}
      <Button
        variant="outline"
        className="h-10 px-4"
        disabled={declared.length === 0}
        onClick={selection.clear}
      >
        Clear
      </Button>
      <Button
        className="h-10 px-4"
        disabled={declared.length === 0}
        onClick={() => answer({ DeclareAttackers: declared.map(({ object, target }) => [object, target]) })}
      >
        Confirm attack
      </Button>
      <Button variant="outline" className="h-10 px-4" onClick={() => answer({ DeclareAttackers: [] })}>
        No attack
      </Button>
      {several && declared.length > 0 ? (
        <ul
          aria-label="Who each creature attacks"
          data-slot="attack-targets"
          className="grid max-h-44 basis-full gap-1 overflow-y-auto"
        >
          {declared.map(({ object, target }) => {
            const name = labels.get(object) ?? 'a creature'
            return (
              <li key={object} className="flex items-center justify-between gap-2 text-sm">
                <span className="min-w-0 truncate">{name}</span>
                <NativeSelect
                  size="sm"
                  className="max-w-[60%]"
                  aria-label={`Who ${name} attacks`}
                  value={targetKey(target)}
                  onChange={(event) => aimOne(object, event.target.value)}
                >
                  {(offered.get(object)?.defenders ?? []).map((candidate) => (
                    <NativeSelectOption key={targetKey(candidate)} value={targetKey(candidate)}>
                      {names.long(candidate)}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </li>
            )
          })}
        </ul>
      ) : null}
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
