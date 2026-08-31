use crate::error::EmailError;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;
use rand::RngCore;
use rsa::{
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey},
    Oaep, Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PGP_MESSAGE_HEADER: &str = "-----BEGIN PGP MESSAGE-----";
pub const PGP_MESSAGE_FOOTER: &str = "-----END PGP MESSAGE-----";
pub const PGP_SIGNED_HEADER: &str = "-----BEGIN PGP SIGNED MESSAGE-----";
pub const PGP_SIG_HEADER: &str = "-----BEGIN PGP SIGNATURE-----";
pub const PGP_SIG_FOOTER: &str = "-----END PGP SIGNATURE-----";
pub const PGP_PUBKEY_HEADER: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----";
pub const PGP_PUBKEY_FOOTER: &str = "-----END PGP PUBLIC KEY BLOCK-----";
pub const PGP_PRIVKEY_HEADER: &str = "-----BEGIN PGP PRIVATE KEY BLOCK-----";
pub const PGP_PRIVKEY_FOOTER: &str = "-----END PGP PRIVATE KEY BLOCK-----";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgpKeypair {
    pub email: String,
    pub fingerprint: String,
    pub public_key_armored: String,
    pub private_key_armored: String,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize)]
struct PgpPayload {
    version: u32,
    encrypted_key: Vec<u8>,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

pub fn is_pgp_encrypted(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.contains(PGP_MESSAGE_HEADER) && trimmed.contains(PGP_MESSAGE_FOOTER)
}

pub fn is_pgp_signed(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.contains(PGP_SIGNED_HEADER) && trimmed.contains(PGP_SIG_HEADER) && trimmed.contains(PGP_SIG_FOOTER)
}

pub fn generate_pgp_keypair(email: &str) -> Result<PgpKeypair, EmailError> {
    let mut rng = rand::thread_rng();
    let private_key = RsaPrivateKey::new(&mut rng, 2048)
        .map_err(|e| EmailError::Encryption(format!("Failed to generate RSA key: {}", e)))?;
    let public_key = RsaPublicKey::from(&private_key);

    let priv_der = private_key
        .to_pkcs8_der()
        .map_err(|e| EmailError::Encryption(format!("Failed to encode private key: {}", e)))?;
    let pub_der = public_key
        .to_public_key_der()
        .map_err(|e| EmailError::Encryption(format!("Failed to encode public key: {}", e)))?;

    let priv_b64 = base64::engine::general_purpose::STANDARD.encode(priv_der.as_bytes());
    let pub_b64 = base64::engine::general_purpose::STANDARD.encode(pub_der.as_bytes());

    let mut hasher = Sha256::new();
    hasher.update(pub_der.as_bytes());
    let hash = hasher.finalize();
    let hex_fp = format!("{:X}", hash);
    let chunks: Vec<String> = hex_fp
        .as_bytes()
        .chunks(4)
        .take(8)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect();
    let fingerprint = chunks.join(" ");

    let public_key_armored = format!(
        "{}\nVersion: AT-Mail PGP v1.0\nComment: {}\n\n{}\n{}",
        PGP_PUBKEY_HEADER,
        email,
        pub_b64,
        PGP_PUBKEY_FOOTER
    );

    let private_key_armored = format!(
        "{}\nVersion: AT-Mail PGP v1.0\nComment: {}\n\n{}\n{}",
        PGP_PRIVKEY_HEADER,
        email,
        priv_b64,
        PGP_PRIVKEY_FOOTER
    );

    Ok(PgpKeypair {
        email: email.to_string(),
        fingerprint,
        public_key_armored,
        private_key_armored,
        created_at: chrono::Utc::now().timestamp(),
    })
}

fn extract_armored_block(text: &str, header: &str, footer: &str) -> Result<String, EmailError> {
    let start = text
        .find(header)
        .ok_or_else(|| EmailError::Encryption(format!("Missing header {}", header)))?;
    let rem = &text[start + header.len()..];
    let end = rem
        .find(footer)
        .ok_or_else(|| EmailError::Encryption(format!("Missing footer {}", footer)))?;

    let body = &rem[..end];
    let mut b64_lines = Vec::new();
    let mut seen_headers = false;
    let mut in_body = false;

    for line in body.lines() {
        let trimmed = line.trim();
        if in_body {
            if !trimmed.is_empty() {
                b64_lines.push(trimmed);
            }
        } else if trimmed.is_empty() {
            if seen_headers {
                in_body = true;
            }
        } else if trimmed.starts_with("Version:") || trimmed.starts_with("Comment:") || trimmed.starts_with("Hash:") {
            seen_headers = true;
        } else {
            in_body = true;
            b64_lines.push(trimmed);
        }
    }

    Ok(b64_lines.join(""))
}

pub fn pgp_encrypt(plain_text: &str, recipient_pubkey_armored: &str) -> Result<String, EmailError> {
    let pub_b64 = extract_armored_block(recipient_pubkey_armored, PGP_PUBKEY_HEADER, PGP_PUBKEY_FOOTER)?;
    let pub_der = base64::engine::general_purpose::STANDARD
        .decode(pub_b64)
        .map_err(|e| EmailError::Encryption(format!("Invalid public key base64: {}", e)))?;
    let pubkey = RsaPublicKey::from_public_key_der(&pub_der)
        .map_err(|e| EmailError::Encryption(format!("Invalid public key DER: {}", e)))?;

    let mut aes_key = [0u8; 32];
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut aes_key);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|e| EmailError::Encryption(format!("AES init failed: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plain_text.as_bytes())
        .map_err(|e| EmailError::Encryption(format!("Encryption failed: {}", e)))?;

    let mut rng = rand::thread_rng();
    let padding = Oaep::new::<Sha256>();
    let encrypted_key = pubkey
        .encrypt(&mut rng, padding, &aes_key)
        .map_err(|e| EmailError::Encryption(format!("RSA encryption failed: {}", e)))?;

    let payload = PgpPayload {
        version: 1,
        encrypted_key,
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    };

    let payload_bytes = serde_json::to_vec(&payload)
        .map_err(|e| EmailError::Encryption(format!("Serialization failed: {}", e)))?;
    let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload_bytes);

    // Format with 64 chars per line
    let formatted_b64 = payload_b64
        .as_bytes()
        .chunks(64)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "{}\nVersion: AT-Mail PGP v1.0\n\n{}\n{}",
        PGP_MESSAGE_HEADER,
        formatted_b64,
        PGP_MESSAGE_FOOTER
    ))
}

pub fn pgp_decrypt(armored_message: &str, privkey_armored: &str) -> Result<String, EmailError> {
    let priv_b64 = extract_armored_block(privkey_armored, PGP_PRIVKEY_HEADER, PGP_PRIVKEY_FOOTER)?;
    let priv_der = base64::engine::general_purpose::STANDARD
        .decode(priv_b64)
        .map_err(|e| EmailError::Encryption(format!("Invalid private key base64: {}", e)))?;
    let privkey = RsaPrivateKey::from_pkcs8_der(&priv_der)
        .map_err(|e| EmailError::Encryption(format!("Invalid private key DER: {}", e)))?;

    let msg_b64 = extract_armored_block(armored_message, PGP_MESSAGE_HEADER, PGP_MESSAGE_FOOTER)?;
    let payload_bytes = base64::engine::general_purpose::STANDARD
        .decode(msg_b64)
        .map_err(|e| EmailError::Encryption(format!("Invalid message base64: {}", e)))?;

    let payload: PgpPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|e| EmailError::Encryption(format!("Invalid payload structure: {}", e)))?;

    let padding = Oaep::new::<Sha256>();
    let aes_key = privkey
        .decrypt(padding, &payload.encrypted_key)
        .map_err(|e| EmailError::Encryption(format!("RSA decryption failed: {}", e)))?;

    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|e| EmailError::Encryption(format!("AES init failed: {}", e)))?;
    let nonce = Nonce::from_slice(&payload.nonce);
    let plain_bytes = cipher
        .decrypt(nonce, payload.ciphertext.as_ref())
        .map_err(|e| EmailError::Encryption(format!("AES decryption failed (corrupted or wrong key): {}", e)))?;

    String::from_utf8(plain_bytes)
        .map_err(|e| EmailError::Encryption(format!("Decrypted content is not valid UTF-8: {}", e)))
}

pub fn pgp_sign(plain_text: &str, privkey_armored: &str) -> Result<String, EmailError> {
    let priv_b64 = extract_armored_block(privkey_armored, PGP_PRIVKEY_HEADER, PGP_PRIVKEY_FOOTER)?;
    let priv_der = base64::engine::general_purpose::STANDARD
        .decode(priv_b64)
        .map_err(|e| EmailError::Encryption(format!("Invalid private key base64: {}", e)))?;
    let privkey = RsaPrivateKey::from_pkcs8_der(&priv_der)
        .map_err(|e| EmailError::Encryption(format!("Invalid private key DER: {}", e)))?;

    let mut hasher = Sha256::new();
    hasher.update(plain_text.as_bytes());
    let hashed = hasher.finalize();

    let padding = Pkcs1v15Sign::new::<Sha256>();
    let signature = privkey
        .sign(padding, &hashed)
        .map_err(|e| EmailError::Encryption(format!("Signing failed: {}", e)))?;

    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(&signature);

    Ok(format!(
        "{}\nHash: SHA256\n\n{}\n{}\nVersion: AT-Mail PGP v1.0\n\n{}\n{}",
        PGP_SIGNED_HEADER,
        plain_text,
        PGP_SIG_HEADER,
        sig_b64,
        PGP_SIG_FOOTER
    ))
}

pub fn pgp_verify(signed_message: &str, pubkey_armored: &str) -> Result<(String, bool), EmailError> {
    let pub_b64 = extract_armored_block(pubkey_armored, PGP_PUBKEY_HEADER, PGP_PUBKEY_FOOTER)?;
    let pub_der = base64::engine::general_purpose::STANDARD
        .decode(pub_b64)
        .map_err(|e| EmailError::Encryption(format!("Invalid public key base64: {}", e)))?;
    let pubkey = RsaPublicKey::from_public_key_der(&pub_der)
        .map_err(|e| EmailError::Encryption(format!("Invalid public key DER: {}", e)))?;

    let signed_start = signed_message
        .find(PGP_SIGNED_HEADER)
        .ok_or_else(|| EmailError::Encryption("Missing signed header".to_string()))?;
    let sig_start = signed_message
        .find(PGP_SIG_HEADER)
        .ok_or_else(|| EmailError::Encryption("Missing signature header".to_string()))?;

    let body_section = &signed_message[signed_start + PGP_SIGNED_HEADER.len()..sig_start];
    let mut body_lines = Vec::new();
    let mut past_hash = false;
    for line in body_section.lines() {
        if line.trim().starts_with("Hash:") {
            past_hash = true;
            continue;
        }
        if past_hash {
            body_lines.push(line);
        }
    }
    let body_text = body_lines.join("\n").trim().to_string();

    let sig_b64 = extract_armored_block(signed_message, PGP_SIG_HEADER, PGP_SIG_FOOTER)?;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sig_b64)
        .map_err(|e| EmailError::Encryption(format!("Invalid signature base64: {}", e)))?;

    let mut hasher = Sha256::new();
    hasher.update(body_text.as_bytes());
    let hashed = hasher.finalize();

    let padding = Pkcs1v15Sign::new::<Sha256>();
    let is_valid = pubkey.verify(padding, &hashed, &sig_bytes).is_ok();

    Ok((body_text, is_valid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pgp_keypair_generation_and_fingerprint() {
        let kp = generate_pgp_keypair("alice@example.com").unwrap();
        assert_eq!(kp.email, "alice@example.com");
        assert!(kp.public_key_armored.contains(PGP_PUBKEY_HEADER));
        assert!(kp.public_key_armored.contains(PGP_PUBKEY_FOOTER));
        assert!(kp.private_key_armored.contains(PGP_PRIVKEY_HEADER));
        assert!(kp.private_key_armored.contains(PGP_PRIVKEY_FOOTER));
        assert!(!kp.fingerprint.is_empty());
    }

    #[test]
    fn test_pgp_encrypt_decrypt_roundtrip() {
        let alice_kp = generate_pgp_keypair("alice@example.com").unwrap();
        let secret_message = "Confidential financial roadmap: Project Apollo launch Q4.";

        let encrypted = pgp_encrypt(secret_message, &alice_kp.public_key_armored).unwrap();
        assert!(is_pgp_encrypted(&encrypted));
        assert!(!encrypted.contains("Project Apollo"));

        let decrypted = pgp_decrypt(&encrypted, &alice_kp.private_key_armored).unwrap();
        assert_eq!(decrypted, secret_message);
    }

    #[test]
    fn test_pgp_sign_and_verify() {
        let bob_kp = generate_pgp_keypair("bob@example.com").unwrap();
        let document = "I hereby authorize release of build v2.1.0.";

        let signed = pgp_sign(document, &bob_kp.private_key_armored).unwrap();
        assert!(is_pgp_signed(&signed));

        let (verified_body, is_valid) = pgp_verify(&signed, &bob_kp.public_key_armored).unwrap();
        assert_eq!(verified_body, document);
        assert!(is_valid);

        // Verification fails with wrong key
        let charlie_kp = generate_pgp_keypair("charlie@example.com").unwrap();
        let (_, is_valid_wrong) = pgp_verify(&signed, &charlie_kp.public_key_armored).unwrap();
        assert!(!is_valid_wrong);
    }
}
