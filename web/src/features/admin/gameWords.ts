import type { AdminGame } from '@/protocol/generated'

/** How a row names a game: its seats, an open one as "open seat". */
export function gameTitle(game: AdminGame): string {
  return game.seats.map((seat) => (seat.name === '' ? 'open seat' : seat.name)).join(' vs ')
}

const FORMATS: Readonly<Record<string, string>> = { normal: 'Normal', commander: 'Commander' }

/** The format and who may find the game, or that it is a tutorial. */
export function modeOf(game: AdminGame): string {
  const format = FORMATS[game.format] ?? game.format
  if (game.tutorial) return `${format} tutorial`
  return `${format}, ${game.visibility === 'public' ? 'public' : 'invite only'}`
}
