/** Colour vision for tests: how a person who lacks one kind of cone sees an sRGB colour, and how far apart two colours look. */
import type { Rgb } from './color'

/** The dichromacies: no long-wave (protan), medium-wave (deutan) or short-wave (tritan) cones. */
export type Dichromacy = 'protanopia' | 'deuteranopia' | 'tritanopia'

/** The dichromacies, in the order the simulations are listed. */
export const DICHROMACIES: readonly Dichromacy[] = ['protanopia', 'deuteranopia', 'tritanopia']

type Matrix = readonly [Rgb, Rgb, Rgb]

// Machado, Oliveira and Fernandes (2009), full severity, applied to linear RGB.
const MACHADO: Record<Dichromacy, Matrix> = {
  protanopia: [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
  ],
  deuteranopia: [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.01182, 0.04294, 0.968881],
  ],
  tritanopia: [
    [1.255528, -0.076749, -0.178779],
    [-0.078411, 0.930809, 0.147602],
    [0.004733, 0.691367, 0.3039],
  ],
}

const decode = (channel: number): number =>
  channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
const encode = (linear: number): number =>
  linear <= 0.0031308 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055
const clamp = (value: number): number => Math.min(1, Math.max(0, value))

/** The sRGB colour a person with `kind` of dichromacy sees in place of `rgb`; the colours of a screen are cut to 0..1. */
export function simulate(rgb: Rgb, kind: Dichromacy): Rgb {
  const linear = rgb.map(decode)
  const [r = 0, g = 0, b = 0] = MACHADO[kind].map((row) =>
    clamp(row[0] * (linear[0] ?? 0) + row[1] * (linear[1] ?? 0) + row[2] * (linear[2] ?? 0)),
  )
  return [encode(r), encode(g), encode(b)]
}

/** A colour in CIELAB under D65: lightness, then the green-red and blue-yellow axes. */
export type Lab = readonly [number, number, number]

/** The CIELAB (D65) colour of a gamma-encoded sRGB colour. */
export function labOf(rgb: Rgb): Lab {
  const [r = 0, g = 0, b = 0] = rgb.map(decode)
  const x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047
  const y = 0.2126729 * r + 0.7151522 * g + 0.072175 * b
  const z = (0.0193339 * r + 0.119192 * g + 0.9503041 * b) / 1.08883
  const f = (t: number): number => (t > (6 / 29) ** 3 ? Math.cbrt(t) : t / (3 * (6 / 29) ** 2) + 4 / 29)
  return [116 * f(y) - 16, 500 * (f(x) - f(y)), 200 * (f(y) - f(z))]
}

const rad = (degrees: number): number => (degrees * Math.PI) / 180
const hueOf = (b: number, a: number): number => {
  if (a === 0 && b === 0) return 0
  const degrees = (Math.atan2(b, a) * 180) / Math.PI
  return degrees < 0 ? degrees + 360 : degrees
}

/** The CIEDE2000 colour difference of two CIELAB colours (Sharma, Wu and Dalal 2005), with unit weights. */
export function deltaE2000Lab(first: Lab, second: Lab): number {
  const [l1, a1, b1] = first
  const [l2, a2, b2] = second
  const chromaMean = (Math.hypot(a1, b1) + Math.hypot(a2, b2)) / 2
  const g = 0.5 * (1 - Math.sqrt(chromaMean ** 7 / (chromaMean ** 7 + 25 ** 7)))
  const a1p = (1 + g) * a1
  const a2p = (1 + g) * a2
  const c1 = Math.hypot(a1p, b1)
  const c2 = Math.hypot(a2p, b2)
  const h1 = hueOf(b1, a1p)
  const h2 = hueOf(b2, a2p)
  const dL = l2 - l1
  const dC = c2 - c1
  let dh = 0
  if (c1 * c2 !== 0) {
    dh = h2 - h1
    if (dh > 180) dh -= 360
    else if (dh < -180) dh += 360
  }
  const dH = 2 * Math.sqrt(c1 * c2) * Math.sin(rad(dh / 2))
  const lMean = (l1 + l2) / 2
  const cMean = (c1 + c2) / 2
  let hMean = h1 + h2
  if (c1 * c2 !== 0) {
    if (Math.abs(h1 - h2) <= 180) hMean /= 2
    else hMean = (h1 + h2 < 360 ? hMean + 360 : hMean - 360) / 2
  }
  const t =
    1 -
    0.17 * Math.cos(rad(hMean - 30)) +
    0.24 * Math.cos(rad(2 * hMean)) +
    0.32 * Math.cos(rad(3 * hMean + 6)) -
    0.2 * Math.cos(rad(4 * hMean - 63))
  const rc = 2 * Math.sqrt(cMean ** 7 / (cMean ** 7 + 25 ** 7))
  const sl = 1 + (0.015 * (lMean - 50) ** 2) / Math.sqrt(20 + (lMean - 50) ** 2)
  const sc = 1 + 0.045 * cMean
  const sh = 1 + 0.015 * cMean * t
  const rt = -Math.sin(rad(2 * 30 * Math.exp(-(((hMean - 275) / 25) ** 2)))) * rc
  const lTerm = dL / sl
  const cTerm = dC / sc
  const hTerm = dH / sh
  return Math.sqrt(lTerm ** 2 + cTerm ** 2 + hTerm ** 2 + rt * cTerm * hTerm)
}

/** How far apart two sRGB colours look, as a CIEDE2000 difference: about 1 is the least a person can see, 10 is plain. */
export function colourDifference(first: Rgb, second: Rgb): number {
  return deltaE2000Lab(labOf(first), labOf(second))
}
