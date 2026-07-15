import type {
  EngineMeta,
  LogEntry,
  PhpExtensionMeta,
  PhpExtInfo,
  PhpVersionInfo,
  ProjectInfo,
  ServiceInfo,
} from '../types'

// ---- PHP versions (demo seed for browser preview; replaced by backend in Tauri) ----
export const DEMO_PHP: PhpVersionInfo[] = [
  { version: '8.2.14', majorMinor: '8.2', status: 'active', isDefault: true, note: 'Default runtime · used by 2 projects' },
  { version: '8.1.27', majorMinor: '8.1', status: 'installed', isDefault: false, note: 'Used by 1 project · api-gateway' },
]

// Latest stable shown as an installable card when not present in the list.
export const AVAILABLE_PHP = {
  version: '8.3.4',
  majorMinor: '8.3',
  note: 'Latest stable release · not installed',
}

// ---- PHP extensions catalog (28). First 8 enabled by default. ----
export const EXT_META: PhpExtensionMeta[] = [
  { id: 'opcache', desc: 'Zend OPcache' },
  { id: 'pdo_mysql', desc: 'MySQL PDO driver' },
  { id: 'gd', desc: 'Image processing' },
  { id: 'mbstring', desc: 'Multibyte strings' },
  { id: 'curl', desc: 'HTTP client' },
  { id: 'openssl', desc: 'TLS & crypto' },
  { id: 'zip', desc: 'Zip archives' },
  { id: 'fileinfo', desc: 'File type detection' },
  { id: 'redis', desc: 'Redis client' },
  { id: 'xdebug', desc: 'Debugger & profiler' },
  { id: 'intl', desc: 'Internationalization' },
  { id: 'bcmath', desc: 'Arbitrary-precision math' },
  { id: 'pdo_sqlite', desc: 'SQLite PDO driver' },
  { id: 'pdo_pgsql', desc: 'PostgreSQL PDO driver' },
  { id: 'imagick', desc: 'ImageMagick bindings' },
  { id: 'sodium', desc: 'Modern cryptography' },
  { id: 'gmp', desc: 'GNU multiple precision' },
  { id: 'soap', desc: 'SOAP protocol client' },
  { id: 'sockets', desc: 'Low-level socket access' },
  { id: 'exif', desc: 'Image metadata' },
  { id: 'ftp', desc: 'FTP client' },
  { id: 'ldap', desc: 'LDAP directory access' },
  { id: 'apcu', desc: 'User-data cache' },
  { id: 'memcached', desc: 'Memcached client' },
  { id: 'xsl', desc: 'XSLT transforms' },
  { id: 'gettext', desc: 'i18n message catalogs' },
  { id: 'calendar', desc: 'Calendar conversions' },
  { id: 'pcntl', desc: 'Process control' },
]

export const DEFAULT_ENABLED_EXT: Record<string, boolean> = {
  opcache: true,
  pdo_mysql: true,
  gd: true,
  mbstring: true,
  curl: true,
  openssl: true,
  zip: true,
  fileinfo: true,
}

// PECL extras (not bundled) for the browser-preview extension list.
const DEMO_PECL = new Set(['xdebug', 'redis', 'apcu', 'igbinary', 'mongodb', 'imagick', 'memcached'])

/** Demo extension list for browser preview (real list comes from the backend). */
export const DEMO_EXTENSIONS: PhpExtInfo[] = EXT_META.map((e) => ({
  name: e.id,
  description: e.desc,
  installed: !DEMO_PECL.has(e.id),
  pecl: DEMO_PECL.has(e.id),
  enabled: !!DEFAULT_ENABLED_EXT[e.id],
}))

// ---- Projects (demo seed for browser preview; replaced by backend in Tauri) ----
export const DEMO_PROJECTS: ProjectInfo[] = [
  {
    id: 'my-shop',
    name: 'my-shop',
    type: 'Framework',
    framework: 'Laravel 11',
    phpVersion: '8.2',
    webServer: 'Nginx',
    database: 'MySQL',
    domain: 'shop.test',
    path: 'C:\\Stackr\\www\\my-shop',
    status: 'running',
    gitUrl: null,
    createdAt: '',
  },
  {
    id: 'api-gateway',
    name: 'api-gateway',
    type: 'Blank PHP',
    framework: null,
    phpVersion: '8.1',
    webServer: 'Nginx',
    database: null,
    domain: 'api.test',
    path: 'C:\\Stackr\\www\\api-gateway',
    status: 'running',
    gitUrl: null,
    createdAt: '',
  },
  {
    id: 'blog',
    name: 'blog',
    type: 'Framework',
    framework: 'Symfony 7',
    phpVersion: '8.2',
    webServer: 'Apache',
    database: 'MySQL',
    domain: 'blog.test',
    path: 'C:\\Stackr\\www\\blog',
    status: 'stopped',
    gitUrl: null,
    createdAt: '',
  },
]

// ---- Engine metas (servers / databases / cache) ----
export const SERVER_ENGINES: EngineMeta[] = [
  {
    component: 'nginx',
    name: 'Nginx',
    mark: 'N',
    markBg: 'rgba(0,150,57,.13)',
    markColor: '#28c24f',
    versions: ['1.27.3', '1.26.2', '1.24.0'],
    size: '8.4 MB',
    desc: 'Event-driven, high-performance web server. The recommended default for modern PHP apps and reverse proxying to PHP-FPM.',
    recommended: true,
  },
  {
    component: 'apache',
    name: 'Apache',
    mark: 'A',
    markBg: 'rgba(210,33,40,.13)',
    markColor: '#e6484f',
    versions: ['2.4.68'],
    size: '13.3 MB',
    desc: 'Battle-tested, ubiquitous web server with rich module support and .htaccess. Ideal for legacy apps and shared-hosting parity.',
  },
]

export const DB_ENGINES: EngineMeta[] = [
  {
    component: 'mysql',
    name: 'MySQL',
    mark: 'My',
    markBg: 'rgba(0,117,143,.16)',
    markColor: '#48b3c9',
    versions: ['8.4.0', '8.0.36'],
    size: '182 MB',
    desc: "World's most popular open-source relational database.",
  },
  {
    component: 'mariadb',
    name: 'MariaDB',
    mark: 'Ma',
    markBg: 'rgba(186,127,60,.16)',
    markColor: '#d2974a',
    versions: ['11.4.2', '11.2.4', '10.11.8', '10.6.18'],
    size: '154 MB',
    desc: 'Drop-in MySQL replacement, community-driven fork.',
  },
  {
    component: 'postgresql',
    name: 'PostgreSQL',
    mark: 'Pg',
    markBg: 'rgba(70,110,180,.18)',
    markColor: '#7d9fd6',
    versions: ['16.2', '15.6', '14.11'],
    size: '96 MB',
    desc: 'Advanced object-relational database with strong SQL compliance.',
  },
]

// Demo installed engines for browser preview (replaced by the backend in Tauri).
export const DEMO_DATABASES: ServiceInfo[] = [
  { id: 'mysql-8.0.36', component: 'mysql', name: 'MySQL', version: '8.0.36', status: 'running', port: 3306 },
  { id: 'mariadb-11.4.2', component: 'mariadb', name: 'MariaDB', version: '11.4.2', status: 'stopped', port: 3306 },
]

export const DEMO_CACHES: ServiceInfo[] = [
  { id: 'redis-5.0.14.1', component: 'redis', name: 'Redis', version: '5.0.14.1', status: 'running', port: 6379 },
  { id: 'memcached-1.6.8', component: 'memcached', name: 'Memcached', version: '1.6.8', status: 'stopped', port: 11211 },
]

export const CACHE_ENGINES: EngineMeta[] = [
  {
    component: 'redis',
    name: 'Redis',
    mark: 'R',
    markBg: 'rgba(210,40,40,.15)',
    markColor: '#e25a52',
    versions: ['5.0.14.1'],
    size: '5 MB',
    desc: 'In-memory data store for caching, sessions and queues.',
  },
  {
    component: 'memcached',
    name: 'Memcached',
    mark: 'Mc',
    markBg: 'rgba(90,140,90,.18)',
    markColor: '#7bbd7b',
    versions: ['1.6.8'],
    size: '1 MB',
    desc: 'High-performance distributed memory object cache.',
  },
]

export const DEMO_MAIL: ServiceInfo[] = [
  { id: 'mailpit-1.30.4', component: 'mailpit', name: 'Mailpit', version: '1.30.4', status: 'stopped', port: 8025 },
]

export const MAIL_ENGINES: EngineMeta[] = [
  {
    component: 'mailpit',
    name: 'Mailpit',
    mark: 'Mp',
    markBg: 'rgba(90,120,200,.16)',
    markColor: '#7d97e0',
    versions: ['1.30.4'],
    size: '13 MB',
    desc: 'Catches all outgoing mail from your projects into a fast web inbox — SMTP on 1025, UI on 8025.',
    recommended: true,
  },
]

// ---- Frameworks (wizard step 2) ----
/** One installable framework version: a display label, the Composer version
 *  constraint (empty = latest stable), and the recommended PHP minor series. */
export interface FrameworkVersion {
  label: string // "11"
  constraint: string // "^11" (empty → latest stable)
  php: string // recommended PHP minor, "8.3"
}

export interface FrameworkMeta {
  name: string
  color: string
  txt: string
  mark: string
  composer: boolean // false = installed by download (WordPress), not Composer
  versions: FrameworkVersion[] // newest first; versions[0] is the default
}

// Curated recent majors per framework, each with its recommended PHP minor.
// Multi-major frameworks pin the major via a constraint; single-major ones use
// latest stable (empty constraint) and the label is informational.
export const FRAMEWORKS: FrameworkMeta[] = [
  {
    name: 'Laravel', color: '#ff2d20', txt: '#fff', mark: 'L', composer: true,
    versions: [
      { label: '12', constraint: '^12', php: '8.3' },
      { label: '11', constraint: '^11', php: '8.3' },
      { label: '10', constraint: '^10', php: '8.2' },
    ],
  },
  {
    name: 'Symfony', color: '#1b1b1b', txt: '#fff', mark: 'Sy', composer: true,
    versions: [
      { label: '7', constraint: '^7', php: '8.3' },
      { label: '6', constraint: '^6', php: '8.2' },
    ],
  },
  {
    name: 'WordPress', color: '#1d5f8a', txt: '#fff', mark: 'W', composer: false,
    versions: [{ label: 'Latest', constraint: '', php: '8.3' }],
  },
  {
    name: 'CodeIgniter', color: '#dd4814', txt: '#fff', mark: 'CI', composer: true,
    versions: [{ label: '4', constraint: '', php: '8.2' }],
  },
  {
    name: 'Yii2', color: '#0a7bbd', txt: '#fff', mark: 'Yi', composer: true,
    versions: [{ label: '2.0', constraint: '', php: '8.2' }],
  },
  {
    name: 'CakePHP', color: '#b8333a', txt: '#fff', mark: 'Ca', composer: true,
    versions: [
      { label: '5', constraint: '^5', php: '8.2' },
      { label: '4', constraint: '^4', php: '8.1' },
    ],
  },
  {
    name: 'Slim', color: '#7ba428', txt: '#fff', mark: 'Sl', composer: true,
    versions: [{ label: '4', constraint: '', php: '8.2' }],
  },
]

// ---- Logs (demo stream) ----
export const ALL_LOGS: LogEntry[] = [
  { svc: 'nginx', lvl: 'info', t: '14:02:09', m: 'nginx/1.27.3 — master process started, spawning 4 workers' },
  { svc: 'php', lvl: 'info', t: '14:02:09', m: 'fpm/pool www: ready to handle connections on 127.0.0.1:9000' },
  { svc: 'mysql', lvl: 'info', t: '14:02:10', m: 'mysqld 8.0.36 ready for connections, port 3306 socket /tmp/mysql.sock' },
  { svc: 'nginx', lvl: 'info', t: '14:02:14', m: 'GET shop.test/ -> 200 in 11ms' },
  { svc: 'nginx', lvl: 'info', t: '14:02:14', m: 'GET shop.test/products?page=2 -> 200 in 34ms' },
  { svc: 'php', lvl: 'warn', t: '14:02:15', m: 'PHP Deprecated: Return type of Carbon::jsonSerialize() should be compatible with JsonSerializable' },
  { svc: 'mysql', lvl: 'warn', t: '14:02:18', m: '[Warning] Aborted connection 214 to db shop (Got timeout reading communication packets)' },
  { svc: 'nginx', lvl: 'info', t: '14:02:21', m: 'GET api.test/v1/orders -> 200 in 8ms' },
  { svc: 'php', lvl: 'error', t: '14:02:22', m: 'PHP Warning: Undefined array key "discount" in C:\\Stackr\\www\\my-shop\\app\\Cart.php on line 88' },
  { svc: 'nginx', lvl: 'error', t: '14:02:24', m: 'connect() failed (111: Connection refused) while connecting to upstream, client 127.0.0.1' },
  { svc: 'mysql', lvl: 'info', t: '14:02:27', m: 'Query OK, 142 rows affected (0.018 sec)' },
  { svc: 'php', lvl: 'info', t: '14:02:30', m: 'fpm: [pool www] child 4831 exited with code 0 after 24.531s of activity' },
]

export const LOG_LEVEL_COLOR: Record<LogEntry['lvl'], string> = {
  info: '#aeb4c0',
  warn: '#d9a93a',
  error: '#f1645a',
}

export const LOG_TAG_COLOR: Record<LogEntry['svc'], string> = {
  nginx: '#3fb950',
  php: '#6c97d8',
  mysql: '#caa14a',
}
