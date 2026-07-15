import type { CSSProperties } from 'react'
import { Check, Download, ExternalLink, File, GitBranch, Layers, X } from 'lucide-react'
import { ModalBackdrop } from './ui/ModalBackdrop'
import { Monogram } from './ui/Monogram'
import { Select } from './ui/Select'
import { ProgressBar } from './ui/ProgressBar'
import { Spinner } from './ui/Spinner'
import { useStore } from '../store/useStore'
import { cn } from '../lib/cn'
import { inputBase, primaryBtn, sectionLabel } from '../lib/styles'
import { FRAMEWORKS } from '../data/catalog'
import { detectDocRoot, pickFolder } from '../lib/api'
import type { ProjectType } from '../types'

const STEP_NAMES = ['Type', 'Framework', 'Configure', 'Install']

// Wizard DB display name → backend engine component (for the "already installed?" check).
const DB_COMPONENT: Record<string, string> = { MySQL: 'mysql', MariaDB: 'mariadb', PostgreSQL: 'postgresql' }

// Read-only display field styling (shared by folder path + framework version).
const readonlyField =
  'w-full bg-inset border border-line-input rounded-md text-fg-muted font-mono text-[13px] px-[11px] py-[9px] outline-none'

/** Descending semver comparator ("8.4.3" before "8.3.16"). */
function cmpSemverDesc(a: string, b: string): number {
  const pa = a.split('.').map(Number)
  const pb = b.split('.').map(Number)
  for (let i = 0; i < 3; i++) {
    const d = (pb[i] ?? 0) - (pa[i] ?? 0)
    if (d) return d
  }
  return 0
}

const TYPE_CARDS: Array<{ type: ProjectType; title: string; desc: string; Icon: typeof File }> = [
  { type: 'Blank PHP', title: 'Blank PHP', desc: 'Empty index.php with a public root.', Icon: File },
  { type: 'Framework', title: 'Framework', desc: 'Scaffold Laravel, Symfony & more.', Icon: Layers },
  { type: 'Clone from Git', title: 'Clone from Git', desc: 'Pull an existing repository.', Icon: GitBranch },
]

export function NewProjectWizard() {
  const wiz = useStore((s) => s.wiz)
  const closeWizard = useStore((s) => s.closeWizard)
  const setWiz = useStore((s) => s.setWiz)
  const selectType = useStore((s) => s.selectType)
  const selectFramework = useStore((s) => s.selectFramework)
  const selectFrameworkVersion = useStore((s) => s.selectFrameworkVersion)
  const wizNext = useStore((s) => s.wizNext)
  const wizBack = useStore((s) => s.wizBack)
  const startProject = useStore((s) => s.startProject)
  const databases = useStore((s) => s.databases)
  const servers = useStore((s) => s.servers)
  const phpVersions = useStore((s) => s.phpVersions)
  const phpAvailable = useStore((s) => s.phpAvailable)
  const settings = useStore((s) => s.settings)

  if (!wiz.open) return null

  const { step, type, framework, name, git, error, done, stepIdx } = wiz
  const domainName = name || 'my-project'
  const tldSuffix = `.${settings.tld.replace(/^\.+/, '')}`
  const path = `${settings.sitesDir}\\${name || 'my-project'}`
  const fwLabel = framework || 'your framework'
  const isGit = type === 'Clone from Git'
  const isImport = type === 'Open existing'
  const configSubtitle = isImport
    ? 'Open an existing project folder from anywhere on your machine.'
    : type === 'Framework'
      ? `Setting up a new ${fwLabel} project.`
      : isGit
        ? 'Configure the cloned repository.'
        : 'Set up a blank PHP project.'

  // Pick a folder to open, prefill the name from its basename, and auto-detect
  // the document root (public/web/webroot).
  const onBrowse = async () => {
    const p = await pickFolder(settings.sitesDir)
    if (!p) return
    const base = p.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || 'project'
    let dr = ''
    try {
      dr = await detectDocRoot(p)
    } catch {
      /* detection is best-effort */
    }
    setWiz({ importPath: p, name: base, docRoot: dr })
  }

  // Selected framework + its installable versions (step 2). The wizard stores the
  // Composer constraint; map it back to the display label for the dropdown.
  const fwMeta = FRAMEWORKS.find((f) => f.name === framework)
  const fwVersionLabel =
    fwMeta?.versions.find((v) => v.constraint === wiz.frameworkVersion)?.label ??
    fwMeta?.versions[0]?.label ??
    ''

  // PHP options: installed builds + installable versions (deduped, newest first).
  const installedPhp = new Set(phpVersions.map((v) => v.version))
  const phpChoices = Array.from(new Set([...phpVersions.map((v) => v.version), ...phpAvailable])).sort(cmpSemverDesc)

  // Web server is global (one active): if one is installed, offer only it; else
  // offer both — the chosen one is installed during creation.
  const serverInstalled = servers.length > 0
  const serverChoices = serverInstalled ? Array.from(new Set(servers.map((s) => s.name))) : ['Nginx', 'Apache']

  // Database is optional: installed engines + installable engines + None.
  const dbInstalled = new Set(databases.map((d) => d.component))
  const dbChoices = Array.from(new Set([...databases.map((d) => d.name), 'MySQL', 'MariaDB', 'PostgreSQL']))

  // Prerequisites the current picks will install (shown as a note; skipped if
  // already installed).
  const willInstall: string[] = []
  if (wiz.php && !installedPhp.has(wiz.php)) willInstall.push(`PHP ${wiz.php}`)
  if (!serverInstalled) willInstall.push(wiz.server)
  if (wiz.db && wiz.db !== 'None' && !dbInstalled.has(DB_COMPONENT[wiz.db])) willInstall.push(wiz.db)

  // Step list for the browser preview only (the Tauri path drives wiz.steps).
  const hasDb = !!wiz.db && wiz.db !== 'None'
  const baseSteps = isGit
    ? ['Creating project folder', 'Cloning repository', 'Installing dependencies', `Configuring ${wiz.server}`, 'Registering domain']
    : type === 'Framework'
      ? framework === 'WordPress'
        ? ['Creating project folder', 'Downloading WordPress', `Configuring ${wiz.server}`, 'Registering domain']
        : ['Creating project folder', 'Installing Composer', `Setting up ${fwLabel}`, `Configuring ${wiz.server}`, 'Registering domain']
      : ['Creating project folder', 'Writing index.php', `Configuring ${wiz.server}`, 'Registering domain']
  const fallbackSteps = hasDb
    ? [...baseSteps.slice(0, -1), 'Creating database', baseSteps[baseSteps.length - 1]]
    : baseSteps
  const stepList = wiz.steps.length ? wiz.steps : fallbackSteps
  const pctNum = Math.round(wiz.progress)
  const pct = `${pctNum}%`
  const pctColor = done ? '#3fb950' : '#7a9bff'
  const barColor = done ? '#3fb950' : '#4f7fff'

  const canNext =
    (step === 1 && !!type) ||
    (step === 2 && !!framework) ||
    (step === 3 &&
      name.trim().length > 0 &&
      (!isGit || git.trim().length > 0) &&
      (!isImport || wiz.importPath.trim().length > 0))
  const nextLabel = step === 3 ? (isImport ? 'Import Project' : 'Create Project') : 'Continue'
  const backEnabled = (step > 1 && step < 4) || (step === 4 && !!error)

  const selectableCard = (selected: boolean, radius: number): CSSProperties => ({
    background: selected ? '#191f2c' : '#161a22',
    border: selected ? '1px solid #4f7fff' : '1px solid #232834',
    borderRadius: radius,
  })

  return (
    <ModalBackdrop onClose={closeWizard} padded dismissable={false}>
      <div
        className="w-[780px] max-h-[92vh] bg-card border border-line-input rounded-[14px] overflow-hidden flex flex-col"
        style={{ boxShadow: '0 30px 70px rgba(0,0,0,.55)' }}
      >
        {/* header */}
        <div className="px-[22px] py-[18px] border-b border-[#1f242f] flex items-center justify-between">
          <div className="text-[15px] font-semibold">{isImport ? 'Open Project' : 'New Project'}</div>
          <button
            onClick={closeWizard}
            className="w-7 h-7 rounded-md bg-transparent border-none text-[#7b818f] flex items-center justify-center cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]"
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>

        {/* stepper (hidden in import mode — it's a single Configure step) */}
        <div className={`px-[22px] py-4 border-b border-[#1f242f] items-center gap-[6px] ${isImport ? 'hidden' : 'flex'}`}>
          {STEP_NAMES.map((label, i) => {
            const n = i + 1
            const stepDone = step > n
            const current = step === n
            const skipFw = n === 2 && !!type && type !== 'Framework'
            const dim = skipFw
            const showCheck = stepDone && !skipFw
            return (
              <div key={label} className="flex items-center gap-[6px]">
                <div className="flex items-center gap-[9px]">
                  <div
                    className="w-[22px] h-[22px] rounded-full flex items-center justify-center text-[11px] font-semibold shrink-0"
                    style={{
                      background: current ? '#4f7fff' : showCheck ? 'rgba(63,185,80,.15)' : '#1b2030',
                      color: current ? '#fff' : showCheck ? '#3fb950' : dim ? '#4b5160' : '#878d9c',
                      border: current ? 'none' : '1px solid #262c38',
                    }}
                  >
                    {showCheck ? <Check size={12} strokeWidth={3} /> : n}
                  </div>
                  <span
                    className="text-[12.5px]"
                    style={{
                      fontWeight: current ? 600 : 500,
                      color: current ? '#e8eaf0' : dim ? '#4b5160' : '#878d9c',
                    }}
                  >
                    {label}
                  </span>
                </div>
                {i < STEP_NAMES.length - 1 && <div className="w-[26px] h-px bg-[#2a3140] mx-1" />}
              </div>
            )
          })}
        </div>

        {/* content */}
        <div className="flex-1 min-h-0 overflow-y-auto px-[22px] py-6">
          {/* step 1: type */}
          {step === 1 && (
            <>
              <div className="text-[14px] font-semibold mb-1">What do you want to create?</div>
              <div className="text-[12.5px] text-fg-muted2 mb-[18px]">
                Choose a starting point for your new project.
              </div>
              <div className="grid grid-cols-3 gap-3">
                {TYPE_CARDS.map(({ type: t, title, desc, Icon }) => (
                  <div
                    key={t}
                    onClick={() => selectType(t)}
                    className="p-[18px] cursor-pointer transition-colors hover:border-line-hover2"
                    style={selectableCard(type === t, 10)}
                  >
                    <div className="w-[38px] h-[38px] rounded-[9px] bg-[#1b1f29] text-[#9aa1ae] flex items-center justify-center mb-3">
                      <Icon size={19} strokeWidth={1.8} />
                    </div>
                    <div className="text-[13.5px] font-semibold">{title}</div>
                    <div className="text-[11.5px] text-[#7f8593] mt-1 leading-[1.45]">{desc}</div>
                  </div>
                ))}
              </div>
            </>
          )}

          {/* step 2: framework */}
          {step === 2 && (
            <>
              <div className="text-[14px] font-semibold mb-1">Choose a framework</div>
              <div className="text-[12.5px] text-fg-muted2 mb-[18px]">
                Stackr will install it via Composer with sensible defaults.
              </div>
              <div className="grid grid-cols-4 gap-3">
                {FRAMEWORKS.map((f) => {
                  const verLabel =
                    f.versions.length > 1
                      ? `${f.versions[f.versions.length - 1].label}–${f.versions[0].label}`
                      : f.versions[0].label
                  return (
                    <div
                      key={f.name}
                      onClick={() => selectFramework(f.name as never)}
                      className="p-[14px] cursor-pointer transition-colors hover:border-line-hover2"
                      style={selectableCard(framework === f.name, 9)}
                    >
                      <Monogram size={40} radius={9} bg={f.color} color={f.txt} fontSize={14} bold>
                        {f.mark}
                      </Monogram>
                      <div className="text-[13px] font-semibold mt-[10px]">{f.name}</div>
                      <div className="font-mono font-medium text-[10.5px] text-fg-dim mt-[2px]">
                        {verLabel}
                      </div>
                    </div>
                  )
                })}
              </div>

              {framework && fwMeta && (
                <div className="grid grid-cols-2 gap-4 mt-5">
                  <div>
                    <label className={`${sectionLabel} mb-2`}>Version</label>
                    {fwMeta.versions.length > 1 ? (
                      <Select value={fwVersionLabel} onChange={(v) => selectFrameworkVersion(v)}>
                        {fwMeta.versions.map((v) => (
                          <option key={v.label} value={v.label}>
                            {v.label}
                          </option>
                        ))}
                      </Select>
                    ) : (
                      <input value={fwMeta.versions[0].label} readOnly className={readonlyField} />
                    )}
                  </div>
                  <div>
                    <label className={`${sectionLabel} mb-2`}>Recommended PHP</label>
                    <input value={wiz.php} readOnly className={readonlyField} />
                  </div>
                </div>
              )}
            </>
          )}

          {/* step 3: configure */}
          {step === 3 && (
            <>
              <div className="text-[14px] font-semibold mb-1">Configure project</div>
              <div className="text-[12.5px] text-fg-muted2 mb-5">{configSubtitle}</div>
              <div className="flex flex-col gap-[18px]">
                {isGit && (
                  <div>
                    <label className={`${sectionLabel} mb-2`}>Repository URL</label>
                    <input
                      value={git}
                      onChange={(e) => setWiz({ git: e.target.value })}
                      placeholder="https://github.com/user/repo.git"
                      className={inputBase}
                    />
                  </div>
                )}
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className={`${sectionLabel} mb-2`}>Project name</label>
                    <input
                      value={name}
                      onChange={(e) => setWiz({ name: e.target.value })}
                      placeholder="my-project"
                      className={inputBase}
                    />
                  </div>
                  <div>
                    <label className={`${sectionLabel} mb-2`}>Domain</label>
                    <div className="flex items-center bg-control border border-line-input rounded-md overflow-hidden">
                      <span className="flex-1 px-[11px] py-[9px] font-mono text-[13px] text-[#d4d9e2] min-w-0 truncate">
                        {domainName}
                      </span>
                      <span className="px-[11px] py-[9px] font-mono text-[13px] text-fg-dim bg-inset border-l border-line-input">
                        {tldSuffix}
                      </span>
                    </div>
                  </div>
                </div>

                <div className="grid grid-cols-3 gap-4">
                  <div>
                    <label className={`${sectionLabel} mb-2`}>PHP version</label>
                    <Select value={wiz.php} onChange={(v) => setWiz({ php: v })}>
                      {phpChoices.length ? (
                        phpChoices.map((v) => (
                          <option key={v} value={v}>
                            {v}
                            {installedPhp.has(v) ? '' : ' — will install'}
                          </option>
                        ))
                      ) : (
                        <option value={wiz.php}>{wiz.php}</option>
                      )}
                    </Select>
                  </div>
                  <div>
                    <label className={`${sectionLabel} mb-2`}>Web server</label>
                    <Select value={wiz.server} onChange={(v) => setWiz({ server: v })}>
                      {serverChoices.map((s) => (
                        <option key={s} value={s}>
                          {s}
                          {serverInstalled ? '' : ' — will install'}
                        </option>
                      ))}
                    </Select>
                  </div>
                  <div>
                    <label className={`${sectionLabel} mb-2`}>Database</label>
                    <Select value={wiz.db} onChange={(v) => setWiz({ db: v })}>
                      {dbChoices.map((d) => (
                        <option key={d} value={d}>
                          {d}
                          {dbInstalled.has(DB_COMPONENT[d]) ? '' : ' — will install'}
                        </option>
                      ))}
                      <option value="None">None</option>
                    </Select>
                  </div>
                </div>

                {isImport ? (
                  <>
                    <div>
                      <label className={`${sectionLabel} mb-2`}>Project folder</label>
                      <div className="flex gap-2">
                        <input
                          value={wiz.importPath}
                          readOnly
                          placeholder="Choose a folder on your machine…"
                          className={`${readonlyField} flex-1`}
                        />
                        <button
                          onClick={onBrowse}
                          className="shrink-0 bg-control border border-line-input rounded-md px-4 text-[13px] font-medium text-[#cfd4de] transition-colors hover:bg-hover"
                        >
                          Browse…
                        </button>
                      </div>
                    </div>
                    <div>
                      <label className={`${sectionLabel} mb-2`}>
                        Document root{' '}
                        <span className="text-fg-dim font-normal">— relative subfolder, empty = folder root</span>
                      </label>
                      <input
                        value={wiz.docRoot}
                        onChange={(e) => setWiz({ docRoot: e.target.value })}
                        placeholder="public"
                        className={inputBase}
                      />
                    </div>
                  </>
                ) : (
                  <div>
                    <label className={`${sectionLabel} mb-2`}>Folder path</label>
                    <input value={path} readOnly className={readonlyField} />
                  </div>
                )}

                {willInstall.length > 0 && (
                  <div className="flex items-start gap-2 text-[11.5px] text-fg-muted2 leading-[1.5]">
                    <Download size={13} strokeWidth={2} className="mt-[2px] shrink-0 text-accent" />
                    <span>Stackr will install the missing pieces first: {willInstall.join(', ')}.</span>
                  </div>
                )}
              </div>
            </>
          )}

          {/* step 4: install — error */}
          {step === 4 && error && (
            <>
              <div className="text-[14px] font-semibold mb-1 text-danger">Project creation failed</div>
              <div className="text-[12.5px] text-fg-muted2 mb-4">
                Something went wrong while setting up {name || 'the project'}.
              </div>
              <div className="bg-term border border-[rgba(248,81,73,.3)] rounded-[9px] px-4 py-[14px] font-mono text-[12px] text-[#f1a7a2] whitespace-pre-wrap break-words max-h-[280px] overflow-y-auto">
                {error}
              </div>
            </>
          )}

          {/* step 4: install — progress */}
          {step === 4 && !error && (
            <>
              <div className="text-[14px] font-semibold mb-1">
                {done ? 'Project ready' : `${isImport ? 'Opening' : 'Setting up'} ${name || 'project'}…`}
              </div>
              <div className="text-[12.5px] text-fg-muted2 mb-5">
                {done
                  ? `${name || 'project'}${tldSuffix} is live and ready to open.`
                  : 'This usually takes under a minute.'}
              </div>
              <ProgressBar pct={pct} color={barColor} className="mb-[6px]" />
              <div className="flex justify-between mb-5">
                <span className="font-mono font-medium text-[11.5px] text-fg-muted2">
                  {done ? 'Done' : stepList[Math.min(stepIdx, stepList.length - 1)]}
                </span>
                <span className="font-mono font-semibold text-[11.5px]" style={{ color: pctColor }}>
                  {pct}
                </span>
              </div>
              <div className="flex flex-col gap-[3px] bg-term border border-line-subtle rounded-[9px] px-4 py-[14px]">
                {stepList.map((label, i) => {
                  const isDone = done || i < stepIdx
                  const isActive = !done && i === stepIdx
                  return (
                    <div key={label} className="flex items-center gap-[11px] py-[5px] font-mono text-[12.5px]">
                      <span className="w-[18px] h-[18px] shrink-0 flex items-center justify-center">
                        {isDone ? (
                          <Check size={15} strokeWidth={2.6} className="text-ok" />
                        ) : isActive ? (
                          <Spinner size={14} strokeWidth={2.6} className="text-accent" />
                        ) : (
                          <span className="w-[6px] h-[6px] rounded-full bg-[#363c48]" />
                        )}
                      </span>
                      <span
                        style={{ color: isDone ? '#c2c7d2' : isActive ? '#e8eaf0' : '#5c626f' }}
                      >
                        {label}
                      </span>
                    </div>
                  )
                })}
              </div>
            </>
          )}
        </div>

        {/* footer */}
        <div className="px-[22px] py-[15px] border-t border-[#1f242f] flex items-center justify-between">
          <button
            onClick={() => {
              if (isImport && step === 3) closeWizard()
              else if (backEnabled) wizBack()
            }}
            className="bg-transparent border border-line-input rounded-md px-4 py-[10px] text-[13px] font-medium transition-colors hover:bg-hover hover:text-[#cfd4de]"
            style={{
              color: backEnabled ? '#9298a6' : '#454b58',
              cursor: backEnabled ? 'pointer' : 'default',
              pointerEvents: backEnabled ? 'auto' : 'none',
              opacity: backEnabled ? 1 : 0.5,
            }}
          >
            Back
          </button>
          <div className="flex gap-[10px]">
            {step !== 4 && (
              <button
                onClick={() => canNext && wizNext()}
                className={cn('border-none rounded-md px-[18px] py-[10px] text-[13px] font-semibold transition-colors', canNext && 'hover:bg-accent-hover')}
                style={{
                  background: canNext ? '#4f7fff' : '#27303f',
                  color: canNext ? '#fff' : '#5c626f',
                  cursor: canNext ? 'pointer' : 'default',
                  pointerEvents: canNext ? 'auto' : 'none',
                }}
              >
                {nextLabel}
              </button>
            )}
            {step === 4 && done && !error && (
              <button
                onClick={() => {
                  void startProject(name)
                  closeWizard()
                }}
                className={`${primaryBtn} gap-2 px-[18px] py-[10px] text-[13px]`}
              >
                <ExternalLink size={15} strokeWidth={2} />
                Open in Browser
              </button>
            )}
            {step === 4 && error && (
              <button onClick={closeWizard} className={`${primaryBtn} px-[18px] py-[10px] text-[13px]`}>
                Close
              </button>
            )}
          </div>
        </div>
      </div>
    </ModalBackdrop>
  )
}
