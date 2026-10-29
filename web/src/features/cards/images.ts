/**
 * Where the picture of a card comes from: the server's picture endpoints (`docs/server.md`, Pictures), never Scryfall's
 * own links, and which size a drawn width needs.
 */
import type { CardColor, CardIdentity, Color, ObjectView } from '@/protocol/generated'

/** A size the picture endpoints serve. */
export type ImageSize = 'small' | 'normal' | 'large' | 'art_crop' | 'png'

/** A token is found by the facts its view shows: name, type line, colours and power/toughness. */
export interface TokenFacts {
  name: string
  /** The type line without "Token", such as "Creature — Soldier". */
  type: string
  colors: readonly CardColor[]
  power?: number | null
  toughness?: number | null
}

/** What a picture is of: a printed card (by printing or oracle id) or a token. */
export type ImageSource = { kind: 'card'; identity: CardIdentity } | ({ kind: 'token' } & TokenFacts)

/** Below this drawn width (px) the `small` picture (146 px wide) is sharp enough. */
export const SMALL_BELOW = 110
/** Below this drawn width (px) the `normal` picture (488 px wide) is sharp enough; above it `large`. */
export const NORMAL_BELOW = 330

/** The picture size for a card drawn this many CSS pixels wide; the margin covers a screen of twice the density. */
export function imageSizeFor(width: number): 'small' | 'normal' | 'large' {
  if (width < SMALL_BELOW) return 'small'
  return width < NORMAL_BELOW ? 'normal' : 'large'
}

const enc = encodeURIComponent

/** The URL of one face (0 for the front) of a picture source at a size. */
export function cardImageUrl(source: ImageSource, face: number, size: ImageSize): string {
  const at = Math.max(0, Math.trunc(face))
  if (source.kind === 'card') {
    const { printing_id: printing, oracle_id: oracle } = source.identity
    return printing === undefined
      ? `/api/images/oracle/${enc(oracle)}/${at}/${size}`
      : `/api/images/printing/${enc(printing)}/${at}/${size}`
  }
  const query = new URLSearchParams({ name: source.name, type: source.type })
  query.set('colors', source.colors.length === 0 ? 'C' : source.colors.join(''))
  if (source.power !== undefined && source.power !== null) query.set('power', String(source.power))
  if (source.toughness !== undefined && source.toughness !== null)
    query.set('toughness', String(source.toughness))
  return `/api/images/token/${at}/${size}?${query.toString()}`
}

/**
 * The picture asked for when a printing's own is missing: the card's (its oracle id), since a database without that
 * printing still has the card. Null for an oracle id or a token, which have nothing to fall back to.
 */
export function cardImageFallbackUrl(source: ImageSource, face: number, size: ImageSize): string | null {
  if (source.kind !== 'card' || source.identity.printing_id === undefined) return null
  return cardImageUrl({ kind: 'card', identity: { oracle_id: source.identity.oracle_id } }, face, size)
}

const LETTER: Record<Color, CardColor> = { White: 'W', Blue: 'U', Black: 'B', Red: 'R', Green: 'G' }

/**
 * The picture source of an object of the table, or null where its back is drawn instead: a face-down or hidden card,
 * an ability, an emblem, and a card the game has no record of.
 */
export function imageSourceOf(object: ObjectView): ImageSource | null {
  if (object.identity !== undefined) return { kind: 'card', identity: object.identity }
  if (object.kind !== 'Token' || object.face_down === true || object.name === '') return null
  const kinds = [...object.supertypes, ...object.types].join(' ')
  return {
    kind: 'token',
    name: object.name,
    type: object.subtypes.length === 0 ? kinds : `${kinds} — ${object.subtypes.join(' ')}`,
    colors: object.colors.map((color) => LETTER[color]),
    power: object.power,
    toughness: object.toughness,
  }
}

/** Whether the table draws a neutral card back for an object: it lies face down or the viewer may not see it. */
export function showsBack(object: ObjectView): boolean {
  return object.identity === undefined && (object.face_down === true || object.name === '')
}

/** The face of a card the object shows: the face in use, else the front. */
export function faceOf(object: ObjectView): number {
  return object.face ?? 0
}
