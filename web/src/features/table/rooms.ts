/**
 * Rooms (CR 709.5): a card with two doors, each one half of the card, locked or unlocked. The server says which doors
 * are unlocked (`PermanentView.doors`); the names of the halves come from the card's printed faces, and from the
 * object's own name while the faces are not known.
 */
import type { ObjectView } from '@/protocol/generated'

/** One door of a Room permanent. */
export interface Door {
  /** The name of the half of the card. */
  name: string
  unlocked: boolean
}

/** Both doors of a Room, in the order of its card. */
export type Doors = readonly [Door, Door]

/** The separator the server puts between the names of a whole card's faces. */
const WHOLE = ' // '

/** What a door is called while its card's faces are unknown and the object's name does not tell. */
const PLACE = ['First door', 'Second door'] as const

/**
 * The two doors of a Room named `name` with these doors unlocked. The printed faces name them (`first` and `second`);
 * without them the name of the object does: both halves of the whole card, or the one door that is unlocked. A door
 * nothing names is "First door" or "Second door".
 */
export function doorsNamed(
  name: string,
  unlocked: readonly [boolean, boolean],
  first?: string,
  second?: string,
): Doors {
  const [a, b] = unlocked
  const [one, two] = name.split(WHOLE)
  const named =
    first !== undefined && second !== undefined
      ? [first, second]
      : one !== undefined && two !== undefined
        ? [one, two]
        : [a && !b && name !== '' ? name : PLACE[0], b && !a && name !== '' ? name : PLACE[1]]
  return [
    { name: named[0] ?? PLACE[0], unlocked: a },
    { name: named[1] ?? PLACE[1], unlocked: b },
  ]
}

/**
 * What a face titles a Room permanent: its name, or, with both doors locked (a Room then has no name, CR 709.5c), the
 * names of the doors as the whole card has them, "Room" while the faces are not known.
 */
export function roomTitle(name: string, doors: Doors | undefined): string {
  if (name !== '' || doors === undefined) return name
  return doors.every((door) => PLACE.some((place) => place === door.name))
    ? 'Room'
    : doors.map((door) => door.name).join(WHOLE)
}

/** The doors of a Room permanent, or undefined for any other object; `faces` are the names of the card's printed faces. */
export function doorsOf(object: ObjectView, faces?: readonly string[]): Doors | undefined {
  const state = object.permanent?.doors
  return state === undefined ? undefined : doorsNamed(object.name, state, faces?.[0], faces?.[1])
}

/** One door in words: "Grand Entryway unlocked". */
export const doorWord = (door: Door): string => `${door.name} ${door.unlocked ? 'unlocked' : 'locked'}`

/** Both doors in words for a screen reader: "Room, Grand Entryway unlocked, Elegant Rotunda locked". */
export const doorWords = (doors: Doors): string => `Room, ${doors.map(doorWord).join(', ')}`
