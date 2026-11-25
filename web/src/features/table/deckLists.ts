/** The seat lists of `DeckPictures` as the deck list dialog shows them: grouped, counted and in words (pure). */
import type {
  CardColor,
  Color,
  ListCard,
  ListGroup,
  PlayerId,
  SeatList,
  TokenPicture,
} from '@/protocol/generated'
import type { ImageSource } from '@/features/cards/images'

/** The groups in the order a list shows them, each with its heading for one card and for several. */
export const GROUPS: ReadonlyArray<{ group: ListGroup; one: string; many: string }> = [
  { group: 'Commander', one: 'Commander', many: 'Commanders' },
  { group: 'Companion', one: 'Companion', many: 'Companion' },
  { group: 'Creature', one: 'Creature', many: 'Creatures' },
  { group: 'Planeswalker', one: 'Planeswalker', many: 'Planeswalkers' },
  { group: 'Battle', one: 'Battle', many: 'Battles' },
  { group: 'Instant', one: 'Instant', many: 'Instants' },
  { group: 'Sorcery', one: 'Sorcery', many: 'Sorceries' },
  { group: 'Artifact', one: 'Artifact', many: 'Artifacts' },
  { group: 'Enchantment', one: 'Enchantment', many: 'Enchantments' },
  { group: 'Land', one: 'Land', many: 'Lands' },
  { group: 'Other', one: 'Other', many: 'Other' },
]

/** One group of a list: its heading, the copies it holds and its lines. */
export interface ListSection {
  group: ListGroup
  heading: string
  copies: number
  cards: ListCard[]
}

/** The cards of a list by group in `GROUPS` order, empty groups left out; the lines keep the server's order. */
export function listSections(cards: readonly ListCard[]): ListSection[] {
  return GROUPS.flatMap(({ group, one, many }) => {
    const lines = cards.filter((card) => card.group === group)
    if (lines.length === 0) return []
    const copies = lines.reduce((sum, card) => sum + card.count, 0)
    return [{ group, heading: copies === 1 ? one : many, copies, cards: lines }]
  })
}

/** Every copy a list holds. */
export const listCopies = (list: SeatList): number => list.cards.reduce((sum, card) => sum + card.count, 0)

/** The list of `seat`, if the server told one. */
export const listOf = (lists: readonly SeatList[], seat: PlayerId): SeatList | null =>
  lists.find((list) => list.seat === seat) ?? null

/** Whether a seat's list has anything the dialog would show beyond what its pod already does: a whole list or a token. */
export const hasMore = (list: SeatList | null): list is SeatList =>
  list !== null && (list.whole || list.tokens.length > 0)

/** The sentence under the dialog's title: how many cards it lists, and what of a hidden deck stays hidden. */
export function listWords(list: SeatList, owner: string, own: boolean): string {
  const copies = listCopies(list)
  const cards = `${copies} ${copies === 1 ? 'card' : 'cards'}`
  if (!list.whole) return `${owner}’s list is hidden: only the command zone is public.`
  return own ? `${cards}, the list you play.` : `${cards}, a public list.`
}

const LETTER: Record<Color, CardColor> = { White: 'W', Blue: 'U', Black: 'B', Red: 'R', Green: 'G' }

/** The type line of a token without "Token", as its picture is found by ("Creature — Goblin"). */
export function tokenType(token: TokenPicture): string {
  const kinds = [...(token.supertypes ?? []), ...token.types].join(' ')
  const subtypes = token.subtypes ?? []
  return subtypes.length === 0 ? kinds : `${kinds} — ${subtypes.join(' ')}`
}

/** The picture source of a token a deck can make, found as the table finds a token on the battlefield. */
export function tokenSource(token: TokenPicture): ImageSource {
  return {
    kind: 'token',
    name: token.name,
    type: tokenType(token),
    colors: (token.colors ?? []).map((color) => LETTER[color]),
    power: token.power ?? null,
    toughness: token.toughness ?? null,
  }
}

/** A token's name with its power and toughness, as its tile says it ("Goblin 1/1"). */
export function tokenLabel(token: TokenPicture): string {
  const stats =
    token.power === undefined || token.toughness === undefined ? '' : ` ${token.power}/${token.toughness}`
  return `${token.name}${stats}`
}
