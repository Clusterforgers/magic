import { useMemo, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ManaOption, ObjectId } from '@/protocol/generated'
import { useSeatNamer } from '../../names'
import type { MenuTarget } from '../../manual/menuStore'
import { ManaChoice } from '../ManaChoice'
import { handNotes, inactiveNotes } from '../../handReason'
import { BringCompanion } from '../BringCompanion'
import { AbilityChooser } from '../AbilityChooser'
import { abilityPlays, activationNotes, blockedPlays, mergeNotes, playsOf } from '../activations'
import type { Play } from '../activations'
import { markCards, NO_NOTES, useCardDrops, useCardMarks, useCardNotes } from '../marks'
import { undoableSources, undoNotes } from '../manaUndo'
import type { CardNotes } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { PassControls } from '../PassControls'
import { SpiritPanel } from '../../campaign/SpiritPanel'
import { spiritEmblem } from '../../campaign/haunt'
import { PromptLayout } from '../PromptLayout'
import { usePrompt } from '../PromptContext'
import { manaByPermanent, priorityText, topOfStack } from '../priorityText'
import type { PromptComponent } from '../types'

const focusCard = (id: ObjectId): void =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)?.focus()

/** The card button a click came from, else the first button the table draws for the card. */
const elementOf = (id: ObjectId, from: HTMLElement | undefined): HTMLElement | null =>
  from ?? document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)

/**
 * Priority: lands you may play, spells you may cast, mana abilities you may use and abilities you may activate (on a
 * permanent, or on a card of your graveyard, exile or command zone) are legal, the rest of your hand and battlefield
 * is dimmed. A click plays, casts, taps or activates, and a second press on a source tapped for mana just now untaps it
 * (`PriorityOptions.undo_mana`; one that cannot be says why on its foot); a card with an ability (or a mana ability of the hand) and another
 * play asks which first, and a permanent with several mana abilities and nothing else asks which mana.
 */
export const PriorityPrompt: PromptComponent<'Priority'> = ({ options }) => {
  const { decision, answer, selection } = usePrompt()
  const own = useOwnCardIds()
  const top = usePlay(useShallow(topOfStack))
  const name = useSeatNamer()
  const seat = usePlay((state) => state.seat)
  const view = usePlay((state) => state.view)
  const byHand = usePlay((state) => state.byHand)
  const objectsById = usePlay((state) => state.objectsById)
  const mana = useMemo(() => manaByPermanent(options.mana_abilities), [options.mana_abilities])
  const abilities = useMemo(() => abilityPlays(options, objectsById, seat), [options, objectsById, seat])
  const undoable = useMemo(() => undoableSources(options.undo_mana), [options.undo_mana])
  const ownSeat = view?.players.find((player) => player.seat === decision.player)
  const [choosing] = selection.ids
  const choices = choosing === undefined ? undefined : mana.get(choosing)
  const [asking, setAsking] = useState<MenuTarget | null>(null)
  const playsFor = (id: ObjectId): readonly Play[] =>
    playsOf(id, objectsById[id]?.name ?? 'the card', options, abilities, mana.get(id) ?? [], objectsById[id])

  const marks = useMemo(
    () =>
      markCards(
        own,
        [...options.playable_lands, ...options.castable, ...mana.keys(), ...abilities.keys(), ...undoable],
        choosing === undefined ? [] : [choosing],
      ),
    [own, options.playable_lands, options.castable, mana, abilities, undoable, choosing],
  )

  /** Plays the card as a land or casts it; false when it is neither now. A click with nothing else to offer and a drop on the table both end here. */
  const playCard = (id: ObjectId): boolean => {
    const land = options.playable_lands.includes(id)
    if (!land && !options.castable.includes(id)) return false
    answer({ Priority: land ? { PlayLand: id } : { CastSpell: id } })
    return true
  }
  const droppable = useMemo(
    () => new Set([...options.playable_lands, ...options.castable]),
    [options.playable_lands, options.castable],
  )
  // A drop on the table only plays or casts: an ability of the card (cycling, channel) is a click's to offer.
  useCardDrops(droppable, playCard)

  const act = (id: ObjectId, from?: HTMLElement): void => {
    // A source tapped for mana just now is pressed again: its mana ability is undone, as a payment's pressed source is.
    if (undoable.includes(id)) {
      answer({ Priority: { UndoMana: id } })
      return
    }
    // A card of the hand that makes mana and can be cast (a Spirit Guide) asks which, as a card with an ability does.
    const playable = options.castable.includes(id) || options.playable_lands.includes(id)
    if (abilities.has(id) || (playable && mana.has(id))) {
      const all = playsFor(id)
      const [only] = all
      if (all.length === 1 && only !== undefined) answer({ Priority: only.action })
      else if (asking?.id === id) setAsking(null)
      else {
        const element = elementOf(id, from)
        if (element === null) return
        const { left, bottom } = element.getBoundingClientRect()
        setAsking({ id, x: left, y: bottom, opener: element })
      }
      return
    }
    if (playCard(id)) return
    const manaAbilities = mana.get(id)
    const only = manaAbilities?.at(0)
    if (manaAbilities === undefined || only === undefined) return
    if (manaAbilities.length === 1) answer({ Priority: { ActivateMana: only.source } })
    else if (selection.has(id)) selection.clear()
    else selection.set([id])
  }
  useCardMarks(marks, act)
  const notes = useMemo<CardNotes>(() => {
    const hand = view === null ? NO_NOTES : handNotes(view, objectsById, options)
    const merged = mergeNotes(
      new Map([...inactiveNotes(objectsById, options), ...hand]),
      activationNotes(abilities),
    )
    return new Map([...merged, ...undoNotes(options.undo_mana, objectsById)])
  }, [view, objectsById, options, abilities])
  useCardNotes(notes)

  const pick = (option: ManaOption): void => {
    answer({ Priority: { ActivateMana: option.source } })
  }
  const cancel = (): void => {
    selection.clear()
    if (choosing !== undefined) focusCard(choosing)
  }
  const choose = (play: Play): void => {
    setAsking(null)
    answer({ Priority: play.action })
  }

  const manaChoice = choosing !== undefined && choices !== undefined && choices.length > 1
  return (
    <PromptLayout
      text={priorityText(options, top, decision.player, name, byHand)}
      inline={ownSeat?.spirit !== true}
      choices={
        manaChoice || ownSeat?.spirit === true ? (
          <>
            {choosing !== undefined && choices !== undefined && choices.length > 1 ? (
              <ManaChoice card={choosing} options={choices} onPick={pick} onCancel={cancel} />
            ) : null}
            {ownSeat?.spirit === true ? (
              <SpiritPanel
                player={ownSeat}
                emblem={spiritEmblem(ownSeat, objectsById)}
                options={options}
                onActivate={(ability) => answer({ Priority: { Activate: ability } })}
              />
            ) : null}
          </>
        ) : undefined
      }
    >
      {asking === null ? null : (
        <AbilityChooser
          target={asking}
          plays={playsFor(asking.id)}
          blocked={blockedPlays(asking.id, options, objectsById)}
          onPick={choose}
          onClose={() => setAsking(null)}
        />
      )}
      <BringCompanion offered={options.companion === true} />
      <PassControls canPass={options.can_pass} />
    </PromptLayout>
  )
}
