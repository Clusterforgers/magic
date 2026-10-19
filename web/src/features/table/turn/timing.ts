/** How long the turn cues last. `turn.css` repeats the banner's fades and `turnCss.test.ts` keeps the two equal. */

/** How long a line of the ticker stays up. */
export const TICKER_LINE_MS = 3000

/** The most lines one batch puts in the ticker. */
export const TICKER_MAX_LINES = 3

/** How long a skipped step is dashed in the ladder before it settles as passed. */
export const SKIPPED_MARK_MS = 1000

/** The three parts of the turn banner: it fades in, is held, and fades out. */
export interface BannerTimes {
  inMs: number
  holdMs: number
  outMs: number
}

/** The banner with motion: a quick fade in, a short hold and a quicker fade out. */
export const BANNER_TIMES: BannerTimes = { inMs: 250, holdMs: 600, outMs: 150 }

/** The banner under reduced motion: a fade only (the design's 100 ms) and a longer hold, so it can be read. */
export const BANNER_TIMES_REDUCED: BannerTimes = { inMs: 100, holdMs: 1500, outMs: 100 }

/** The times of the banner for the person's motion setting, stretched by `slow` (the dev route's `?slow=`). */
export function bannerTimes(reduced: boolean, slow = 1): BannerTimes {
  const base = reduced ? BANNER_TIMES_REDUCED : BANNER_TIMES
  return { inMs: base.inMs * slow, holdMs: base.holdMs * slow, outMs: base.outMs * slow }
}
