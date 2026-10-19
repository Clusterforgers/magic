/** The widths a page takes, narrowest first. */
export const PAGE_WIDTHS = ['narrow', 'page', 'wide'] as const

/**
 * The scale of page widths: `narrow` (28rem) for a form, `page` (48rem) for reading and lists of cards, `wide` (72rem)
 * for a table of rows.
 */
export type PageWidth = (typeof PAGE_WIDTHS)[number]
