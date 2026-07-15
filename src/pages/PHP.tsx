import { useEffect } from 'react'
import { Bug, ChevronDown, Download, LayoutList, Search, Star } from 'lucide-react'
import { ScreenHeader } from '../components/ui/ScreenHeader'
import { Monogram } from '../components/ui/Monogram'
import { Toggle } from '../components/ui/Toggle'
import { Spinner } from '../components/ui/Spinner'
import { useStore } from '../store/useStore'
import { cn } from '../lib/cn'
import { dangerBtn, ghostBtn, primaryBtn } from '../lib/styles'
import type { PhpExtInfo, PhpVersionInfo } from '../types'

const extBtn = `${ghostBtn} inline-flex items-center gap-[6px]`
const setDefaultDisabled =
  'opacity-45 cursor-default bg-[#1a1e28] text-fg-muted border border-line-input rounded-md px-3 py-[7px] text-[12.5px] font-medium'

export function ExtensionRow({ e }: { e: PhpExtInfo }) {
  const toggleExt = useStore((s) => s.toggleExt)
  const installExt = useStore((s) => s.installExt)
  const installing = useStore((s) => !!s.extInstalling[e.name])
  const needsInstall = e.pecl && !e.installed

  return (
    <div className="flex items-center justify-between gap-3 px-3 py-[9px] bg-control border border-line-soft rounded-lg">
      <div className="min-w-0">
        <div className="flex items-center gap-[6px]">
          <span className="font-mono font-semibold text-[12.5px] text-[#cfd4de]">{e.name}</span>
          {e.pecl && (
            <span className="text-[9px] font-semibold tracking-[.06em] uppercase px-[5px] py-px rounded bg-[rgba(120,127,139,.16)] text-fg-dim">
              pecl
            </span>
          )}
        </div>
        <div className="text-[11px] text-[#6b7180] mt-px truncate">{e.description || '—'}</div>
      </div>
      {needsInstall ? (
        <button
          onClick={() => void installExt(e.name)}
          disabled={installing}
          className="shrink-0 inline-flex items-center gap-[5px] bg-[#1a1e28] text-[#c2c7d2] border border-line-input rounded-md px-[9px] py-[5px] text-[11.5px] font-medium cursor-pointer transition-colors hover:bg-hover2 hover:border-line-hover2 disabled:opacity-60 disabled:cursor-default"
        >
          {installing ? <Spinner size={12} strokeWidth={2.4} /> : <Download size={12} strokeWidth={2} />}
          {installing ? 'Installing…' : 'Install'}
        </button>
      ) : (
        <Toggle on={e.enabled} onClick={() => toggleExt(e.name)} />
      )}
    </div>
  )
}

function ExtensionsPanel() {
  const extensions = useStore((s) => s.extensions)
  const extSearch = useStore((s) => s.extSearch)
  const setExtSearch = useStore((s) => s.setExtSearch)
  const extTouched = useStore((s) => s.extTouched)
  const openAllExtensions = useStore((s) => s.openAllExtensions)

  const enabled = extensions.filter((e) => e.enabled).length
  const q = extSearch.trim().toLowerCase()
  const matched = q
    ? extensions.filter((e) => e.name.includes(q) || e.description.toLowerCase().includes(q))
    : extensions.filter(
        (e) => e.enabled || (e.pecl && !e.installed) || extTouched.includes(e.name),
      )
  const trimmed = extSearch.trim()
  const hint = q
    ? `${matched.length} ${matched.length === 1 ? 'extension' : 'extensions'} matching "${trimmed}"`
    : `Showing ${enabled} enabled + installable extras · search to find and enable more`
  const emptyText = q ? `No extensions match "${trimmed}"` : 'No extensions found for this build.'

  return (
    <div className="border-t border-line-subtle bg-inset px-[18px] py-4">
      <div className="flex items-center justify-between gap-4 mb-3">
        <span className="text-[11px] font-semibold tracking-[.08em] uppercase text-fg-dim shrink-0">
          Extensions
        </span>
        <div className="relative flex-1 max-w-[300px]">
          <Search
            size={14}
            strokeWidth={2}
            className="absolute left-[10px] top-1/2 -translate-y-1/2 text-fg-dim pointer-events-none"
          />
          <input
            value={extSearch}
            onChange={(e) => setExtSearch(e.target.value)}
            placeholder="Search extensions to enable…"
            className="w-full bg-control border border-line-input rounded-[7px] text-[#d4d9e2] text-[12.5px] outline-none focus:border-accent pl-[31px] pr-[11px] py-2"
          />
        </div>
        <button
          onClick={openAllExtensions}
          className="shrink-0 inline-flex items-center gap-[6px] bg-[#1a1e28] text-[#c2c7d2] border border-line-input rounded-md px-[10px] py-[6px] text-[12px] font-medium cursor-pointer transition-colors hover:bg-hover2 hover:border-line-hover2"
        >
          <LayoutList size={13} strokeWidth={2} />
          Show all
          <span className="font-mono text-[11px] text-fg-faint">{extensions.length}</span>
        </button>
      </div>
      <div className="text-[11.5px] text-fg-dim mb-3">{hint}</div>

      {matched.length > 0 ? (
        <div className="grid grid-cols-3 gap-2">
          {matched.map((e) => (
            <ExtensionRow key={e.name} e={e} />
          ))}
        </div>
      ) : (
        <div className="py-[22px] text-center text-[12.5px] text-fg-dim">{emptyText}</div>
      )}
    </div>
  )
}

/** VS Code PHP-Debug listener config for the given Xdebug port. */
const launchJson = (port: number) =>
  JSON.stringify(
    { version: '0.2.0', configurations: [{ name: 'Listen for Xdebug', type: 'php', request: 'launch', port }] },
    null,
    2,
  )

/** One-click step-debug toggle per PHP version + a copyable launch.json. */
function XdebugControl({ version }: { version: string }) {
  const xd = useStore((s) => s.xdebug[version])
  const busy = useStore((s) => !!s.xdebugBusy[version])
  const loadXdebug = useStore((s) => s.loadXdebug)
  const toggleXdebug = useStore((s) => s.toggleXdebug)
  const showToast = useStore((s) => s.showToast)

  useEffect(() => {
    void loadXdebug(version)
  }, [version, loadXdebug])

  const enabled = xd?.enabled ?? false
  const copyLaunch = async () => {
    try {
      await navigator.clipboard.writeText(launchJson(xd?.port ?? 9003))
      showToast('launch.json copied — paste into .vscode/launch.json.', 'info')
    } catch {
      showToast('Could not copy to clipboard.', 'error')
    }
  }

  return (
    <div className="flex items-center gap-[7px] mr-[3px]" title="Step debugging (Xdebug) — downloads on first enable">
      <Bug size={14} strokeWidth={2} className={enabled ? 'text-[#8aa0e6]' : 'text-fg-dim'} />
      {enabled && (
        <button onClick={() => void copyLaunch()} className="text-[11.5px] text-accent-link hover:underline">
          launch.json
        </button>
      )}
      {busy ? (
        <Spinner size={14} strokeWidth={2.4} />
      ) : (
        <Toggle on={enabled} onClick={() => void toggleXdebug(version, !enabled)} />
      )}
    </div>
  )
}

function VersionCard({ v }: { v: PhpVersionInfo }) {
  const phpPanel = useStore((s) => s.phpPanel)
  const togglePhpPanel = useStore((s) => s.togglePhpPanel)
  const setDefaultPhpVersion = useStore((s) => s.setDefaultPhpVersion)
  const uninstallPhp = useStore((s) => s.uninstallPhp)
  const openConfig = useStore((s) => s.openConfig)

  // No version is "active" on its own — php-cgi starts per project on its own
  // port. `default` only marks the version pre-selected for new projects (and the
  // fallback when a project's chosen version is gone).
  const isDefault = v.isDefault
  const open = phpPanel === v.version
  const tile = isDefault
    ? { bg: 'rgba(99,126,201,.16)', color: '#8aa0e6' }
    : { bg: 'rgba(120,127,139,.14)', color: '#9aa1ae' }
  const note = v.note ?? (isDefault ? 'Default for new projects' : 'Installed')

  return (
    <div className="bg-card border border-line rounded-[10px] overflow-hidden">
      <div className="px-[18px] py-4 flex items-center gap-[15px]">
        <Monogram size={42} radius={10} bg={tile.bg} color={tile.color} fontSize={13} mono>
          {v.majorMinor}
        </Monogram>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-[10px]">
            <span className="text-[14.5px] font-semibold">
              PHP <span className="font-mono font-medium">{v.version}</span>
            </span>
            {isDefault ? (
              <span className="inline-flex items-center gap-[5px] px-[9px] py-[3px] rounded-[20px] bg-[rgba(99,126,201,.16)] text-[#8aa0e6] text-[11px] font-semibold">
                <Star size={10} strokeWidth={2.4} fill="currentColor" />
                default
              </span>
            ) : (
              <span className="inline-flex items-center gap-[6px] px-[9px] py-[3px] rounded-[20px] bg-[rgba(255,255,255,.05)] text-fg-muted text-[11px] font-semibold">
                <span className="w-[6px] h-[6px] rounded-full bg-current" />
                installed
              </span>
            )}
          </div>
          <div className="text-[12px] text-fg-dim mt-1">{note}</div>
        </div>
        <div className="flex items-center gap-[7px]">
          <XdebugControl version={v.version} />
          <button onClick={() => togglePhpPanel(v.version)} className={extBtn}>
            Extensions
            <ChevronDown
              size={13}
              strokeWidth={2}
              className={cn('transition-transform duration-150', open && 'rotate-180')}
            />
          </button>
          <button onClick={() => void openConfig('php', v.version)} className={ghostBtn}>
            php.ini
          </button>
          {v.isDefault ? (
            <button className={setDefaultDisabled}>Set default</button>
          ) : (
            <button onClick={() => setDefaultPhpVersion(v.version)} className={ghostBtn}>
              Set default
            </button>
          )}
          <button onClick={() => uninstallPhp(v.version)} className={dangerBtn}>
            Uninstall
          </button>
        </div>
      </div>
      {open && <ExtensionsPanel />}
    </div>
  )
}

export function Php() {
  const phpVersions = useStore((s) => s.phpVersions)
  const openPhpInstall = useStore((s) => s.openPhpInstall)
  const loadPhpAvailable = useStore((s) => s.loadPhpAvailable)

  // Prefetch the installable versions so the picker opens instantly.
  useEffect(() => {
    void loadPhpAvailable()
  }, [loadPhpAvailable])

  return (
    <div className="max-w-[1080px] w-full">
      <ScreenHeader
        title="PHP"
        subtitle="Installed runtimes, extensions and configuration."
        className="mb-6"
      />

      <div className="flex flex-col gap-3">
        {phpVersions.map((v) => (
          <VersionCard key={v.version} v={v} />
        ))}

        <div className="bg-transparent border border-dashed border-[#2a303c] rounded-[10px] px-[18px] py-4 flex items-center gap-[15px]">
          <Monogram
            size={42}
            radius={10}
            bg="#13161d"
            color="#6f7686"
            fontSize={20}
            bold
            border="1px solid #232834"
          >
            +
          </Monogram>
          <div className="flex-1 min-w-0">
            <div className="text-[14.5px] font-semibold text-[#c0c5d0]">
              Install another PHP version
            </div>
            <div className="text-[12px] text-fg-dim mt-1">
              Pick any release from 7.4 to the latest — fetched from windows.php.net.
            </div>
          </div>
          <button
            onClick={() => void openPhpInstall()}
            className={`${primaryBtn} gap-[7px] px-[14px] py-2 text-[12.5px]`}
          >
            <Download size={14} strokeWidth={2} />
            Install
          </button>
        </div>
      </div>
    </div>
  )
}
