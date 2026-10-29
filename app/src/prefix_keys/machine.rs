use warpui::keymap::Keystroke;

use super::config::PrefixKeymap;

pub enum KeyInput<'a> {
    KeyDown {
        keystroke: &'a Keystroke,
        is_repeat: bool,
        is_composing: bool,
    },
    Modifier,
    MarkedText {
        composing: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrefixOutcome {
    PassThrough,
    Consume,
    EnterPending { generation: u64 },
    Dispatch { binding: String },
    DispatchRoot { binding: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PrefixState {
    #[default]
    Idle,
    Pending {
        generation: u64,
    },
}

#[derive(Debug, Default)]
pub struct PrefixMachine {
    state: PrefixState,
    next_generation: u64,
    ime_composing: bool,
    held_after_pending: Option<Keystroke>,
}

impl PrefixMachine {
    pub fn on_input(&mut self, input: KeyInput<'_>, keymap: &PrefixKeymap) -> PrefixOutcome {
        match (self.state, input) {
            (
                PrefixState::Idle | PrefixState::Pending { .. },
                KeyInput::MarkedText { composing },
            ) => {
                self.ime_composing = composing;
                PrefixOutcome::PassThrough
            }
            (PrefixState::Idle | PrefixState::Pending { .. }, KeyInput::Modifier) => {
                PrefixOutcome::PassThrough
            }
            (
                PrefixState::Idle,
                KeyInput::KeyDown {
                    keystroke,
                    is_repeat,
                    is_composing,
                },
            ) => self.on_idle_key(keystroke, is_repeat, is_composing, keymap),
            (
                PrefixState::Pending { .. },
                KeyInput::KeyDown {
                    keystroke,
                    is_repeat,
                    ..
                },
            ) => self.on_pending_key(keystroke, is_repeat, keymap),
        }
    }

    pub fn on_timeout(&mut self, generation: u64) -> bool {
        if self.state != (PrefixState::Pending { generation }) {
            return false;
        }
        self.state = PrefixState::Idle;
        true
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.state, PrefixState::Pending { .. })
    }

    fn on_idle_key(
        &mut self,
        keystroke: &Keystroke,
        is_repeat: bool,
        is_composing: bool,
        keymap: &PrefixKeymap,
    ) -> PrefixOutcome {
        if !is_repeat {
            self.held_after_pending = None;
        } else if self.held_after_pending.as_ref() == Some(keystroke) {
            return PrefixOutcome::Consume;
        }
        if is_composing || self.ime_composing {
            return PrefixOutcome::PassThrough;
        }
        if !is_repeat && keymap.is_prefix(keystroke) {
            self.next_generation += 1;
            let generation = self.next_generation;
            self.state = PrefixState::Pending { generation };
            self.held_after_pending = Some(keystroke.clone());
            return PrefixOutcome::EnterPending { generation };
        }
        match keymap.root_binding_for(keystroke) {
            Some(binding) => PrefixOutcome::DispatchRoot {
                binding: binding.to_owned(),
            },
            None => PrefixOutcome::PassThrough,
        }
    }

    fn on_pending_key(
        &mut self,
        keystroke: &Keystroke,
        is_repeat: bool,
        keymap: &PrefixKeymap,
    ) -> PrefixOutcome {
        if is_repeat {
            return PrefixOutcome::Consume;
        }
        self.state = PrefixState::Idle;
        let is_escape = keystroke.is_unmodified_key("escape");
        if !is_escape && keymap.is_prefix(keystroke) {
            self.held_after_pending = None;
            return PrefixOutcome::PassThrough;
        }
        self.held_after_pending = Some(keystroke.clone());
        if is_escape {
            return PrefixOutcome::Consume;
        }
        match keymap.binding_for(keystroke) {
            Some(binding) => PrefixOutcome::Dispatch {
                binding: binding.to_owned(),
            },
            None => PrefixOutcome::Consume,
        }
    }
}

#[cfg(test)]
#[path = "machine_tests.rs"]
mod tests;
