#[doc(hidden)]
pub fn __init_from_macro(manifest_dir: &str) {
    let Some(value) = std::env::var_os("PEEPS_DASHBOARD") else {
        return;
    };
    if value.to_string_lossy().trim().is_empty() {
        return;
    }

    DASHBOARD_DISABLED_WARNING_ONCE.call_once(|| {
        eprintln!(
            "\n\x1b[1;31m\
======================================================================\n\
 PEEPS WARNING: PEEPS_DASHBOARD is set, but peeps diagnostics is disabled.\n\
 This process will NOT connect to peeps-web in this build.\n\
 Enable the `diagnostics` cargo feature of `peeps` to use dashboard push.\n\
======================================================================\x1b[0m\n"
        );
    });
}

#[macro_export]
macro_rules! peep {
    ($fut:expr, $name:expr $(,)?) => {{
        $crate::instrument_future_named($name, $fut, $crate::Source::caller())
    }};
    ($fut:expr, $name:expr, {$($k:literal => $v:expr),* $(,)?} $(,)?) => {{
        let _ = ($((&$k, &$v)),*);
        $crate::instrument_future_named($name, $fut, $crate::Source::caller())
    }};
}
