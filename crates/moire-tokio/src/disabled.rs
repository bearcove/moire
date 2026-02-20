use std::sync::Once;

static DASHBOARD_DISABLED_WARNING_ONCE: Once = Once::new();

#[used]
#[cfg_attr(target_os = "macos", link_section = "__DATA,__mod_init_func")]
#[cfg_attr(
    any(target_os = "linux", target_os = "android", target_os = "freebsd"),
    link_section = ".init_array"
)]
static INIT_DISABLED_RUNTIME: extern "C" fn() = {
    extern "C" fn init() {
        emit_disabled_dashboard_warning_once();
    }
    init
};

fn emit_disabled_dashboard_warning_once() {
    // r[impl config.dashboard-feature-gate]
    let Some(value) = std::env::var_os("MOIRE_DASHBOARD") else {
        return;
    };
    if value.to_string_lossy().trim().is_empty() {
        return;
    }

    DASHBOARD_DISABLED_WARNING_ONCE.call_once(|| {
        eprintln!(
            "\n\x1b[1;31m\
======================================================================\n\
 MOIRE WARNING: MOIRE_DASHBOARD is set, but moire diagnostics is disabled.\n\
 This process will NOT connect to moire-web in this build.\n\
 Enable the `diagnostics` cargo feature of `moire` to use dashboard push.\n\
======================================================================\x1b[0m\n"
        );
    });
}
