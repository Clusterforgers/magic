import type { Frame, Highlight, PipTone } from './cardModel'

/** How big a card face is drawn: compact for crowded tables, large for the preview. */
export type CardSize = 'compact' | 'normal' | 'medium' | 'large'

interface SizeClasses {
  /** Sets the box (`--card-w` by `--card-h`); every size keeps the 5:7 shape of a card. */
  vars: string
  name: string
  text: string
  pip: string
}

// Whole class names, so Tailwind can see them.
export const SIZE: Record<CardSize, SizeClasses> = {
  compact: {
    vars: '[--card-w:3.75rem] [--card-h:5.25rem]',
    name: 'text-[0.62rem]',
    text: 'text-[0.56rem]',
    pip: 'size-3 text-[0.5rem]',
  },
  normal: {
    vars: '[--card-w:6rem] [--card-h:8.4rem]',
    name: 'text-[0.72rem]',
    text: 'text-[0.62rem]',
    pip: 'size-3.5 text-[0.56rem]',
  },
  medium: {
    vars: '[--card-w:10rem] [--card-h:14rem]',
    name: 'text-sm',
    text: 'text-[0.7rem]',
    pip: 'size-4 text-[0.65rem]',
  },
  large: {
    vars: '[--card-w:15rem] [--card-h:21rem]',
    name: 'text-base',
    text: 'text-xs',
    pip: 'size-5 text-xs',
  },
}

/** The border and tint of each frame colour. */
export const FRAME: Record<Frame, string> = {
  white: 'border-mana-white bg-mana-white/25',
  blue: 'border-mana-blue bg-mana-blue/12',
  black: 'border-mana-black bg-mana-black/12',
  red: 'border-mana-red bg-mana-red/12',
  green: 'border-mana-green bg-mana-green/12',
  gold: 'border-mana-gold bg-mana-gold/15',
  colorless: 'border-mana-colorless bg-mana-colorless/12',
}

/** The fill of a mana symbol. */
export const PIP: Record<PipTone, string> = {
  white: 'bg-mana-white text-foreground',
  blue: 'bg-mana-blue text-white',
  black: 'bg-mana-black text-white',
  red: 'bg-mana-red text-white',
  green: 'bg-mana-green text-white',
  gold: 'bg-mana-gold text-foreground',
  colorless: 'bg-mana-colorless text-foreground',
  generic: 'bg-muted text-foreground',
}

/** How the decision layer's marks look on a card. */
export const HIGHLIGHT: Record<Highlight, string> = {
  legal: 'ring-2 ring-primary',
  selected: 'ring-4 ring-primary',
  dimmed: 'opacity-40 saturate-50',
}
