use hpke::{
    Deserializable, Kem as _, OpModeR, OpModeS, Serializable, aead::ChaCha20Poly1305, kdf::HkdfSha384,
    kem::X25519HkdfSha256,
};
use rand::{SeedableRng, rngs::StdRng};

use crate::error::ServiceResult;

// Define the HPKE cipher suite to be used throughout the application
type Kem = X25519HkdfSha256;
type Aead = ChaCha20Poly1305;
type Kdf = HkdfSha384;

const INFO_STR: &[u8] = b"syncstore hpke v1";

/// generate new HPKE keypair
/// return (private_key_bytes, public_key_bytes)
pub fn generate_keypair() -> (Vec<u8>, Vec<u8>) {
    let mut rng = StdRng::from_os_rng();
    let (sk, pk) = Kem::gen_keypair(&mut rng);
    (sk.to_bytes().to_vec(), pk.to_bytes().to_vec())
}

/// decrypt function: typically used in server middleware to decrypt user data
/// usage: get user's private key from DB, then call this function to decrypt incoming data
/// arguments:
/// - ciphertext: the encrypted data received from client
/// - encapped_key_bytes: the encapsulated key bytes received from client
/// - private_key_bytes: the user's private key bytes retrieved from DB
/// - aad: associated additional data, should be the same as used in encryption (e.g., API path)
pub fn decrypt_data(
    ciphertext: &[u8],
    encapped_key_bytes: &[u8],
    private_key_bytes: &[u8],
    aad: &[u8],
) -> ServiceResult<Vec<u8>> {
    let sk = <Kem as hpke::kem::Kem>::PrivateKey::from_bytes(private_key_bytes)?;
    let encapped_key = <Kem as hpke::kem::Kem>::EncappedKey::from_bytes(encapped_key_bytes)?;
    let mut receiver_ctx = hpke::setup_receiver::<Aead, Kdf, Kem>(&OpModeR::Base, &sk, &encapped_key, INFO_STR)?;
    let plaintext = receiver_ctx.open(ciphertext, aad)?;
    Ok(plaintext)
}

/// encrypt function: typically used in client to encrypt data before sending to server
///
/// here is the server side responding user's request, using the a temporary user generated public key to encrypt data
///
/// arguments:
/// - plaintext: the raw data to be encrypted
/// - public_key_bytes: the user generated public key bytes obtained from request header or other means
/// - aad: associated additional data, e.g., API path to bind the encryption context
/// 
/// return: (encapsulated_key_bytes, ciphertext)
pub fn encrypt_data(plaintext: &[u8], public_key_bytes: &[u8], aad: &[u8]) -> ServiceResult<(Vec<u8>, Vec<u8>)> {
    let mut rng = StdRng::from_os_rng();
    let pk = <Kem as hpke::kem::Kem>::PublicKey::from_bytes(public_key_bytes)?;
    let (encapped_key, mut sender_ctx) =
        hpke::setup_sender::<Aead, Kdf, Kem, _>(&OpModeS::Base, &pk, INFO_STR, &mut rng)?;
    let ciphertext = sender_ctx.seal(plaintext, aad)?;
    Ok((encapped_key.to_bytes().to_vec(), ciphertext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hpke_flow() {
        // 1. Server: simulate generating a keypair for the user and storing it in DB
        let (sk_bytes, pk_bytes) = generate_keypair();

        // 2. Client: simulate encrypting business data using the obtained public key
        let raw_payload = b"{\"order_id\": 12345, \"amount\": 99.9}";
        let aad = b"/api/v1/order"; // Bind path as AAD for added security
        println!("Raw Payload: {:?}", raw_payload);
        println!("raw payload utf8: {}", String::from_utf8_lossy(raw_payload));

        let (enc_key, ciphertext) = encrypt_data(raw_payload, &pk_bytes, aad).expect("Client encryption failed");
        println!("Encapsulated Key: {:?}", enc_key);
        println!("Ciphertext: {:?}", ciphertext);
        println!("ciphertext utf8: {}", String::from_utf8_lossy(&ciphertext));

        // 3. Server middleware: upon receiving data, retrieve user's private key from DB to decrypt
        let decrypted_payload = decrypt_data(&ciphertext, &enc_key, &sk_bytes, aad).expect("Server decryption failed");

        assert_eq!(raw_payload.to_vec(), decrypted_payload);
    }

    #[test]
    fn test_wrong_aad_fails() {
        let (sk_bytes, pk_bytes) = generate_keypair();
        let (enc_key, ciphertext) = encrypt_data(b"secret", &pk_bytes, b"path_a").unwrap();

        // Attempt to decrypt with incorrect AAD, should fail
        let result = decrypt_data(&ciphertext, &enc_key, &sk_bytes, b"path_b");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_private_key_fails() {
        let (_sk1, pk1) = generate_keypair();
        let (sk2, _pk2) = generate_keypair();
        let (enc_key, ciphertext) = encrypt_data(b"secret", &pk1, b"path").unwrap();

        // Attempt to decrypt with incorrect private key, should fail
        let result = decrypt_data(&ciphertext, &enc_key, &sk2, b"path");
        assert!(result.is_err());
    }

    #[test]
    fn encrypt_twice_differs() {
        let (_sk, pk) = generate_keypair();
        let aad = b"/test/path";

        let (_enc1, ct1) = encrypt_data(b"data", &pk, aad).unwrap();
        let (_enc2, ct2) = encrypt_data(b"data", &pk, aad).unwrap();

        // Encrypting the same plaintext twice should yield different ciphertext
        assert_ne!(ct1, ct2);
    }
}
