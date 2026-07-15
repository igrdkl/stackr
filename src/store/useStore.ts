import { create } from 'zustand'
import type {
  AppSettings,
  ConfigEditorState,
  ConfirmState,
  FrameworkName,
  InstallState,
  ProjectType,
  TabId,
  ToastState,
  WizardState,
} from '../types'
import {
  ALL_LOGS,
  CACHE_ENGINES,
  DB_ENGINES,
  DEMO_CACHES,
  DEMO_DATABASES,
  DEMO_MAIL,
  DEMO_EXTENSIONS,
  DEMO_PHP,
  DEMO_PROJECTS,
  FRAMEWORKS,
  MAIL_ENGINES,
  SERVER_ENGINES,
} from '../data/catalog'
import {
  clearAllLogs,
  clearLog as apiClearLog,
  createProject,
  deleteProject as apiDeleteProject,
  detectIdes,
  getCaches,
  getDatabases,
  getMail,
  getInstalled,
  getPhpAvailable,
  getPhpVersions,
  getProjects,
  getServers,
  systemReport,
  type SystemReport,
  getRootInfo,
  setRoot as apiSetRoot,
  type RootInfo,
  defenderStatus,
  addDefenderExclusion as apiAddDefenderExclusion,
  type DefenderStatus,
  httpsStatus,
  enableHttps as apiEnableHttps,
  disableHttps as apiDisableHttps,
  type HttpsStatus,
  getSettings,
  installComponent,
  installPhpExtension,
  isTauri,
  listPhpExtensions,
  exportDatabases as apiExportDatabases,
  onDownloadProgress,
  onProjectProgress,
  openAdminer as apiOpenAdminer,
  openInIde,
  openTerminal,
  xdebugStatus,
  setXdebug,
  type XdebugState,
  readAllLogs,
  readLog,
  readServiceConfig,
  resetServiceConfig,
  restartService as apiRestartService,
  saveServiceConfig,
  saveSettings,
  takeRestoreNotice,
  setDefaultPhp,
  setProjectDb as apiSetProjectDb,
  setProjectPhp as apiSetProjectPhp,
  startProject as apiStartProject,
  startService as apiStartService,
  stopPhpRuntime,
  stopProject as apiStopProject,
  stopService as apiStopService,
  toggleExtensionApi,
  uninstallComponent,
  type IdeInfo,
  type InstalledComponent,
  type LogRaw,
} from '../lib/api'
import {
  appVersion as apiAppVersion,
  checkForUpdate as apiCheckForUpdate,
  installPendingUpdate,
  type UpdateInfo,
} from '../lib/updater'
import type {
  PhpExtInfo,
  PhpVersionInfo,
  ProjectConfigInput,
  ProjectInfo,
  ServiceInfo,
} from '../types'
import type { UnlistenFn } from '@tauri-apps/api/event'

// Module-level handles (singleton store) — kept out of state.
let instTimer: ReturnType<typeof setInterval> | null = null
let wizTimer: ReturnType<typeof setInterval> | null = null
let instUnlisten: UnlistenFn | null = null
let wizUnlisten: UnlistenFn | null = null
let wizCeiling = 0
let cfgSavedTimer: ReturnType<typeof setTimeout> | null = null
let toastTimer: ReturnType<typeof setTimeout> | null = null
let confirmResolver: ((result: boolean) => void) | null = null

// Sample config text shown in browser preview (no Tauri backend).
const DEMO_CONFIG: Record<string, string> = {
  nginx: 'worker_processes  auto;\n\nhttp {\n    include       mime.types;\n    default_type  application/octet-stream;\n    sendfile      on;\n\n    include       "C:/Stackr/config/nginx/sites/*.conf";\n}\n',
  apache:
    'Define SRVROOT "C:/Stackr/bin/apache/2.4"\nServerRoot "${SRVROOT}"\nListen 80\nServerName localhost\nDirectoryIndex index.php index.html\nIncludeOptional "C:/Stackr/config/apache/sites/*.conf"\n',
  php: '[PHP]\nmemory_limit = 256M\nupload_max_filesize = 64M\npost_max_size = 64M\nmax_execution_time = 120\ndate.timezone = UTC\n\nextension=openssl\nextension=mbstring\nextension=curl\nextension=pdo_mysql\n',
}

const INITIAL_INST: InstallState = {
  open: false,
  name: '',
  component: '',
  versions: [],
  version: '',
  size: '',
  latest: false,
  phase: 'idle',
  progress: 0,
}

const INITIAL_CFG: ConfigEditorState = {
  open: false,
  component: '',
  version: '',
  label: '',
  path: '',
  hint: '',
  generated: false,
  content: '',
  original: '',
  loading: false,
  saving: false,
  saved: false,
  error: null,
}

const INITIAL_WIZ: WizardState = {
  open: false,
  step: 1,
  type: null,
  framework: null,
  frameworkVersion: null,
  name: '',
  git: '',
  importPath: '',
  docRoot: '',
  php: '8.2.14',
  server: 'Nginx',
  db: 'None',
  progress: 0,
  stepIdx: 0,
  steps: [],
  done: false,
  error: null,
}

// Map a wizard engine display name to its backend component id.
const SERVER_COMPONENT: Record<string, string> = { Nginx: 'nginx', Apache: 'apache' }
const DB_COMPONENT: Record<string, string> = { MySQL: 'mysql', MariaDB: 'mariadb', PostgreSQL: 'postgresql' }

/** Newest catalog version for an engine component (catalog lists newest first). */
function newestCatalogVersion(component: string): string | undefined {
  return [...SERVER_ENGINES, ...DB_ENGINES].find((e) => e.component === component)?.versions[0]
}

/** Resolve a recommended PHP minor ("8.3") to a concrete version: an installed
 *  build of that minor if present, else the newest installable patch, else the
 *  newest installable overall. */
function recommendPhp(minor: string, installed: PhpVersionInfo[], available: string[]): string {
  const have = installed.find((v) => v.majorMinor === minor)
  if (have) return have.version
  const avail = available.find((v) => v.startsWith(minor + '.'))
  return avail ?? available[0] ?? installed[0]?.version ?? `${minor}.0`
}

/** Scaffold step labels (mirrors the backend's emitted progress) for the UI. */
function scaffoldSteps(
  type: ProjectType,
  framework: string | null,
  server: string,
  hasDb: boolean,
): string[] {
  let steps: string[]
  if (type === 'Open existing') {
    steps = ['Preparing project', `Configuring ${server}`, 'Registering domain']
  } else if (type === 'Clone from Git') {
    steps = ['Creating project folder', 'Cloning repository', 'Installing dependencies', `Configuring ${server}`, 'Registering domain']
  } else if (type === 'Framework') {
    steps =
      framework === 'WordPress'
        ? ['Creating project folder', 'Downloading WordPress', `Configuring ${server}`, 'Registering domain']
        : ['Creating project folder', 'Installing Composer', `Setting up ${framework}`, `Configuring ${server}`, 'Registering domain']
  } else {
    steps = ['Creating project folder', 'Writing index.php', `Configuring ${server}`, 'Registering domain']
  }
  if (hasDb) steps = [...steps.slice(0, -1), 'Creating database', steps[steps.length - 1]]
  return steps
}

interface AppState {
  // nav
  nav: TabId
  go: (t: TabId) => void

  // php / extensions
  phpVersions: PhpVersionInfo[]
  loadPhpVersions: () => Promise<void>
  phpAvailable: string[]
  loadPhpAvailable: () => Promise<void>
  openPhpInstall: () => Promise<void>
  setDefaultPhpVersion: (version: string) => Promise<void>
  uninstallPhp: (version: string) => Promise<void>
  phpPanel: string | null
  togglePhpPanel: (version: string) => void
  extensions: PhpExtInfo[]
  extVersion: string | null
  extInstalling: Record<string, boolean>
  extTouched: string[]
  loadExtensions: (version: string) => Promise<void>
  toggleExt: (name: string) => void
  installExt: (name: string) => Promise<void>
  xdebug: Record<string, XdebugState>
  xdebugBusy: Record<string, boolean>
  loadXdebug: (version: string) => Promise<void>
  toggleXdebug: (version: string, enabled: boolean) => Promise<void>
  extSearch: string
  setExtSearch: (v: string) => void
  extModalOpen: boolean
  extModalSearch: string
  openAllExtensions: () => void
  closeAllExtensions: () => void
  setExtModalSearch: (v: string) => void

  // projects
  projects: ProjectInfo[]
  loadProjects: () => Promise<void>
  startProject: (id: string) => Promise<void>
  stopProject: (id: string) => Promise<void>
  setProjectPhp: (id: string, version: string) => Promise<void>
  setProjectDb: (id: string, database: string | null) => Promise<void>
  deleteProject: (id: string, deleteFiles?: boolean) => Promise<void>
  ides: IdeInfo[]
  loadIdes: () => Promise<void>
  openProjectInIde: (id: string, ide: string) => Promise<void>
  openProjectTerminal: (id: string) => Promise<void>

  // logs
  logComponent: string
  setLogComponent: (c: string) => void
  logRaw: LogRaw[]
  logLoading: boolean
  loadLog: () => Promise<void>
  clearLog: () => Promise<void>
  autoScroll: boolean
  toggleAutoScroll: () => void

  // settings
  settings: AppSettings
  loadSettings: () => Promise<void>
  system: SystemReport | null
  checkSystem: () => Promise<void>
  rootInfo: RootInfo | null
  checkRoot: () => Promise<void>
  chooseRoot: (path: string) => Promise<boolean>
  boot: () => Promise<void>
  defender: DefenderStatus | null
  defenderBusy: boolean
  checkDefender: () => Promise<void>
  addDefenderExclusion: () => Promise<void>
  https: HttpsStatus | null
  httpsBusy: boolean
  checkHttps: () => Promise<void>
  toggleHttps: (on: boolean) => Promise<void>
  appVersion: string
  update: UpdateInfo | null
  updateBusy: boolean
  updateProgress: number
  loadAppVersion: () => Promise<void>
  checkUpdate: (manual: boolean) => Promise<void>
  installUpdate: () => Promise<void>
  checkRestoreNotice: () => Promise<void>
  toggleSetting: (k: 'startup' | 'autostart' | 'notify') => void
  setSetting: <K extends keyof AppSettings>(k: K, v: AppSettings[K]) => void

  // installed components (from backend stackr.json)
  installed: InstalledComponent[]
  loadInstalled: () => Promise<void>

  // process services: servers / databases / caches / mail (shared start/stop/restart)
  servers: ServiceInfo[]
  databases: ServiceInfo[]
  caches: ServiceInfo[]
  mail: ServiceInfo[]
  loadServers: () => Promise<void>
  loadDatabases: () => Promise<void>
  loadCaches: () => Promise<void>
  loadMail: () => Promise<void>
  startService: (id: string) => Promise<void>
  stopService: (id: string) => Promise<void>
  restartService: (id: string) => Promise<void>
  uninstallService: (id: string) => Promise<void>
  exportDatabases: (component: string, version: string) => Promise<boolean>
  openEngineInstall: (component: string) => void
  openAdminer: () => Promise<void>

  // toast
  toast: ToastState | null
  showToast: (msg: string, kind?: ToastState['kind']) => void
  dismissToast: () => void

  // confirm dialog (styled replacement for window.confirm)
  confirm: ConfirmState | null
  confirmChecked: boolean // last dialog's opt-in checkbox state
  askConfirm: (opts: Partial<ConfirmState> & { message: string }) => Promise<boolean>
  resolveConfirm: (result: boolean, checked?: boolean) => void

  // install modal
  inst: InstallState
  openInstall: (
    name: string,
    component: string,
    versions: string[],
    size: string,
    latest?: boolean,
  ) => void
  closeInstall: () => void
  setInstVersion: (v: string) => void
  runInstall: () => void

  // config editor
  cfg: ConfigEditorState
  openConfig: (component: string, version: string) => Promise<void>
  closeConfig: () => void
  setConfigContent: (content: string) => void
  saveConfig: () => Promise<void>
  resetConfig: () => Promise<void>

  // wizard
  wiz: WizardState
  openWizard: () => void
  openImport: () => void
  closeWizard: () => void
  setWiz: (patch: Partial<WizardState>) => void
  selectType: (t: ProjectType) => void
  selectFramework: (name: FrameworkName) => void
  selectFrameworkVersion: (label: string) => void
  wizNext: () => void
  wizBack: () => void
  runWizInstall: () => void
}

export const useStore = create<AppState>((set, get) => ({
  // nav
  nav: 'projects',
  go: (nav) => set({ nav }),

  // php / extensions
  phpVersions: DEMO_PHP,
  loadPhpVersions: async () => {
    if (!isTauri()) return
    try {
      const versions = await getPhpVersions()
      // Extensions panels always start collapsed — opened on demand per version.
      set({ phpVersions: versions, phpPanel: null, extSearch: '', extTouched: [] })
    } catch (e) {
      console.error('loadPhpVersions failed', e)
    }
  },
  phpAvailable: [],
  loadPhpAvailable: async () => {
    if (!isTauri()) {
      // Demo list for browser preview.
      set({ phpAvailable: ['8.4.3', '8.3.16', '8.2.27', '8.1.31', '8.0.30', '7.4.33'] })
      return
    }
    try {
      set({ phpAvailable: await getPhpAvailable() })
    } catch (e) {
      console.error('get_php_available failed', e)
      get().showToast(`Could not load PHP versions: ${String(e)}`, 'error')
    }
  },
  openPhpInstall: async () => {
    if (!get().phpAvailable.length) await get().loadPhpAvailable()
    const installed = new Set(get().phpVersions.map((v) => v.version))
    const list = get().phpAvailable.filter((v) => !installed.has(v))
    if (!list.length) {
      get().showToast('No new PHP versions available to install.', 'info')
      return
    }
    get().openInstall('PHP', 'php', list, '30 MB')
  },
  setDefaultPhpVersion: async (version) => {
    if (!isTauri()) {
      set((s) => ({
        phpVersions: s.phpVersions.map((v) => ({
          ...v,
          isDefault: v.version === version,
          status: v.version === version ? 'active' : 'installed',
        })),
      }))
      return
    }
    await setDefaultPhp(version)
    await get().loadPhpVersions()
  },
  uninstallPhp: async (version) => {
    const ok = await get().askConfirm({
      title: `Uninstall PHP ${version}?`,
      message:
        'This PHP runtime will be removed. The shared PHP-FPM process is stopped first — restart your projects afterwards.',
      confirmLabel: 'Uninstall',
      danger: true,
    })
    if (!ok) return

    if (!isTauri()) {
      set((s) => ({
        phpVersions: s.phpVersions.filter((v) => v.version !== version),
        phpPanel: s.phpPanel === version ? null : s.phpPanel,
      }))
      return
    }

    try {
      await stopPhpRuntime(version) // release this version's php.exe lock before removing
      await uninstallComponent('php', version)
    } catch (e) {
      get().showToast(`Could not uninstall PHP ${version}: ${String(e)}`, 'error')
      await get().loadPhpVersions()
      return
    }
    await get().loadPhpVersions() // authoritative: reflects a reassigned default
    void get().loadInstalled()
  },
  phpPanel: null,
  togglePhpPanel: (version) => {
    const { phpPanel } = get()
    if (phpPanel === version) {
      set({ phpPanel: null, extSearch: '', extTouched: [] })
      return
    }
    // Search + sticky rows are per-version — clear them so state from one
    // runtime doesn't carry over to the next version's extension list.
    set({ phpPanel: version, extSearch: '', extTouched: [] })
    void get().loadExtensions(version)
  },
  extensions: DEMO_EXTENSIONS,
  extVersion: DEMO_PHP[0]?.version ?? null,
  extInstalling: {},
  extTouched: [],
  loadExtensions: async (version) => {
    if (!isTauri()) {
      set({ extensions: DEMO_EXTENSIONS, extVersion: version })
      return
    }
    try {
      set({ extensions: await listPhpExtensions(version), extVersion: version })
    } catch (e) {
      console.error('list_php_extensions failed', e)
      get().showToast(`Could not load extensions: ${String(e)}`, 'error')
    }
  },
  toggleExt: (name) => {
    const { extensions, extVersion } = get()
    const ext = extensions.find((e) => e.name === name)
    if (!ext) return
    // Not installed PECL extension → download + enable instead of toggling.
    if (!ext.installed && ext.pecl) {
      void get().installExt(name)
      return
    }
    const next = !ext.enabled
    // Keep the row visible after toggling off so it doesn't vanish from the
    // default (enabled-only) view — the user can flip it straight back.
    set((s) => ({
      extensions: s.extensions.map((e) => (e.name === name ? { ...e, enabled: next } : e)),
      extTouched: s.extTouched.includes(name) ? s.extTouched : [...s.extTouched, name],
    }))
    if (isTauri() && extVersion) {
      toggleExtensionApi(extVersion, name, next).catch((e) => {
        console.error('toggle_extension failed', e)
        get().showToast(`Could not toggle ${name}: ${String(e)}`, 'error')
        set((s) => ({
          extensions: s.extensions.map((x) => (x.name === name ? { ...x, enabled: !next } : x)),
        }))
      })
    }
  },
  installExt: async (name) => {
    const version = get().extVersion
    if (!version) return
    if (!isTauri()) {
      // Browser preview: simulate install.
      set((s) => ({
        extensions: s.extensions.map((e) =>
          e.name === name ? { ...e, installed: true, enabled: true } : e,
        ),
      }))
      return
    }
    set((s) => ({ extInstalling: { ...s.extInstalling, [name]: true } }))
    try {
      await installPhpExtension(version, name)
      await get().loadExtensions(version)
      get().showToast(`${name} installed — restart PHP to load it.`, 'info')
    } catch (e) {
      get().showToast(`Could not install ${name}: ${String(e)}`, 'error')
    } finally {
      set((s) => {
        const next = { ...s.extInstalling }
        delete next[name]
        return { extInstalling: next }
      })
    }
  },
  xdebug: {},
  xdebugBusy: {},
  loadXdebug: async (version) => {
    if (!isTauri()) return
    try {
      const st = await xdebugStatus(version)
      set((s) => ({ xdebug: { ...s.xdebug, [version]: st } }))
    } catch (e) {
      console.error('xdebug_status failed', e)
    }
  },
  toggleXdebug: async (version, enabled) => {
    if (!isTauri()) return
    set((s) => ({ xdebugBusy: { ...s.xdebugBusy, [version]: true } }))
    try {
      await setXdebug(version, enabled)
      const st = await xdebugStatus(version)
      set((s) => ({ xdebug: { ...s.xdebug, [version]: st } }))
      get().showToast(
        enabled
          ? `Xdebug on for PHP ${version} — step debugging on port ${st.port}.`
          : `Xdebug off for PHP ${version}.`,
        'info',
      )
    } catch (e) {
      get().showToast(`Could not toggle Xdebug: ${String(e)}`, 'error')
    } finally {
      set((s) => {
        const next = { ...s.xdebugBusy }
        delete next[version]
        return { xdebugBusy: next }
      })
    }
  },
  extSearch: '',
  setExtSearch: (extSearch) => set({ extSearch }),
  extModalOpen: false,
  extModalSearch: '',
  openAllExtensions: () => set({ extModalOpen: true, extModalSearch: '' }),
  closeAllExtensions: () => set({ extModalOpen: false }),
  setExtModalSearch: (extModalSearch) => set({ extModalSearch }),

  // projects
  projects: DEMO_PROJECTS,
  loadProjects: async () => {
    if (!isTauri()) return
    try {
      set({ projects: await getProjects() })
    } catch (e) {
      console.error('loadProjects failed', e)
    }
  },
  startProject: async (id) => {
    if (isTauri()) {
      try {
        await apiStartProject(id)
      } catch (e) {
        get().showToast(String(e), 'error')
      }
      await Promise.all([get().loadProjects(), get().loadServers()])
    } else {
      set((s) => ({ projects: s.projects.map((p) => (p.id === id ? { ...p, status: 'running' } : p)) }))
    }
  },
  stopProject: async (id) => {
    if (isTauri()) {
      try {
        await apiStopProject(id)
      } catch (e) {
        get().showToast(String(e), 'error')
      }
      await Promise.all([get().loadProjects(), get().loadServers()])
    } else {
      set((s) => ({ projects: s.projects.map((p) => (p.id === id ? { ...p, status: 'stopped' } : p)) }))
    }
  },
  setProjectPhp: async (id, version) => {
    if (!isTauri()) {
      set((s) => ({ projects: s.projects.map((p) => (p.id === id ? { ...p, phpVersion: version } : p)) }))
      return
    }
    try {
      await apiSetProjectPhp(id, version)
    } catch (e) {
      get().showToast(String(e), 'error')
    }
    await get().loadProjects()
  },
  setProjectDb: async (id, database) => {
    if (!isTauri()) {
      set((s) => ({ projects: s.projects.map((p) => (p.id === id ? { ...p, database } : p)) }))
      return
    }
    try {
      await apiSetProjectDb(id, database)
      const label = database && database.toLowerCase() !== 'none'
      get().showToast(label ? `Database set to ${database}.` : 'Database removed from project.', 'info')
    } catch (e) {
      get().showToast(String(e), 'error')
    }
    await get().loadProjects()
  },
  deleteProject: async (id, deleteFiles = false) => {
    if (!isTauri()) {
      set((s) => ({ projects: s.projects.filter((p) => p.id !== id) }))
      return
    }
    try {
      await apiDeleteProject(id, deleteFiles)
    } catch (e) {
      get().showToast(String(e), 'error')
    }
    await get().loadProjects()
  },
  ides: [],
  loadIdes: async () => {
    if (!isTauri()) {
      set({ ides: [{ id: 'vscode', name: 'VS Code' }] })
      return
    }
    try {
      set({ ides: await detectIdes() })
    } catch (e) {
      console.error('detect_ides failed', e)
    }
  },
  openProjectInIde: async (id, ide) => {
    if (!isTauri()) return
    try {
      await openInIde(id, ide)
    } catch (e) {
      get().showToast(String(e), 'error')
    }
  },
  openProjectTerminal: async (id) => {
    if (!isTauri()) return
    try {
      await openTerminal(id)
    } catch (e) {
      get().showToast(String(e), 'error')
    }
  },

  // logs
  logComponent: '',
  setLogComponent: (logComponent) => {
    set({ logComponent, logRaw: [] })
    void get().loadLog()
  },
  logRaw: [],
  logLoading: false,
  loadLog: async () => {
    const c = get().logComponent
    if (!c) {
      set({ logRaw: [], logLoading: false })
      return
    }
    // Only show the loader on the first fetch (empty view) — not on each poll.
    if (get().logRaw.length === 0) set({ logLoading: true })
    if (isTauri()) {
      try {
        const raw =
          c === 'all'
            ? await readAllLogs(500)
            : (await readLog(c, 500)).map((line) => ({ service: c, line }))
        set({ logRaw: raw })
      } catch (e) {
        console.error('read log failed', e)
      } finally {
        set({ logLoading: false })
      }
    } else {
      // Browser preview: synthesize a tagged stream from the demo entries.
      set({
        logRaw: ALL_LOGS.filter((l) => c === 'all' || l.svc === c).map((l) => ({
          service: l.svc,
          line: `[${l.t}] ${l.lvl.toUpperCase()}: ${l.m}`,
        })),
        logLoading: false,
      })
    }
  },
  clearLog: async () => {
    const c = get().logComponent
    if (isTauri() && c) {
      try {
        if (c === 'all') await clearAllLogs()
        else await apiClearLog(c)
      } catch (e) {
        console.error('clear log failed', e)
      }
    }
    set({ logRaw: [] })
  },
  autoScroll: true,
  toggleAutoScroll: () => set((s) => ({ autoScroll: !s.autoScroll })),

  // settings
  settings: { startup: false, autostart: false, notify: true, sitesDir: 'C:\\Stackr\\www', tld: '.test' },
  loadSettings: async () => {
    if (!isTauri()) return
    try {
      set({ settings: await getSettings() })
    } catch (e) {
      console.error('loadSettings failed', e)
    }
  },
  system: null,
  checkSystem: async () => {
    if (!isTauri()) return
    try {
      const report = await systemReport()
      set({ system: report })
      if (!report.vcredist) {
        get().showToast(
          'Microsoft Visual C++ x64 Redistributable is missing — PHP, MySQL and other engines may fail to start. Install it from aka.ms/vs/17/release/vc_redist.x64.exe',
          'error',
        )
      }
    } catch (e) {
      console.error('checkSystem failed', e)
    }
  },
  checkRestoreNotice: async () => {
    if (!isTauri()) return
    try {
      if (await takeRestoreNotice()) {
        get().showToast(
          'Stackr recovered its state from a backup — the last save was interrupted.',
          'info',
        )
      }
    } catch (e) {
      console.error('checkRestoreNotice failed', e)
    }
  },
  rootInfo: null,
  checkRoot: async () => {
    if (!isTauri()) return
    try {
      set({ rootInfo: await getRootInfo() })
    } catch (e) {
      console.error('checkRoot failed', e)
    }
  },
  chooseRoot: async (path) => {
    if (!isTauri()) return false
    try {
      await apiSetRoot(path)
      set({ rootInfo: await getRootInfo() })
      return true
    } catch (e) {
      get().showToast(`Could not set the data folder: ${String(e)}`, 'error')
      return false
    }
  },
  boot: async () => {
    await Promise.all([
      get().loadInstalled(),
      get().loadServers(),
      get().loadDatabases(),
      get().loadCaches(),
      get().loadMail(),
      get().loadPhpVersions(),
      get().loadProjects(),
      get().loadSettings(),
    ])
  },
  defender: null,
  defenderBusy: false,
  checkDefender: async () => {
    if (!isTauri()) return
    try {
      set({ defender: await defenderStatus() })
    } catch (e) {
      console.error('checkDefender failed', e)
    }
  },
  addDefenderExclusion: async () => {
    if (!isTauri() || get().defenderBusy) return
    set({ defenderBusy: true })
    try {
      await apiAddDefenderExclusion()
      set({ defender: await defenderStatus() })
      get().showToast('Added C:\\Stackr to Windows Defender exclusions.', 'info')
    } catch (e) {
      get().showToast(String(e), 'error')
    } finally {
      set({ defenderBusy: false })
    }
  },
  https: null,
  httpsBusy: false,
  checkHttps: async () => {
    if (!isTauri()) return
    try {
      set({ https: await httpsStatus() })
    } catch (e) {
      console.error('checkHttps failed', e)
    }
  },
  toggleHttps: async (on) => {
    if (!isTauri() || get().httpsBusy) return
    set({ httpsBusy: true })
    try {
      const status = on ? await apiEnableHttps() : await apiDisableHttps()
      set({ https: status })
      get().showToast(
        on
          ? 'HTTPS enabled — start or restart a project to serve it over https://.'
          : 'HTTPS disabled — projects revert to http:// on next start.',
        'info',
      )
    } catch (e) {
      get().showToast(String(e), 'error')
    } finally {
      set({ httpsBusy: false })
    }
  },
  appVersion: '',
  update: null,
  updateBusy: false,
  updateProgress: 0,
  loadAppVersion: async () => {
    if (!isTauri()) return
    try {
      set({ appVersion: await apiAppVersion() })
    } catch (e) {
      console.error('loadAppVersion failed', e)
    }
  },
  checkUpdate: async (manual) => {
    if (!isTauri()) return
    try {
      const info = await apiCheckForUpdate()
      set({ update: info })
      if (manual && !info) get().showToast("You're on the latest version.", 'info')
    } catch (e) {
      // Endpoint may 404 until the first release is published — stay quiet on the
      // automatic boot check; only surface a manual "Check for updates" failure.
      if (manual) get().showToast(`Update check failed: ${String(e)}`, 'error')
      else console.error('checkUpdate failed', e)
    }
  },
  installUpdate: async () => {
    if (!isTauri() || get().updateBusy || !get().update) return
    set({ updateBusy: true, updateProgress: 0 })
    try {
      await installPendingUpdate((pct) => set({ updateProgress: pct }))
      // On success the app relaunches; this line is effectively unreachable.
    } catch (e) {
      get().showToast(`Update failed: ${String(e)}`, 'error')
      set({ updateBusy: false })
    }
  },
  toggleSetting: (k) => {
    const next = { ...get().settings, [k]: !get().settings[k] }
    set({ settings: next })
    if (isTauri()) saveSettings(next).catch((e) => console.error('save_settings failed', e))
  },
  setSetting: (k, v) => {
    const next = { ...get().settings, [k]: v }
    set({ settings: next })
    if (isTauri()) saveSettings(next).catch((e) => console.error('save_settings failed', e))
  },

  // installed components
  installed: [],
  loadInstalled: async () => {
    if (!isTauri()) return
    try {
      set({ installed: await getInstalled() })
    } catch (e) {
      console.error('loadInstalled failed', e)
    }
  },

  // process services
  servers: [],
  databases: isTauri() ? [] : DEMO_DATABASES,
  caches: isTauri() ? [] : DEMO_CACHES,
  mail: isTauri() ? [] : DEMO_MAIL,
  loadServers: async () => {
    if (!isTauri()) return
    try {
      set({ servers: await getServers() })
    } catch (e) {
      console.error('loadServers failed', e)
    }
  },
  loadDatabases: async () => {
    if (!isTauri()) return
    try {
      set({ databases: await getDatabases() })
    } catch (e) {
      console.error('loadDatabases failed', e)
    }
  },
  loadCaches: async () => {
    if (!isTauri()) return
    try {
      set({ caches: await getCaches() })
    } catch (e) {
      console.error('loadCaches failed', e)
    }
  },
  loadMail: async () => {
    if (!isTauri()) return
    try {
      set({ mail: await getMail() })
    } catch (e) {
      console.error('loadMail failed', e)
    }
  },
  startService: async (id) => {
    try {
      await apiStartService(id)
    } catch (e) {
      get().showToast(String(e), 'error')
    }
    await Promise.all([get().loadServers(), get().loadDatabases(), get().loadCaches(), get().loadMail()])
  },
  stopService: async (id) => {
    try {
      await apiStopService(id)
    } catch (e) {
      get().showToast(String(e), 'error')
    }
    await Promise.all([get().loadServers(), get().loadDatabases(), get().loadCaches(), get().loadMail()])
  },
  restartService: async (id) => {
    try {
      await apiRestartService(id)
    } catch (e) {
      get().showToast(String(e), 'error')
    }
    await Promise.all([get().loadServers(), get().loadDatabases(), get().loadCaches(), get().loadMail()])
  },
  uninstallService: async (id) => {
    const i = id.indexOf('-')
    const component = i === -1 ? id : id.slice(0, i)
    const version = i === -1 ? '' : id.slice(i + 1)
    if (!isTauri()) {
      set((s) => ({ servers: s.servers.filter((x) => x.id !== id) }))
      return
    }
    try {
      await apiStopService(id) // stop first so the files aren't locked
      await uninstallComponent(component, version)
    } catch (e) {
      get().showToast(String(e), 'error')
    }
    await Promise.all([
      get().loadInstalled(),
      get().loadServers(),
      get().loadDatabases(),
      get().loadCaches(),
      get().loadMail(),
    ])
  },
  exportDatabases: async (component, version) => {
    if (!isTauri()) return false
    try {
      const path = await apiExportDatabases(component, version)
      get().showToast(`Databases exported to ${path}`, 'info')
      return true
    } catch (e) {
      get().showToast(`Export failed: ${String(e)}`, 'error')
      return false
    }
  },
  openEngineInstall: (component) => {
    const meta = [...DB_ENGINES, ...CACHE_ENGINES, ...MAIL_ENGINES].find((e) => e.component === component)
    if (!meta) return
    // Offer only versions not already installed for this engine.
    const installed = new Set(
      [...get().databases, ...get().caches, ...get().mail]
        .filter((s) => s.component === component)
        .map((s) => s.version),
    )
    const list = meta.versions.filter((v) => !installed.has(v))
    if (!list.length) {
      get().showToast(`All available ${meta.name} versions are installed.`, 'info')
      return
    }
    get().openInstall(meta.name, meta.component, list, meta.size)
  },
  openAdminer: async () => {
    if (!isTauri()) {
      window.open('https://www.adminer.org', '_blank')
      return
    }
    try {
      await apiOpenAdminer()
      await get().loadServers()
    } catch (e) {
      get().showToast(`Could not open Adminer: ${String(e)}`, 'error')
    }
  },

  // toast
  toast: null,
  showToast: (msg, kind = 'info') => {
    if (toastTimer) clearTimeout(toastTimer)
    set({ toast: { msg, kind } })
    toastTimer = setTimeout(() => set({ toast: null }), 6000)
  },
  dismissToast: () => {
    if (toastTimer) {
      clearTimeout(toastTimer)
      toastTimer = null
    }
    set({ toast: null })
  },

  // confirm dialog
  confirm: null,
  confirmChecked: false,
  askConfirm: (opts) =>
    new Promise<boolean>((resolve) => {
      // Resolve any previous, unanswered dialog as cancelled.
      if (confirmResolver) confirmResolver(false)
      confirmResolver = resolve
      set({
        confirm: {
          title: opts.title ?? 'Are you sure?',
          message: opts.message,
          confirmLabel: opts.confirmLabel ?? 'Confirm',
          cancelLabel: opts.cancelLabel ?? 'Cancel',
          danger: opts.danger ?? false,
          checkbox: opts.checkbox,
        },
        confirmChecked: opts.checkbox?.defaultChecked ?? false,
      })
    }),
  resolveConfirm: (result, checked = false) => {
    const r = confirmResolver
    confirmResolver = null
    set({ confirm: null, confirmChecked: checked })
    if (r) r(result)
  },

  // install modal
  inst: { ...INITIAL_INST },
  openInstall: (name, component, versions, size, latest = false) =>
    set({
      inst: {
        open: true,
        name,
        component,
        versions,
        version: latest ? 'latest' : versions[0],
        size,
        latest,
        phase: 'idle',
        progress: 0,
      },
    }),
  closeInstall: () => {
    if (instTimer) {
      clearInterval(instTimer)
      instTimer = null
    }
    if (instUnlisten) {
      instUnlisten()
      instUnlisten = null
    }
    set((s) => ({ inst: { ...s.inst, open: false } }))
  },
  setInstVersion: (version) => set((s) => ({ inst: { ...s.inst, version } })),
  runInstall: () => {
    const { inst } = get()

    // Browser/Vite dev (no Tauri): keep the simulated progress for previewing.
    if (!isTauri()) {
      if (instTimer) clearInterval(instTimer)
      set((s) => ({ inst: { ...s.inst, phase: 'installing', progress: 0 } }))
      instTimer = setInterval(() => {
        set((s) => {
          const p = Math.min(100, s.inst.progress + Math.random() * 15 + 7)
          const done = p >= 100
          if (done && instTimer) {
            clearInterval(instTimer)
            instTimer = null
          }
          return { inst: { ...s.inst, progress: p, phase: done ? 'done' : 'installing' } }
        })
      }, 300)
      return
    }

    // Real install through the backend, driven by download-progress events.
    set((s) => ({ inst: { ...s.inst, phase: 'installing', progress: 0 } }))
    const component = inst.component
    onDownloadProgress((p) => {
      if (p.component !== component) return
      set((s) => (s.inst.phase === 'installing' ? { inst: { ...s.inst, progress: p.percent } } : s))
    }).then((un) => {
      instUnlisten = un
    })
    installComponent(inst.component, inst.version)
      .then(() => {
        set((s) => ({ inst: { ...s.inst, progress: 100, phase: 'done' } }))
        void get().loadInstalled()
        void get().loadServers()
        void get().loadDatabases()
        void get().loadCaches()
        void get().loadMail()
        void get().loadPhpVersions()
      })
      .catch((err) => {
        console.error('install_component failed', err)
        set((s) => ({ inst: { ...s.inst, phase: 'idle', progress: 0 } }))
      })
      .finally(() => {
        if (instUnlisten) {
          instUnlisten()
          instUnlisten = null
        }
      })
  },

  // config editor
  cfg: { ...INITIAL_CFG },
  openConfig: async (component, version) => {
    if (cfgSavedTimer) {
      clearTimeout(cfgSavedTimer)
      cfgSavedTimer = null
    }
    set({ cfg: { ...INITIAL_CFG, open: true, component, version, loading: true } })

    if (!isTauri()) {
      const content = DEMO_CONFIG[component] ?? '# config\n'
      const label = component === 'nginx' ? 'nginx.conf' : component === 'apache' ? 'httpd.conf' : 'php.ini'
      set((s) => ({
        cfg: {
          ...s.cfg,
          loading: false,
          label,
          path: `C:\\Stackr\\config\\${label}`,
          hint: 'Preview only — no backend connected.',
          generated: component !== 'php',
          content,
          original: content,
        },
      }))
      return
    }

    try {
      const doc = await readServiceConfig(component, version)
      set((s) =>
        // Ignore if the user already closed/switched while loading.
        s.cfg.open && s.cfg.component === component
          ? {
              cfg: {
                ...s.cfg,
                loading: false,
                label: doc.label,
                path: doc.path,
                hint: doc.hint,
                generated: doc.generated,
                content: doc.content,
                original: doc.content,
              },
            }
          : s,
      )
    } catch (e) {
      set((s) => ({ cfg: { ...s.cfg, loading: false, error: String(e) } }))
    }
  },
  closeConfig: () => {
    if (cfgSavedTimer) {
      clearTimeout(cfgSavedTimer)
      cfgSavedTimer = null
    }
    set((s) => ({ cfg: { ...s.cfg, open: false } }))
  },
  setConfigContent: (content) =>
    set((s) => ({ cfg: { ...s.cfg, content, saved: false, error: null } })),
  saveConfig: async () => {
    const { cfg } = get()
    if (cfg.saving) return
    set((s) => ({ cfg: { ...s.cfg, saving: true, error: null } }))
    try {
      if (isTauri()) await saveServiceConfig(cfg.component, cfg.version, cfg.content)
      set((s) => ({ cfg: { ...s.cfg, saving: false, saved: true, original: s.cfg.content } }))
      if (cfgSavedTimer) clearTimeout(cfgSavedTimer)
      cfgSavedTimer = setTimeout(() => set((s) => ({ cfg: { ...s.cfg, saved: false } })), 1800)
    } catch (e) {
      set((s) => ({ cfg: { ...s.cfg, saving: false, error: String(e) } }))
    }
  },
  resetConfig: async () => {
    const { cfg } = get()
    if (!cfg.generated) return
    set((s) => ({ cfg: { ...s.cfg, saving: true, error: null } }))
    try {
      if (isTauri()) {
        const doc = await resetServiceConfig(cfg.component, cfg.version)
        set((s) => ({
          cfg: { ...s.cfg, saving: false, saved: true, content: doc.content, original: doc.content },
        }))
      } else {
        const content = DEMO_CONFIG[cfg.component] ?? '# config\n'
        set((s) => ({ cfg: { ...s.cfg, saving: false, saved: true, content, original: content } }))
      }
      if (cfgSavedTimer) clearTimeout(cfgSavedTimer)
      cfgSavedTimer = setTimeout(() => set((s) => ({ cfg: { ...s.cfg, saved: false } })), 1800)
    } catch (e) {
      set((s) => ({ cfg: { ...s.cfg, saving: false, error: String(e) } }))
    }
  },

  // wizard
  wiz: { ...INITIAL_WIZ },
  openWizard: async () => {
    // Defaults from current state: installed DB (else None), running/installed
    // web server (else Nginx), default/installed PHP (else newest installable).
    const pick = () => {
      const { databases, servers, phpVersions, phpAvailable } = get()
      const db = databases[0]?.name ?? 'None'
      const server =
        servers.find((s) => s.status === 'running')?.name ??
        servers.find((s) => s.component === 'nginx')?.name ??
        servers[0]?.name ??
        'Nginx'
      const php =
        phpVersions.find((v) => v.isDefault)?.version ??
        phpVersions[0]?.version ??
        phpAvailable[0] ??
        INITIAL_WIZ.php
      return { db, server, php }
    }
    // Open immediately so the modal is responsive…
    set({ wiz: { ...INITIAL_WIZ, ...pick(), open: true } })
    if (!isTauri()) return
    // …then refresh what's installed + what's installable, and re-derive the
    // defaults (so the Configure step can offer to install missing prerequisites).
    await Promise.all([
      get().loadInstalled(),
      get().loadPhpVersions(),
      get().loadServers(),
      get().loadDatabases(),
      get().loadPhpAvailable(),
    ])
    const s = get()
    if (s.wiz.open && !s.wiz.framework) set({ wiz: { ...s.wiz, ...pick() } })
  },
  openImport: async () => {
    // "Open existing" opens the same modal in import mode, straight to Configure.
    const pick = () => {
      const { servers, phpVersions, phpAvailable } = get()
      const server =
        servers.find((s) => s.status === 'running')?.name ??
        servers.find((s) => s.component === 'nginx')?.name ??
        servers[0]?.name ??
        'Nginx'
      const php =
        phpVersions.find((v) => v.isDefault)?.version ??
        phpVersions[0]?.version ??
        phpAvailable[0] ??
        INITIAL_WIZ.php
      return { server, php }
    }
    set({ wiz: { ...INITIAL_WIZ, type: 'Open existing', step: 3, db: 'None', ...pick(), open: true } })
    if (!isTauri()) return
    await Promise.all([
      get().loadInstalled(),
      get().loadPhpVersions(),
      get().loadServers(),
      get().loadDatabases(),
      get().loadPhpAvailable(),
    ])
    const s = get()
    if (s.wiz.open && s.wiz.type === 'Open existing' && !s.wiz.importPath) {
      set({ wiz: { ...s.wiz, ...pick() } })
    }
  },
  closeWizard: () => {
    if (wizTimer) {
      clearInterval(wizTimer)
      wizTimer = null
    }
    if (wizUnlisten) {
      wizUnlisten()
      wizUnlisten = null
    }
    set((s) => ({ wiz: { ...s.wiz, open: false } }))
  },
  setWiz: (patch) => set((s) => ({ wiz: { ...s.wiz, ...patch } })),
  selectType: (type) => set((s) => ({ wiz: { ...s.wiz, type } })),
  selectFramework: (framework) =>
    set((s) => {
      const meta = FRAMEWORKS.find((f) => f.name === framework)
      const v = meta?.versions[0]
      return {
        wiz: {
          ...s.wiz,
          framework,
          frameworkVersion: v ? v.constraint : null,
          php: v ? recommendPhp(v.php, s.phpVersions, s.phpAvailable) : s.wiz.php,
          name: s.wiz.name || framework.toLowerCase() + '-app',
        },
      }
    }),
  selectFrameworkVersion: (label) =>
    set((s) => {
      const meta = FRAMEWORKS.find((f) => f.name === s.wiz.framework)
      const v = meta?.versions.find((x) => x.label === label)
      if (!v) return s
      return {
        wiz: {
          ...s.wiz,
          frameworkVersion: v.constraint,
          php: recommendPhp(v.php, s.phpVersions, s.phpAvailable),
        },
      }
    }),
  wizNext: () => {
    const { wiz } = get()
    let step = wiz.step
    if (step === 1) step = wiz.type === 'Framework' ? 2 : 3
    else if (step === 2) step = 3
    else if (step === 3) step = 4
    set((s) => ({ wiz: { ...s.wiz, step: step as WizardState['step'] } }))
    if (step === 4) get().runWizInstall()
  },
  wizBack: () => {
    const { wiz } = get()
    let step = wiz.step
    if (step === 4) step = 3
    else if (step === 3) step = wiz.type === 'Framework' ? 2 : 1
    else if (step === 2) step = 1
    if (wizTimer) {
      clearInterval(wizTimer)
      wizTimer = null
    }
    set((s) => ({ wiz: { ...s.wiz, step: step as WizardState['step'], error: null } }))
  },
  runWizInstall: () => {
    if (wizTimer) clearInterval(wizTimer)
    if (wizUnlisten) {
      wizUnlisten()
      wizUnlisten = null
    }
    set((s) => ({ wiz: { ...s.wiz, progress: 0, stepIdx: 0, done: false, error: null } }))

    // Browser preview (no Tauri): keep the simulated full animation.
    if (!isTauri()) {
      const total = 5
      wizTimer = setInterval(() => {
        set((s) => {
          const p = Math.min(100, s.wiz.progress + 100 / (total * 3.2) + Math.random() * 3)
          const stepIdx = Math.min(total, Math.floor(p / (100 / total)))
          const done = p >= 100
          if (done && wizTimer) {
            clearInterval(wizTimer)
            wizTimer = null
          }
          return { wiz: { ...s.wiz, progress: p, stepIdx, done } }
        })
      }, 300)
      return
    }

    // Real creation. First install any missing prerequisites (PHP / web server /
    // database) the user picked — skipping ones already installed — then scaffold
    // the project. The backend raises a percent "ceiling" via progress events; we
    // ease the bar toward it so it keeps moving while work runs.
    const w = get().wiz
    const dbChosen = w.db && w.db !== 'None' ? w.db : null
    const { phpVersions, servers, databases } = get()

    // Missing prerequisites, in install order. PHP is the exact chosen build; the
    // web server is global (needed only when none is installed); the database is
    // optional (only if chosen and its engine isn't installed).
    const prereqs: Array<{ component: string; version: string; label: string }> = []
    if (w.php && !phpVersions.some((v) => v.version === w.php)) {
      prereqs.push({ component: 'php', version: w.php, label: `Installing PHP ${w.php}` })
    }
    if (!servers.length) {
      const comp = SERVER_COMPONENT[w.server] ?? 'nginx'
      prereqs.push({ component: comp, version: newestCatalogVersion(comp) ?? 'latest', label: `Installing ${w.server}` })
    }
    if (dbChosen) {
      const comp = DB_COMPONENT[dbChosen]
      if (comp && !databases.some((d) => d.component === comp)) {
        prereqs.push({ component: comp, version: newestCatalogVersion(comp) ?? 'latest', label: `Installing ${dbChosen}` })
      }
    }

    const scaffold = scaffoldSteps(w.type ?? 'Blank PHP', w.framework, w.server, !!dbChosen)
    const steps = [...prereqs.map((p) => p.label), ...scaffold]
    const P = prereqs.length
    const preEnd = P > 0 ? 45 : 0 // installs occupy [0, preEnd]; scaffold the rest
    set((s) => ({ wiz: { ...s.wiz, steps, stepIdx: 0 } }))

    const config: ProjectConfigInput = {
      name: w.name,
      type: w.type ?? 'Blank PHP',
      framework: w.type === 'Framework' ? w.framework : null,
      frameworkVersion: w.type === 'Framework' ? w.frameworkVersion : null,
      phpVersion: w.php,
      webServer: w.server,
      database: dbChosen,
      domain: `${w.name || 'my-project'}.${get().settings.tld.replace(/^\.+/, '')}`,
      path: w.type === 'Open existing' ? w.importPath : '',
      docRoot: w.type === 'Open existing' ? w.docRoot : null,
      gitUrl: w.type === 'Clone from Git' ? w.git.trim() : null,
    }

    wizCeiling = 0
    wizTimer = setInterval(() => {
      set((s) => {
        if (s.wiz.done || s.wiz.error) return s
        const p = s.wiz.progress + (Math.min(wizCeiling, 99) - s.wiz.progress) * 0.12
        return { wiz: { ...s.wiz, progress: p } }
      })
    }, 150)

    const finish = (patch: Partial<WizardState>) => {
      if (wizTimer) {
        clearInterval(wizTimer)
        wizTimer = null
      }
      if (wizUnlisten) {
        wizUnlisten()
        wizUnlisten = null
      }
      set((s) => ({ wiz: { ...s.wiz, ...patch } }))
    }

    const run = async () => {
      // 1) Install missing prerequisites one at a time.
      for (let i = 0; i < P; i++) {
        set((s) => ({ wiz: { ...s.wiz, stepIdx: i } }))
        wizCeiling = (preEnd * (i + 0.92)) / P
        await installComponent(prereqs[i].component, prereqs[i].version)
        wizCeiling = (preEnd * (i + 1)) / P
      }
      if (P > 0) {
        await Promise.all([
          get().loadInstalled(),
          get().loadPhpVersions(),
          get().loadServers(),
          get().loadDatabases(),
        ])
      }

      // 2) Scaffold — map the backend's 0–100% into the remaining [preEnd, 100].
      wizUnlisten = await onProjectProgress((p) => {
        wizCeiling = Math.max(wizCeiling, preEnd + ((100 - preEnd) * p.percent) / 100)
        const sIdx = Math.min(scaffold.length - 1, Math.floor((p.percent / 100) * scaffold.length))
        set((s) => (s.wiz.done || s.wiz.error ? s : { wiz: { ...s.wiz, stepIdx: P + sIdx } }))
      })
      wizCeiling = Math.max(wizCeiling, preEnd + 3)
      await createProject(config)
      finish({ progress: 100, stepIdx: steps.length, done: true })
      void get().loadProjects()
    }

    run().catch((e) => finish({ error: String(e) }))
  },
}))
