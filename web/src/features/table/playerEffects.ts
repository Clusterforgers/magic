/**
 * The effects on a player in words (pure): each rule in force on the seat (`PlayerView.effects`) and each emblem in its
 * command zone, as a short label for the pod's chip and a sentence for its hover detail. Whether an effect helps or
 * hinders the seat is a word and an icon on the chip as well as a colour, never the colour alone.
 */
import type { ObjectId, ObjectView, PlayerEffect, PlayerId, PlayerView } from '@/protocol/generated'
import { abilityText } from './cardModel'

/** Whether an effect helps the seat, hinders it, or neither. */
export type EffectTone = 'help' | 'hinder' | 'neutral'

/** The icon of an effect's chip, by kind. */
export type EffectIcon =
  'shield' | 'ban' | 'heart' | 'eye' | 'hand' | 'land' | 'emblem' | 'keyword' | 'spirit' | 'boss'

/** One effect on a seat as its chip shows it. */
export interface EffectFacts {
  /** Unique among the seat's effects. */
  key: string
  icon: EffectIcon
  /** A few words on the chip: "Can't lose", "Hexproof". */
  label: string
  /** The hover detail: what the effect does, a sentence. */
  detail: string
  tone: EffectTone
  /** An emblem's object, which the chip can show. */
  object?: ObjectId
}

type Fixed = Exclude<
  PlayerEffect,
  { LifeFloor: number } | { MaxHandSize: number } | { ExtraLandPlays: number } | { Keyword: string }
>

const FIXED: Record<Fixed, Omit<EffectFacts, 'key' | 'detail'> & { detail: (owner: string) => string }> = {
  CantLose: { icon: 'shield', label: "Can't lose", tone: 'help', detail: (o) => `${o} can't lose the game.` },
  LifeSpared: {
    icon: 'shield',
    label: 'Lives at 0',
    tone: 'help',
    detail: (o) => `${o} doesn't lose the game for having 0 or less life.`,
  },
  CantWin: { icon: 'ban', label: "Can't win", tone: 'hinder', detail: (o) => `${o} can't win the game.` },
  CantCastSpells: {
    icon: 'ban',
    label: 'No spells',
    tone: 'hinder',
    detail: (o) => `${o} can't cast spells now.`,
  },
  CantGainLife: {
    icon: 'heart',
    label: 'No life gain',
    tone: 'hinder',
    detail: (o) => `${o} can't gain life.`,
  },
  LifeCantChange: {
    icon: 'heart',
    label: 'Life fixed',
    tone: 'neutral',
    detail: (o) => `${o}'s life total can't change.`,
  },
  NoLifeLossFromDamage: {
    icon: 'heart',
    label: 'No damage loss',
    tone: 'help',
    detail: (o) => `Damage dealt to ${o} doesn't cause a loss of life.`,
  },
  CantDraw: { icon: 'ban', label: 'No draws', tone: 'hinder', detail: (o) => `${o} can't draw cards.` },
  SkipsDrawStep: {
    icon: 'ban',
    label: 'Skips draw',
    tone: 'hinder',
    detail: (o) => `${o} skips their draw step.`,
  },
  CantSearch: {
    icon: 'ban',
    label: 'No searches',
    tone: 'hinder',
    detail: (o) => `${o} can't search libraries.`,
  },
  SkipsExtraTurns: {
    icon: 'ban',
    label: 'No extra turns',
    tone: 'hinder',
    detail: (o) => `${o} skips each extra turn they would take.`,
  },
  NoMaxHandSize: {
    icon: 'hand',
    label: 'No hand limit',
    tone: 'help',
    detail: (o) => `${o} has no maximum hand size.`,
  },
  HandRevealed: {
    icon: 'eye',
    label: 'Hand revealed',
    tone: 'hinder',
    detail: (o) => `${o} plays with their hand revealed.`,
  },
  TopRevealed: {
    icon: 'eye',
    label: 'Top revealed',
    tone: 'neutral',
    detail: (o) => `${o} plays with the top card of their library revealed.`,
  },
  LooksAtTop: {
    icon: 'eye',
    label: 'Sees top card',
    tone: 'help',
    detail: (o) => `${o} may look at the top card of their library any time.`,
  },
}

/** The facts of one rule on the seat `owner`. */
export function effectFacts(effect: PlayerEffect, owner: string): EffectFacts {
  if (typeof effect === 'string') {
    const fixed = FIXED[effect]
    return { key: effect, ...fixed, detail: fixed.detail(owner) }
  }
  if ('LifeFloor' in effect) {
    const floor = effect.LifeFloor
    return {
      key: 'LifeFloor',
      icon: 'heart',
      label: `Life ≥ ${floor}`,
      tone: 'help',
      detail: `Damage can't take ${owner}'s life total below ${floor}.`,
    }
  }
  if ('MaxHandSize' in effect) {
    const max = effect.MaxHandSize
    return {
      key: 'MaxHandSize',
      icon: 'hand',
      label: `Hand max ${max}`,
      tone: 'neutral',
      detail: `${owner}'s maximum hand size is ${max}.`,
    }
  }
  if ('ExtraLandPlays' in effect) {
    const more = effect.ExtraLandPlays
    return {
      key: 'ExtraLandPlays',
      icon: 'land',
      label: `+${more} ${more === 1 ? 'land' : 'lands'}`,
      tone: 'help',
      detail: `${owner} may play ${more} additional ${more === 1 ? 'land' : 'lands'} each turn.`,
    }
  }
  const word = effect.Keyword
  return {
    key: `Keyword:${word}`,
    icon: 'keyword',
    label: word,
    tone: 'help',
    detail: `${owner} has ${word.toLowerCase()}.`,
  }
}

/** The emblems in the seat's command zone, as chips that say their rules. */
export function emblemFacts(
  player: PlayerView,
  objects: Readonly<Record<ObjectId, ObjectView>>,
  owner: string,
): EffectFacts[] {
  return (player.command.cards ?? []).flatMap((id) => {
    const object = objects[id]
    if (object?.kind !== 'Emblem') return []
    const rules = object.abilities.map(abilityText).join(' ')
    return [
      {
        key: `Emblem:${id}`,
        icon: 'emblem',
        label: object.name === '' ? 'Emblem' : object.name,
        tone: 'neutral',
        detail: rules === '' ? `${owner} has an emblem.` : `${owner}'s emblem: ${rules}`,
        object: id,
      },
    ]
  })
}

/**
 * A campaign fight's seat states: a hero who became a Spirit, a boss's phase with the one still to come, the Omen (the
 * scheme it sets in motion next) and the hero a seat of the boss's side eyes under the spotlight.
 */
export function campaignFacts(
  player: PlayerView,
  owner: string,
  nameOf: (seat: PlayerId) => string = (seat) => `Seat ${seat}`,
): EffectFacts[] {
  const facts: EffectFacts[] = []
  if (player.boss && player.boss.phases.length > 0) {
    const { phases, begun } = player.boss
    const next = phases[begun]
    facts.push({
      key: 'Boss',
      icon: 'boss',
      label: `Phase ${begun + 1} of ${phases.length + 1}`,
      tone: 'neutral',
      detail:
        next === undefined
          ? `${owner} is in their last phase.`
          : `${owner} is in phase ${begun + 1}; next comes ${next} as their life drops.`,
    })
  }
  const scheme = player.boss?.scheme
  if (scheme !== undefined) {
    facts.push({
      key: 'Scheme',
      icon: 'boss',
      label: `Scheme: ${scheme}`,
      tone: 'neutral',
      detail: `${owner} set ${scheme} in motion this turn.`,
    })
  }
  const omen = player.boss?.omen
  if (omen !== undefined) {
    facts.push({
      key: 'Omen',
      icon: 'boss',
      label: `Omen: ${omen}`,
      tone: 'neutral',
      detail: `${owner} sets ${omen} in motion as their next turn begins.`,
    })
  }
  if (player.eyes !== undefined) {
    const hero = nameOf(player.eyes)
    facts.push({
      key: 'Eyes',
      icon: 'boss',
      label: `Eyes ${hero}`,
      tone: 'neutral',
      detail: `${owner} eyes ${hero}: the hero it presses next. Its attention moves on to the others once a hero has taken their share.`,
    })
  }
  if (player.spirit) {
    facts.push({
      key: 'Spirit',
      icon: 'spirit',
      label: 'Spirit',
      tone: 'neutral',
      detail: `${owner} fell and haunts the fight as a Spirit: enemies can't attack or target them, and they spend ectoplasm on haunt actions or save 5 to rise again. The party loses only if every hero falls.`,
    })
  }
  return facts
}

/** Every effect on the seat: the campaign states, the emblems, then the rules in force. */
export function seatEffects(
  player: PlayerView,
  objects: Readonly<Record<ObjectId, ObjectView>>,
  owner: string,
  nameOf?: (seat: PlayerId) => string,
): EffectFacts[] {
  return [
    ...campaignFacts(player, owner, nameOf),
    ...emblemFacts(player, objects, owner),
    ...(player.effects ?? []).map((effect) => effectFacts(effect, owner)),
  ]
}

/**
 * A boss's effects less those the abilities bar over its side already shows: its phase, scheme and Omen and its
 * emblems, which are its rules.
 */
export function withoutBossBar(effects: readonly EffectFacts[]): EffectFacts[] {
  return effects.filter(
    (effect) => !['Boss', 'Scheme', 'Omen'].includes(effect.key) && !effect.key.startsWith('Emblem:'),
  )
}

/** The name of the chip group: "Effects on Bo: Can't lose, Hexproof". */
export function effectsName(owner: string, effects: readonly EffectFacts[]): string {
  return `Effects on ${owner}: ${effects.map((effect) => effect.label).join(', ')}`
}
