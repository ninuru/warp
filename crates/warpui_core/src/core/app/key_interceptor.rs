use anyhow::Result;

use super::AppContext;
use crate::keymap::MatchResult;
use crate::{Event, WindowId};

pub type KeyInterceptor = dyn Fn(&Event, WindowId, &mut AppContext) -> bool;

impl AppContext {
    pub fn set_key_interceptor<F>(&mut self, interceptor: F)
    where
        F: 'static + Fn(&Event, WindowId, &mut AppContext) -> bool,
    {
        self.key_interceptor = Some(Box::new(interceptor));
    }

    pub(super) fn run_key_interceptor(&mut self, event: &Event, window_id: WindowId) -> bool {
        let is_key_event = match event {
            Event::KeyDown { .. }
            | Event::ModifierKeyChanged { .. }
            | Event::SetMarkedText { .. }
            | Event::ClearMarkedText => true,
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
            | Event::DragFileExit => false,
        };
        if !is_key_event {
            return false;
        }
        let Some(interceptor) = self.key_interceptor.take() else {
            return false;
        };
        let handled = interceptor(event, window_id, self);
        if self.key_interceptor.is_none() {
            self.key_interceptor = Some(interceptor);
        }
        handled
    }

    pub fn dispatch_binding_by_name(&mut self, window_id: WindowId, name: &str) -> Result<bool> {
        let responder_chain = self.get_responder_chain(window_id);
        let context_chain = self.contexts_from_responder_chain(window_id, &responder_chain)?;
        for (i, ctx) in context_chain.iter().enumerate().rev() {
            let handled = match self.keystroke_matcher.match_binding_name(name, ctx) {
                MatchResult::Action(action) => self.dispatch_typed_action(
                    window_id,
                    &responder_chain[0..=i],
                    action.as_ref(),
                    log::Level::Info,
                ),
                MatchResult::None | MatchResult::Pending => false,
            };
            if handled {
                self.dispatch_self_or_child_interacted_with(window_id, &responder_chain);
                return Ok(true);
            }
        }
        Ok(false)
    }
}
