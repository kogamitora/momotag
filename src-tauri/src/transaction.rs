use crate::filesystem::retry_io;
use crate::logger::{log_error, log_info, log_warn};
use crate::models::MusicFile;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct MutationJournal {
    original_folder: PathBuf,
    target_folder: Option<PathBuf>,
    backup_dir: PathBuf,
    backups: Vec<(PathBuf, PathBuf)>,
    modified_indices: Vec<usize>,
    renamed_files: Vec<(PathBuf, PathBuf)>,
    copied_cover: Option<PathBuf>,
    replaced_cover: Option<(PathBuf, PathBuf)>,
    folder_renamed: bool,
}

impl MutationJournal {
    pub(crate) fn begin(folder_path: &str, files: &[MusicFile]) -> Result<Self, String> {
        let folder = Path::new(folder_path);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "Could not create a temporary backup directory.".to_string())?
            .as_nanos();
        let backup_dir = std::env::temp_dir().join(format!(".momotag-backup-{stamp}"));
        fs::create_dir_all(&backup_dir)
            .map_err(|err| format!("Could not create a temporary backup directory: {err}"))?;

        let mut backups = Vec::with_capacity(files.len());
        for (index, file) in files.iter().enumerate() {
            let source = Path::new(&file.path);
            let backup = backup_dir.join(format!("{index}.backup"));
            if let Err(error) = retry_io(|| fs::copy(source, &backup)) {
                log_error(
                    "journal",
                    format!(
                        "failed to create backup for {} -> {}: {error}",
                        source.display(),
                        backup.display()
                    ),
                );
                let _ = fs::remove_dir_all(&backup_dir);
                return Err(format!(
                    "Could not prepare backup for {}: {error}",
                    file.file_name
                ));
            }
            backups.push((source.to_path_buf(), backup));
        }

        log_info(
            "journal",
            format!(
                "created backup journal dir={} for {} files",
                backup_dir.display(),
                backups.len()
            ),
        );

        Ok(Self {
            original_folder: folder.to_path_buf(),
            target_folder: None,
            backup_dir,
            backups,
            modified_indices: Vec::new(),
            renamed_files: Vec::new(),
            copied_cover: None,
            replaced_cover: None,
            folder_renamed: false,
        })
    }

    pub(crate) fn record_modified(&mut self, index: usize) {
        if !self.modified_indices.contains(&index) {
            self.modified_indices.push(index);
        }
    }

    pub(crate) fn record_rename(&mut self, original: PathBuf, target: PathBuf) {
        if original != target {
            self.renamed_files.push((original, target));
        }
    }

    #[allow(dead_code)]
    pub(crate) fn record_folder_rename(&mut self, target: &Path) {
        self.target_folder = Some(target.to_path_buf());
        self.folder_renamed = true;
    }

    pub(crate) fn record_cover(&mut self, path: PathBuf, previous: Option<PathBuf>) {
        self.copied_cover = Some(path.clone());
        self.replaced_cover = previous.map(|value| (path, value));
    }

    pub(crate) fn backup_path(&self, name: &str) -> PathBuf {
        self.backup_dir.join(name)
    }

    pub(crate) fn commit(&mut self) -> Option<String> {
        log_info(
            "journal",
            format!("committing journal and removing {}", self.backup_dir.display()),
        );
        match retry_io(|| fs::remove_dir_all(&self.backup_dir)) {
            Ok(()) => None,
            Err(error) => {
                log_error(
                    "journal",
                    format!(
                        "could not remove committed backup directory {}: {error}",
                        self.backup_dir.display()
                    ),
                );
                Some(format!(
                    "Could not remove temporary backup directory {}: {error}",
                    self.backup_dir.display()
                ))
            }
        }
    }

    pub(crate) fn rollback(&mut self) -> Option<String> {
        log_warn(
            "journal::rollback",
            format!(
                "starting rollback (modified_indices={:?}, renamed_files={}, folder_renamed={})",
                self.modified_indices,
                self.renamed_files.len(),
                self.folder_renamed
            ),
        );
        let mut errors = Vec::new();
        if self.folder_renamed {
            if let Some(current) = &self.target_folder {
                if current.exists() && !self.original_folder.exists() {
                    if let Err(error) = retry_io(|| fs::rename(current, &self.original_folder)) {
                        log_error(
                            "journal::rollback",
                            format!("failed to restore album folder: {error}"),
                        );
                        errors.push(format!("could not restore album folder: {error}"));
                    }
                }
            }
        }

        // Remove renamed target files first so restoring the backup to `original`
        // happens in a single write instead of rename + immediate overwrite (which
        // triggers Windows SearchIndexer sharing violations on `original`).
        for (original, target) in self.renamed_files.iter().rev() {
            if target.exists() && target != original {
                let has_backup = self
                    .backups
                    .iter()
                    .any(|(orig, bak)| orig == original && bak.exists());
                if has_backup {
                    if let Err(error) = retry_io(|| fs::remove_file(target)) {
                        log_warn(
                            "journal::rollback",
                            format!("could not remove renamed target {}: {error}", target.display()),
                        );
                        if !original.exists() {
                            let _ = retry_io(|| fs::rename(target, original));
                        } else {
                            errors.push(format!("could not clean up {}: {error}", target.display()));
                        }
                    }
                } else if !original.exists() {
                    if let Err(error) = retry_io(|| fs::rename(target, original)) {
                        errors.push(format!("could not restore {}: {error}", original.display()));
                    }
                }
            }
        }

        for &index in &self.modified_indices {
            if let Some((original, backup)) = self.backups.get(index) {
                if let Err(error) = retry_io(|| fs::copy(backup, original)) {
                    log_error(
                        "journal::rollback",
                        format!("could not restore {} from backup: {error}", original.display()),
                    );
                    errors.push(format!("could not restore {}: {error}", original.display()));
                } else {
                    log_info(
                        "journal::rollback",
                        format!("restored {} from backup", original.display()),
                    );
                }
            }
        }

        if let Some((cover, previous)) = &self.replaced_cover {
            if let Err(error) = retry_io(|| fs::copy(previous, cover)) {
                errors.push(format!("could not restore cover: {error}"));
            }
        } else if let Some(cover) = &self.copied_cover {
            if let Err(error) = retry_io(|| fs::remove_file(cover)) {
                log_error(
                    "journal::rollback",
                    format!("could not remove newly copied cover {}: {error}", cover.display()),
                );
                errors.push(format!("could not remove cover {}: {error}", cover.display()));
            }
        }

        if errors.is_empty() {
            if let Err(error) = retry_io(|| fs::remove_dir_all(&self.backup_dir)) {
                log_warn(
                    "journal::rollback",
                    format!(
                        "rollback succeeded but could not remove backup directory {}: {error}",
                        self.backup_dir.display()
                    ),
                );
                return Some(format!(
                    "could not remove temporary backup directory {}: {error}",
                    self.backup_dir.display()
                ));
            }
            None
        } else {
            // Preserve the only recovery copy when any restore/cleanup step
            // failed. Include its path in the returned diagnostic for recovery.
            log_error(
                "journal::rollback",
                format!(
                    "rollback incomplete; preserving backup directory {}",
                    self.backup_dir.display()
                ),
            );
            Some(format!(
                "{}; recovery backups preserved at {}",
                errors.join("; "),
                self.backup_dir.display()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MutationJournal;
    use crate::models::MusicFile;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rollback_restores_modified_and_renamed_files_without_touching_unmodified_ones() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(".momotag-test-journal-{stamp}"));
        fs::create_dir_all(&temp_dir).unwrap();

        let file1_orig = temp_dir.join("01 orig.flac");
        let file1_target = temp_dir.join("01 renamed.flac");
        let file2_orig = temp_dir.join("02 orig.flac");

        fs::write(&file1_orig, b"original-1").unwrap();
        fs::write(&file2_orig, b"original-2").unwrap();

        let files = vec![
            MusicFile {
                path: file1_orig.display().to_string(),
                file_name: "01 orig.flac".to_string(),
                artist: None,
            },
            MusicFile {
                path: file2_orig.display().to_string(),
                file_name: "02 orig.flac".to_string(),
                artist: None,
            },
        ];

        let mut journal = MutationJournal::begin(&temp_dir.display().to_string(), &files).unwrap();

        // Simulate modifying and renaming file 0, then failing before file 1 is touched.
        journal.record_modified(0);
        fs::write(&file1_orig, b"mutated-1").unwrap();
        fs::rename(&file1_orig, &file1_target).unwrap();
        journal.record_rename(file1_orig.clone(), file1_target.clone());

        let rollback_err = journal.rollback();
        assert!(rollback_err.is_none());
        assert!(file1_orig.exists());
        assert!(!file1_target.exists());
        assert_eq!(fs::read(&file1_orig).unwrap(), b"original-1");
        assert_eq!(fs::read(&file2_orig).unwrap(), b"original-2");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
