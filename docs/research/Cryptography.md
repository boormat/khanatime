# Cryptography

> Extracted from: PLAN.md (original planning document)

---

## Identity and Cryptography

### Key Management
- **Algorithm:** Ed25519 (RFC 8032)
- **Persistence:** Keys persist across events (officials build identity over time)
- **Generation:** Each app instance generates a keypair on first launch
- **Storage:**
  - PWA: Web Crypto API CryptoKey objects in IndexedDB (non-extractable)
  - iOS: CryptoKit + Keychain (+ Secure Enclave on iOS 15+)
  - Android: Keystore API 33+ or BouncyCastle wrapper + EncryptedSharedPrefs
  - Fallback: @noble/ed25519 (3.9KB, audited, MIT)
- **QR Exchange:** Public key encoded as `did:key:z6Mk...` (W3C standard, ~55 chars)
- **Libraries:** noble-ed25519, react-native-quick-crypto, Flutter cryptography package

### Signing Model
- Every data submission signed by submitter's private key
- Signature + public key reference attached to each submission
- Timekeeper signs the **derived results** (not individual raw data)
- All data always accepted and logged (unsigned or signed)

### Identity Verification
- Out-of-band: QR code scan, voice call, in-person
- Matrix device verification (SAS / QR) can complement
- Fingerprint comparison (first 8 hex chars) for quick verification

### Trust Model
- Any authorized official can authorize new keys
- Timekeeper can revoke keys
- Emergency: all data accepted regardless of signature status
- Timekeeper role is transferable during event

---

## Ed25519 Crypto Libraries

| Platform | Key Gen + Sign | Secure Storage | QR Generation | QR Scanning |
|----------|---------------|----------------|---------------|-------------|
| iOS | CryptoKit | Keychain | Any QR lib | AVFoundation |
| Android | Keystore (API 33+) | Keystore + EncryptedSharedPrefs | Any QR lib | ZXing / ML Kit |
| PWA | Web Crypto API + noble fallback | IndexedDB (CryptoKey) | qrcode.react | html5-qrcode |
| React Native | react-native-quick-crypto | expo-secure-store | react-qr-code | react-native-vision-camera |
| Flutter | cryptography package | flutter_secure_storage | qr_flutter | mobile_scanner |

### Matrix Identity Integration
- Matrix already uses Ed25519 for device fingerprint keys
- Cross-signing hierarchy: Master → Self-Signing → User-Signing
- Element supports SAS and QR verification
- **Recommendation:** Keep separate application keypair, use Matrix verification for OOB trust
