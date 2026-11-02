//! Import resolution on `tests/fixtures/imports`.
//!
//! The expected edges were written from the fixture's source, not from the tool's output. The fixture holds:
//! two classes called `User` in sibling modules, a second copy of the whole package, relative imports from a
//! sub-package, aliases, a re-export through `__init__`, a star import, `TYPE_CHECKING`, a base that is never
//! imported, and test code outside the package's source root.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chartedcode_core::{analyze, Options};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

type Edge = (String, String, String, String);

fn counted<'a>(
    rows: impl Iterator<Item = (&'a str, &'a str, &'a str, &'a str)>,
) -> BTreeMap<Edge, usize> {
    let mut counts = BTreeMap::new();
    for (kind, from, to, evidence) in rows {
        *counts
            .entry((kind.into(), from.into(), to.into(), evidence.into()))
            .or_default() += 1;
    }
    counts
}

#[test]
fn imports_resolve_to_the_expected_classes() {
    let analysis = analyze(
        &fixture("imports"),
        &Options {
            respect_gitignore: false,
            ..Options::default()
        },
    )
    .expect("fixture analyses");
    let got = counted(analysis.edges.iter().map(|e| {
        (
            e.kind.as_str(),
            e.from.as_str(),
            e.to.as_str(),
            e.evidence.as_str(),
        )
    }));
    let expected = counted(
        [
            (
                "composes",
                "app/services::Svc",
                "app/core::Engine",
                "import",
            ),
            (
                "composes",
                "app/services::Svc",
                "app/core::Engine",
                "re-export",
            ),
            (
                "composes",
                "app/services::Svc",
                "app/models::User",
                "import",
            ),
            ("composes", "app/services::Svc", "app/other::User", "import"),
            (
                "composes",
                "copy2/app/services::Svc",
                "copy2/app/core::Engine",
                "import",
            ),
            (
                "composes",
                "copy2/app/services::Svc",
                "copy2/app/core::Engine",
                "re-export",
            ),
            (
                "composes",
                "copy2/app/services::Svc",
                "copy2/app/models::User",
                "import",
            ),
            (
                "composes",
                "copy2/app/services::Svc",
                "copy2/app/other::User",
                "import",
            ),
            (
                "implements",
                "app/services::FileReader",
                "app/services::Reader",
                "method-names",
            ),
            (
                "implements",
                "copy2/app/services::FileReader",
                "copy2/app/services::Reader",
                "method-names",
            ),
            (
                "implements",
                "tests_dir/test_fakes::FakePort",
                "uniq_pkg/ports::Port",
                "method-names",
            ),
            (
                "implements",
                "tests_dir/test_fakes::FakeReader",
                "app/services::Reader",
                "method-names",
            ),
            (
                "implements",
                "tests_dir/test_fakes::Unrelated",
                "uniq_pkg/ports::Port",
                "method-names",
            ),
            (
                "inherits",
                "app/models::Admin",
                "app/models::User",
                "same-module",
            ),
            ("inherits", "app/models::User", "app/base::Base", "import"),
            (
                "inherits",
                "app/services::Child",
                "app/models::User",
                "import",
            ),
            ("inherits", "app/services::Local", "ext:Admin", "external"),
            (
                "inherits",
                "app/services::Reader",
                "ext:Protocol",
                "external",
            ),
            ("inherits", "app/services::Svc", "app/base::Base", "import"),
            (
                "inherits",
                "app/sub/deep::Deep",
                "app/models::Admin",
                "import",
            ),
            (
                "inherits",
                "app/sub/deep::Deeper",
                "app/base::Base",
                "import",
            ),
            (
                "inherits",
                "app/sub/deep::H",
                "app/sub/helper::Mixin",
                "import",
            ),
            ("inherits", "app/sub/stars::S", "app/base::Base", "import"),
            (
                "inherits",
                "copy2/app/models::Admin",
                "copy2/app/models::User",
                "same-module",
            ),
            (
                "inherits",
                "copy2/app/models::User",
                "copy2/app/base::Base",
                "import",
            ),
            (
                "inherits",
                "copy2/app/services::Child",
                "copy2/app/models::User",
                "import",
            ),
            (
                "inherits",
                "copy2/app/services::Local",
                "ext:Admin",
                "external",
            ),
            (
                "inherits",
                "copy2/app/services::Reader",
                "ext:Protocol",
                "external",
            ),
            (
                "inherits",
                "copy2/app/services::Svc",
                "copy2/app/base::Base",
                "import",
            ),
            (
                "inherits",
                "uniq_pkg/ports::Port",
                "ext:Protocol",
                "external",
            ),
        ]
        .into_iter(),
    );
    let missing: Vec<_> = expected
        .iter()
        .filter(|(k, n)| got.get(*k) != Some(n))
        .collect();
    let extra: Vec<_> = got
        .iter()
        .filter(|(k, n)| expected.get(*k) != Some(n))
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "expected but not got: {missing:#?}\ngot but not expected: {extra:#?}"
    );
    assert_eq!(expected.values().sum::<usize>(), 30);
}
