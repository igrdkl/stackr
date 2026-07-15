//! Generators for web-server config: nginx master + per-site vhost, Apache vhost.
//! Pure string rendering (unit-tested) plus a helper that writes the nginx
//! master config referencing an install dir's bundled mime.types.

use std::path::Path;

/// Forward-slash a path so nginx/apache configs are valid on Windows.
fn fwd(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Master nginx.conf: logs + an include glob for generated per-site vhosts.
pub fn nginx_master_conf(install_dir: &Path, sites_glob: &str, log_dir: &Path) -> String {
    let mime = fwd(&install_dir.join("conf").join("mime.types"));
    let log = fwd(log_dir);
    format!(
        r#"worker_processes  auto;
error_log  "{log}/error.log";
pid        "{log}/nginx.pid";

events {{
    worker_connections  1024;
}}

http {{
    include       "{mime}";
    default_type  application/octet-stream;
    sendfile      on;
    access_log    "{log}/access.log";

    # Catch-all: an unknown host (or a site whose vhost isn't loaded) gets a clean
    # 404 instead of falling through to the first site's block — which may point at
    # a php-cgi port that isn't running and would return a misleading 502.
    server {{
        listen       127.0.0.1:80 default_server;
        listen       [::1]:80 default_server;
        server_name  _;
        return       404;
    }}

    include       "{sites_glob}";
}}
"#
    )
}

/// The shared body (root + index + location blocks) of an nginx server block.
fn nginx_locations(root: &str, fcgi_port: u16) -> String {
    format!(
        r#"    root         "{root}";
    index        index.php index.html;

    location / {{
        try_files $uri $uri/ /index.php?$query_string;
    }}

    location ~ \.php$ {{
        fastcgi_pass    127.0.0.1:{fcgi_port};
        fastcgi_index   index.php;
        fastcgi_param   SCRIPT_FILENAME   $document_root$fastcgi_script_name;
        fastcgi_param   QUERY_STRING      $query_string;
        fastcgi_param   REQUEST_METHOD    $request_method;
        fastcgi_param   CONTENT_TYPE      $content_type;
        fastcgi_param   CONTENT_LENGTH    $content_length;
        fastcgi_param   SCRIPT_NAME       $fastcgi_script_name;
        fastcgi_param   REQUEST_URI       $request_uri;
        fastcgi_param   DOCUMENT_URI      $document_uri;
        fastcgi_param   DOCUMENT_ROOT     $document_root;
        fastcgi_param   SERVER_PROTOCOL   $server_protocol;
        fastcgi_param   GATEWAY_INTERFACE CGI/1.1;
        fastcgi_param   SERVER_SOFTWARE   nginx;
        fastcgi_param   REMOTE_ADDR       $remote_addr;
        fastcgi_param   SERVER_NAME       $server_name;
        fastcgi_param   HTTPS             $https if_not_empty;
    }}
"#
    )
}

/// Per-project nginx vhost. Self-contained FastCGI params (no external include).
/// `fcgi_port` selects which php-cgi runtime serves this site, so each project
/// can run on its own PHP version (one php-cgi per version, distinct ports).
/// When `tls` is `Some((cert, key))`, a second `listen 443 ssl` block is added
/// so the site is also served over HTTPS with the local CA-signed cert.
pub fn nginx_vhost(domain: &str, root: &Path, port: u16, fcgi_port: u16, tls: Option<(&Path, &Path)>) -> String {
    let root = fwd(root);
    let body = nginx_locations(&root, fcgi_port);
    let mut out = format!(
        "server {{\n    listen       127.0.0.1:{port};\n    listen       [::1]:{port};\n    server_name  {domain};\n{body}}}\n"
    );
    if let Some((cert, key)) = tls {
        out.push('\n');
        out.push_str(&format!(
            "server {{\n    listen       127.0.0.1:443 ssl;\n    listen       [::1]:443 ssl;\n    server_name  {domain};\n    ssl_certificate      \"{}\";\n    ssl_certificate_key  \"{}\";\n{body}}}\n",
            fwd(cert),
            fwd(key)
        ));
    }
    out
}

/// The shared inner directives of an Apache vhost (server name, docroot, PHP proxy).
fn apache_vhost_inner(domain: &str, root: &str, fcgi_port: u16) -> String {
    format!(
        r#"    ServerName {domain}
    DocumentRoot "{root}"
    DirectoryIndex index.php index.html
    <Directory "{root}">
        Options Indexes FollowSymLinks
        AllowOverride All
        Require all granted
    </Directory>
    ProxyPassMatch ^/(.*\.php(/.*)?)$ "fcgi://127.0.0.1:{fcgi_port}/{root}/$1"
"#
    )
}

/// Per-project Apache vhost. `fcgi_port` selects this project's php-cgi runtime
/// (one per PHP version), so sites can run on different PHP versions at once.
/// When `tls` is `Some((cert, key))`, a second `<VirtualHost :443>` with
/// `SSLEngine on` is added (requires the SSL bootstrap — see [`apache_ssl_bootstrap`]).
pub fn apache_vhost(domain: &str, root: &Path, port: u16, fcgi_port: u16, tls: Option<(&Path, &Path)>) -> String {
    let root = fwd(root);
    let inner = apache_vhost_inner(domain, &root, fcgi_port);
    let mut out = format!("<VirtualHost 127.0.0.1:{port} [::1]:{port}>\n{inner}</VirtualHost>\n");
    if let Some((cert, key)) = tls {
        out.push_str(&format!(
            "<VirtualHost 127.0.0.1:443 [::1]:443>\n{inner}    SSLEngine on\n    SSLCertificateFile \"{}\"\n    SSLCertificateKeyFile \"{}\"\n</VirtualHost>\n",
            fwd(cert),
            fwd(key)
        ));
    }
    out
}

/// Server-scope directives that make Apache SSL-capable, written to a
/// `_ssl.conf` in the sites dir (included before the per-project vhosts, since
/// `_` sorts first). Kept out of the master so the master stays untouched; the
/// bundled httpd.conf leaves ssl_module commented, so loading it here is safe.
pub fn apache_ssl_bootstrap() -> String {
    r#"# --- Stackr HTTPS (managed) ---
LoadModule ssl_module modules/mod_ssl.so
LoadModule socache_shmcb_module modules/mod_socache_shmcb.so
Listen 127.0.0.1:443
Listen [::1]:443
SSLSessionCache "shmcb:logs/ssl_scache(512000)"
SSLProtocol all -SSLv3 -TLSv1 -TLSv1.1
"#
    .to_string()
}

/// Directives Stackr appends to Apache's bundled httpd.conf: enable rewrite +
/// FastCGI PHP (each vhost proxies to its project's php-cgi port) and include
/// the generated per-site vhosts.
pub fn apache_stackr_block(sites_glob: &str) -> String {
    format!(
        r#"
# --- Stackr ---
LoadModule rewrite_module modules/mod_rewrite.so
LoadModule proxy_module modules/mod_proxy.so
LoadModule proxy_fcgi_module modules/mod_proxy_fcgi.so
ServerName localhost
DirectoryIndex index.php index.html
# Each vhost routes PHP to its own php-cgi port (one per PHP version). The proxy
# URL hands php-cgi a Windows path with a stray leading slash ("/C:/..."); strip
# it so SCRIPT_FILENAME resolves to a real file.
ProxyFCGISetEnvIf "%{{REQUEST_FILENAME}} =~ m#fcgi://[^/]+/(.*)#" SCRIPT_FILENAME "$1"
IncludeOptional "{sites_glob}"
"#
    )
}

/// Build a Stackr-managed Apache config from the install's bundled `httpd.conf`:
/// repoint `SRVROOT` at the real install dir (Apache Lounge hardcodes
/// `c:/Apache24`) and append the Stackr block. Written right before httpd starts.
pub fn write_apache_master(install_dir: &Path) -> Result<(), String> {
    let bundled = install_dir.join("conf").join("httpd.conf");
    let original = std::fs::read_to_string(&bundled)
        .map_err(|e| format!("reading {}: {e}", bundled.display()))?;
    let srvroot = fwd(install_dir);

    let mut out = String::with_capacity(original.len() + 512);
    let mut fixed_srvroot = false;
    for line in original.lines() {
        let t = line.trim_start();
        if t.starts_with("Define SRVROOT") {
            out.push_str(&format!("Define SRVROOT \"{srvroot}\"\n"));
            fixed_srvroot = true;
        } else if t.starts_with("ServerRoot") && !fixed_srvroot {
            out.push_str(&format!("ServerRoot \"{srvroot}\"\n"));
        } else if t.starts_with("Listen ") {
            // Bind Apache to loopback only — a dev server must not be reachable
            // from the LAN. Bind BOTH families: browsers resolve *.localhost to ::1
            // (IPv6) first. Preserve whatever port the bundled conf declared.
            let port = t.trim_start_matches("Listen ").trim().rsplit(':').next().unwrap_or("80");
            out.push_str(&format!("Listen 127.0.0.1:{port}\n"));
            out.push_str(&format!("Listen [::1]:{port}\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    let sites = crate::paths::apache_sites_dir();
    crate::paths::ensure_dir(&sites).map_err(|e| e.to_string())?;
    out.push_str(&apache_stackr_block(&format!("{}/*.conf", fwd(&sites))));

    // Route Apache's error log to a predictable Stackr path (for the Logs tab).
    let log_dir = crate::paths::apache_log_dir();
    crate::paths::ensure_dir(&log_dir).map_err(|e| e.to_string())?;
    out.push_str(&format!("ErrorLog \"{}\"\n", fwd(&log_dir.join("error.log"))));

    let dest = crate::paths::apache_conf();
    if let Some(parent) = dest.parent() {
        crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&dest, out).map_err(|e| e.to_string())
}

/// Ensure the nginx master config exists (generating defaults on first use),
/// but never clobber it afterwards — so edits made in the in-app config editor
/// survive restarts/reloads. Always ensures the sites/ and log dirs exist.
pub fn ensure_nginx_master(install_dir: &Path) -> Result<(), String> {
    let sites = crate::paths::nginx_sites_dir();
    let logs = crate::paths::nginx_log_dir();
    crate::paths::ensure_dir(&sites).map_err(|e| e.to_string())?;
    crate::paths::ensure_dir(&logs).map_err(|e| e.to_string())?;
    if crate::paths::nginx_conf().exists() {
        return Ok(());
    }
    write_nginx_master(install_dir)
}

/// Apache counterpart of [`ensure_nginx_master`]: generate the managed
/// httpd.conf only if absent, preserving in-app edits across restarts.
pub fn ensure_apache_master(install_dir: &Path) -> Result<(), String> {
    let sites = crate::paths::apache_sites_dir();
    crate::paths::ensure_dir(&sites).map_err(|e| e.to_string())?;
    if crate::paths::apache_conf().exists() {
        return Ok(());
    }
    write_apache_master(install_dir)
}

/// Write the nginx master config for a given nginx install dir, ensuring the
/// sites/ and log directories exist. Called right before nginx starts.
pub fn write_nginx_master(install_dir: &Path) -> Result<(), String> {
    let sites = crate::paths::nginx_sites_dir();
    let logs = crate::paths::nginx_log_dir();
    crate::paths::ensure_dir(&sites).map_err(|e| e.to_string())?;
    crate::paths::ensure_dir(&logs).map_err(|e| e.to_string())?;

    let glob = format!("{}/*.conf", fwd(&sites));
    let conf = nginx_master_conf(install_dir, &glob, &logs);
    let path = crate::paths::nginx_conf();
    if let Some(parent) = path.parent() {
        crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, conf).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn vhost_has_fastcgi_and_root() {
        let v = nginx_vhost("app.test", &PathBuf::from("C:\\Stackr\\www\\app\\public"), 80, 9082, None);
        assert!(v.contains("listen       127.0.0.1:80;"), "vhost must bind IPv4 loopback");
        assert!(v.contains("listen       [::1]:80;"), "vhost must also bind IPv6 loopback (.localhost → ::1)");
        assert!(v.contains("server_name  app.test;"));
        assert!(v.contains("fastcgi_pass    127.0.0.1:9082;"));
        assert!(v.contains(r#"root         "C:/Stackr/www/app/public";"#));
        assert!(v.contains("try_files $uri $uri/ /index.php?$query_string;"));
        assert!(!v.contains("ssl"), "no SSL block without a cert");
    }

    #[test]
    fn nginx_vhost_adds_ssl_block_with_cert() {
        let root = PathBuf::from("C:\\Stackr\\www\\app\\public");
        let cert = PathBuf::from("C:\\Stackr\\config\\certs\\app.test.crt");
        let key = PathBuf::from("C:\\Stackr\\config\\certs\\app.test.key");
        let v = nginx_vhost("app.test", &root, 80, 9082, Some((&cert, &key)));
        assert!(v.contains("listen       127.0.0.1:80;"), "keeps the HTTP block");
        assert!(v.contains("listen       127.0.0.1:443 ssl;"), "adds an HTTPS block");
        assert!(v.contains("listen       [::1]:443 ssl;"));
        assert!(v.contains(r#"ssl_certificate      "C:/Stackr/config/certs/app.test.crt";"#));
        assert!(v.contains(r#"ssl_certificate_key  "C:/Stackr/config/certs/app.test.key";"#));
        // Two server blocks, both proxying PHP.
        assert_eq!(v.matches("fastcgi_pass    127.0.0.1:9082;").count(), 2);
    }

    #[test]
    fn master_includes_sites_and_mime() {
        let m = nginx_master_conf(
            &PathBuf::from("C:\\Stackr\\bin\\nginx\\1.27.3"),
            "C:/Stackr/config/nginx/sites/*.conf",
            &PathBuf::from("C:\\Stackr\\logs\\nginx"),
        );
        assert!(m.contains(r#"include       "C:/Stackr/bin/nginx/1.27.3/conf/mime.types";"#));
        assert!(m.contains(r#"include       "C:/Stackr/config/nginx/sites/*.conf";"#));
        assert!(m.contains(r#"error_log  "C:/Stackr/logs/nginx/error.log";"#));
    }

    #[test]
    fn apache_vhost_has_docroot() {
        let a = apache_vhost("blog.test", &PathBuf::from("C:\\Stackr\\www\\blog\\public"), 80, 9083, None);
        assert!(
            a.contains("<VirtualHost 127.0.0.1:80 [::1]:80>"),
            "vhost must bind both loopback families (.localhost → ::1)"
        );
        assert!(a.contains("ServerName blog.test"));
        assert!(a.contains(r#"DocumentRoot "C:/Stackr/www/blog/public""#));
        assert!(a.contains("fcgi://127.0.0.1:9083/"));
        assert!(!a.contains("SSLEngine"), "no SSL vhost without a cert");
    }

    #[test]
    fn apache_vhost_adds_ssl_vhost_with_cert() {
        let root = PathBuf::from("C:\\Stackr\\www\\blog\\public");
        let cert = PathBuf::from("C:\\Stackr\\config\\certs\\blog.test.crt");
        let key = PathBuf::from("C:\\Stackr\\config\\certs\\blog.test.key");
        let a = apache_vhost("blog.test", &root, 80, 9083, Some((&cert, &key)));
        assert!(a.contains("<VirtualHost 127.0.0.1:80 [::1]:80>"), "keeps the HTTP vhost");
        assert!(a.contains("<VirtualHost 127.0.0.1:443 [::1]:443>"), "adds an HTTPS vhost");
        assert!(a.contains("SSLEngine on"));
        assert!(a.contains(r#"SSLCertificateFile "C:/Stackr/config/certs/blog.test.crt""#));
        assert!(a.contains(r#"SSLCertificateKeyFile "C:/Stackr/config/certs/blog.test.key""#));
        assert_eq!(a.matches("ServerName blog.test").count(), 2);
    }

    /// Full live proof: download nginx + PHP, generate a vhost, run nginx +
    /// php-cgi, and confirm a real HTTP request executes PHP. Heavy + networked,
    /// so it's `#[ignore]`d — run with:
    ///   cargo test serves_php_end_to_end -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads nginx+php and serves a live request"]
    async fn serves_php_end_to_end() {
        use std::process::Command;
        use std::time::Duration;

        let base = std::env::temp_dir().join("stackr-serve-test");
        let _ = std::fs::remove_dir_all(&base);
        let nginx_dir = base.join("nginx");
        let php_dir = base.join("php");
        let www = base.join("www").join("app").join("public");
        let sites = base.join("sites");
        let logs = base.join("logs");

        crate::download::download_and_extract(
            "https://nginx.org/download/nginx-1.27.3.zip",
            &nginx_dir,
            |_, _| {},
        )
        .await
        .expect("nginx download");
        crate::download::download_and_extract(
            "https://windows.php.net/downloads/releases/archives/php-8.3.4-Win32-vs16-x64.zip",
            &php_dir,
            |_, _| {},
        )
        .await
        .expect("php download");

        std::fs::create_dir_all(&www).unwrap();
        std::fs::write(www.join("index.php"), "<?php echo \"STACKR_OK \".PHP_VERSION;").unwrap();
        std::fs::create_dir_all(&sites).unwrap();
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(sites.join("app.conf"), nginx_vhost("app.test", &www, 8088, 9000, None)).unwrap();

        let glob = format!("{}/*.conf", fwd(&sites));
        let conf_path = base.join("nginx.conf");
        std::fs::write(&conf_path, nginx_master_conf(&nginx_dir, &glob, &logs)).unwrap();

        let mut php = Command::new(php_dir.join("php-cgi.exe"))
            .args(["-b", "127.0.0.1:9000"])
            .current_dir(&php_dir)
            .spawn()
            .expect("spawn php-cgi");
        let mut ng = Command::new(nginx_dir.join("nginx.exe"))
            .args(["-p", &fwd(&nginx_dir), "-c", &fwd(&conf_path)])
            .current_dir(&nginx_dir)
            .spawn()
            .expect("spawn nginx");

        tokio::time::sleep(Duration::from_millis(2000)).await;

        let res = reqwest::Client::new()
            .get("http://127.0.0.1:8088/")
            .header("Host", "app.test")
            .send()
            .await;

        // teardown regardless of outcome
        let _ = Command::new(nginx_dir.join("nginx.exe"))
            .args(["-p", &fwd(&nginx_dir), "-s", "stop"])
            .current_dir(&nginx_dir)
            .output();
        let _ = ng.kill();
        let _ = ng.wait();
        let _ = php.kill();
        let _ = php.wait();

        let body = res.expect("HTTP request failed").text().await.unwrap();
        assert!(body.contains("STACKR_OK"), "expected PHP output, got: {body}");

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Live proof of multi-version PHP: two projects served at once by PHP 8.2 and
    /// PHP 8.3, each on its own php-cgi port (via `php_fcgi_port`), through a single
    /// nginx. Asserts each domain reports its own PHP version. Heavy + networked:
    ///   cargo test serves_two_php_versions_at_once -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads two PHP versions + nginx and serves them simultaneously"]
    async fn serves_two_php_versions_at_once() {
        use crate::commands::services::php_fcgi_port;
        use std::process::Command;
        use std::time::Duration;

        let base = std::env::temp_dir().join("stackr-multi-php-test");
        let nginx_dir = base.join("nginx");
        let sites = base.join("sites");
        let logs = base.join("logs");
        std::fs::create_dir_all(&sites).unwrap();
        std::fs::create_dir_all(&logs).unwrap();

        // (version, php install dir, web root) for each runtime.
        let cases = [("8.2.12", "vs16"), ("8.3.4", "vs16")];
        let mut children = Vec::new();

        if !nginx_dir.join("nginx.exe").exists() {
            crate::download::download_and_extract(
                "https://nginx.org/download/nginx-1.27.3.zip",
                &nginx_dir,
                |_, _| {},
            )
            .await
            .expect("nginx download");
        }

        for (ver, tag) in cases {
            let php_dir = base.join(format!("php-{ver}"));
            if !php_dir.join("php-cgi.exe").exists() {
                let url = format!(
                    "https://windows.php.net/downloads/releases/archives/php-{ver}-Win32-{tag}-x64.zip"
                );
                crate::download::download_and_extract(&url, &php_dir, |_, _| {})
                    .await
                    .unwrap_or_else(|e| panic!("php {ver} download: {e}"));
            }
            let www = base.join(format!("www-{ver}")).join("public");
            std::fs::create_dir_all(&www).unwrap();
            std::fs::write(www.join("index.php"), "<?php echo \"STACKR_MV \".PHP_VERSION;").unwrap();

            let port = php_fcgi_port(ver);
            let domain = format!("mv{}.test", ver.replace('.', ""));
            std::fs::write(sites.join(format!("{domain}.conf")), nginx_vhost(&domain, &www, 8087, port, None))
                .unwrap();

            let php = Command::new(php_dir.join("php-cgi.exe"))
                .args(["-b", &format!("127.0.0.1:{port}")])
                .current_dir(&php_dir)
                .spawn()
                .unwrap_or_else(|e| panic!("spawn php-cgi {ver}: {e}"));
            children.push(php);
        }

        let glob = format!("{}/*.conf", fwd(&sites));
        let conf_path = base.join("nginx.conf");
        std::fs::write(&conf_path, nginx_master_conf(&nginx_dir, &glob, &logs)).unwrap();
        let mut ng = Command::new(nginx_dir.join("nginx.exe"))
            .args(["-p", &fwd(&nginx_dir), "-c", &fwd(&conf_path)])
            .current_dir(&nginx_dir)
            .spawn()
            .expect("spawn nginx");

        tokio::time::sleep(Duration::from_millis(2500)).await;

        // Each domain must report ITS OWN PHP version — proving isolation.
        let client = reqwest::Client::new();
        let mut results = Vec::new();
        for (ver, _) in cases {
            let domain = format!("mv{}.test", ver.replace('.', ""));
            let body = client
                .get("http://127.0.0.1:8087/")
                .header("Host", &domain)
                .send()
                .await
                .map(|r| async { r.text().await.unwrap_or_default() });
            let body = match body {
                Ok(f) => f.await,
                Err(e) => format!("<request error: {e}>"),
            };
            results.push((ver, body));
        }

        // teardown
        let _ = Command::new(nginx_dir.join("nginx.exe"))
            .args(["-p", &fwd(&nginx_dir), "-s", "stop"])
            .current_dir(&nginx_dir)
            .output();
        let _ = ng.kill();
        let _ = ng.wait();
        for mut php in children {
            let _ = php.kill();
            let _ = php.wait();
        }

        for (ver, body) in &results {
            assert!(
                body.contains("STACKR_MV") && body.contains(ver),
                "domain for PHP {ver} should report {ver}, got: {body}"
            );
        }

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Live proof of the HTTPS path: generate a local CA + a leaf cert with
    /// rcgen, run nginx with an `ssl` server block using that cert, and confirm a
    /// real HTTPS request validates against the CA and serves content. nginx is a
    /// small (~1.8MB) download, so this is cheap:
    ///   cargo test serves_https_with_local_ca -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads nginx and serves a live HTTPS request validated against the local CA"]
    async fn serves_https_with_local_ca() {
        use rcgen::{
            BasicConstraints, CertificateParams, DistinguishedName, DnType,
            ExtendedKeyUsagePurpose, IsCa, KeyPair,
        };
        use std::net::SocketAddr;
        use std::process::Command;
        use std::time::Duration;

        let base = std::env::temp_dir().join("stackr-https-test");
        let nginx_dir = base.join("nginx");
        let www = base.join("www");
        let conf_dir = base.join("conf");
        let logs = base.join("logs");
        for d in [&www, &conf_dir, &logs] {
            std::fs::create_dir_all(d).unwrap();
        }

        // Local CA.
        let ca_key = KeyPair::generate().unwrap();
        let mut cap = CertificateParams::new(Vec::new()).unwrap();
        cap.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Stackr Local CA");
        cap.distinguished_name = dn;
        cap.not_before = rcgen::date_time_ymd(2024, 1, 1);
        cap.not_after = rcgen::date_time_ymd(2035, 1, 1);
        let ca_cert = cap.self_signed(&ca_key).unwrap();

        // Leaf for ssltest.test signed by the CA.
        let leaf_key = KeyPair::generate().unwrap();
        let mut lp = CertificateParams::new(vec!["ssltest.test".to_string()]).unwrap();
        lp.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        lp.not_before = rcgen::date_time_ymd(2024, 1, 1);
        lp.not_after = rcgen::date_time_ymd(2035, 1, 1);
        let leaf = lp.signed_by(&leaf_key, &ca_cert, &ca_key).unwrap();

        let cert_path = conf_dir.join("leaf.crt");
        let key_path = conf_dir.join("leaf.key");
        std::fs::write(&cert_path, leaf.pem()).unwrap();
        std::fs::write(&key_path, leaf_key.serialize_pem()).unwrap();
        std::fs::write(www.join("index.html"), "STACKR_TLS_OK").unwrap();

        if !nginx_dir.join("nginx.exe").exists() {
            crate::download::download_and_extract(
                "https://nginx.org/download/nginx-1.27.3.zip",
                &nginx_dir,
                |_, _| {},
            )
            .await
            .expect("nginx download");
        }

        let mime = fwd(&nginx_dir.join("conf").join("mime.types"));
        let conf = format!(
            "worker_processes 1;\nerror_log \"{log}/error.log\";\npid \"{log}/nginx.pid\";\nevents {{ worker_connections 64; }}\nhttp {{\n  include \"{mime}\";\n  server {{\n    listen 127.0.0.1:8443 ssl;\n    server_name ssltest.test;\n    ssl_certificate \"{cert}\";\n    ssl_certificate_key \"{key}\";\n    root \"{root}\";\n    index index.html;\n  }}\n}}\n",
            log = fwd(&logs),
            mime = mime,
            cert = fwd(&cert_path),
            key = fwd(&key_path),
            root = fwd(&www),
        );
        let conf_path = base.join("nginx.conf");
        std::fs::write(&conf_path, conf).unwrap();

        let mut ng = Command::new(nginx_dir.join("nginx.exe"))
            .args(["-p", &fwd(&nginx_dir), "-c", &fwd(&conf_path)])
            .current_dir(&nginx_dir)
            .spawn()
            .expect("spawn nginx");
        tokio::time::sleep(Duration::from_millis(1500)).await;

        let ca_root = reqwest::Certificate::from_pem(ca_cert.pem().as_bytes()).unwrap();
        let client = reqwest::Client::builder()
            .add_root_certificate(ca_root)
            .resolve("ssltest.test", SocketAddr::from(([127, 0, 0, 1], 8443)))
            .build()
            .unwrap();
        let res = client.get("https://ssltest.test:8443/index.html").send().await;

        // teardown
        let _ = Command::new(nginx_dir.join("nginx.exe"))
            .args(["-p", &fwd(&nginx_dir), "-s", "stop"])
            .current_dir(&nginx_dir)
            .output();
        let _ = ng.kill();
        let _ = ng.wait();

        let resp = res.expect("HTTPS request failed (cert did not validate against the CA?)");
        assert!(resp.status().is_success(), "expected 200, got {}", resp.status());
        let body = resp.text().await.unwrap();
        assert!(body.contains("STACKR_TLS_OK"), "expected TLS-served body, got: {body}");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Live proof that Apache serves PHP via mod_proxy_fcgi → php-cgi (the same
    /// runtime nginx uses). Downloads Apache + PHP, augments the bundled
    /// httpd.conf the way `write_apache_master` does, and asserts a real request
    /// executes PHP. Heavy + networked:
    ///   cargo test apache_serves_php_end_to_end -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads apache+php and serves a live request"]
    async fn apache_serves_php_end_to_end() {
        use std::process::Command;
        use std::time::Duration;

        let base = std::env::temp_dir().join("stackr-apache-test");
        let apache_dir = base.join("apache");
        let php_dir = base.join("php");
        let www = base.join("www");
        std::fs::create_dir_all(&www).unwrap();

        // Cache the (heavy) downloads across iterations.
        if !apache_dir.join("bin").join("httpd.exe").exists() {
            crate::download::download_and_extract(
                "https://www.apachelounge.com/download/VS18/binaries/httpd-2.4.68-260617-Win64-VS18.zip",
                &apache_dir,
                |_, _| {},
            )
            .await
            .expect("apache download");
        }
        if !php_dir.join("php-cgi.exe").exists() {
            crate::download::download_and_extract(
                "https://windows.php.net/downloads/releases/archives/php-8.3.4-Win32-vs16-x64.zip",
                &php_dir,
                |_, _| {},
            )
            .await
            .expect("php download");
        }

        std::fs::write(www.join("index.php"), "<?php echo \"STACKR_APACHE_OK \".PHP_VERSION;").unwrap();

        // Mirror write_apache_master's transform, but on a test port with an
        // inline vhost + a dedicated php-cgi port (so it can run alongside others).
        let bundled = std::fs::read_to_string(apache_dir.join("conf").join("httpd.conf")).unwrap();
        let srvroot = fwd(&apache_dir);
        let mut conf = String::new();
        for line in bundled.lines() {
            let t = line.trim_start();
            if t.starts_with("Define SRVROOT") {
                conf.push_str(&format!("Define SRVROOT \"{srvroot}\"\n"));
            } else if t.starts_with("Listen ") {
                conf.push_str("Listen 8090\n");
            } else {
                conf.push_str(line);
                conf.push('\n');
            }
        }
        conf.push_str(&format!(
            r#"
LoadModule rewrite_module modules/mod_rewrite.so
LoadModule proxy_module modules/mod_proxy.so
LoadModule proxy_fcgi_module modules/mod_proxy_fcgi.so
ServerName localhost
<VirtualHost *:8090>
    DocumentRoot "{www}"
    DirectoryIndex index.php
    <Directory "{www}">
        AllowOverride All
        Require all granted
    </Directory>
    ProxyPassMatch ^/(.*\.php(/.*)?)$ "fcgi://127.0.0.1:9001/{www}/$1"
    ProxyFCGISetEnvIf "%{{REQUEST_FILENAME}} =~ m#fcgi://[^/]+/(.*)#" SCRIPT_FILENAME "$1"
</VirtualHost>
"#,
            www = fwd(&www)
        ));
        let conf_path = base.join("httpd.conf");
        std::fs::write(&conf_path, &conf).unwrap();

        let httpd = apache_dir.join("bin").join("httpd.exe");

        // Config must parse cleanly first.
        let check = Command::new(&httpd)
            .args(["-f", &fwd(&conf_path), "-t"])
            .current_dir(&apache_dir)
            .output()
            .expect("run httpd -t");
        assert!(
            check.status.success(),
            "httpd config test failed:\n{}",
            String::from_utf8_lossy(&check.stderr)
        );

        let mut php = Command::new(php_dir.join("php-cgi.exe"))
            .args(["-b", "127.0.0.1:9001"])
            .current_dir(&php_dir)
            .spawn()
            .expect("spawn php-cgi");
        let mut ap = Command::new(&httpd)
            .args(["-f", &fwd(&conf_path)])
            .current_dir(&apache_dir)
            .spawn()
            .expect("spawn httpd");

        let mut last = String::new();
        let mut ok = false;
        for _ in 0..15 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            match reqwest::Client::new().get("http://127.0.0.1:8090/").send().await {
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    if body.contains("STACKR_APACHE_OK") {
                        ok = true;
                        break;
                    }
                    last = format!("HTTP {status} :: {}", body.chars().take(300).collect::<String>());
                }
                Err(e) => last = e.to_string(),
            }
        }
        let errlog = std::fs::read_to_string(apache_dir.join("logs").join("error.log")).unwrap_or_default();
        let errtail: String = errlog.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");

        // teardown (kill the httpd process tree + php-cgi)
        let _ = Command::new("taskkill")
            .args(["/PID", &ap.id().to_string(), "/T", "/F"])
            .output();
        let _ = ap.kill();
        let _ = ap.wait();
        let _ = php.kill();
        let _ = php.wait();

        assert!(ok, "Apache never served PHP.\nlast: {last}\nerror.log:\n{errtail}");
        // Downloads are intentionally left cached under temp for fast re-runs.
    }
}
