import type { Update } from '@tauri-apps/plugin-updater'
import { isTauri } from './api'

export interface UpdateInfo {
  version: string
  currentVersion: string
  notes: string
}

// The Update handle from the last successful check() — held here (not in the
// store) so downloadAndInstall reuses it instead of re-checking.
let pending: Update | null = null

/** Check the update endpoint. Returns info if a newer version is available,
 *  null if up to date. Throws on a network/endpoint error (e.g. no release yet). */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  if (!isTauri()) return null
  const { check } = await import('@tauri-apps/plugin-updater')
  pending = await check()
  if (!pending) return null
  return { version: pending.version, currentVersion: pending.currentVersion, notes: pending.body ?? '' }
}

/** Download + install the update found by the last check, then relaunch. */
export async function installPendingUpdate(onProgress?: (percent: number) => void): Promise<void> {
  if (!pending) return
  const { relaunch } = await import('@tauri-apps/plugin-process')
  let downloaded = 0
  let total = 0
  await pending.downloadAndInstall((e) => {
    if (e.event === 'Started') {
      total = e.data.contentLength ?? 0
    } else if (e.event === 'Progress') {
      downloaded += e.data.chunkLength
      if (total > 0) onProgress?.(Math.round((downloaded / total) * 100))
    }
  })
  await relaunch()
}

/** The running app's version (from tauri). */
export async function appVersion(): Promise<string> {
  if (!isTauri()) return ''
  const { getVersion } = await import('@tauri-apps/api/app')
  return getVersion()
}
