//! Call resolution, call order and Protocol dispatch on `tests/fixtures/calls`.
//!
//! The expected edges and step numbers were worked out from the fixture's source, in evaluation order
//! (a call's arguments before the call itself). The fixture also holds recursion, private names,
//! callbacks passed as arguments, and calls the tool must leave untraced.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chartedcode_core::{analyze, Analysis, Options};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn analysis() -> Analysis {
    analyze(
        &fixture("calls"),
        &Options {
            respect_gitignore: false,
            ..Options::default()
        },
    )
    .expect("fixture analyses")
}

type Key = (String, String, String, String);
/// One expected edge: kind, from, to, evidence, call sites, and their step numbers in the caller.
type Row<'a> = (&'a str, &'a str, &'a str, &'a str, usize, &'a [usize]);

#[test]
fn calls_are_resolved_with_evidence_and_order() {
    let a = analysis();
    let got: BTreeMap<Key, (usize, Vec<usize>)> = a
        .calls
        .iter()
        .map(|c| {
            (
                (
                    c.kind.clone(),
                    c.from.clone(),
                    c.to.clone(),
                    c.evidence.clone(),
                ),
                (c.count, c.steps.clone()),
            )
        })
        .collect();
    let rows: [Row; 39] = [
        (
            "calls",
            "pkg/_internal::internal_fn",
            "pkg/util::helper",
            "import",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/app::<module>",
            "pkg/app::main",
            "same-module",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/app::Service.run",
            "ext:json.dumps",
            "external",
            1,
            &[9],
        ),
        (
            "calls",
            "pkg/app::Service.run",
            "pkg/app::Port.send",
            "field-type",
            1,
            &[3],
        ),
        (
            "calls",
            "pkg/app::Service.run",
            "pkg/util::Sub.go",
            "local-type",
            1,
            &[7],
        ),
        (
            "calls",
            "pkg/app::Service.run",
            "pkg/util::Tool.make",
            "import",
            1,
            &[8],
        ),
        (
            "calls",
            "pkg/app::Service.run",
            "pkg/util::Tool.work",
            "field-type",
            1,
            &[4],
        ),
        (
            "calls",
            "pkg/app::Service.run",
            "pkg/util::Tool.work",
            "param-type",
            1,
            &[5],
        ),
        (
            "calls",
            "pkg/app::Service.run",
            "pkg/util::helper",
            "re-export",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/app::Service.run",
            "pkg/util::other",
            "import",
            1,
            &[2],
        ),
        (
            "calls",
            "pkg/priv::Public._hidden_method",
            "pkg/util::helper",
            "import",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/priv::Public.visible",
            "pkg/priv::Public._hidden_method",
            "self",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/priv::Public.visible",
            "pkg/util::helper",
            "import",
            1,
            &[3],
        ),
        (
            "calls",
            "pkg/priv::_PrivateClass.run",
            "pkg/util::helper",
            "import",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/priv::_private_fn",
            "pkg/util::helper",
            "import",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/priv::public_fn",
            "pkg/_internal::internal_fn",
            "import",
            1,
            &[2],
        ),
        (
            "calls",
            "pkg/priv::public_fn",
            "pkg/priv::_private_fn",
            "same-module",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/priv::public_fn",
            "pkg/util::helper",
            "import",
            1,
            &[3],
        ),
        (
            "calls",
            "pkg/rec::fact",
            "pkg/rec::fact",
            "same-module",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/rec::is_even",
            "pkg/rec::is_odd",
            "same-module",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/rec::is_odd",
            "pkg/rec::is_even",
            "same-module",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/rec::top",
            "pkg/rec::fact",
            "same-module",
            1,
            &[2],
        ),
        (
            "calls",
            "pkg/rec::top",
            "pkg/rec::is_even",
            "same-module",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/rec::top",
            "pkg/rec::is_odd",
            "same-module",
            1,
            &[3],
        ),
        (
            "calls",
            "pkg/util::Sub.go",
            "pkg/util::Tool.run",
            "inherited",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/util::Sub.work",
            "pkg/util::Tool.work",
            "super",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/util::Tool.run",
            "pkg/util::Tool.work",
            "self",
            1,
            &[1],
        ),
        (
            "calls",
            "pkg/util::Tool.work",
            "pkg/util::helper",
            "same-module",
            1,
            &[1],
        ),
        (
            "constructs",
            "pkg/app::Service.__init__",
            "pkg/util::Tool.__init__",
            "import",
            1,
            &[1],
        ),
        (
            "constructs",
            "pkg/app::Service.run",
            "pkg/util::Tool.__init__",
            "import",
            1,
            &[6],
        ),
        (
            "constructs",
            "pkg/app::main",
            "pkg/app::Impl.__init__",
            "same-module",
            1,
            &[1],
        ),
        (
            "constructs",
            "pkg/app::main",
            "pkg/app::Service.__init__",
            "same-module",
            1,
            &[3],
        ),
        (
            "constructs",
            "pkg/app::main",
            "pkg/util::Tool.__init__",
            "import",
            2,
            &[2, 4],
        ),
        (
            "constructs",
            "pkg/priv::Public.visible",
            "pkg/priv::_PrivateClass.__init__",
            "same-module",
            1,
            &[2],
        ),
        (
            "constructs",
            "pkg/util::Tool.make",
            "pkg/util::Tool.__init__",
            "same-module",
            1,
            &[1],
        ),
        (
            "dispatches",
            "pkg/app::Port.send",
            "pkg/app::Impl.send",
            "method-names",
            1,
            &[],
        ),
        (
            "dispatches",
            "pkg/app::Port.send",
            "pkg/app::Other.send",
            "method-names",
            1,
            &[],
        ),
        (
            "passes",
            "pkg/app::Service.run",
            "pkg/app::Port.send",
            "field-type",
            1,
            &[11],
        ),
        (
            "passes",
            "pkg/app::Service.run",
            "pkg/util::helper",
            "re-export",
            1,
            &[10],
        ),
    ];
    let expected: BTreeMap<Key, (usize, Vec<usize>)> = rows
        .into_iter()
        .map(|(kind, from, to, evidence, count, steps)| {
            (
                (kind.into(), from.into(), to.into(), evidence.into()),
                (count, steps.to_vec()),
            )
        })
        .collect();
    let missing: Vec<_> = expected
        .iter()
        .filter(|(k, v)| got.get(*k) != Some(v))
        .collect();
    let extra: Vec<_> = got
        .iter()
        .filter(|(k, v)| expected.get(*k) != Some(v))
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "expected but not got: {missing:#?}\ngot but not expected: {extra:#?}"
    );
}

#[test]
fn untraced_calls_are_counted_per_function() {
    let a = analysis();
    let counts: BTreeMap<&str, usize> = a
        .functions
        .iter()
        .filter(|f| f.unresolved > 0)
        .map(|f| (f.id.as_str(), f.unresolved))
        .collect();
    assert_eq!(counts, BTreeMap::from([("pkg/app::Service.run", 3)]));
    let service_run = a
        .functions
        .iter()
        .find(|f| f.id == "pkg/app::Service.run")
        .expect("function");
    assert_eq!(
        service_run.unresolved_examples,
        ["items.append", "undefined_var.foo", "self.callback"]
    );
}

#[test]
fn classes_without_an_init_get_an_implicit_constructor() {
    let a = analysis();
    let synthetic: Vec<&str> = a
        .functions
        .iter()
        .filter(|f| f.synthetic)
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(
        synthetic,
        ["pkg/app::Impl.__init__", "pkg/priv::_PrivateClass.__init__"]
    );
}
