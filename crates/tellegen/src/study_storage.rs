//! Atomic filesystem persistence for portable Study bundles.

#[cfg(unix)]
use std::fs::File;

use crate::document::StudyBundle;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct FileStudyStore {
    path: PathBuf,
}

struct WriteLock {
    path: PathBuf,
}
impl Drop for WriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl FileStudyStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn load(&self) -> Result<StudyBundle, String> {
        let metadata = fs::metadata(&self.path)
            .map_err(|e| format!("cannot read Study {}: {e}", self.path.display()))?;
        if metadata.len() > 512 * 1024 * 1024 {
            return Err("Study file exceeds 512 MiB; use a smaller branch bundle".into());
        }
        let text = fs::read_to_string(&self.path)
            .map_err(|e| format!("cannot read Study {}: {e}", self.path.display()))?;
        StudyBundle::import(&text)
    }

    /// Create a new store. Existing Studies require a revision-checked commit.
    pub fn create(&self, bundle: &StudyBundle) -> Result<(), String> {
        let _lock = self.lock()?;
        if self.path.exists() {
            return Err("Study file already exists; load it or select a new path".into());
        }
        self.write(bundle)
    }

    pub fn commit(&self, expected_revision: u64, bundle: &StudyBundle) -> Result<(), String> {
        let _lock = self.lock()?;
        let current = self.load()?;
        current.check_revision(expected_revision)?;
        if current.document.id != bundle.document.id
            || bundle.document.revision
                != expected_revision
                    .checked_add(1)
                    .ok_or("Study revision exhausted")?
        {
            return Err(
                "saved Study identity or next revision does not match the current file".into(),
            );
        }
        self.write(bundle)
    }

    fn lock(&self) -> Result<WriteLock, String> {
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if !parent.is_dir() {
            return Err(format!(
                "Study directory {} does not exist; create it or select another destination",
                parent.display()
            ));
        }
        let name = self
            .path
            .file_name()
            .ok_or("Study path requires a filename")?
            .to_string_lossy();
        let path = parent.join(format!(".{name}.lock"));
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| {
            format!("cannot lock Study {}: {e}. If a previous writer stopped, verify it is no longer running before removing {}", self.path.display(), path.display())
        })?;
        let lock = WriteLock { path };
        writeln!(file, "pid={}", std::process::id())
            .map_err(|e| format!("cannot write Study lock: {e}"))?;
        Ok(lock)
    }

    fn write(&self, bundle: &StudyBundle) -> Result<(), String> {
        let text = bundle.export()?;
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = self
            .path
            .file_name()
            .ok_or("Study path requires a filename")?
            .to_string_lossy();
        let temporary = parent.join(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let operation = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &self.path)?;
            Ok(())
        })();
        if let Err(error) = operation {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Study was not saved to {}: {error}. Free disk space or choose a writable destination, then retry", self.path.display()));
        }
        #[cfg(unix)]
        File::open(parent).and_then(|directory| directory.sync_all()).map_err(|error| format!(
            "Study file was written but directory durability could not be confirmed: {error}. Reload {} before retrying", self.path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn store_checks_revision_and_keeps_previous_data_on_failure() {
        let dir = std::env::temp_dir().join(format!(
            "tellegen-study-store-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let store = FileStudyStore::new(dir.join("study.json"));
        let mut bundle = StudyBundle::empty("test".into(), "Study".into()).unwrap();
        store.create(&bundle).unwrap();
        assert!(store.create(&bundle).is_err());
        bundle
            .transaction(0, |next| {
                next.document.title = "Updated".into();
                Ok(())
            })
            .unwrap();
        store.commit(0, &bundle).unwrap();
        assert!(store.commit(0, &bundle).unwrap_err().contains("stale"));
        assert_eq!(store.load().unwrap().document.title, "Updated");
        bundle.document.id = "different".into();
        bundle.document.revision = 2;
        assert!(store.commit(1, &bundle).is_err());
        assert_eq!(store.load().unwrap().document.id, "test");
        fs::write(dir.join(".study.json.lock"), "active writer").unwrap();
        assert!(store
            .commit(1, &bundle)
            .unwrap_err()
            .contains("cannot lock"));
        assert_eq!(store.load().unwrap().document.revision, 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
