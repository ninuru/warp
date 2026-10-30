use std::collections::HashMap;

use settings::Setting as _;
use warpui::r#async::Timer;
use warpui::keymap::Keystroke;
use warpui::{AppContext, Entity, Event, ModelContext, SingletonEntity, WindowId};

use super::config::PrefixKeymap;
use super::machine::{KeyInput, PrefixMachine, PrefixOutcome};
use crate::terminal::keys_settings::KeysSettings;

pub enum PrefixKeysEvent {
    PendingChanged { window_id: WindowId },
}

pub struct PrefixKeysState {
    enabled: bool,
    keymap: PrefixKeymap,
    machines: HashMap<WindowId, PrefixMachine>,
}

impl PrefixKeysState {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        ctx.subscribe_to_model(&KeysSettings::handle(ctx), |state, _, _, ctx| {
            (state.enabled, state.keymap) = load_config(ctx);
        });
        let (enabled, keymap) = load_config(ctx);
        Self {
            enabled,
            keymap,
            machines: HashMap::new(),
        }
    }

    pub fn handle_event(
        &mut self,
        window_id: WindowId,
        event: &Event,
        ctx: &mut ModelContext<Self>,
    ) -> PrefixOutcome {
        if !self.enabled {
            return PrefixOutcome::PassThrough;
        }
        let Some(input) = key_input(event) else {
            return PrefixOutcome::PassThrough;
        };
        let machine = self.machines.entry(window_id).or_default();
        let was_pending = machine.is_pending();
        let outcome = machine.on_input(input, &self.keymap);
        let is_pending = machine.is_pending();
        if let PrefixOutcome::EnterPending { generation } = outcome {
            self.schedule_timeout(window_id, generation, ctx);
        }
        if was_pending != is_pending {
            ctx.emit(PrefixKeysEvent::PendingChanged { window_id });
        }
        outcome
    }

    pub fn is_pending(&self, window_id: WindowId) -> bool {
        self.machines
            .get(&window_id)
            .is_some_and(PrefixMachine::is_pending)
    }

    pub fn prefix_key(&self) -> &Keystroke {
        self.keymap.prefix()
    }

    fn schedule_timeout(
        &mut self,
        window_id: WindowId,
        generation: u64,
        ctx: &mut ModelContext<Self>,
    ) {
        ctx.spawn(Timer::after(self.keymap.timeout()), move |state, _, ctx| {
            let cancelled = state
                .machines
                .get_mut(&window_id)
                .is_some_and(|machine| machine.on_timeout(generation));
            if cancelled {
                ctx.emit(PrefixKeysEvent::PendingChanged { window_id });
            }
        });
    }
}

fn load_config(ctx: &AppContext) -> (bool, PrefixKeymap) {
    let config = KeysSettings::as_ref(ctx).prefix_keys.value();
    let (keymap, errors) = PrefixKeymap::parse(config);
    for error in errors {
        log::warn!("Ignoring invalid prefix key setting {error}");
    }
    (config.enabled, keymap)
}

fn key_input(event: &Event) -> Option<KeyInput<'_>> {
    match event {
        Event::KeyDown {
            keystroke,
            details,
            is_composing,
            ..
        } => Some(KeyInput::KeyDown {
            keystroke,
            is_repeat: details.is_repeat,
            is_composing: *is_composing,
        }),
        Event::ModifierKeyChanged { .. } => Some(KeyInput::Modifier),
        Event::SetMarkedText { marked_text, .. } => Some(KeyInput::MarkedText {
            composing: !marked_text.is_empty(),
        }),
        Event::ClearMarkedText => Some(KeyInput::MarkedText { composing: false }),
        Event::ScrollWheel { .. }
        | Event::LeftMouseDown { .. }
        | Event::LeftMouseUp { .. }
        | Event::LeftMouseDragged { .. }
        | Event::MiddleMouseDown { .. }
        | Event::RightMouseDown { .. }
        | Event::ForwardMouseDown { .. }
        | Event::BackMouseDown { .. }
        | Event::MouseMoved { .. }
        | Event::ModifierStateChanged { .. }
        | Event::TypedCharacters { .. }
        | Event::DragAndDropFiles { .. }
        | Event::DragFiles { .. }
        | Event::DragFileExit => None,
    }
}

impl Entity for PrefixKeysState {
    type Event = PrefixKeysEvent;
}

impl SingletonEntity for PrefixKeysState {}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
