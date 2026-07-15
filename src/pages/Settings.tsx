import { useEffect, useState } from 'react'
import { ShieldCheck } from 'lucide-react'
import { ScreenHeader } from '../components/ui/ScreenHeader'
import { Toggle } from '../components/ui/Toggle'
import { useStore } from '../store/useStore'
import { isTauri, pickFolder } from '../lib/api'
import { ghostBtnAlt, inputBase, primaryBtn, sectionLabel } from '../lib/styles'

const TOGGLES: Array<{ key: 'startup' | 'autostart' | 'notify'; label: string; desc: string }> = [
  { key: 'startup', label: 'Launch Stackr at login', desc: 'Start the app automatically when Windows boots.' },
  { key: 'autostart', label: 'Auto-start services', desc: 'Start your last-running services when Stackr opens.' },
  { key: 'notify', label: 'Desktop notifications', desc: 'Notify on install completion and service crashes.' },
]

// Normalise to a single leading dot, e.g. "test" / "..local" → ".test" / ".local".
const normalizeTld = (raw: string) => '.' + raw.trim().replace(/^\.+/, '').replace(/\s+/g, '')

export function Settings() {
  const settings = useStore((s) => s.settings)
  const toggleSetting = useStore((s) => s.toggleSetting)
  const setSetting = useStore((s) => s.setSetting)
  const system = useStore((s) => s.system)
  const checkSystem = useStore((s) => s.checkSystem)
  const rootInfo = useStore((s) => s.rootInfo)
  const defender = useStore((s) => s.defender)
  const defenderBusy = useStore((s) => s.defenderBusy)
  const checkDefender = useStore((s) => s.checkDefender)
  const addDefenderExclusion = useStore((s) => s.addDefenderExclusion)
  const https = useStore((s) => s.https)
  const httpsBusy = useStore((s) => s.httpsBusy)
  const checkHttps = useStore((s) => s.checkHttps)
  const toggleHttps = useStore((s) => s.toggleHttps)
  const appVersion = useStore((s) => s.appVersion)
  const update = useStore((s) => s.update)
  const updateBusy = useStore((s) => s.updateBusy)
  const updateProgress = useStore((s) => s.updateProgress)
  const checkUpdate = useStore((s) => s.checkUpdate)
  const installUpdate = useStore((s) => s.installUpdate)

  // Populate the system report if it isn't loaded yet (App also runs this on boot).
  useEffect(() => {
    if (!system) void checkSystem()
  }, [system, checkSystem])

  // Defender state isn't cheap (spawns PowerShell) — fetch once when Settings opens.
  useEffect(() => {
    if (!defender) void checkDefender()
  }, [defender, checkDefender])

  // HTTPS status (also spawns certutil) — fetch once when Settings opens.
  useEffect(() => {
    if (!https) void checkHttps()
  }, [https, checkHttps])

  // Local TLD edits stay uncommitted until blur/Enter so typing isn't fought by the store.
  const [tld, setTld] = useState(settings.tld)
  // Sync when settings load asynchronously (won't fire mid-typing — only commit changes the store).
  useEffect(() => setTld(settings.tld), [settings.tld])

  const commitTld = () => {
    const next = normalizeTld(tld) || '.test'
    setTld(next)
    if (next !== settings.tld) setSetting('tld', next)
  }

  const browse = async () => {
    if (!isTauri()) return
    const picked = await pickFolder(settings.sitesDir)
    if (picked) setSetting('sitesDir', picked)
  }

  return (
    <div className="max-w-[680px] w-full">
      <ScreenHeader title="Settings" subtitle="Global preferences for Stackr." className="mb-6" />

      <div className="flex flex-col gap-[14px]">
        {/* toggle rows */}
        <div className="bg-card border border-line rounded-[10px] px-[18px] py-[6px]">
          {TOGGLES.map(({ key, label, desc }, i) => (
            <div
              key={key}
              className="flex items-center justify-between gap-4 py-[14px]"
              style={{ borderBottom: i < TOGGLES.length - 1 ? '1px solid #1e222c' : 'none' }}
            >
              <div>
                <div className="text-[13.5px] font-medium text-fg-soft">{label}</div>
                <div className="text-[12px] text-fg-dim mt-[2px]">{desc}</div>
              </div>
              <Toggle on={settings[key]} onClick={() => toggleSetting(key)} />
            </div>
          ))}
        </div>

        {/* paths */}
        <div className="bg-card border border-line rounded-[10px] p-[18px]">
          {rootInfo && (
            <div className="mb-[18px]">
              <label className={`${sectionLabel} mb-2`}>Data folder</label>
              <input value={rootInfo.root} readOnly className={`${inputBase} w-full`} />
              <div className="text-[11.5px] text-fg-dim mt-2">
                Engines, projects, configs and logs live here (chosen on first run). Moving it means
                relocating the folder manually.
              </div>
            </div>
          )}

          <label className={`${sectionLabel} mb-2`}>Sites directory</label>
          <div className="flex gap-[10px]">
            <input value={settings.sitesDir} readOnly className={`${inputBase} flex-1`} />
            <button onClick={() => void browse()} className={`${ghostBtnAlt} px-[14px] py-2 text-[12.5px]`}>
              Browse
            </button>
          </div>
          <div className="text-[11.5px] text-fg-dim mt-2">New projects are created here.</div>

          <div className="mt-[18px]">
            <label className={`${sectionLabel} mb-2`}>Local TLD</label>
            <div className="flex items-center gap-[8px]">
              <input
                value={tld}
                onChange={(e) => setTld(e.target.value)}
                onBlur={commitTld}
                onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
                spellCheck={false}
                className={`${inputBase} w-[140px]`}
              />
              {['.localhost', '.test'].map((preset) => {
                const active = normalizeTld(tld) === preset
                return (
                  <button
                    key={preset}
                    onClick={() => {
                      setTld(preset)
                      if (preset !== settings.tld) setSetting('tld', preset)
                    }}
                    className={`px-[10px] py-[6px] rounded-md text-[12px] font-medium font-mono border transition-colors ${
                      active
                        ? 'bg-[rgba(79,140,255,.14)] border-[rgba(79,140,255,.5)] text-accent-link'
                        : 'bg-control border-line-input text-fg-muted hover:bg-hover'
                    }`}
                  >
                    {preset}
                  </button>
                )
              })}
            </div>
            <div className="text-[11.5px] text-fg-dim mt-2 leading-[1.6]">
              Domain suffix for new projects, e.g. <span className="font-mono">my-app{normalizeTld(tld) || '.test'}</span>.
              <br />
              <span className="font-mono">.localhost</span> needs no admin rights — browsers resolve it automatically (no
              hosts entry). <span className="font-mono">.test</span> is written to your hosts file (one UAC prompt). Either
              way, CLI tools and PHP self-requests to <span className="font-mono">*.localhost</span> still need a hosts entry.
            </div>
          </div>
        </div>

        {/* HTTPS (local CA) */}
        {isTauri() && (
          <div className="bg-card border border-line rounded-[10px] p-[18px]">
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <div className="text-[13.5px] font-medium text-fg-soft">HTTPS for projects</div>
                <div className="text-[12px] text-fg-dim mt-[2px] leading-[1.6]">
                  Serve every project over <span className="font-mono">https://</span> with a locally-trusted
                  certificate. Enabling generates a Stackr root CA and imports it into your trust store
                  (one Windows prompt) — no browser warnings.
                </div>
              </div>
              <Toggle
                on={!!https?.enabled}
                onClick={() => void toggleHttps(!https?.enabled)}
                disabled={httpsBusy}
              />
            </div>
            {https?.enabled && (
              <div className="text-[11.5px] mt-3 flex items-center gap-[7px]">
                <span
                  className="w-[7px] h-[7px] rounded-full shrink-0"
                  style={{ background: https.trusted ? '#3fb950' : '#d29922' }}
                />
                <span className="text-fg-dim">
                  {https.trusted
                    ? 'Local CA is trusted. Start or restart a project to serve it over HTTPS.'
                    : 'CA not detected in the trust store — re-enable to import it, or check your antivirus.'}
                </span>
              </div>
            )}
          </div>
        )}

        {/* Windows Defender exclusion */}
        {isTauri() && (
          <div className="bg-card border border-line rounded-[10px] p-[18px]">
            <div className="flex items-center justify-between gap-3 mb-2">
              <label className={sectionLabel}>Windows Defender</label>
              {defender && (
                <span className="inline-flex items-center gap-[7px]">
                  <span
                    className="w-[7px] h-[7px] rounded-full shrink-0"
                    style={{
                      background:
                        defender.excluded === true
                          ? '#3fb950'
                          : defender.excluded === false
                            ? '#d29922'
                            : '#6b7280',
                    }}
                  />
                  <span
                    className="font-mono font-medium text-[12px]"
                    style={{ color: defender.excluded === true ? '#3fb950' : '#cfd4de' }}
                  >
                    {defender.excluded === true
                      ? 'excluded'
                      : defender.excluded === false
                        ? 'not excluded'
                        : 'unknown'}
                  </span>
                </span>
              )}
            </div>
            <div className="text-[12px] text-fg-dim leading-[1.6]">
              Real-time scanning of <span className="font-mono">{defender?.path ?? 'C:\\Stackr'}</span>{' '}
              slows Composer, Artisan and PHP file access. Excluding it removes that tax.
              {defender?.excluded === null && (
                <> Couldn't verify the current state — Defender may be off or managed by another antivirus.</>
              )}
              {defender && defender.excluded !== true && (
                <> Adding an exclusion opens a Windows admin (UAC) prompt.</>
              )}
            </div>
            {defender && defender.excluded !== true && (
              <div className="flex gap-[10px] mt-3">
                <button
                  onClick={() => void addDefenderExclusion()}
                  disabled={defenderBusy}
                  className={`${primaryBtn} px-[14px] py-2 text-[12.5px] disabled:opacity-60 disabled:cursor-default`}
                >
                  <ShieldCheck size={14} className="mr-[6px]" />
                  {defenderBusy ? 'Adding…' : 'Add exclusion'}
                </button>
                <button
                  onClick={() => void navigator.clipboard.writeText(defender?.path ?? 'C:\\Stackr')}
                  className={`${ghostBtnAlt} px-[14px] py-2 text-[12.5px]`}
                >
                  Copy path
                </button>
              </div>
            )}
          </div>
        )}

        {/* updates */}
        {isTauri() && (
          <div className="bg-card border border-line rounded-[10px] p-[18px]">
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <div className="text-[13.5px] font-medium text-fg-soft">Updates</div>
                <div className="text-[12px] text-fg-dim mt-[2px]">
                  Stackr <span className="font-mono">{appVersion ? `v${appVersion}` : ''}</span>
                  {update ? ` · v${update.version} available` : ' · up to date'}
                </div>
              </div>
              {update ? (
                <button
                  onClick={() => void installUpdate()}
                  disabled={updateBusy}
                  className={`${primaryBtn} px-[14px] py-2 text-[12.5px] disabled:opacity-60 disabled:cursor-default`}
                >
                  {updateBusy ? `Installing… ${updateProgress}%` : `Download & install`}
                </button>
              ) : (
                <button
                  onClick={() => void checkUpdate(true)}
                  className={`${ghostBtnAlt} px-[14px] py-2 text-[12.5px]`}
                >
                  Check for updates
                </button>
              )}
            </div>
            {update?.notes && (
              <div className="text-[11.5px] text-fg-dim mt-3 whitespace-pre-line leading-[1.6] max-h-[120px] overflow-y-auto">
                {update.notes}
              </div>
            )}
          </div>
        )}

        {/* system / compatibility */}
        {system && (
          <div className="bg-card border border-line rounded-[10px] p-[18px]">
            <label className={`${sectionLabel} mb-3`}>System</label>
            <div className="flex flex-col gap-[10px]">
              <SysRow label="Windows" value={system.windows} />
              <SysRow label="WebView2" value={system.webview2 ?? 'not detected'} ok={!!system.webview2} />
              <SysRow
                label="VC++ x64 runtime"
                value={system.vcredist ? 'installed' : 'missing — engines may not start'}
                ok={system.vcredist}
              />
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function SysRow({ label, value, ok }: { label: string; value: string; ok?: boolean }) {
  const showDot = ok !== undefined
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-[12.5px] text-fg-dim">{label}</span>
      <span className="inline-flex items-center gap-[7px] min-w-0">
        {showDot && (
          <span
            className="w-[7px] h-[7px] rounded-full shrink-0"
            style={{ background: ok ? '#3fb950' : '#f1645a' }}
          />
        )}
        <span
          className="font-mono font-medium text-[12px] truncate"
          style={{ color: ok === false ? '#f1645a' : '#cfd4de' }}
          title={value}
        >
          {value}
        </span>
      </span>
    </div>
  )
}
