use crate::core::BranchPairs;
use crate::git::GitRepository;
use atomic_write_file::AtomicWriteFile;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MetadataStore {
    path: PathBuf,
}

impl MetadataStore {
    pub fn for_repository(repository: &GitRepository) -> Self {
        Self {
            path: repository.git_dir().join("zaphod").join("pairs.toml"),
        }
    }

    #[cfg(test)]
    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<BranchPairs, MetadataError> {
        if !self.path.exists() {
            return Ok(BranchPairs::default());
        }

        let contents = fs::read_to_string(&self.path).map_err(|source| MetadataError::Read {
            path: self.path.clone(),
            source,
        })?;

        toml::from_str(&contents).map_err(|source| MetadataError::Decode {
            path: self.path.clone(),
            source,
        })
    }

    pub fn save(&self, pairs: &BranchPairs) -> Result<(), MetadataError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| MetadataError::MissingParent {
                path: self.path.clone(),
            })?;
        fs::create_dir_all(parent).map_err(|source| MetadataError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;

        let contents =
            toml::to_string_pretty(pairs).map_err(|source| MetadataError::Encode { source })?;
        let mut file =
            AtomicWriteFile::open(&self.path).map_err(|source| MetadataError::Write {
                path: self.path.clone(),
                source,
            })?;
        file.write_all(contents.as_bytes())
            .map_err(|source| MetadataError::Write {
                path: self.path.clone(),
                source,
            })?;
        file.commit().map_err(|source| MetadataError::Write {
            path: self.path.clone(),
            source,
        })
    }
}

#[derive(Debug)]
pub enum MetadataError {
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    Decode {
        path: PathBuf,
        source: toml::de::Error,
    },
    Encode {
        source: toml::ser::Error,
    },
    MissingParent {
        path: PathBuf,
    },
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl Display for MetadataError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDirectory { path, .. } => {
                write!(
                    formatter,
                    "failed to create metadata directory {}",
                    path.display()
                )
            }
            Self::Decode { path, .. } => {
                write!(
                    formatter,
                    "failed to parse metadata file {}",
                    path.display()
                )
            }
            Self::Encode { .. } => write!(formatter, "failed to encode metadata"),
            Self::MissingParent { path } => {
                write!(
                    formatter,
                    "metadata path has no parent directory: {}",
                    path.display()
                )
            }
            Self::Read { path, .. } => {
                write!(formatter, "failed to read metadata file {}", path.display())
            }
            Self::Write { path, .. } => {
                write!(
                    formatter,
                    "failed to write metadata file {}",
                    path.display()
                )
            }
        }
    }
}

impl Error for MetadataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CreateDirectory { source, .. } => Some(source),
            Self::Decode { source, .. } => Some(source),
            Self::Encode { source } => Some(source),
            Self::Read { source, .. } => Some(source),
            Self::Write { source, .. } => Some(source),
            Self::MissingParent { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MetadataStore;
    use crate::core::{BranchPair, BranchPairs};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("zaphod-metadata-{}-{id}", std::process::id()));
            fs::create_dir(&path).expect("create test directory");

            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn missing_metadata_loads_empty_pairs() {
        let dir = TestDir::new();
        let store = MetadataStore::at_path(dir.path().join("zaphod").join("pairs.toml"));

        let pairs = store.load().expect("load pairs");

        assert!(pairs.is_empty());
    }

    #[test]
    fn saves_and_loads_pairs() {
        let dir = TestDir::new();
        let store = MetadataStore::at_path(dir.path().join("zaphod").join("pairs.toml"));
        let mut pairs = BranchPairs::default();
        pairs.upsert(
            BranchPair::new(
                "default".to_owned(),
                "feature/api".to_owned(),
                "feature/ui".to_owned(),
            )
            .expect("valid pair"),
        );

        store.save(&pairs).expect("save pairs");

        let loaded = store.load().expect("load pairs");
        assert_eq!(loaded, pairs);
    }

    #[test]
    fn save_replaces_existing_metadata() {
        let dir = TestDir::new();
        let store = MetadataStore::at_path(dir.path().join("zaphod").join("pairs.toml"));
        let mut first = BranchPairs::default();
        first.upsert(
            BranchPair::new(
                "default".to_owned(),
                "feature/api".to_owned(),
                "feature/ui".to_owned(),
            )
            .expect("valid pair"),
        );
        let mut second = BranchPairs::default();
        second.upsert(
            BranchPair::new(
                "default".to_owned(),
                "main".to_owned(),
                "feature/search".to_owned(),
            )
            .expect("valid pair"),
        );

        store.save(&first).expect("save first pairs");
        store.save(&second).expect("replace pairs");

        let loaded = store.load().expect("load pairs");
        assert_eq!(loaded, second);
    }
}
