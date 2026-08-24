use crate::models::MusicFile;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct MutationJournal {
    original_folder: PathBuf,
    target_folder: Option<PathBuf>,
    backup_dir: PathBuf,
    backups: Vec<(PathBuf, PathBuf)>,
    renamed_files: Vec<(PathBuf, PathBuf)>,
    copied_cover: Option<PathBuf>,
    replaced_cover: Option<(PathBuf, PathBuf)>,
    folder_renamed: bool,
}

impl MutationJournal {
    pub(crate) fn begin(folder_path: &str, files: &[MusicFile]) -> Result<Self, String> {
        let folder = Path::new(folder_path);
        let parent = folder
            .parent()
            .ok_or_else(|| "Could not find the parent folder for the album folder.".to_string())?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "Could not create a temporary backup directory.".to_string())?
            .as_nanos();
        let backup_dir = parent.join(format!(".momotag-backup-{stamp}"));
        fs::create_dir(&backup_dir)
            .map_err(|err| format!("Could not create a temporary backup directory: {err}"))?;

        let mut backups = Vec::with_capacity(files.len());
        for (index, file) in files.iter().enumerate() {
            let source = Path::new(&file.path);
            let backup = backup_dir.join(format!("{index}.backup"));
            if let Err(error) = fs::copy(source, &backup) {
                let _ = fs::remove_dir_all(&backup_dir);
                return Err(format!(
                    "Could not prepare backup for {}: {error}",
                    file.file_name
                ));
            }
            backups.push((source.to_path_buf(), backup));
        }

        Ok(Self {
            original_folder: folder.to_path_buf(),
            target_folder: None,
            backup_dir,
            backups,
            renamed_files: Vec::new(),
            copied_cover: None,
            replaced_cover: None,
            folder_renamed: false,
        })
    }

    pub(crate) fn record_rename(&mut self, original: PathBuf, target: PathBuf) {
        if original != target {
            self.renamed_files.push((original, target));
        }
    }

    pub(crate) fn record_folder_rename(&mut self, target: &Path) {
        self.target_folder = Some(target.to_path_buf());
        self.folder_renamed = true;
    }

    pub(crate) fn record_cover(&mut self, path: PathBuf, previous: Option<PathBuf>) {
        self.copied_cover = Some(path.clone());
        self.replaced_cover = previous.map(|value| (path.clone(), value));
    }

    pub(crate) fn backup_path(&self, name: &str) -> PathBuf {
        self.backup_dir.join(name)
    }

    pub(crate) fn commit(&mut self) {
        let _ = fs::remove_dir_all(&self.backup_dir);
    }

    pub(crate) fn rollback(&mut self) -> Option<String> {
        let mut errors = Vec::new();
        if self.folder_renamed {
            if let Some(current) = &self.target_folder {
                if current.exists() && !self.original_folder.exists() {
                    if let Err(error) = fs::rename(current, &self.original_folder) {
                        errors.push(format!("could not restore album folder: {error}"));
                    }
                }
            }
        }

        for (original, target) in self.renamed_files.iter().rev() {
            if target.exists() && !original.exists() {
                if let Err(error) = fs::rename(target, original) {
                    errors.push(format!("could not restore {}: {error}", original.display()));
                }
            }
        }

        for (original, backup) in &self.backups {
            if let Err(error) = fs::copy(backup, original) {
                errors.push(format!("could not restore {}: {error}", original.display()));
            }
        }

        if let Some((cover, previous)) = &self.replaced_cover {
            if let Err(error) = fs::copy(previous, cover) {
                errors.push(format!("could not restore cover: {error}"));
            }
        } else if let Some(cover) = &self.copied_cover {
            let _ = fs::remove_file(cover);
        }

        let _ = fs::remove_dir_all(&self.backup_dir);
        (!errors.is_empty()).then(|| errors.join("; "))
    }
}
