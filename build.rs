//! Embeds the app icon into the Windows executables, so the file has a face in Explorer and on
//! the taskbar. Does nothing anywhere else: on Linux the window icon set at run time is all there
//! is to set.
//!
//! Also settles what the About window shows for third-party notices, which differs by profile.
//! A release binary is distributed, so it must carry notices that match what is linked into it;
//! any other build is not, so it carries a note saying where they come from and ignores whatever
//! happens to be lying in the tree.

use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Written by `scripts/gen-notices.sh`. Gitignored, so absent in a fresh checkout.
const NOTICES: &str = "THIRD-PARTY-NOTICES.md";

/// What the notices are made from. `Cargo.lock` fixes every crate version and crates.io versions
/// are immutable, so these three files determine the generated output completely. Keep this list,
/// and its order, identical to `scripts/gen-notices.sh`.
const INPUTS: [&str; 3] = ["Cargo.lock", "about.toml", "about.md.hbs"];

/// The comment `scripts/gen-notices.sh` appends, naming what the file was generated from and what
/// it holds.
///
/// Split on the opener rather than on the first field's name, so that the fields below can be read
/// by name — splitting on `inputs-sha256:` would consume the very token the check looks for.
const STAMP: &str = "\n<!-- ";

/// Shown by every build that is not a release. Says so plainly: a developer meeting this in the
/// About window should not go looking for a bug.
const PLACEHOLDER: &str = "\
Third-party notices are generated for release builds and are not present in this one.

Released binaries carry the full list, which also ships as THIRD-PARTY-NOTICES.md in the release
archive. To produce it here:

    bash scripts/gen-notices.sh
";

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        // A failure here would mean no icon, which is not worth failing a build over.
        let _ = res.compile();
    }

    notices();
}

/// Puts the notices where `include_str!` can reach them.
///
/// Writing into `OUT_DIR` rather than including the tree's copy directly is what lets that copy be
/// absent: `include_str!` is resolved at compile time and cannot fall back on its own.
fn notices() {
    println!("cargo:rerun-if-changed={NOTICES}");
    for input in INPUTS {
        println!("cargo:rerun-if-changed={input}");
    }

    // Release-like profiles report "release". Anything else is a build nobody receives.
    let released = env::var("PROFILE").as_deref() == Ok("release");
    let text = if released {
        release_notices()
    } else {
        PLACEHOLDER.to_owned()
    };

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set for build scripts"));
    fs::write(out.join("third-party-notices.md"), text)
        .expect("the notices must be written: the About window will not compile without them");
}

/// The generated notices, or a build failure explaining how to produce them.
///
/// Failing is the point. Embedding a placeholder, or a file left over from an older dependency
/// graph, would ship a binary whose stated licences are not the ones it was built from, and
/// nothing downstream would catch it.
///
/// Two things are checked, because the file has two ways to be wrong. The inputs hash catches
/// notices that have fallen behind `Cargo.lock`; the body hash catches the text having been edited,
/// which the template's own advice assumes cannot happen.
fn release_notices() -> String {
    let Ok(text) = fs::read_to_string(Path::new(NOTICES)) else {
        panic!("{}", complaint("they have not been generated"));
    };

    let Some((body, stamp)) = text.rsplit_once(STAMP) else {
        panic!("{}", complaint("they carry no inputs-sha256 stamp"));
    };
    let Some((stamp, _)) = stamp.split_once(" -->") else {
        panic!("{}", complaint("their stamp is not closed"));
    };
    // `inputs-sha256: <hex> body-sha256: <hex>`, read by name so a third field can be added
    // without every check below moving.
    let field = |name: &str| -> Option<&str> {
        let key = format!("{name}-sha256:");
        let fields: Vec<&str> = stamp.split_whitespace().collect();
        let at = fields.iter().position(|f| *f == key)?;
        fields.get(at + 1).copied()
    };

    match field("inputs") {
        Some(hash) if hash == input_hash() => {}
        Some(_) => panic!("{}", complaint("they are older than the dependency graph")),
        None => panic!("{}", complaint("they carry no inputs-sha256 stamp")),
    }
    match field("body") {
        Some(hash) if hash == sha256(body.as_bytes()) => {}
        Some(_) => panic!("{}", complaint("they have been edited since they were generated")),
        None => panic!("{}", complaint("they carry no body-sha256 stamp")),
    }
    text
}

/// Hashes the generating inputs, in the order `scripts/gen-notices.sh` concatenates them.
fn input_hash() -> String {
    let mut hasher = Sha256::new();
    for input in INPUTS {
        let bytes = fs::read(input).unwrap_or_else(|e| panic!("{input} must be readable: {e}"));
        hasher.update(&bytes);
    }
    hex(hasher.finalize())
}

fn sha256(bytes: &[u8]) -> String {
    hex(Sha256::digest(bytes))
}

fn hex(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn complaint(reason: &str) -> String {
    format!(
        "release builds must embed generated third-party notices, and {reason}.\n\
         Run:\n\
         \x20   bash scripts/gen-notices.sh\n\
         Then build again. Debug builds do not need this."
    )
}
