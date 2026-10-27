// Stretches the clock budgets and timeouts of tests by the machine's load, so a shared busy machine slows them without failing them.
import { availableParallelism, loadavg } from 'node:os'

/** The most a budget is stretched, however busy the machine. */
export const MAX_LOAD_FACTOR = 12

/** What the factor is computed from: the two switches and the measured load. */
export interface LoadInput {
  /** `MTG_STRICT_PERF`: `1` keeps every budget at its own number. */
  strict?: string
  /** `MTG_LOAD_FACTOR`: a factor that replaces the measured one. */
  override?: string
  /** The one-minute load average. */
  load: number
  /** The CPUs the process may use. */
  cores: number
}

/** The factor for an input: 1 when strict, else the override or `1 + load / cores`, between 1 and the maximum. */
export const loadFactorOf = ({ strict, override, load, cores }: LoadInput): number => {
  if (strict === '1') return 1
  const asked = override === undefined || override.trim() === '' ? Number.NaN : Number(override)
  const raw = Number.isFinite(asked) ? asked : 1 + Math.max(0, load) / Math.max(1, cores)
  return Math.min(MAX_LOAD_FACTOR, Math.max(1, raw))
}

/** The factor of this machine now, from the environment and the load average. */
export const loadFactor = (): number =>
  loadFactorOf({
    strict: process.env.MTG_STRICT_PERF,
    override: process.env.MTG_LOAD_FACTOR,
    load: loadavg()[0] ?? 0,
    cores: availableParallelism(),
  })

/** A budget in milliseconds stretched by the factor, rounded up. */
export const scaled = (ms: number, factor: number = loadFactor()): number => Math.ceil(ms * factor)
