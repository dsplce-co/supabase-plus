use std::sync::Arc;

use anyhow::Context;
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, ring},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use tokio_postgres::{Client, NoTls, config::Host, config::SslMode};
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::errors::NoWay;

const LOCAL_PORTS: &[u16] = &[54322, 54320];

pub struct DatabaseUrl {
    config: tokio_postgres::Config,
}

impl DatabaseUrl {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let config: tokio_postgres::Config =
            value.parse().context("The database url is not valid")?;

        for host in config.get_hosts() {
            let Host::Tcp(name) = host else {
                crate::styled_bail!("Unix socket sources are always local");
            };

            if name == "localhost"
                || name == "host.docker.internal"
                || name.starts_with("127.")
                || name == "::1"
            {
                crate::styled_bail!(
                    "`{}` points to this machine, not to production",
                    (name, "id")
                );
            }
        }

        if config
            .get_ports()
            .iter()
            .any(|port| LOCAL_PORTS.contains(port))
        {
            crate::styled_bail!("The port is used by the local Supabase");
        }

        Ok(Self { config })
    }

    pub async fn connect(&self) -> anyhow::Result<Client> {
        let mut config = self.config.clone();
        config.application_name("sbp-clone");

        if config.get_ssl_mode() == SslMode::Disable {
            let (client, connection) = config
                .connect(NoTls)
                .await
                .context("Couldn't connect to the production database")?;

            tokio::spawn(connection);

            return Ok(client);
        }

        let provider = Arc::new(ring::default_provider());

        let tls = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .no_way_because("the ring provider supports the default protocol versions")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(EncryptOnly(provider)))
            .with_no_client_auth();

        let (client, connection) = config
            .connect(MakeRustlsConnect::new(tls))
            .await
            .context("Couldn't connect to the production database")?;

        tokio::spawn(connection);

        Ok(client)
    }
}

/// Encrypts the connection without verifying the server certificate, the same way
/// `sslmode=require` does in libpq. Supabase databases present certificates signed by
/// Supabase's own CA, which isn't publicly trusted, so a verifying connector would refuse them
#[derive(Debug)]
struct EncryptOnly(Arc<CryptoProvider>);

impl ServerCertVerifier for EncryptOnly {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_database_is_never_a_source() {
        for url in [
            "postgresql://u:p@127.0.0.1:54322/postgres",
            "postgresql://u:p@localhost:5432/postgres",
            "postgresql://u:p@db.example.com:54322/postgres",
        ] {
            assert!(DatabaseUrl::parse(url).is_err(), "{url}");
        }

        assert!(DatabaseUrl::parse("postgresql://u:p@db.abcd.supabase.co:5432/postgres").is_ok());
    }
}
