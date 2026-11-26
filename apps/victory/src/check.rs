//! GTA VI installation checks: metadata and short-lived read handles only.
use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

const REQUIRED: &[&str] = &[
    "appdata0.rpf",
    "common.rpf",
    "movies.rpf",
    "script.rpf",
    "shaders.rpf",
    "data.rpf",
    "textures.rpf",
    "levels.rpf",
    "audio.rpf",
    "navmeshes.rpf",
    "packs.rpf",
    "anim.rpf",
    "pack/",
];
// Package metadata is useful to retain, but is not required to open the archives.
const RECOMMENDED: &[&str] = &[
    "gameos.xvd",
    "MicrosoftGame.Config",
    "appxmanifest.xml",
    "Catalog000.bin",
];

fn readable(path: &Path) -> io::Result<()> {
    if !fs::metadata(path)?.is_file() {
        return Err(io::Error::other("expected a regular file"));
    }
    let mut open = OpenOptions::new();
    open.read(true);
    // Match exclusive title opens; even a shared reader can prevent one.
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        open.share_mode(0);
    }
    open.open(path).map(drop)
}

fn status(error: &io::Error) -> &'static str {
    if error.kind() == io::ErrorKind::NotFound {
        "missing"
    } else if cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33)) {
        "locked"
    } else {
        "error"
    }
}

/// A snapshot, not an integrity check: unknown missing pack members cannot be detected.
/// In warning mode, keep launching and leave successful paths out of the log.
pub(crate) fn scan(root: &Path, warn: bool) -> i32 {
    let mut failures = [0usize; 2];
    for (index, (level, names)) in [("required", REQUIRED), ("recommended", RECOMMENDED)]
        .into_iter()
        .enumerate()
    {
        if !warn {
            println!("{level}:");
        }
        let mut note = |path: &Path, result: io::Result<()>| {
            let name = path.strip_prefix(root).unwrap_or(path).display();
            match result {
                Ok(()) if !warn => println!("  ok {name}"),
                Ok(()) => {}
                Err(e) => {
                    failures[index] += 1;
                    let status = status(&e);
                    if warn {
                        tracing::warn!(file = %path.display(), level, status, error = %e,
                            "archive preflight; run victory check for recovery advice");
                    } else {
                        println!("  {status} {name}: {e}");
                    }
                }
            }
        };
        for name in names {
            let path = root.join(name);
            if !name.ends_with('/') {
                note(&path, readable(&path));
                continue;
            }
            let directory = fs::metadata(&path).and_then(|m| {
                if m.is_dir() {
                    Ok(())
                } else {
                    Err(io::Error::other("expected a directory"))
                }
            });
            if directory.is_err() {
                note(&path, directory);
                continue;
            }
            let mut members = 0;
            for entry in walkdir::WalkDir::new(&path).min_depth(1) {
                match entry {
                    Ok(e) if e.file_type().is_dir() => {}
                    Ok(e) => {
                        members += 1;
                        note(e.path(), readable(e.path()));
                    }
                    Err(e) => {
                        members += 1;
                        let at = e.path().unwrap_or(&path).to_owned();
                        note(&at, Err(e.into()));
                    }
                }
            }
            if members == 0 {
                let empty = io::Error::new(io::ErrorKind::NotFound, "empty pack directory");
                note(&path, Err(empty));
            } else {
                note(&path, Ok(()));
            }
        }
    }
    if !warn {
        let advice = if failures.iter().any(|&n| n > 0) {
            "Restore missing files; close extractors or other instances for locked files; check permissions for errors."
        } else {
            "Try victory run <exe> --execute; contents and pack completeness are not validated."
        };
        println!(
            "Summary: {} required issues, {} recommended issues. {advice}",
            failures[0], failures[1]
        );
    }
    i32::from(failures[0] > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installation(root: &Path) {
        for name in REQUIRED.iter().filter(|n| !n.ends_with('/')) {
            fs::write(root.join(name), []).unwrap();
        }
        fs::create_dir_all(root.join("pack/nested")).unwrap();
        fs::write(root.join("pack/nested/member.rpf"), []).unwrap();
    }

    #[test]
    fn required_recommended_and_directory_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(scan(dir.path(), false), 1);
        installation(dir.path());
        assert_eq!(scan(dir.path(), false), 0); // Recommended omissions are non-fatal.
        let movies = dir.path().join("movies.rpf");
        fs::rename(&movies, dir.path().join("movies.saved")).unwrap();
        assert_eq!(scan(dir.path(), false), 1);
        fs::rename(dir.path().join("movies.saved"), &movies).unwrap();
        fs::remove_file(dir.path().join("pack/nested/member.rpf")).unwrap();
        assert_eq!(scan(dir.path(), false), 1); // An empty pack is not a passing check.
    }

    #[cfg(windows)]
    #[test]
    fn nested_pack_shared_reader_blocks_exclusive_open() {        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        installation(dir.path());
        let path = dir.path().join("pack/nested/member.rpf");
        let mut options = OpenOptions::new();
        options.read(true).share_mode(1);
        let held = options.open(&path).unwrap();
        assert!(fs::File::open(&path).is_ok());
        assert_eq!(status(&readable(&path).unwrap_err()), "locked");
        assert_eq!(scan(dir.path(), true), 1);
        drop(held);
        assert_eq!(scan(dir.path(), false), 0);
        assert!(options.share_mode(0).open(&path).is_ok());
    }

    #[test]
    fn a_directory_where_a_file_belongs_fails() {
        let dir = tempfile::tempdir().unwrap();
        installation(dir.path());
        let movies = dir.path().join("movies.rpf");
        fs::remove_file(&movies).unwrap();
        fs::create_dir(&movies).unwrap();
        // Opening a directory for reading succeeds on some platforms, so the
        // check has to reject it on the kind, not only on the open result.
        assert_eq!(scan(dir.path(), false), 1);
    }

    #[test]
    fn the_warning_does_not_change_the_verdict() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(scan(dir.path(), false), 1);
        assert_eq!(scan(dir.path(), true), 1);
        installation(dir.path());
        assert_eq!(scan(dir.path(), true), 0);
        assert_eq!(scan(dir.path(), false), 0);
    }
}
