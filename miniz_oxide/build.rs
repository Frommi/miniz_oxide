use autocfg;

fn main() {
    autocfg::new().emit_sysroot_crate("alloc");
}
