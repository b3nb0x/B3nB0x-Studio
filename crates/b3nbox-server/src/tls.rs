use axum_server::tls_rustls::RustlsConfig;
use std::path::Path;

/// Laedt ein TLS-Zertifikat oder generiert ein Self-Signed-Zertifikat
/// (fuer lokale Installationen), wenn keine Pfade konfiguriert sind oder
/// die Dateien fehlen.
///
/// Fuer echte Domaenen: Zertifikate z.B. per certbot (Let's Encrypt)
/// beziehen und cert_path/key_path auf die Pfade setzen.
pub async fn load_or_generate_tls(
    cert_path: Option<&str>,
    key_path: Option<&str>,
) -> Result<RustlsConfig, String> {
    match (cert_path, key_path) {
        (Some(cert), Some(key)) => {
            if !Path::new(cert).exists() || !Path::new(key).exists() {
                return Err(format!(
                    "Konfigurierte Zertifikate nicht gefunden: {cert} / {key}. \
                     Fuer Let's Encrypt: certbot certonly -d deine.domaene.de und Pfade auf \
                     /etc/letsencrypt/live/<domain>/fullchain.pem bzw. privkey.pem setzen."
                ));
            }
            RustlsConfig::from_pem_file(cert, key)
                .await
                .map_err(|e| format!("TLS-Konfiguration fehlgeschlagen ({cert} / {key}): {e}"))
        }
        _ => {
            let cert_file = "data/certs/b3nbox-cert.pem";
            let key_file = "data/certs/b3nbox-key.pem";
            if !(Path::new(cert_file).exists() && Path::new(key_file).exists()) {
                generate_self_signed(cert_file, key_file)?;
            } else {
                tracing::info!("Self-Signed-Zertifikat geladen: {cert_file}");
            }
            RustlsConfig::from_pem_file(cert_file, key_file)
                .await
                .map_err(|e| format!("TLS-Konfiguration fehlgeschlagen ({cert_file} / {key_file}): {e}"))
        }
    }
}

fn generate_self_signed(cert_file: &str, key_file: &str) -> Result<(), String> {
    let cert_dir = Path::new(cert_file).parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(cert_dir).map_err(|e| e.to_string())?;

    let certified_key = rcgen::generate_simple_self_signed(vec![
        "b3nbox-studio.local".to_string(),
        "localhost".to_string(),
    ])
    .map_err(|e| format!("Self-Signed-Generierung fehlgeschlagen: {e}"))?;

    let cert_pem = certified_key.cert.pem();
    let key_pem = certified_key.key_pair.serialize_pem();

    // Private Keys duerfen nicht world-readable landen (Unix: 0600).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let key_permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::write(cert_file, &cert_pem).map_err(|e| e.to_string())?;
        std::fs::write(key_file, &key_pem).map_err(|e| e.to_string())?;
        std::fs::set_permissions(key_file, key_permissions).map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(cert_file, &cert_pem).map_err(|e| e.to_string())?;
        std::fs::write(key_file, &key_pem).map_err(|e| e.to_string())?;
    }
    tracing::info!(
        "Self-Signed-Zertifikat generiert: {cert_file} (lokale Nutzung - fuer Domaenen Let's Encrypt verwenden)"
    );
    Ok(())
}
