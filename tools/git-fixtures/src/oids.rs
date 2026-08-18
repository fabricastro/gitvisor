//! Hardcoded OID constants the determinism test asserts against.
//!
//! Backfilled from a real `cargo test -p git-fixtures` run (task 2.6), not
//! guessed. Drift here means the fixture's ambient-state isolation broke.

/// `(alias, expected commit OID)`, in build order.
pub const COMMIT_OIDS: &[(&str, &str)] = &[
    ("c1", "05ac0b6171bf93e5417b5c6f642f6fd685859c67"),
    ("c2", "0a4ed91abcc1f495967477fd6192e477f68b21f5"),
    ("c3", "12664d83f2c45a3255caded7894595693f7b6ffe"),
    ("base", "32f830c4a5511a00c2a5e3cc41d61248f267f051"),
    ("m1", "3be87f5c3d2b374f232deb35960564a5cb7b6a8c"),
    ("fa1", "b9d0d727e23fab9cba3e0f25b6016ca488ce9854"),
    ("rp1", "bf0d539fdc079b4104272327790311b7553dd033"),
    ("m2", "4e5f7419023e6437a1a67f9fd7b1290dc5a82501"),
    ("fa2", "9f4f16ce824a33f5a595f714a07bb63761c6639f"),
    ("rp2", "8e857cbd2d7c249f7210f7aa84d992b437144b2d"),
    ("fb1", "d2ca0857b211b3d9e0e4bfb52d48ded371216978"),
    ("merge1", "931d4a54e1ee511f4a67cd6f4de17b33b4c8ba16"),
    ("m3", "dc4832ba3135144811e7e98ba9d13e704daef22a"),
    ("fa3", "6e103b425cda4acb36259501a7f19e9dd108e545"),
    ("fb2", "eb7506a8d164ac0d5917a846a3d0c2f4b68f334f"),
    ("m4", "a9214624572004adc825f586b788cb2bfa7d1f19"),
];

/// Backfilled from `cargo test -p git-fixtures` (task 2.6, 2026-08-18).
pub const HEAD_TREE: &str = "1cb4aa45b36ff8b9670d3758ebc77079aa1fcc95";

/// Backfilled from `cargo test -p git-fixtures` (task 2.6, 2026-08-18).
pub const TAG_V0_1_0: &str = "e1e7ad964046b61abf02a7635c5308b2e0c143d7";

pub fn commit_oid(alias: &str) -> &'static str {
    COMMIT_OIDS
        .iter()
        .find(|(a, _)| *a == alias)
        .map(|(_, oid)| *oid)
        .unwrap_or_else(|| panic!("no expected OID recorded for alias `{alias}`"))
}
