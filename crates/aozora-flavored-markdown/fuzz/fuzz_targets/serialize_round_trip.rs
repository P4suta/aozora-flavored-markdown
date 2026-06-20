//! Fuzz target — `aozora_flavored_markdown::serialize` fixed point on arbitrary UTF-8.
//!
//! `serialize(serialize(src))` must be byte-identical to
//! `serialize(src)`: the lex pipeline canonicalises the source on
//! the first pass; oscillation on the second pass would mean the
//! classifier and serializer disagree on the canonical form.
//!
//! Run with: `just fuzz serialize_round_trip -- -runs=10000`

#![no_main]

use aozora_flavored_markdown::serialize;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = core::str::from_utf8(data) else {
        return;
    };
    let first = serialize(src);
    let second = serialize(&first);
    assert_eq!(
        first, second,
        "I3 fixed-point broken for src={src:?}: first vs second differ"
    );
});
