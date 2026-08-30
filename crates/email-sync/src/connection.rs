use async_imap::Session;
use email_core::error::{EmailError, Result};
use email_core::models::Account;
use log::{debug, info};
use rustls::pki_types::ServerName;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

pub type ImapTlsStream = tokio_rustls::client::TlsStream<TcpStream>;
pub type ImapSession = Session<ImapTlsStream>;

pub async fn connect_imap(account: &Account, password: &str) -> Result<ImapSession> {
    let addr = format!("{}:{}", account.imap_host, account.imap_port);
    debug!("Connecting to IMAP at {}", addr);

    let tcp_stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| EmailError::Network(format!("Failed to connect to {}: {}", addr, e)))?;

    let root_store = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };

    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| EmailError::Network(format!("Failed to configure TLS protocol versions: {}", e)))?
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = ServerName::try_from(account.imap_host.clone())
        .map_err(|e| EmailError::Network(format!("Invalid server host {}: {}", account.imap_host, e)))?
        .to_owned();

    let tls_stream = connector
        .connect(server_name, tcp_stream)
        .await
        .map_err(|e| EmailError::Network(format!("TLS handshake failed with {}: {}", account.imap_host, e)))?;

    let client = async_imap::Client::new(tls_stream);
    let session = client
        .login(&account.email, password)
        .await
        .map_err(|e| EmailError::Auth(format!("IMAP login failed for {}: {}", account.email, e.0)))?;

    info!("IMAP login successful for {}", account.email);
    Ok(session)
}
