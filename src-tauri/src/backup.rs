//! ZIP backup (SQLite snapshot + `images/`) and optional `AMBK` AES-GCM encryption.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use bip39::{Language, Mnemonic};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::{ZipArchive, ZipWriter};

const MAGIC: &[u8; 4] = b"AMBK";
const VERSION_V1: u8 = 1;
const VERSION_V2: u8 = 2;
/// Plaintext ZIP at or below this size uses single-blob v1 encryption.
const V1_MAX_PLAINTEXT_BYTES: u64 = 32 * 1024 * 1024;
/// Plaintext chunk size for v2 (4 MiB).
const V2_CHUNK_PLAINTEXT: u32 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupFileKind {
    Zip,
    Ambak,
    Unknown,
}

pub fn inspect_backup_file(path: &Path) -> Result<BackupFileKind, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut head = [0u8; 4];
    let n = f.read(&mut head).map_err(|e| e.to_string())?;
    if n < 4 {
        return Ok(BackupFileKind::Unknown);
    }
    if &head == MAGIC {
        return Ok(BackupFileKind::Ambak);
    }
    if head[0] == 0x50 && head[1] == 0x4b && head[2] == 0x03 && head[3] == 0x04 {
        return Ok(BackupFileKind::Zip);
    }
    Ok(BackupFileKind::Unknown)
}

fn normalize_mnemonic_phrase(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_mnemonic(phrase: &str) -> Result<Mnemonic, String> {
    let n = normalize_mnemonic_phrase(phrase);
    if n.is_empty() {
        return Err("Recovery phrase is required for this backup.".into());
    }
    Mnemonic::parse_in(Language::English, &n).map_err(|_| "Invalid recovery phrase.".into())
}

fn aes_key_from_mnemonic(mnemonic: &Mnemonic, passphrase: &str) -> [u8; 32] {
    let seed = mnemonic.to_seed(passphrase);
    let mut k = [0u8; 32];
    k.copy_from_slice(&seed[..32]);
    k
}

fn snapshot_db(live_db: &Path, snapshot_path: &Path) -> Result<(), String> {
    use rusqlite::backup::Backup;
    use std::time::Duration;

    let src = rusqlite::Connection::open(live_db).map_err(|e| e.to_string())?;
    let mut dst = rusqlite::Connection::open(snapshot_path).map_err(|e| e.to_string())?;
    let backup = Backup::new(&src, &mut dst).map_err(|e| e.to_string())?;
    backup
        .run_to_completion(5, Duration::from_millis(250), None)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn zip_options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

fn add_images_tree(
    zip: &mut ZipWriter<File>,
    images_dir: &Path,
    options: SimpleFileOptions,
) -> Result<(), String> {
    zip.add_directory("images/", options)
        .map_err(|e| e.to_string())?;
    if !images_dir.exists() {
        return Ok(());
    }
    fn walk(
        zip: &mut ZipWriter<File>,
        disk_dir: &Path,
        zip_prefix: &str,
        options: SimpleFileOptions,
    ) -> Result<(), String> {
        for entry in fs::read_dir(disk_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let zpath = format!("{zip_prefix}/{name}");
            if path.is_dir() {
                zip.add_directory(format!("{zpath}/"), options)
                    .map_err(|e| e.to_string())?;
                walk(zip, &path, &zpath, options)?;
            } else {
                zip.start_file(&zpath, options).map_err(|e| e.to_string())?;
                let mut src = File::open(&path).map_err(|e| e.to_string())?;
                std::io::copy(&mut src, zip).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    walk(zip, images_dir, "images", options)
}

/// Build a ZIP at `zip_path` (caller-chosen temp path).
pub fn build_backup_zip(db_path: &Path, images_dir: &Path, zip_path: &Path) -> Result<(), String> {
    let work_dir = zip_path
        .parent()
        .ok_or_else(|| "Invalid zip path.".to_string())?;
    fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let snap = work_dir.join(format!("am-snap-{}.db", Uuid::new_v4()));
    snapshot_db(db_path, &snap)?;

    let file = File::create(zip_path).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let opts = zip_options();

    zip.start_file("asset_manager.db", opts)
        .map_err(|e| e.to_string())?;
    let mut dbf = File::open(&snap).map_err(|e| e.to_string())?;
    std::io::copy(&mut dbf, &mut zip).map_err(|e| e.to_string())?;
    add_images_tree(&mut zip, images_dir, opts)?;
    zip.finish().map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&snap);
    Ok(())
}

fn encrypt_zip_v1(zip_path: &Path, out_path: &Path, key: &[u8; 32]) -> Result<(), String> {
    let plaintext = fs::read(zip_path).map_err(|e| e.to_string())?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_ref())
        .map_err(|_| "Encryption failed.".to_string())?;

    let mut out = File::create(out_path).map_err(|e| e.to_string())?;
    out.write_all(MAGIC).map_err(|e| e.to_string())?;
    out.write_all(&[VERSION_V1]).map_err(|e| e.to_string())?;
    out.write_all(nonce.as_slice()).map_err(|e| e.to_string())?;
    out.write_all(&ciphertext).map_err(|e| e.to_string())?;
    Ok(())
}

fn encrypt_zip_v2(zip_path: &Path, out_path: &Path, key: &[u8; 32]) -> Result<(), String> {
    let mut input = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut out = File::create(out_path).map_err(|e| e.to_string())?;
    out.write_all(MAGIC).map_err(|e| e.to_string())?;
    out.write_all(&[VERSION_V2]).map_err(|e| e.to_string())?;
    out.write_all(&V2_CHUNK_PLAINTEXT.to_le_bytes())
        .map_err(|e| e.to_string())?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let chunk = V2_CHUNK_PLAINTEXT as usize;
    let mut buf = vec![0u8; chunk];
    loop {
        let n = read_fill_partial(&mut input, &mut buf)?;
        if n == 0 {
            break;
        }
        let slice = &buf[..n];
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = cipher
            .encrypt(&nonce, slice)
            .map_err(|_| "Encryption failed.".to_string())?;
        out.write_all(nonce.as_slice()).map_err(|e| e.to_string())?;
        let len_u32 = (ct.len() as u32).to_le_bytes();
        out.write_all(&len_u32).map_err(|e| e.to_string())?;
        out.write_all(&ct).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_fill_partial(r: &mut File, buf: &mut [u8]) -> Result<usize, String> {
    let mut total = 0usize;
    while total < buf.len() {
        let n = r.read(&mut buf[total..]).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

fn encrypt_zip_file(zip_path: &Path, ambak_path: &Path, key: &[u8; 32]) -> Result<(), String> {
    let len = fs::metadata(zip_path).map_err(|e| e.to_string())?.len();
    if len <= V1_MAX_PLAINTEXT_BYTES {
        encrypt_zip_v1(zip_path, ambak_path, key)
    } else {
        encrypt_zip_v2(zip_path, ambak_path, key)
    }
}

fn decrypt_ambak_to_zip_path(
    ambak_path: &Path,
    zip_out: &Path,
    key: &[u8; 32],
) -> Result<(), String> {
    let mut f = File::open(ambak_path).map_err(|e| e.to_string())?;
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != MAGIC {
        return Err("Not an encrypted Asset Manager backup.".into());
    }
    let mut ver = [0u8; 1];
    f.read_exact(&mut ver).map_err(|e| e.to_string())?;
    match ver[0] {
        VERSION_V1 => decrypt_ambak_v1(&mut f, zip_out, key),
        VERSION_V2 => decrypt_ambak_v2(&mut f, zip_out, key),
        _ => Err("Unsupported encrypted backup version.".into()),
    }
}

fn decrypt_ambak_v1(f: &mut File, zip_out: &Path, key: &[u8; 32]) -> Result<(), String> {
    let mut nonce = [0u8; 12];
    f.read_exact(&mut nonce).map_err(|e| e.to_string())?;
    let mut ciphertext = Vec::new();
    f.read_to_end(&mut ciphertext).map_err(|e| e.to_string())?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let n = *Nonce::from_slice(&nonce);
    let plain = cipher
        .decrypt(&n, ciphertext.as_ref())
        .map_err(|_| wrong_key_err())?;
    fs::write(zip_out, plain).map_err(|e| e.to_string())?;
    Ok(())
}

fn wrong_key_err() -> String {
    "Wrong recovery phrase or passphrase.".into()
}

fn decrypt_ambak_v2(f: &mut File, zip_out: &Path, key: &[u8; 32]) -> Result<(), String> {
    let mut cs = [0u8; 4];
    f.read_exact(&mut cs).map_err(|e| e.to_string())?;
    let _chunk_size = u32::from_le_bytes(cs);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut out = File::create(zip_out).map_err(|e| e.to_string())?;
    loop {
        let mut nonce = [0u8; 12];
        match f.read_exact(&mut nonce) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        }
        let mut lenb = [0u8; 4];
        f.read_exact(&mut lenb).map_err(|e| e.to_string())?;
        let ct_len = u32::from_le_bytes(lenb) as usize;
        let mut ciphertext = vec![0u8; ct_len];
        f.read_exact(&mut ciphertext).map_err(|e| e.to_string())?;
        let n = *Nonce::from_slice(&nonce);
        let chunk = cipher
            .decrypt(&n, ciphertext.as_ref())
            .map_err(|_| wrong_key_err())?;
        out.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn validate_sqlite_db(path: &Path) -> Result<(), String> {
    let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("PRAGMA quick_check")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let row = rows.next().map_err(|e| e.to_string())?;
    let Some(row) = row else {
        return Err("Backup database is empty or invalid.".into());
    };
    let status: String = row.get(0).map_err(|e| e.to_string())?;
    if status != "ok" {
        return Err(format!("Database check failed: {status}"));
    }
    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut zf = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(enclosed) = zf.enclosed_name() else {
            continue;
        };
        let outpath = out_dir.join(enclosed);
        if zf.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut zf, &mut outfile).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Replace live DB and images from an extracted temp dir containing `asset_manager.db` and optional `images/`.
pub fn apply_extracted_backup(
    db_path: &Path,
    images_dir: &Path,
    extract_dir: &Path,
) -> Result<(), String> {
    let backup_db = extract_dir.join("asset_manager.db");
    if !backup_db.is_file() {
        return Err("Backup is missing asset_manager.db at the archive root.".into());
    }
    validate_sqlite_db(&backup_db)?;

    if db_path.exists() {
        fs::remove_file(db_path).map_err(|e| e.to_string())?;
    }
    fs::copy(&backup_db, db_path).map_err(|e| e.to_string())?;

    if images_dir.exists() {
        fs::remove_dir_all(images_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(images_dir).map_err(|e| e.to_string())?;
    let src_images = extract_dir.join("images");
    if src_images.is_dir() {
        copy_dir_all(&src_images, images_dir)?;
    }
    Ok(())
}

fn temp_zip_path() -> PathBuf {
    std::env::temp_dir().join(format!("am-backup-{}.zip", Uuid::new_v4()))
}

/// Plain ZIP export.
pub fn export_plain_zip(db_path: &Path, images_dir: &Path, dest: &Path) -> Result<(), String> {
    let tmp = temp_zip_path();
    build_backup_zip(db_path, images_dir, &tmp)?;
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    if dest.exists() {
        fs::remove_file(dest).map_err(|e| e.to_string())?;
    }
    match fs::rename(&tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(&tmp, dest).map_err(|e| e.to_string())?;
            fs::remove_file(&tmp).map_err(|e| e.to_string())?;
            Ok(())
        }
    }
}

/// Encrypted export; returns mnemonic phrase (space-separated).
pub fn export_encrypted(
    db_path: &Path,
    images_dir: &Path,
    dest: &Path,
    word_count: usize,
    passphrase: &str,
) -> Result<String, String> {
    if word_count != 12 && word_count != 24 {
        return Err("wordCount must be 12 or 24.".into());
    }
    let mnemonic =
        Mnemonic::generate_in(Language::English, word_count).map_err(|e| e.to_string())?;
    let phrase = mnemonic.to_string();
    let key = aes_key_from_mnemonic(&mnemonic, passphrase);

    let tmp_zip = temp_zip_path();
    build_backup_zip(db_path, images_dir, &tmp_zip)?;
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    if dest.exists() {
        fs::remove_file(dest).map_err(|e| e.to_string())?;
    }
    let tmp_ambak = std::env::temp_dir().join(format!("am-ambak-{}.part", Uuid::new_v4()));
    encrypt_zip_file(&tmp_zip, &tmp_ambak, &key)?;
    let _ = fs::remove_file(&tmp_zip);
    match fs::rename(&tmp_ambak, dest) {
        Ok(()) => {}
        Err(_) => {
            fs::copy(&tmp_ambak, dest).map_err(|e| e.to_string())?;
            fs::remove_file(&tmp_ambak).map_err(|e| e.to_string())?;
        }
    }
    Ok(phrase)
}

/// Import from `.zip` or `.ambak` at `src`.
pub fn import_from_path(
    db_path: &Path,
    images_dir: &Path,
    src: &Path,
    mnemonic_phrase: Option<&str>,
    passphrase: &str,
) -> Result<(), String> {
    let work = std::env::temp_dir().join(format!("am-import-{}", Uuid::new_v4()));
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let zip_work = work.join("payload.zip");

    let head = inspect_backup_file(src)?;
    match head {
        BackupFileKind::Ambak => {
            let phrase = mnemonic_phrase.ok_or_else(|| {
                "This backup is encrypted. Enter your recovery phrase.".to_string()
            })?;
            let mnemonic = parse_mnemonic(phrase)?;
            let key = aes_key_from_mnemonic(&mnemonic, passphrase);
            decrypt_ambak_to_zip_path(src, &zip_work, &key)?;
        }
        BackupFileKind::Zip => {
            fs::copy(src, &zip_work).map_err(|e| e.to_string())?;
        }
        BackupFileKind::Unknown => {
            return Err(
                "Unrecognized backup file (expected a .zip or encrypted Asset Manager backup)."
                    .into(),
            );
        }
    }

    let extract_dir = work.join("extract");
    extract_zip_to_dir(&zip_work, &extract_dir)?;
    apply_extracted_backup(db_path, images_dir, &extract_dir)?;
    let _ = fs::remove_dir_all(&work);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn sample_part(name: &str) -> crate::db::AssetInput {
        crate::db::AssetInput {
            kind: "part".into(),
            name: name.into(),
            manufacturer: None,
            model: None,
            serial_number: None,
            caliber: None,
            quantity: Some(1),
            purchase_date: None,
            purchase_price: None,
            notes: None,
            extra_json: Some("{}".into()),
            maintenance_every_n_rounds: None,
            maintenance_every_n_days: None,
            subtype: None,
            tags: None,
        }
    }

    #[test]
    fn v1_roundtrip_encrypt_decrypt() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("a.zip");
        fs::write(&zip, b"hello zip payload").unwrap();
        let ambak = dir.path().join("out.ambak");
        let key = [7u8; 32];
        encrypt_zip_v1(&zip, &ambak, &key).unwrap();
        let out = dir.path().join("b.zip");
        decrypt_ambak_to_zip_path(&ambak, &out, &key).unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"hello zip payload");
    }

    #[test]
    fn v2_roundtrip_and_wrong_key() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("big.zip");
        let mut body = vec![0u8; V2_CHUNK_PLAINTEXT as usize + 100];
        for (i, b) in body.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        fs::write(&zip, &body).unwrap();
        let ambak = dir.path().join("out.ambak");
        let key = [9u8; 32];
        encrypt_zip_v2(&zip, &ambak, &key).unwrap();
        let out = dir.path().join("back.zip");
        decrypt_ambak_to_zip_path(&ambak, &out, &key).unwrap();
        assert_eq!(fs::read(&out).unwrap(), body);
        let bad = [0u8; 32];
        let out_bad = dir.path().join("x.zip");
        let r = decrypt_ambak_to_zip_path(&ambak, &out_bad, &bad);
        assert!(r.is_err());
    }

    #[test]
    fn mnemonic_key_and_parse() {
        let m = Mnemonic::generate_in(Language::English, 12).unwrap();
        let k1 = aes_key_from_mnemonic(&m, "secret");
        let k2 = aes_key_from_mnemonic(&parse_mnemonic(&m.to_string()).unwrap(), "secret");
        assert_eq!(k1, k2);
        let k3 = aes_key_from_mnemonic(&m, "other");
        assert_ne!(k1, k3);
    }

    #[test]
    fn inspect_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let short = dir.path().join("short");
        fs::write(&short, [1, 2]).unwrap();
        assert_eq!(
            inspect_backup_file(&short).unwrap(),
            BackupFileKind::Unknown
        );

        let ambak = dir.path().join("x.ambak");
        fs::write(&ambak, b"AMBK\x01").unwrap();
        assert_eq!(inspect_backup_file(&ambak).unwrap(), BackupFileKind::Ambak);
    }

    #[test]
    fn plain_export_import_roundtrip() {
        let d1 = tempfile::tempdir().unwrap();
        let db1 = d1.path().join("asset_manager.db");
        let img1 = d1.path().join("images");
        crate::db::init(&db1, &img1).unwrap();
        let c = Connection::open(&db1).unwrap();
        let a = crate::db::create_asset(&c, sample_part("BackupProbe")).unwrap();
        drop(c);

        let zip = d1.path().join("out.zip");
        export_plain_zip(&db1, &img1, &zip).unwrap();
        assert_eq!(inspect_backup_file(&zip).unwrap(), BackupFileKind::Zip);

        let d2 = tempfile::tempdir().unwrap();
        let db2 = d2.path().join("asset_manager.db");
        let img2 = d2.path().join("images");
        crate::db::init(&db2, &img2).unwrap();
        let c2 = Connection::open(&db2).unwrap();
        let _ = crate::db::create_asset(&c2, sample_part("Other")).unwrap();
        drop(c2);

        import_from_path(&db2, &img2, &zip, None, "").unwrap();

        let c3 = Connection::open(&db2).unwrap();
        let got = crate::db::get_asset(&c3, &a.id).unwrap().unwrap();
        assert_eq!(got.name, "BackupProbe");
    }

    #[test]
    fn encrypted_export_import_roundtrip() {
        let d1 = tempfile::tempdir().unwrap();
        let db1 = d1.path().join("asset_manager.db");
        let img1 = d1.path().join("images");
        crate::db::init(&db1, &img1).unwrap();
        let c = Connection::open(&db1).unwrap();
        let a = crate::db::create_asset(&c, sample_part("EncProbe")).unwrap();
        drop(c);

        let ambak = d1.path().join("e.ambak");
        let phrase = export_encrypted(&db1, &img1, &ambak, 12, "pp").unwrap();
        assert_eq!(inspect_backup_file(&ambak).unwrap(), BackupFileKind::Ambak);

        let d2 = tempfile::tempdir().unwrap();
        let db2 = d2.path().join("asset_manager.db");
        let img2 = d2.path().join("images");
        crate::db::init(&db2, &img2).unwrap();

        import_from_path(&db2, &img2, &ambak, Some(phrase.as_str()), "pp").unwrap();

        let c3 = Connection::open(&db2).unwrap();
        let got = crate::db::get_asset(&c3, &a.id).unwrap().unwrap();
        assert_eq!(got.name, "EncProbe");
    }

    #[test]
    fn encrypted_wrong_passphrase_fails() {
        let d1 = tempfile::tempdir().unwrap();
        let db1 = d1.path().join("asset_manager.db");
        let img1 = d1.path().join("images");
        crate::db::init(&db1, &img1).unwrap();
        let c = Connection::open(&db1).unwrap();
        crate::db::create_asset(&c, sample_part("X")).unwrap();
        drop(c);

        let ambak = d1.path().join("e.ambak");
        let phrase = export_encrypted(&db1, &img1, &ambak, 24, "good").unwrap();

        let d2 = tempfile::tempdir().unwrap();
        let db2 = d2.path().join("asset_manager.db");
        let img2 = d2.path().join("images");
        crate::db::init(&db2, &img2).unwrap();

        let r = import_from_path(&db2, &img2, &ambak, Some(phrase.as_str()), "bad");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("Wrong"));
    }

    #[test]
    fn encrypted_requires_mnemonic() {
        let d1 = tempfile::tempdir().unwrap();
        let db1 = d1.path().join("asset_manager.db");
        let img1 = d1.path().join("images");
        crate::db::init(&db1, &img1).unwrap();
        let c = Connection::open(&db1).unwrap();
        crate::db::create_asset(&c, sample_part("Y")).unwrap();
        drop(c);

        let ambak = d1.path().join("e.ambak");
        let _phrase = export_encrypted(&db1, &img1, &ambak, 12, "").unwrap();

        let d2 = tempfile::tempdir().unwrap();
        let db2 = d2.path().join("asset_manager.db");
        let img2 = d2.path().join("images");
        crate::db::init(&db2, &img2).unwrap();

        let r = import_from_path(&db2, &img2, &ambak, None, "");
        assert!(r.is_err());
    }

    #[test]
    fn import_unknown_file_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("junk.bin");
        fs::write(&junk, b"hello").unwrap();

        let d2 = tempfile::tempdir().unwrap();
        let db2 = d2.path().join("asset_manager.db");
        let img2 = d2.path().join("images");
        crate::db::init(&db2, &img2).unwrap();

        let r = import_from_path(&db2, &img2, &junk, None, "");
        assert!(r.is_err());
    }

    #[test]
    fn export_encrypted_rejects_word_count() {
        let d = tempfile::tempdir().unwrap();
        let db = d.path().join("asset_manager.db");
        let img = d.path().join("images");
        crate::db::init(&db, &img).unwrap();
        let out = d.path().join("x.ambak");
        let r = export_encrypted(&db, &img, &out, 18, "");
        assert!(r.is_err());
    }
}
