mod config;
mod machine;
mod model;

use warp_errors::report_error;
use warpui::{AppContext, Event, SingletonEntity, WindowId};

pub use config::PrefixKeysConfig;
use machine::PrefixOutcome;
pub use model::{PrefixKeysEvent, PrefixKeysState};

pub fn init(ctx: &mut AppContext) {
    ctx.add_singleton_model(PrefixKeysState::new);
    ctx.set_key_interceptor(intercept);
}

fn intercept(event: &Event, window_id: WindowId, ctx: &mut AppContext) -> bool {
    let outcome = PrefixKeysState::handle(ctx)
        .update(ctx, |state, ctx| state.handle_event(window_id, event, ctx));
    match outcome {
        PrefixOutcome::PassThrough => false,
        PrefixOutcome::Consume | PrefixOutcome::EnterPending { .. } => true,
        PrefixOutcome::Dispatch { binding } => {
            dispatch(window_id, &binding, ctx);
            true
        }
        PrefixOutcome::DispatchRoot { binding } => dispatch(window_id, &binding, ctx),
    }
}

fn dispatch(window_id: WindowId, binding: &str, ctx: &mut AppContext) -> bool {
    match ctx.dispatch_binding_by_name(window_id, binding) {
        Ok(true) => true,
        Ok(false) => {
            if ctx.get_binding_by_name(binding).is_none() {
                log::warn!("Unknown keybinding name {binding} in keys.prefix");
            } else {
                log::debug!("Prefix binding {binding} did not apply to the focused view");
            }
            false
        }
        Err(error) => {
            report_error!(error.context(format!("Failed to dispatch prefix binding {binding}")));
            false
        }
    }
}
