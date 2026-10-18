use crate::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
fn function(
    index: &mut Index,
    fw: &str,
    unit: &str,
    a: u64,
    text: &[&str],
    calls: &[(usize, u64)],
    names: &[&str],
) {
    let tokens = text
        .iter()
        .map(|s| {
            let n = index.tokens.len() as u32;
            *index.intern.entry(s.to_string()).or_insert_with(|| {
                index.tokens.push(s.to_string());
                n
            })
        })
        .collect();
    index.functions.push(Function {
        key: FunctionKey {
            firmware: fw.into(),
            unit: unit.into(),
            address: a,
        },
        architecture: Architecture::Arm32,
        names: names.iter().map(|s| s.to_string()).collect(),
        annotations: vec![],
        ranges: vec![(a, a + 4 * text.len() as u64)],
        tokens,
        signature: Sha256::digest(text.join("\n")).into(),
        instructions: text.len(),
        calls: calls.into(),
        references: vec![],
        constants: BTreeSet::new(),
        strings: BTreeSet::new(),
        ngrams: BTreeSet::new(),
    });
}
const CALLER: &[&str] = &["push R", "mov R", "bl T", "bl T", "pop pc"];
const LEAF: &[&str] = &["push R", "mov R #1", "str R", "pop pc"];
const CHANGED: &[&str] = &["push R", "mov R #2", "str R", "pop pc"];
#[test]
fn propagates_across_three_firmwares_and_keeps_grades() {
    let mut i = Index::new();
    function(
        &mut i,
        "a",
        "main",
        100,
        CALLER,
        &[(2, 200), (3, 300)],
        &["caller"],
    );
    function(&mut i, "a", "main", 200, LEAF, &[], &["leaf"]);
    function(&mut i, "a", "main", 300, LEAF, &[], &["other"]);
    function(
        &mut i,
        "b",
        "main",
        1000,
        CALLER,
        &[(2, 2000), (3, 3000)],
        &[],
    );
    function(&mut i, "b", "main", 2000, LEAF, &[], &[]);
    function(&mut i, "b", "main", 3000, CHANGED, &[], &["bridge"]);
    // The changed leaf is uniquely shared by B and C, and receives A's name through B.
    function(&mut i, "c", "main", 30000, CHANGED, &[], &[]);
    i.build();
    let pairs = i.pairs("a", "b");
    assert_eq!(pairs.len(), 3);
    assert_eq!(pairs[0].grade, Grade::Identical);
    assert_eq!(pairs[1].grade, Grade::Identical);
    assert_eq!(pairs[2].grade, Grade::CallSite);
    let c = i.find(None, Some(30000), Some("c"), None)[0].id;
    assert!(i.inherited_names(c, None, false).contains_key("bridge"));
    assert_eq!(i.members(i.membership[c], None).len(), 3);
    let baseline = i.subset(&BTreeSet::from(["a".into(), "c".into()]));
    assert_eq!(baseline.functions.len(), 4);
    assert!(baseline.pairs("a", "c").is_empty());
}
#[test]
fn overlapping_banks_and_unresolved_calls_stay_distinct() {
    let mut i = Index::new();
    function(&mut i, "a", "main", 100, CALLER, &[(2, 200), (3, 999)], &[]);
    function(&mut i, "a", "one", 200, LEAF, &[], &[]);
    function(&mut i, "a", "two", 200, CHANGED, &[], &[]);
    function(
        &mut i,
        "b",
        "main",
        1000,
        CALLER,
        &[(2, 2000), (3, 999)],
        &[],
    );
    function(&mut i, "b", "main", 2000, LEAF, &[], &[]);
    i.build();
    let caller = i.find(None, Some(100), Some("a"), None)[0].id;
    assert!(i.calls(caller).iter().all(|(_, v)| v.is_none()));
    assert_eq!(i.pairs("a", "b")[0].grade, Grade::SameCode);
    let one = i.find(None, Some(200), Some("a"), Some("one"))[0].id;
    assert_eq!(i.resolve(one, 200), Some(one));
}
#[test]
fn tiny_code_is_not_a_seed_and_weak_does_not_propagate() {
    let mut i = Index::new();
    function(&mut i, "a", "main", 1, &["bx lr"], &[], &[]);
    function(&mut i, "b", "main", 2, &["bx lr"], &[], &[]);
    i.build();
    assert!(i.pairs("a", "b").is_empty());
    let mut i = Index::new();
    function(&mut i, "a", "main", 100, CALLER, &[(2, 200)], &[]);
    function(&mut i, "b", "main", 1000, CALLER, &[(2, 2000)], &[]);
    function(&mut i, "a", "main", 200, LEAF, &[(1, 300)], &[]);
    let big: Vec<_> = (0..40).map(|_| "different").collect();
    function(&mut i, "b", "main", 2000, &big, &[(1, 3000)], &[]);
    function(&mut i, "a", "main", 300, &["one", "two"], &[], &[]);
    function(&mut i, "b", "main", 3000, &["one", "three"], &[], &[]);
    i.build();
    assert_eq!(i.pairs("a", "b").len(), 2);
    assert_eq!(i.pairs("a", "b")[1].grade, Grade::Weak);
}
fn input(fw: &str, arch: Architecture) -> UnitInput {
    UnitInput {
        firmware: fw.into(),
        unit: "main".into(),
        architecture: arch,
        address_ranges: vec![(0x40000, 0x10000000)],
        functions: vec![Boundary {
            address: 0x1000,
            ranges: vec![(0x1000, 0x1014)],
            names: vec!["synthetic".into()],
            annotations: vec![],
            references: vec![],
        }],
    }
}
#[test]
fn decoding_normalization_pools_and_locator_queries() {
    // Synthetic A32: mov r0,#1; ldr r1,[pc,#4]; bx lr; nop; literal.
    let words = [
        0xe3a00001u32,
        0xe59f1004,
        0xe12fff1e,
        0xe320f000,
        0x00800000,
    ];
    let bytes: Vec<_> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut i = Index::new();
    i.add_unit(
        input("a", Architecture::Arm32),
        vec![Region {
            address: 0x1000,
            bytes: bytes.clone(),
        }],
    )
    .unwrap();
    i.build();
    assert_eq!(i.tokens[i.functions[0].tokens[0] as usize], "mov R, #1");
    assert_eq!(
        i.tokens[i.functions[0].tokens[1] as usize],
        "ldr R, [pc, =A]"
    );
    assert_eq!(i.tokens[i.functions[0].tokens[4] as usize], "|");
    assert!(i.data_references(0).iter().any(|r| r.target == 0x800000));
    assert_eq!(i.references_to("a", Some("main"), 0x800000).len(), 1);
    assert_eq!(
        i.search("a", None, &bytes[0..4], None).unwrap()[0].address,
        0x1000
    );
    assert!(i.search("a", None, &[], None).is_err());
    assert_eq!(
        i.search("a", None, &[0, 0, 0, 0], Some(&[0, 0, 0, 0]))
            .unwrap()
            .len(),
        17
    );
    assert!(i.add_unit(input("a", Architecture::Arm32), vec![]).is_err());
    // Synthetic AArch64: mov w0,#1; add w0,w0,#2; nop; nop; ret.
    let words = [
        0x52800020u32,
        0x11000800,
        0xd503201f,
        0xd503201f,
        0xd65f03c0,
    ];
    let mut j = Index::new();
    j.add_unit(
        input("b", Architecture::Aarch64),
        vec![Region {
            address: 0x1000,
            bytes: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
        }],
    )
    .unwrap();
    j.build();
    assert_eq!(j.functions[0].instructions, 5);
    i.merge(j).unwrap();
    assert_eq!(i.functions.len(), 2);
}
#[test]
fn persisted_index_retains_annotations_and_rejects_bad_version() {
    let mut i = Index::new();
    function(&mut i, "a", "main", 100, LEAF, &[], &["leaf"]);
    i.build();
    i.annotate(
        0,
        Annotation {
            kind: "note".into(),
            text: "synthetic".into(),
            source: "test".into(),
        },
    )
    .unwrap();
    let path = {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/corpus-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("test-{}.index", std::process::id()))
    };
    i.save(&path).unwrap();
    let j = Index::open(&path).unwrap();
    assert_eq!(j.families[0].annotations[0].text, "synthetic");
    assert_eq!(j.find(Some("leaf"), None, None, None).len(), 1);
    std::fs::write(&path, b"INVALID!").unwrap();
    assert!(Index::open(&path).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn feature_seeds_bridge_architectures_but_do_not_claim_equal_code() {
    let mut i = Index::new();
    function(&mut i, "a", "main", 100, LEAF, &[], &["from_a"]);
    function(&mut i, "b", "main", 200, CHANGED, &[], &[]);
    i.functions[1].architecture = Architecture::Aarch64;
    for f in &mut i.functions {
        f.strings = BTreeSet::from([
            "synthetic unique alpha".into(),
            "synthetic unique beta".into(),
        ]);
        f.constants = BTreeSet::from([123456, 456789]);
    }
    i.build();
    assert_eq!(i.pairs("a", "b")[0].grade, Grade::CallSite);
    let b = i.find(Some("from_a"), None, Some("b"), None)[0].id;
    assert!(i.inherited_names(b, None, true).is_empty());
    assert!(!i.inherited_names(b, None, false).is_empty());
}

#[test]
fn merge_is_atomic_on_duplicate_unit_and_annotations_survive_rebuild() {
    let mut i = Index::new();
    let body: Vec<u8> = [
        0xe3a00001u32,
        0xe2800002,
        0xe2800003,
        0xe2800004,
        0xe12fff1e,
    ]
    .iter()
    .flat_map(|w| w.to_le_bytes())
    .collect();
    i.add_unit(
        input("a", Architecture::Arm32),
        vec![Region {
            address: 0x1000,
            bytes: body,
        }],
    )
    .unwrap();
    i.build();
    i.annotate(
        0,
        Annotation {
            kind: "type".into(),
            text: "synthetic signature".into(),
            source: "test".into(),
        },
    )
    .unwrap();
    assert!(i.merge(i.clone()).is_err());
    assert_eq!(i.functions.len(), 1);
    i.build();
    assert_eq!(i.families[0].annotations[0].text, "synthetic signature");
}
