import { useEffect, useRef, useState, type ReactNode } from 'react'
import { openUrl } from '@tauri-apps/plugin-opener'
import { Check, ChevronDown, Code2, ExternalLink, FolderOpen, Play, Plus, Square, SquareTerminal, Trash2 } from 'lucide-react'
import { ScreenHeader } from '../components/ui/ScreenHeader'
import { Monogram } from '../components/ui/Monogram'
import { useStore } from '../store/useStore'
import { cn } from '../lib/cn'
import { iconBtn, primaryBtn } from '../lib/styles'
import { isTauri, openProjectFolder } from '../lib/api'
import { projectVisual } from '../lib/projectVisual'
import type { ProjectInfo } from '../types'

const toggleBtn =
  'w-[30px] h-[30px] rounded-md bg-transparent border-none flex items-center justify-center cursor-pointer transition-colors hover:bg-hover'
const deleteBtn =
  'w-[30px] h-[30px] rounded-md bg-transparent border-none text-[#7b818f] flex items-center justify-center cursor-pointer transition-colors hover:bg-[rgba(248,81,73,.14)] hover:text-danger'
const chip =
  'inline-flex items-center gap-[5px] font-mono font-medium text-[11px] text-fg-muted2 bg-chip border border-line-chip px-2 py-[3px] rounded-[5px] cursor-pointer transition-colors hover:border-line-hover2 hover:text-[#cfd4de]'

interface MenuItem {
  key: string
  label: string
  active?: boolean
  disabled?: boolean
  onClick: () => void
}

/** Small dropdown: a trigger node + a popover list (closes on outside click). */
function Menu({
  trigger,
  items,
  className,
}: {
  trigger: ReactNode
  items: MenuItem[]
  className?: string
}) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', onDoc)
    return () => document.removeEventListener('mousedown', onDoc)
  }, [open])

  return (
    <div ref={ref} className={cn('relative shrink-0', className)}>
      <div onClick={() => setOpen((o) => !o)}>{trigger}</div>
      {open && (
        <div className="absolute right-0 top-full mt-1 z-40 min-w-[150px] max-h-[240px] overflow-y-auto bg-card border border-line-input rounded-lg py-1 shadow-[0_12px_30px_rgba(0,0,0,.45)]">
          {items.map((it) =>
            it.disabled ? (
              <div key={it.key} className="px-3 py-[6px] text-[12px] text-fg-dim italic cursor-default">
                {it.label}
              </div>
            ) : (
              <button
                key={it.key}
                onClick={() => {
                  setOpen(false)
                  it.onClick()
                }}
                className="w-full text-left px-3 py-[6px] text-[12.5px] text-[#c9ced8] hover:bg-hover2 flex items-center justify-between gap-3 cursor-pointer"
              >
                <span className="truncate">{it.label}</span>
                {it.active && <Check size={13} strokeWidth={2.4} className="text-accent-link shrink-0" />}
              </button>
            ),
          )}
        </div>
      )}
    </div>
  )
}

function ProjectRow({ p }: { p: ProjectInfo }) {
  const startProject = useStore((s) => s.startProject)
  const stopProject = useStore((s) => s.stopProject)
  const deleteProject = useStore((s) => s.deleteProject)
  const setProjectPhp = useStore((s) => s.setProjectPhp)
  const setProjectDb = useStore((s) => s.setProjectDb)
  const askConfirm = useStore((s) => s.askConfirm)
  const phpVersions = useStore((s) => s.phpVersions)
  const databases = useStore((s) => s.databases)
  const ides = useStore((s) => s.ides)
  const openProjectInIde = useStore((s) => s.openProjectInIde)
  const openProjectTerminal = useStore((s) => s.openProjectTerminal)
  const showToast = useStore((s) => s.showToast)

  const v = projectVisual(p.framework)
  const running = p.status === 'running'
  const url = `http://${p.domain}`

  const openBrowser = () => {
    if (isTauri()) void openUrl(url)
    else window.open(url, '_blank')
  }

  // PHP version switcher — installed runtimes. The project keeps its stored
  // version even after it's uninstalled, so flag when it's no longer available.
  const phpMissing = !phpVersions.some((pv) => pv.version === p.phpVersion)
  const phpItems: MenuItem[] = phpVersions.length
    ? phpVersions.map((pv) => ({
        key: pv.version,
        label: `PHP ${pv.version}`,
        active: pv.version === p.phpVersion,
        onClick: () => void setProjectPhp(p.id, pv.version),
      }))
    : [{ key: '__none', label: 'No PHP installed — see PHP tab', disabled: true, onClick: () => {} }]

  // Database switcher — installed engines (deduped by name) + None.
  const dbNames = Array.from(new Set(databases.map((d) => d.name)))
  const dbItems: MenuItem[] = [...dbNames, 'None'].map((name) => ({
    key: name,
    label: name,
    active: (p.database ?? 'None') === name,
    onClick: () => void setProjectDb(p.id, name),
  }))

  const openIde = () => {
    if (!ides.length) {
      showToast('No supported IDE detected (VS Code, PhpStorm, Cursor, Sublime).', 'info')
      return
    }
    void openProjectInIde(p.id, ides[0].id)
  }

  return (
    <div className="flex items-center gap-[14px] bg-card border border-line rounded-[10px] px-[15px] py-[13px] transition-colors hover:border-line-hover">
      <Monogram size={38} radius={8} bg={v.markBg} color={v.markColor} fontSize={13} bold>
        {v.mark}
      </Monogram>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-[9px]">
          <span className="text-[14px] font-semibold whitespace-nowrap">{p.name}</span>
          <span
            className="font-mono font-medium text-[11px] px-[7px] py-[2px] rounded-[5px] whitespace-nowrap"
            style={{ color: v.fwColor, background: v.fwBg }}
          >
            {v.label}
          </span>
        </div>
        <a
          href={url}
          onClick={(e) => {
            e.preventDefault()
            openBrowser()
          }}
          className="inline-block mt-1 font-mono font-medium text-[12px] text-accent-link no-underline hover:underline"
        >
          {p.domain}
        </a>
      </div>

      {/* PHP version — click to switch */}
      <Menu
        trigger={
          <span
            className={cn(chip, phpMissing && '!text-[#e0a93a] !border-[#6b5524]')}
            title={
              phpMissing
                ? `PHP ${p.phpVersion} is no longer installed — pick another version`
                : 'Switch PHP version'
            }
          >
            {phpVersions.length === 0 ? 'No PHP' : `PHP ${p.phpVersion}`}
            <ChevronDown size={11} strokeWidth={2} className="opacity-70" />
          </span>
        }
        items={phpItems}
      />

      {/* Database — click to switch */}
      <Menu
        trigger={
          <span className={chip} title="Switch database">
            {p.database ?? 'No DB'}
            <ChevronDown size={11} strokeWidth={2} className="opacity-70" />
          </span>
        }
        items={dbItems}
      />

      <span
        className="inline-flex items-center justify-center gap-[7px] w-[92px] px-[11px] py-1 rounded-[20px] text-[11.5px] font-semibold shrink-0"
        style={{
          background: running ? 'rgba(63,185,80,.12)' : 'rgba(255,255,255,.045)',
          color: running ? '#3fb950' : '#7c8493',
        }}
      >
        <span
          className="w-[6px] h-[6px] rounded-full"
          style={{ background: 'currentColor', boxShadow: running ? '0 0 6px rgba(63,185,80,.6)' : '0 0 6px transparent' }}
        />
        {running ? 'running' : 'stopped'}
      </span>

      <div className="flex gap-[2px] shrink-0">
        <button onClick={openBrowser} title="Open in browser" className={iconBtn}>
          <ExternalLink size={15} strokeWidth={1.9} />
        </button>

        {/* Open in IDE — direct when one is detected, a picker when several are */}
        {ides.length > 1 ? (
          <Menu
            trigger={
              <span className={iconBtn} title="Open in IDE">
                <Code2 size={15} strokeWidth={1.9} />
              </span>
            }
            items={ides.map((i) => ({
              key: i.id,
              label: i.name,
              onClick: () => void openProjectInIde(p.id, i.id),
            }))}
          />
        ) : (
          <button onClick={openIde} title="Open in IDE" className={iconBtn}>
            <Code2 size={15} strokeWidth={1.9} />
          </button>
        )}

        <button onClick={() => isTauri() && void openProjectFolder(p.id)} title="Open folder" className={iconBtn}>
          <FolderOpen size={15} strokeWidth={1.9} />
        </button>
        <button onClick={() => void openProjectTerminal(p.id)} title="Open terminal (php, composer, git on PATH)" className={iconBtn}>
          <SquareTerminal size={15} strokeWidth={1.9} />
        </button>
        <button
          onClick={() => (running ? stopProject(p.id) : startProject(p.id))}
          title={running ? 'Stop' : 'Start'}
          className={toggleBtn}
          style={{ color: running ? '#caa14a' : '#3fb950' }}
        >
          {running ? (
            <Square size={13} fill="currentColor" stroke="none" />
          ) : (
            <Play size={14} fill="currentColor" stroke="none" />
          )}
        </button>
        <button
          onClick={async () => {
            const ok = await askConfirm({
              title: `Delete "${p.name}"?`,
              message: 'The project is removed from Stackr and its domain unregistered.',
              confirmLabel: 'Delete',
              danger: true,
              checkbox: { label: 'Also delete project files from disk (irreversible)' },
            })
            if (ok) void deleteProject(p.id, useStore.getState().confirmChecked)
          }}
          title="Delete"
          className={deleteBtn}
        >
          <Trash2 size={15} strokeWidth={1.9} />
        </button>
      </div>
    </div>
  )
}

export function Projects() {
  const projects = useStore((s) => s.projects)
  const openWizard = useStore((s) => s.openWizard)
  const openImport = useStore((s) => s.openImport)
  const loadIdes = useStore((s) => s.loadIdes)
  const up = projects.filter((p) => p.status === 'running').length

  // Detect installed IDEs once for the "Open in IDE" picker.
  useEffect(() => {
    void loadIdes()
  }, [loadIdes])

  return (
    <div className="max-w-[1320px] w-full">
      <ScreenHeader
        title="Projects"
        subtitle={`${up} of ${projects.length} sites running`}
        className="mb-[22px]"
        right={
          <div className="flex items-center gap-2">
            <button
              onClick={openImport}
              className="inline-flex items-center gap-[7px] bg-control border border-line-input rounded-md px-[15px] py-[9px] text-[13px] font-medium text-[#cfd4de] transition-colors hover:bg-hover"
            >
              <FolderOpen size={15} strokeWidth={2} />
              Open Project
            </button>
            <button onClick={openWizard} className={`${primaryBtn} gap-[7px] px-[15px] py-[9px] text-[13px]`}>
              <Plus size={15} strokeWidth={2.2} />
              New Project
            </button>
          </div>
        }
      />

      <div className="flex flex-col gap-[9px]">
        {projects.map((p) => (
          <ProjectRow key={p.id} p={p} />
        ))}
      </div>
    </div>
  )
}
