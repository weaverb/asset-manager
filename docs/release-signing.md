# Release code signing (maintainers)

This document describes how to configure **optional** code signing for GitHub Actions release builds. If the secrets below are absent, releases still build **unsigned** installers (the workflow skips signing steps).

Official references: [Tauri v2 — macOS code signing](https://v2.tauri.app/distribute/sign/macos/), [Tauri v2 — Windows code signing](https://v2.tauri.app/distribute/sign/windows/), and the workflow in [`.github/workflows/release.yml`](../.github/workflows/release.yml).

## macOS: Developer ID and notarization

### Prerequisites

- Paid [Apple Developer Program](https://developer.apple.com/) membership.
- A **Developer ID Application** certificate exported as `.p12` (see Apple’s docs on creating and exporting certificates).

### Repository secrets

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` (e.g. `openssl base64 -A -in cert.p12 \| tr -d '\n'`). |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` export. |
| `KEYCHAIN_PASSWORD` | Ephemeral keychain password used only on the CI runner (choose any strong random string). |

For **notarization** via [App Store Connect API keys](https://developer.apple.com/documentation/appstoreconnectapi/creating_api_keys_for_app_store_connect_api):

| Secret | Purpose |
|--------|---------|
| `APPLE_API_ISSUER` | Issuer ID from App Store Connect → Users and Access → Integrations. |
| `APPLE_API_KEY` | Key ID of the API key. |
| `APPLE_API_KEY_P8_BASE64` | Base64-encoded contents of the `.p8` private key file (downloaded once from Apple). |

Alternatively, Tauri supports Apple ID-based notarization credentials; the release workflow also forwards `APPLE_ID` and `APPLE_PASSWORD` if you set those secrets (see Tauri’s macOS signing guide for details and app-specific password guidance).

### Rotation

- Replace `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` when the Developer ID certificate is renewed or reissued.
- API keys: revoke the old key in App Store Connect, create a new `.p8`, and update `APPLE_API_KEY`, `APPLE_API_ISSUER`, and `APPLE_API_KEY_P8_BASE64`.
- Prefer a **dedicated** CI/API key with minimal access, not a personal Apple ID password, where possible.

### Verification

On a clean Mac, download the release `.dmg` or `.app` from GitHub Releases, open it, and confirm Gatekeeper does not show “unidentified developer” or “damaged” warnings.

## Windows: Authenticode (PFX)

### Prerequisites

- A valid **code signing** certificate (Authenticode), exported as `.pfx` with a known password—not an SSL/TLS cert.

### Repository secrets

| Secret | Purpose |
|--------|---------|
| `WINDOWS_CERTIFICATE` | Base64-encoded `.pfx` (on Windows: `certutil -encode cert.pfx out.txt` and paste the inner base64; or encode on any OS with OpenSSL). |
| `WINDOWS_CERTIFICATE_PASSWORD` | Export password for the `.pfx`. |

The workflow imports the PFX into the runner’s user certificate store, reads the **thumbprint** from the imported cert, and injects `certificateThumbprint`, `digestAlgorithm`, and `timestampUrl` into `tauri.conf.json` only for that job (nothing is committed).

### Rotation

- When the certificate expires or is reissued, update both secrets with the new `.pfx` and password.

### Verification

- Open **Properties → Digital Signatures** on the `.msi` / `.exe` and confirm the signature is present and valid.

## Workflow notes

GitHub Actions does **not** allow the `secrets` context in step `if` conditions (see [workflow syntax](https://docs.github.com/en/actions/learn-github-actions/expressions#about-expressions)). The release workflow runs optional signing steps only on the relevant OS matrix legs and **no-ops inside the script** when the corresponding secrets are unset, so forks and repos without secrets still build unsigned artifacts.

## Security notes

- Never commit certificates, `.p12`, `.pfx`, or `.p8` files to the repository.
- Do not echo secret values in workflow logs; GitHub masks configured secrets, but avoid printing decoded material in debug steps.
- Use **repository or organization secrets** with restricted access; rotate on maintainer offboarding.

## Who owns this

Assign an owner (team or individual) for Apple and Windows certificates: ordering renewals, storing offline backups of keys where policy requires, and updating GitHub secrets.
