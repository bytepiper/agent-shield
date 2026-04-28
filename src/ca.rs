use anyhow::Result;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, SanType, PKCS_ECDSA_P256_SHA256,
};
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::EncodePrivateKey;
use rsa::RsaPrivateKey;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::io::BufReader;
use std::net::IpAddr;
use tracing::info;

pub(crate) struct Ca {
    pub(crate) cert_pem: String,
    pub(crate) key_pem: String,
}

impl Ca {
    pub(crate) fn load_or_gen() -> Result<Self> {
        let dir = "/tmp/agent-shield-ca";
        let cp = format!("{dir}/ca.crt");
        let kp = format!("{dir}/ca.key");

        let home = std::env::var("HOME").unwrap_or("/root".into());
        let mc = format!("{home}/.mitmproxy/mitmproxy-ca-cert.pem");
        let mk = format!("{home}/.mitmproxy/mitmproxy-ca.pem");
        if std::path::Path::new(&mc).exists() {
            info!("Reusing mitmproxy CA");
            return Ok(Self {
                cert_pem: std::fs::read_to_string(&mc)?,
                key_pem: std::fs::read_to_string(&mk)?,
            });
        }

        if std::path::Path::new(&cp).exists() {
            info!("Reusing cached CA");
            return Ok(Self {
                cert_pem: std::fs::read_to_string(&cp)?,
                key_pem: std::fs::read_to_string(&kp)?,
            });
        }

        info!("Generating CA");
        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| anyhow::anyhow!("keygen: {e}"))?;
        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "Agent Shield CA");
        let cert = params
            .self_signed(&ca_key)
            .map_err(|e| anyhow::anyhow!("self_signed: {e}"))?;
        let cert_pem = cert.pem();
        let key_pem = ca_key.serialize_pem();
        std::fs::create_dir_all(dir)?;
        std::fs::write(&cp, &cert_pem)?;
        std::fs::write(&kp, &key_pem)?;
        Ok(Self { cert_pem, key_pem })
    }

    pub(crate) fn issue(
        &self,
        host: &str,
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        let ca_key = parse_ca_key_pair(&self.key_pem)?;
        let issuer = Issuer::from_ca_cert_pem(&self.cert_pem, ca_key)
            .map_err(|e| anyhow::anyhow!("issuer: {e}"))?;

        let mut params = if let Ok(ip) = host.parse::<IpAddr>() {
            let mut params =
                CertificateParams::new(Vec::new()).map_err(|e| anyhow::anyhow!("params: {e}"))?;
            params.subject_alt_names.push(SanType::IpAddress(ip));
            params
        } else {
            CertificateParams::new(vec![host.to_string()])
                .map_err(|e| anyhow::anyhow!("params: {e}"))?
        };
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, host);
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];

        let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| anyhow::anyhow!("leaf keygen: {e}"))?;
        let leaf_cert = params
            .signed_by(&leaf_key, &issuer)
            .map_err(|e| anyhow::anyhow!("sign: {e}"))?;

        let leaf_pem = leaf_cert.pem();
        let leaf_key_pem = leaf_key.serialize_pem();

        let leaf_der = parse_pem_cert(&leaf_pem)?;
        let ca_der = parse_pem_cert(&self.cert_pem)?;
        let key_der = parse_pem_key(&leaf_key_pem)?;

        Ok((vec![leaf_der, ca_der], key_der))
    }
}

fn parse_pem_cert(pem: &str) -> Result<CertificateDer<'static>> {
    let mut reader = BufReader::new(pem.as_bytes());
    let certs = rustls_pemfile::certs(&mut reader).collect::<std::result::Result<Vec<_>, _>>()?;
    certs
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no cert"))
}

fn parse_pem_key(pem: &str) -> Result<PrivateKeyDer<'static>> {
    let mut reader = BufReader::new(pem.as_bytes());
    rustls_pemfile::private_key(&mut reader)?.ok_or_else(|| anyhow::anyhow!("no key"))
}

fn parse_ca_key_pair(pem: &str) -> Result<KeyPair> {
    if let Ok(key_pair) = KeyPair::from_pem(pem) {
        return Ok(key_pair);
    }

    let mut reader = BufReader::new(pem.as_bytes());
    let key = rustls_pemfile::private_key(&mut reader)?
        .ok_or_else(|| anyhow::anyhow!("no private key found in CA PEM"))?;

    match key {
        PrivateKeyDer::Pkcs8(pkcs8) => KeyPair::try_from(pkcs8.secret_pkcs8_der())
            .map_err(|e| anyhow::anyhow!("ca pkcs8 key parse: {e}")),
        PrivateKeyDer::Pkcs1(pkcs1) => {
            let rsa = RsaPrivateKey::from_pkcs1_der(pkcs1.secret_pkcs1_der())
                .map_err(|e| anyhow::anyhow!("ca pkcs1 key parse: {e}"))?;
            let pkcs8 = rsa
                .to_pkcs8_der()
                .map_err(|e| anyhow::anyhow!("ca pkcs1->pkcs8 convert: {e}"))?;
            KeyPair::try_from(pkcs8.as_bytes())
                .map_err(|e| anyhow::anyhow!("ca converted pkcs8 key parse: {e}"))
        }
        PrivateKeyDer::Sec1(_) => Err(anyhow::anyhow!(
            "unsupported SEC1 CA private key format with rcgen ring backend"
        )),
        _ => Err(anyhow::anyhow!("unsupported CA private key format")),
    }
}
