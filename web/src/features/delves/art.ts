/** The ground of a deck or boss without a picture: a gradient in its colours. */
import { cn } from '@/lib/utils'

// Whole class names, so Tailwind can see them.
const FROM: Record<string, string> = {
  W: 'from-mana-white',
  U: 'from-mana-blue',
  B: 'from-mana-black',
  R: 'from-mana-red',
  G: 'from-mana-green',
}
const TO: Record<string, string> = {
  W: 'to-mana-white',
  U: 'to-mana-blue',
  B: 'to-mana-black',
  R: 'to-mana-red',
  G: 'to-mana-green',
}

/** The gradient classes of a set of colours: first to last, colourless grey, several colours over gold. */
export function colourGround(colours: string): string {
  const letters = [...colours].filter((c) => c in FROM)
  const first = letters[0]
  const last = letters[letters.length - 1]
  if (first === undefined || last === undefined) return 'from-mana-colorless to-mana-colorless'
  if (letters.length === 1) return cn(FROM[first], 'to-mana-gold')
  return cn(FROM[first], TO[last])
}
