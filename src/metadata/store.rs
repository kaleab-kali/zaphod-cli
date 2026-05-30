use crate::core::{AgentClaims, BranchPairs, METADATA_SCHEMA_VERSION};
use crate::git::GitRepository;
use atomic_write_file::AtomicWriteFile;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{ErrorKind, Write};
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

    pub(crate) fn lock(&self) -> Result<MetadataLock, MetadataError> {
        MetadataLock::acquire_for_metadata_path(&self.path)
    }

    pub fn load(&self) -> Result<BranchPairs, MetadataError> {
        if !self.path.exists() {
            return Ok(BranchPairs::default());
        }

        let contents = fs::read_to_string(&self.path).map_err(|source| MetadataError::Read {
            path: self.path.clone(),
            source,
        })?;

        let pairs: BranchPairs =
            toml::from_str(&contents).map_err(|source| MetadataError::Decode {
                path: self.path.clone(),
                source,
            })?;
        validate_schema_version(self.path(), pairs.schema_version())?;

        Ok(pairs)
    }

    pub fn save(&self, pairs: &BranchPairs) -> Result<(), MetadataError> {
        validate_schema_version(self.path(), pairs.schema_version())?;
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

#[derive(Debug, Clone)]
pub struct ClaimStore {
    path: PathBuf,
}

impl ClaimStore {
    pub fn for_repository(repository: &GitRepository) -> Self {
        Self {
            path: repository.git_dir().join("zaphod").join("claims.toml"),
        }
    }

    #[cfg(test)]
    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn lock(&self) -> Result<MetadataLock, MetadataError> {
        MetadataLock::acquire_for_metadata_path(&self.path)
    }

    pub fn load(&self) -> Result<AgentClaims, MetadataError> {
        if !self.path.exists() {
            return Ok(AgentClaims::default());
        }

        let contents = fs::read_to_string(&self.path).map_err(|source| MetadataError::Read {
            path: self.path.clone(),
            source,
        })?;

        let claims: AgentClaims =
            toml::from_str(&contents).map_err(|source| MetadataError::Decode {
                path: self.path.clone(),
                source,
            })?;
        validate_schema_version(self.path(), claims.schema_version())?;

        Ok(claims)
    }

    pub fn save(&self, claims: &AgentClaims) -> Result<(), MetadataError> {
        validate_schema_version(self.path(), claims.schema_version())?;
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
            toml::to_string_pretty(claims).map_err(|source| MetadataError::Encode { source })?;
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
#[must_use = "metadata locks are released when dropped"]
pub(crate) struct MetadataLock {
    path: PathBuf,
}

impl MetadataLock {
    fn acquire_for_metadata_path(metadata_path: &Path) -> Result<Self, MetadataError> {
        let metadata_dir = metadata_path
            .parent()
            .ok_or_else(|| MetadataError::MissingParent {
                path: metadata_path.to_path_buf(),
            })?;
        fs::create_dir_all(metadata_dir).map_err(|source| MetadataError::CreateDirectory {
            path: metadata_dir.to_path_buf(),
            source,
        })?;

        let lock_path = metadata_dir.join("metadata.lock");
        match fs::create_dir(&lock_path) {
            Ok(()) => Ok(Self { path: lock_path }),
            Err(source) if source.kind() == ErrorKind::AlreadyExists => {
                Err(MetadataError::Locked { path: lock_path })
            }
            Err(source) => Err(MetadataError::AcquireLock {
                path: lock_path,
                source,
            }),
        }
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for MetadataLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

#[derive(Debug)]
pub enum MetadataError {
    AcquireLock {
        path: PathBuf,
        source: std::io::Error,
    },
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
    Locked {
        path: PathBuf,
    },
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    UnsupportedSchemaVersion {
        path: PathBuf,
        version: u32,
        supported: u32,
    },
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl Display for MetadataError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AcquireLock { path, .. } => {
                write!(
                    formatter,
                    "failed to acquire metadata lock {}",
                    path.display()
                )
            }
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
            Self::Locked { path } => write!(
                formatter,
                "metadata is locked by another Zaphod process ({}); retry after it finishes",
                path.display()
            ),
            Self::Read { path, .. } => {
                write!(formatter, "failed to read metadata file {}", path.display())
            }
            Self::UnsupportedSchemaVersion {
                path,
                version,
                supported,
            } => write!(
                formatter,
                "unsupported metadata schema version {version} in {}; supported version is {supported}",
                path.display()
            ),
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
            Self::AcquireLock { source, .. } => Some(source),
            Self::CreateDirectory { source, .. } => Some(source),
            Self::Decode { source, .. } => Some(source),
            Self::Encode { source } => Some(source),
            Self::Read { source, .. } => Some(source),
            Self::Write { source, .. } => Some(source),
            Self::Locked { .. }
            | Self::MissingParent { .. }
            | Self::UnsupportedSchemaVersion { .. } => None,
        }
    }
}

fn validate_schema_version(path: &Path, version: u32) -> Result<(), MetadataError> {
    if version == METADATA_SCHEMA_VERSION {
        return Ok(());
    }

    Err(MetadataError::UnsupportedSchemaVersion {
        path: path.to_path_buf(),
        version,
        supported: METADATA_SCHEMA_VERSION,
    })
}

#[cfg(test)]
mod tests {
    use super::{ClaimStore, MetadataError, MetadataStore};
    use crate::core::{AgentClaim, AgentClaims, BranchPair, BranchPairs, METADATA_SCHEMA_VERSION};
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
        assert_eq!(pairs.schema_version(), METADATA_SCHEMA_VERSION);
    }

    #[test]
    fn metadata_lock_refuses_existing_lock() {
        let dir = TestDir::new();
        let store = MetadataStore::at_path(dir.path().join("zaphod").join("pairs.toml"));
        let lock = store.lock().expect("acquire metadata lock");

        let error = store.lock().expect_err("reject second metadata lock");

        assert!(matches!(error, MetadataError::Locked { .. }));
        assert!(lock.path().ends_with("metadata.lock"));
    }

    #[test]
    fn metadata_lock_is_released_on_drop() {
        let dir = TestDir::new();
        let store = MetadataStore::at_path(dir.path().join("zaphod").join("pairs.toml"));
        let first_lock_path = {
            let lock = store.lock().expect("acquire metadata lock");
            let path = lock.path().to_path_buf();
            assert!(path.exists());
            path
        };

        assert!(!first_lock_path.exists());
        let second_lock = store.lock().expect("reacquire metadata lock");
        assert!(second_lock.path().exists());
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
        let contents = fs::read_to_string(store.path()).expect("read pairs metadata");
        assert!(contents.contains("schema_version = 1"));
    }

    #[test]
    fn loads_legacy_pairs_without_schema_version() {
        let dir = TestDir::new();
        let store = MetadataStore::at_path(dir.path().join("zaphod").join("pairs.toml"));
        fs::create_dir_all(store.path().parent().expect("metadata parent"))
            .expect("create metadata directory");
        fs::write(
            store.path(),
            r#"[[pairs]]
name = "default"
left = "feature/api"
right = "feature/ui"
"#,
        )
        .expect("write legacy metadata");

        let loaded = store.load().expect("load legacy pairs");

        assert_eq!(loaded.schema_version(), METADATA_SCHEMA_VERSION);
        assert_eq!(loaded.pairs().len(), 1);
        assert_eq!(loaded.pairs()[0].name, "default");
    }

    #[test]
    fn rejects_unsupported_pairs_schema_version() {
        let dir = TestDir::new();
        let store = MetadataStore::at_path(dir.path().join("zaphod").join("pairs.toml"));
        fs::create_dir_all(store.path().parent().expect("metadata parent"))
            .expect("create metadata directory");
        fs::write(
            store.path(),
            r#"schema_version = 2

[[pairs]]
name = "default"
left = "feature/api"
right = "feature/ui"
"#,
        )
        .expect("write future metadata");

        let error = store.load().expect_err("reject unsupported schema");

        assert!(matches!(
            error,
            MetadataError::UnsupportedSchemaVersion {
                version: 2,
                supported: METADATA_SCHEMA_VERSION,
                ..
            }
        ));
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

    #[test]
    fn missing_claims_metadata_loads_empty_claims() {
        let dir = TestDir::new();
        let store = ClaimStore::at_path(dir.path().join("zaphod").join("claims.toml"));

        let claims = store.load().expect("load claims");

        assert!(claims.is_empty());
        assert_eq!(claims.schema_version(), METADATA_SCHEMA_VERSION);
    }

    #[test]
    fn saves_and_loads_claims() {
        let dir = TestDir::new();
        let store = ClaimStore::at_path(dir.path().join("zaphod").join("claims.toml"));
        let mut claims = AgentClaims::default();
        claims.upsert(
            AgentClaim::new(
                "codex".to_owned(),
                "default".to_owned(),
                "feature/api".to_owned(),
                42,
            )
            .expect("valid claim"),
        );

        store.save(&claims).expect("save claims");

        let loaded = store.load().expect("load claims");
        assert_eq!(loaded, claims);
        let contents = fs::read_to_string(store.path()).expect("read claims metadata");
        assert!(contents.contains("schema_version = 1"));
    }

    #[test]
    fn loads_legacy_claims_without_schema_version() {
        let dir = TestDir::new();
        let store = ClaimStore::at_path(dir.path().join("zaphod").join("claims.toml"));
        fs::create_dir_all(store.path().parent().expect("metadata parent"))
            .expect("create metadata directory");
        fs::write(
            store.path(),
            r#"[[claims]]
agent = "codex"
pair = "default"
branch = "feature/api"
created_at_unix = 42
"#,
        )
        .expect("write legacy claims");

        let loaded = store.load().expect("load legacy claims");

        assert_eq!(loaded.schema_version(), METADATA_SCHEMA_VERSION);
        assert_eq!(loaded.claims().len(), 1);
        assert_eq!(loaded.claims()[0].agent, "codex");
    }

    #[test]
    fn rejects_unsupported_claims_schema_version() {
        let dir = TestDir::new();
        let store = ClaimStore::at_path(dir.path().join("zaphod").join("claims.toml"));
        fs::create_dir_all(store.path().parent().expect("metadata parent"))
            .expect("create metadata directory");
        fs::write(
            store.path(),
            r#"schema_version = 2

[[claims]]
agent = "codex"
pair = "default"
branch = "feature/api"
created_at_unix = 42
"#,
        )
        .expect("write future claims");

        let error = store.load().expect_err("reject unsupported schema");

        assert!(matches!(
            error,
            MetadataError::UnsupportedSchemaVersion {
                version: 2,
                supported: METADATA_SCHEMA_VERSION,
                ..
            }
        ));
    }
}
