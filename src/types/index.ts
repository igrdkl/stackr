// ---- Navigation ----
export type TabId =
  | 'servers'
  | 'php'
  | 'databases'
  | 'cache'
  | 'mail'
  | 'projects'
  | 'logs'
  | 'settings'

// ---- Service / status ----
export type ServiceStatus = 'running' | 'stopped' | 'installing' | 'error'
export type ServerType = 'nginx' | 'apache'
export type DatabaseType = 'mysql' | 'mariadb' | 'postgresql' | 'sqlite'
export type CacheType = 'redis' | 'memcached'

/** Live run-state of a service, from the backend's process + port health probe. */
export type ServiceRunState = 'running' | 'stopped' | 'starting' | 'unhealthy'

/** A running/installed service as reported by the backend. */
export interface ServiceInfo {
  id: string // "{component}-{version}"
  component: string
  name: string
  version: string
  status: ServiceRunState
  port: number
}

// ---- PHP ----
export interface PhpExtensionMeta {
  id: string // "opcache"
  desc: string // "Zend OPcache"
}

export interface PhpVersionInfo {
  version: string // "8.2.14"
  majorMinor: string // "8.2"
  status: 'active' | 'installed'
  isDefault: boolean
  note?: string // optional sub-line (demo text); generated when absent
}

/** An extension as reported by the backend (from the build's real ext/ dir). */
export interface PhpExtInfo {
  name: string // "pdo_mysql"
  enabled: boolean // active (uncommented) in php.ini
  installed: boolean // DLL present in ext/
  pecl: boolean // installable via PECL (not bundled)
  description: string
}

// ---- Projects ----
export type ProjectType = 'Blank PHP' | 'Framework' | 'Clone from Git' | 'Open existing'

export type FrameworkName =
  | 'Laravel'
  | 'Symfony'
  | 'WordPress'
  | 'CodeIgniter'
  | 'Yii2'
  | 'CakePHP'
  | 'Slim'

export interface ProjectMeta {
  id: string // slug, e.g. "my-shop"
  mark: string // monogram letter(s)
  markBg: string
  markColor: string
  fw: string // "Laravel 11"
  fwColor: string
  fwBg: string
  php: string // "8.2"
  server: string // "Nginx"
  domain: string // "shop.test"
}

// ---- Project (from backend) ----
export interface ProjectInfo {
  id: string
  name: string
  type: string
  framework: string | null
  phpVersion: string
  webServer: string
  database: string | null
  domain: string
  path: string
  status: 'running' | 'stopped'
  gitUrl: string | null
  createdAt: string
}

export interface ProjectConfigInput {
  name: string
  type: string
  framework: string | null
  frameworkVersion?: string | null
  docRoot?: string | null
  phpVersion: string
  webServer: string
  database: string | null
  domain: string
  path: string
  gitUrl: string | null
}

// ---- Logs ----
export type LogLevel = 'info' | 'warn' | 'error'
export type LogService = 'all' | 'nginx' | 'php' | 'mysql'

export interface LogEntry {
  svc: Exclude<LogService, 'all'>
  lvl: LogLevel
  t: string // timestamp "14:02:09"
  m: string // message
}

// ---- Install modal ----
export type InstallPhase = 'idle' | 'installing' | 'done'

export interface InstallState {
  open: boolean
  name: string
  component: string // canonical id passed to the backend: "nginx", "php", …
  versions: string[]
  version: string
  size: string
  latest: boolean // install newest available (no version picker)
  phase: InstallPhase
  progress: number
}

// ---- Toast (transient notifications) ----
export interface ToastState {
  msg: string
  kind: 'error' | 'info'
}

// ---- Confirm dialog (in-app replacement for window.confirm) ----
export interface ConfirmState {
  title: string
  message: string
  confirmLabel: string
  cancelLabel: string
  danger: boolean
  /** Optional opt-in checkbox (e.g. "also delete files"); its state is reported
   *  back via the store's `confirmChecked` after the dialog resolves. */
  checkbox?: { label: string; defaultChecked?: boolean }
}

// ---- New Project wizard ----
export interface WizardState {
  open: boolean
  step: 1 | 2 | 3 | 4
  type: ProjectType | null
  framework: FrameworkName | null
  frameworkVersion: string | null // Composer constraint for the chosen version, e.g. "^11"
  name: string
  git: string
  importPath: string // chosen folder for "Open existing"
  docRoot: string // document-root subdir for "Open existing" (relative to importPath)
  php: string
  server: string
  db: string
  progress: number
  stepIdx: number
  steps: string[] // full ordered step labels for the install phase (prereqs + scaffold)
  done: boolean
  error: string | null
}

// ---- Config editor ----
export interface ConfigEditorState {
  open: boolean
  component: string // "nginx" | "apache" | "php"
  version: string
  label: string // "nginx.conf" | "httpd.conf" | "php.ini"
  path: string
  hint: string
  generated: boolean // backend can regenerate defaults → show "Reset"
  content: string
  original: string // last-saved contents (dirty check)
  loading: boolean
  saving: boolean
  saved: boolean // brief post-save confirmation
  error: string | null
}

// ---- Settings ----
export interface AppSettings {
  startup: boolean
  autostart: boolean
  notify: boolean
  sitesDir: string
  tld: string
}

// ---- Engine metadata (servers / databases / cache install + installed cards) ----
export interface EngineMeta {
  component: string // backend id: "nginx", "mysql", "redis", …
  name: string
  mark: string
  markBg: string
  markColor: string
  versions: string[]
  size: string
  desc: string
  recommended?: boolean // primary install button (else ghost)
}
