export type ObjectId = number
export type PlayerId = number

export type Zone = 'hand' | 'battlefield'

/** A game object as the client sees it; derived values come from the server. */
export interface GameObject {
  id: ObjectId
  name: string
  controller: PlayerId
  zone: Zone
  tapped: boolean
  attacking: boolean
  power: number
  toughness: number
  damage: number
  /** Server says this object can be played or activated right now. */
  playable: boolean
}

/** Partial update for one object, as sent by the server. */
export interface ObjectPatch {
  id: ObjectId
  changes: Partial<Omit<GameObject, 'id'>>
}

/** A pending choice of targets with the server-computed legal set. */
export interface TargetDecision {
  source: ObjectId
  legal: ReadonlySet<ObjectId>
}
