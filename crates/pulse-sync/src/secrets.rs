//! On-disk secrets in the data directory: the device key and the API token.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail};
use ed25519_dalek::SigningKey;
use pulse_core::DeviceId;

const DEVICE_KEY_FILE: &str = "device.key";
pub const API_TOKEN_FILE: &str = "api-token";

/// Loads this device's Ed25519 key, creating it on first run. The public key
/// is the device id that peers will pair with (M2).
pub fn load_or_create_device_id(data_dir: &Path) -> Result<DeviceId> {
    let secret = load_or_create_hex(&data_dir.join(DEVICE_KEY_FILE))?;
    let key = SigningKey::from_bytes(&secret);
    Ok(DeviceId::from_bytes(key.verifying_key().to_bytes()))
}

/// Loads the local API token, creating it on first run. The dev UI proxy reads
/// this file; the embedded UI receives it as a cookie.
pub fn load_or_create_api_token(data_dir: &Path) -> Result<String> {
    Ok(hex::encode(load_or_create_hex(
        &data_dir.join(API_TOKEN_FILE),
    )?))
}

fn load_or_create_hex(path: &Path) -> Result<[u8; 32]> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let bytes = hex::decode(text.trim())
                .with_context(|| format!("{} is corrupt", path.display()))?;
            match <[u8; 32]>::try_from(bytes) {
                Ok(arr) => Ok(arr),
                Err(_) => bail!("{} has the wrong length", path.display()),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut bytes = [0u8; 32];
            getrandom::fill(&mut bytes)
                .map_err(|e| anyhow::anyhow!("system RNG unavailable: {e}"))?;
            write_private(path, hex::encode(bytes).as_bytes())?;
            Ok(bytes)
        }
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Creates `path` readable only by the current user (on Unix).
fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .with_context(|| format!("cannot create {}", path.display()))?;
    f.write_all(contents)?;
    f.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_and_token_are_stable() {
        let dir = tempfile::tempdir().unwrap();
        let id = load_or_create_device_id(dir.path()).unwrap();
        assert_eq!(id, load_or_create_device_id(dir.path()).unwrap());
        let token = load_or_create_api_token(dir.path()).unwrap();
        assert_eq!(token.len(), 64);
        assert_eq!(token, load_or_create_api_token(dir.path()).unwrap());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.path().join(DEVICE_KEY_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
