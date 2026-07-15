import type { ServiceRunState } from '../types'

interface StatusVisual {
  label: string
  color: string
  bg: string
  glow: boolean
}

/** Badge label + colors for a service run-state. */
export function statusVisual(status: ServiceRunState): StatusVisual {
  switch (status) {
    case 'running':
      return { label: 'running', color: '#3fb950', bg: 'rgba(63,185,80,.13)', glow: true }
    case 'starting':
      return { label: 'starting', color: '#4f8cff', bg: 'rgba(79,140,255,.14)', glow: true }
    case 'unhealthy':
      return { label: 'unhealthy', color: '#e0a93a', bg: 'rgba(224,169,58,.14)', glow: false }
    default:
      return { label: 'stopped', color: '#9298a6', bg: 'rgba(255,255,255,.05)', glow: false }
  }
}

/** A process exists (so the toggle should offer Stop, not Start). */
export function hasProcess(status: ServiceRunState): boolean {
  return status !== 'stopped'
}
