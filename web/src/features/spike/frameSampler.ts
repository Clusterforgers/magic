export interface FrameStats {
  frames: number
  meanMs: number
  p95Ms: number
  maxMs: number
  fps: number
}

/** Summarizes frame durations in milliseconds. */
export function summarizeFrames(durations: readonly number[]): FrameStats {
  if (durations.length === 0) return { frames: 0, meanMs: 0, p95Ms: 0, maxMs: 0, fps: 0 }
  const sorted = [...durations].sort((a, b) => a - b)
  const mean = durations.reduce((sum, value) => sum + value, 0) / durations.length
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))] ?? 0
  return {
    frames: durations.length,
    meanMs: mean,
    p95Ms: p95,
    maxMs: sorted[sorted.length - 1] ?? 0,
    fps: 1000 / mean,
  }
}

/** Records requestAnimationFrame deltas until stopped; returns a stop function. */
export function startFrameSampler(sink: number[]): () => void {
  let last = performance.now()
  let handle = requestAnimationFrame(function tick(now) {
    sink.push(now - last)
    last = now
    handle = requestAnimationFrame(tick)
  })
  return () => cancelAnimationFrame(handle)
}
