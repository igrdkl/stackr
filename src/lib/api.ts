import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AppSettings,
  PhpExtInfo,
  PhpVersionInfo,
  ProjectConfigInput,
  ProjectInfo,
  ServiceInfo,
} from '../types'

/** True only when running inside the Tauri shell (not plain Vite/browser dev). */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export interface DownloadProgress {
  component: string
  percent: number
  bytesDownloaded: number
  totalBytes: number
}

export interface InstalledComponent {
  component: string
  name: string
  version: string
  path: string
}

export function installComponent(componentType: string, version: string): Promise<void> {
  return invoke('install_component', { componentType, version })
}

export function uninstallComponent(componentType: string, version: string): Promise<void> {
  return invoke('uninstall_component', { componentType, version })
}

export function getInstalled(): Promise<InstalledComponent[]> {
  return invoke('get_installed')
}

export function onDownloadProgress(cb: (p: DownloadProgress) => void): Promise<UnlistenFn> {
  return listen<DownloadProgress>('download-progress', (e) => cb(e.payload))
}

export interface ProjectProgress {
  step: string
  percent: number
}

export function onProjectProgress(cb: (p: ProjectProgress) => void): Promise<UnlistenFn> {
  return listen<ProjectProgress>('project-install-progress', (e) => cb(e.payload))
}

// ---- Process services (servers / databases / caches) ----
export function getServers(): Promise<ServiceInfo[]> {
  return invoke('get_servers')
}
export function getDatabases(): Promise<ServiceInfo[]> {
  return invoke('get_databases')
}
export function getCaches(): Promise<ServiceInfo[]> {
  return invoke('get_caches')
}
export function getMail(): Promise<ServiceInfo[]> {
  return invoke('get_mail')
}
export function startService(id: string): Promise<void> {
  return invoke('start_service', { id })
}
export function stopService(id: string): Promise<void> {
  return invoke('stop_service', { id })
}
export function restartService(id: string): Promise<void> {
  return invoke('restart_service', { id })
}
/** Dump all databases from a running DB engine to C:\Stackr\backups; resolves to the file path. */
export function exportDatabases(component: string, version: string): Promise<string> {
  return invoke('export_databases', { component, version })
}

// ---- PHP ----
interface RawPhpVersion {
  version: string
  majorMinor: string
  status: string
  isDefault: boolean
}
export async function getPhpVersions(): Promise<PhpVersionInfo[]> {
  const raw = await invoke<RawPhpVersion[]>('get_php_versions')
  return raw.map((v) => ({
    version: v.version,
    majorMinor: v.majorMinor,
    status: v.status === 'active' ? 'active' : 'installed',
    isDefault: v.isDefault,
  }))
}
export function getPhpExtensions(phpVersion: string): Promise<string[]> {
  return invoke('get_php_extensions', { phpVersion })
}
/** Full extension list (real ext/ DLLs + installable PECL extras). */
export function listPhpExtensions(phpVersion: string): Promise<PhpExtInfo[]> {
  return invoke('list_php_extensions', { phpVersion })
}
/** Download a PECL extension's DLL into ext/ and enable it. */
export function installPhpExtension(phpVersion: string, name: string): Promise<void> {
  return invoke('install_php_extension', { phpVersion, name })
}
export function toggleExtensionApi(
  phpVersion: string,
  extension: string,
  enabled: boolean,
): Promise<void> {
  return invoke('toggle_extension', { phpVersion, extension, enabled })
}
export function setDefaultPhp(version: string): Promise<void> {
  return invoke('set_default_php', { version })
}
export interface XdebugState {
  installed: boolean
  enabled: boolean
  port: number
}
export function xdebugStatus(phpVersion: string): Promise<XdebugState> {
  return invoke('xdebug_status', { phpVersion })
}
export function setXdebug(phpVersion: string, enabled: boolean): Promise<void> {
  return invoke('set_xdebug', { phpVersion, enabled })
}
/** Stop a PHP version's php-cgi runtime — releases its php.exe file lock. */
export function stopPhpRuntime(version: string): Promise<void> {
  return invoke('stop_php_runtime', { version })
}
/** Installable PHP versions (latest patch per minor, 7.4 → newest). */
export function getPhpAvailable(): Promise<string[]> {
  return invoke('get_php_available')
}

// ---- Projects ----
export function getProjects(): Promise<ProjectInfo[]> {
  return invoke('get_projects')
}
export function createProject(config: ProjectConfigInput): Promise<ProjectInfo> {
  return invoke('create_project', { config })
}
export function startProject(id: string): Promise<void> {
  return invoke('start_project', { id })
}
export function stopProject(id: string): Promise<void> {
  return invoke('stop_project', { id })
}
export function deleteProject(id: string, deleteFiles = false): Promise<void> {
  return invoke('delete_project', { id, deleteFiles })
}
export function setProjectPhp(id: string, version: string): Promise<void> {
  return invoke('set_project_php', { id, version })
}
export function setProjectDb(id: string, database: string | null): Promise<void> {
  return invoke('set_project_db', { id, database })
}
export function openProjectFolder(id: string): Promise<void> {
  return invoke('open_project_folder', { id })
}
export interface IdeInfo {
  id: string
  name: string
}
export function detectIdes(): Promise<IdeInfo[]> {
  return invoke('detect_ides')
}
export function openInIde(id: string, ide: string): Promise<void> {
  return invoke('open_in_ide', { id, ide })
}
/** Open a terminal in the project folder with php/composer/git on PATH. */
export function openTerminal(id: string): Promise<void> {
  return invoke('open_terminal', { id })
}
/** Suggested document-root subdir for an existing folder ("" = folder root). */
export function detectDocRoot(path: string): Promise<string> {
  return invoke('detect_doc_root', { path })
}

// ---- Tools ----
export function openAdminer(): Promise<void> {
  return invoke('open_adminer')
}

// ---- Settings ----
export function getSettings(): Promise<AppSettings> {
  return invoke('get_settings')
}
export function saveSettings(settings: AppSettings): Promise<void> {
  return invoke('save_settings', { settings })
}
/** One-shot: true if state was recovered from the .bak on the last load. */
export function takeRestoreNotice(): Promise<boolean> {
  return invoke('take_restore_notice')
}

/** Host runtime prerequisites (VC++ redist, Windows version, WebView2 version). */
export interface SystemReport {
  vcredist: boolean
  windows: string
  webview2: string | null
  supported: boolean
}
export function systemReport(): Promise<SystemReport> {
  return invoke('system_report')
}

/** Whether C:\Stackr is excluded from Windows Defender scanning. */
export interface DefenderStatus {
  /** true = excluded, false = scanned, null = couldn't determine. */
  excluded: boolean | null
  /** The path Stackr manages (its root). */
  path: string
}
export function defenderStatus(): Promise<DefenderStatus> {
  return invoke('defender_status')
}
/** Add C:\Stackr to Defender's exclusions — fires one UAC prompt. */
export function addDefenderExclusion(): Promise<void> {
  return invoke('add_defender_exclusion')
}

/** HTTPS feature state: enabled + whether the local CA is OS-trusted. */
export interface HttpsStatus {
  enabled: boolean
  trusted: boolean
}
export function httpsStatus(): Promise<HttpsStatus> {
  return invoke('https_status')
}
/** Enable HTTPS: generate + trust the local CA (one Windows prompt). */
export function enableHttps(): Promise<HttpsStatus> {
  return invoke('enable_https')
}
export function disableHttps(): Promise<HttpsStatus> {
  return invoke('disable_https')
}

/** Data-root location + whether this is a fresh install (first-run picker). */
export interface RootInfo {
  root: string
  defaultRoot: string
  isFirstRun: boolean
}
export function getRootInfo(): Promise<RootInfo> {
  return invoke('get_root_info')
}
/** Choose the data root (writes the %APPDATA% pointer). */
export function setRoot(path: string): Promise<void> {
  return invoke('set_root', { path })
}

/** Native folder picker; resolves to the chosen path or null if cancelled. */
export async function pickFolder(defaultPath?: string): Promise<string | null> {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const picked = await open({ directory: true, multiple: false, defaultPath })
  return typeof picked === 'string' ? picked : null
}

// ---- Config editor ----
export interface ConfigDoc {
  component: string
  label: string
  path: string
  content: string
  generated: boolean
  hint: string
}
export function readServiceConfig(component: string, version: string): Promise<ConfigDoc> {
  return invoke('read_service_config', { component, version })
}
export function saveServiceConfig(
  component: string,
  version: string,
  content: string,
): Promise<void> {
  return invoke('save_service_config', { component, version, content })
}
export function resetServiceConfig(component: string, version: string): Promise<ConfigDoc> {
  return invoke('reset_service_config', { component, version })
}

// ---- Logs ----
export interface LogRaw {
  service: string
  line: string
}
export function readLog(component: string, maxLines = 500): Promise<string[]> {
  return invoke('read_log', { component, maxLines })
}
export function readAllLogs(maxLines = 500): Promise<LogRaw[]> {
  return invoke('read_all_logs', { maxLines })
}
export function clearLog(component: string): Promise<void> {
  return invoke('clear_log', { component })
}
export function clearAllLogs(): Promise<void> {
  return invoke('clear_all_logs')
}
