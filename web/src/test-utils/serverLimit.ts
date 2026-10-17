/**
 * The server's per-socket rate limiter (crates/mtg-server/src/limit.rs, defaults in config.rs: 40 at once, then 20 a
 * second), written out so tests can check a client's send times against it.
 */
export function withinServerLimit(times: readonly number[], burst = 40, perSecond = 20): boolean {
  let tokens = burst
  let last = times[0] ?? 0
  for (const at of times) {
    tokens = Math.min(burst, tokens + (Math.max(0, at - last) / 1000) * perSecond)
    last = Math.max(last, at)
    if (tokens < 1) return false
    tokens -= 1
  }
  return true
}
