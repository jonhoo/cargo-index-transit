mod util;
use util::roundtrip;

#[test]
fn roundtrip_simplest() {
    roundtrip(|_| {}, |_, _, _| {});
}
