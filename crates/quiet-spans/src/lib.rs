use tracing::Span;
use tracing::Subscriber;
use tracing_subscriber::filter::dynamic_filter_fn;
use tracing_subscriber::filter::Directive;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

const TARGET: &str = "quiet_children";
const ALWAYS_QUIET: &str = "always_quiet_children";
const SOMETIMES_QUIET: &str = "sometimes_quiet_children";

/// Children of this span are always ignored.
pub fn always_quiet_children() -> Span {
    tracing::info_span!(target: TARGET, ALWAYS_QUIET)
}

/// Children of this span are ignored if `verbose_spans` is false in [`disable_noisy_spans`]
pub fn sometimes_quiet_children() -> Span {
    tracing::info_span!(target: TARGET, SOMETIMES_QUIET)
}

/// Layer which disables noisy spans - children of [`always_quiet_children`] and children of
/// [`sometimes_quiet_children`] (when `verbose_spans` is false).
pub fn disable_noisy_spans<S>(verbose_spans: bool) -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    dynamic_filter_fn(move |meta, ctx| {
        if meta.is_span() && !["tokio", "runtime"].contains(&meta.target()) {
            match ctx.lookup_current() {
                Some(parent) => {
                    !(parent.name() == ALWAYS_QUIET
                        || (!verbose_spans && parent.name() == SOMETIMES_QUIET))
                }
                _ => true,
            }
        } else {
            true
        }
    })
}

/// Directive to enable the target of the quiet children spans. These must always be enabled, or the
/// filtering will not work.
pub fn enable_target_directive() -> Directive {
    format!("{}=info", TARGET)
        .parse()
        .expect("to parse directive")
}
