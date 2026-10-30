/** Colour maths for tests: OKLCH in, sRGB out, and the WCAG 2 contrast ratio between two colours. */

/** A colour as CSS writes it: lightness 0 to 1, chroma, hue in degrees, alpha 0 to 1. */
export interface Oklch {
  l: number
  c: number
  h: number
  alpha: number
}

/** Gamma-encoded sRGB, each channel from 0 to 1. */
export type Rgb = readonly [number, number, number]

type Triple = readonly [number, number, number]

/** How a colour outside the sRGB gamut is brought into it: cut off per channel, or by lowering its chroma (CSS Color 4). */
export type Mapping = 'clip' | 'chroma'

const rad = (degrees: number): number => (degrees * Math.PI) / 180

/** Reads `oklch(L C H)` or `oklch(L C H / A)`; L and A may be percentages. Anything else throws. */
export function parseOklch(text: string): Oklch {
  const match = /^oklch\(\s*([\d.]+%?)\s+([\d.]+)\s+([\d.]+)(?:\s*\/\s*([\d.]+%?))?\s*\)$/.exec(text.trim())
  if (match === null) throw new Error(`not an oklch() color: ${text}`)
  const unit = (value: string): number =>
    value.endsWith('%') ? Number.parseFloat(value) / 100 : Number(value)
  return { l: unit(match[1] ?? ''), c: Number(match[2]), h: Number(match[3]), alpha: unit(match[4] ?? '1') }
}

function toOklab({ l, c, h }: Oklch): Triple {
  return [l, c * Math.cos(rad(h)), c * Math.sin(rad(h))]
}

function oklabToLinear([l, a, b]: Triple): Triple {
  const lp = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const mp = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const sp = (l - 0.0894841775 * a - 1.291485548 * b) ** 3
  return [
    4.0767416621 * lp - 3.3077115913 * mp + 0.2309699292 * sp,
    -1.2684380046 * lp + 2.6097574011 * mp - 0.3413193965 * sp,
    -0.0041960863 * lp - 0.7034186147 * mp + 1.707614701 * sp,
  ]
}

function linearToOklab([r, g, b]: Triple): Triple {
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ]
}

const inGamut = (rgb: Triple): boolean => rgb.every((channel) => channel >= -1e-6 && channel <= 1 + 1e-6)
const clip = (rgb: Triple): Triple => [clamp(rgb[0]), clamp(rgb[1]), clamp(rgb[2])]
const clamp = (value: number): number => Math.min(1, Math.max(0, value))

function distance(a: Triple, b: Triple): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2])
}

// CSS Color 4, "gamut mapping to an RGB destination": the largest chroma whose clipped colour is within a just
// noticeable difference of the colour itself.
function chromaReduced(color: Oklch): Triple {
  const JND = 0.02
  const EPSILON = 0.0001
  if (color.l >= 1) return [1, 1, 1]
  if (color.l <= 0) return [0, 0, 0]
  const clippedOf = (chroma: number): { linear: Triple; error: number; inside: boolean } => {
    const lab = toOklab({ ...color, c: chroma })
    const raw = oklabToLinear(lab)
    const linear = clip(raw)
    return { linear, error: distance(linearToOklab(linear), lab), inside: inGamut(raw) }
  }
  const whole = clippedOf(color.c)
  if (whole.inside) return whole.linear
  if (whole.error < JND) return whole.linear
  let low = 0
  let high = color.c
  let lowInside = true
  let best = whole.linear
  while (high - low > EPSILON) {
    const chroma = (low + high) / 2
    const trial = clippedOf(chroma)
    if (lowInside && trial.inside) {
      low = chroma
      best = trial.linear
      continue
    }
    if (trial.error < JND) {
      best = trial.linear
      if (JND - trial.error < EPSILON) return trial.linear
      lowInside = false
      low = chroma
    } else {
      high = chroma
    }
  }
  return best
}

const encode = (linear: number): number =>
  linear <= 0.0031308 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055

/** The sRGB colour a browser paints for `color`, mapped into the gamut the given way. Alpha is not applied. */
export function toSrgb(color: Oklch, mapping: Mapping): Rgb {
  const linear = mapping === 'clip' ? clip(oklabToLinear(toOklab(color))) : chromaReduced(color)
  return [encode(linear[0]), encode(linear[1]), encode(linear[2])]
}

/** `top` painted at `alpha` over `bottom`, mixed in gamma-encoded sRGB as a browser composites. */
export function over(top: Rgb, bottom: Rgb, alpha: number): Rgb {
  return [
    top[0] * alpha + bottom[0] * (1 - alpha),
    top[1] * alpha + bottom[1] * (1 - alpha),
    top[2] * alpha + bottom[2] * (1 - alpha),
  ]
}

const decode = (channel: number): number =>
  channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4

/** WCAG 2 relative luminance. */
export function luminance(rgb: Rgb): number {
  return 0.2126 * decode(rgb[0]) + 0.7152 * decode(rgb[1]) + 0.0722 * decode(rgb[2])
}

/** WCAG 2 contrast ratio, from 1 to 21, whichever colour is lighter. */
export function contrastRatio(a: Rgb, b: Rgb): number {
  const [first, second] = [luminance(a), luminance(b)]
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05)
}
