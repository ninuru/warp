use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;

use super::*;
use crate::elements::*;
use crate::keymap::macros::*;
use crate::keymap::{EditableBinding, FixedBinding, Keystroke};
use crate::{Event, Presenter};

struct View {
    keymap_context: keymap::Context,
    actions: Vec<String>,
}

impl View {
    fn new(context: &'static str) -> Self {
        let mut keymap_context = keymap::Context::default();
        keymap_context.set.insert(context);
        View {
            keymap_context,
            actions: Vec::new(),
        }
    }
}

impl Entity for View {
    type Event = ();
}

impl super::View for View {
    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }

    fn ui_name() -> &'static str {
        "View"
    }

    fn keymap_context(&self, _: &AppContext) -> keymap::Context {
        self.keymap_context.clone()
    }
}

#[derive(Debug)]
struct Action(String);

impl TypedActionView for View {
    type Action = Action;

    fn handle_action(&mut self, action: &Action, _: &mut ViewContext<Self>) {
        self.actions.push(action.0.clone());
    }
}

fn ctrl_a_event() -> Result<Event> {
    Ok(Event::KeyDown {
        keystroke: Keystroke::parse("ctrl-a")?,
        chars: "a".to_string(),
        details: Default::default(),
        is_composing: false,
    })
}

fn install_counting_interceptor(app: &mut App, consume: bool) -> Rc<RefCell<usize>> {
    let calls = Rc::new(RefCell::new(0));
    let interceptor_calls = calls.clone();
    app.update(|ctx| {
        ctx.set_key_interceptor(move |_, _, _| {
            *interceptor_calls.borrow_mut() += 1;
            consume
        });
    });
    calls
}

fn simulate(app: &mut App, event: Event, window_id: WindowId) -> bool {
    let presenter = Rc::new(RefCell::new(Presenter::new(window_id)));
    app.update(|ctx| ctx.simulate_window_event(event, window_id, presenter))
}

fn setup_ctrl_a_binding(app: &mut App) -> (WindowId, ViewHandle<View>) {
    let (window_id, view) = app.add_window(WindowStyle::NotStealFocus, |_| View::new("a"));
    app.update(|ctx| {
        ctx.register_fixed_bindings(vec![FixedBinding::new(
            "ctrl-a",
            Action("binding".into()),
            id!("a"),
        )]);
    });
    view.update(app, |_, ctx| ctx.focus_self());
    (window_id, view)
}

#[test]
fn test_key_interceptor_consuming_event_blocks_keystroke_binding() -> Result<()> {
    App::test((), |mut app| async move {
        let (window_id, view) = setup_ctrl_a_binding(&mut app);
        let calls = install_counting_interceptor(&mut app, true);

        let handled = simulate(&mut app, ctrl_a_event()?, window_id);

        assert!(handled);
        assert_eq!(*calls.borrow(), 1);
        assert!(view.read(&app, |view, _| view.actions.is_empty()));
        Ok(())
    })
}

#[test]
fn test_key_interceptor_passing_event_lets_keystroke_binding_fire() -> Result<()> {
    App::test((), |mut app| async move {
        let (window_id, view) = setup_ctrl_a_binding(&mut app);
        let calls = install_counting_interceptor(&mut app, false);

        let handled = simulate(&mut app, ctrl_a_event()?, window_id);

        assert!(handled);
        assert_eq!(*calls.borrow(), 1);
        assert_eq!(
            view.read(&app, |view, _| view.actions.clone()),
            vec!["binding".to_string()]
        );
        Ok(())
    })
}

#[test]
fn test_key_interceptor_not_called_when_key_bindings_disabled() -> Result<()> {
    App::test((), |mut app| async move {
        let (window_id, view) = setup_ctrl_a_binding(&mut app);
        let calls = install_counting_interceptor(&mut app, true);
        app.update(|ctx| ctx.disable_key_bindings(window_id));

        let handled = simulate(&mut app, ctrl_a_event()?, window_id);

        assert!(!handled);
        assert_eq!(*calls.borrow(), 0);
        assert!(view.read(&app, |view, _| view.actions.is_empty()));
        Ok(())
    })
}

#[test]
fn test_key_interceptor_ignores_non_key_events() -> Result<()> {
    App::test((), |mut app| async move {
        let (window_id, _) = setup_ctrl_a_binding(&mut app);
        let calls = install_counting_interceptor(&mut app, true);

        simulate(
            &mut app,
            Event::TypedCharacters {
                chars: "a".to_string(),
            },
            window_id,
        );

        assert_eq!(*calls.borrow(), 0);
        Ok(())
    })
}

#[test]
fn test_dispatch_binding_by_name_picks_binding_matching_focused_context() -> Result<()> {
    App::test((), |mut app| async move {
        let (window_id, view) = app.add_window(WindowStyle::NotStealFocus, |_| View::new("a"));
        app.update(|ctx| {
            ctx.register_editable_bindings(vec![
                EditableBinding::new("test:act", "Act elsewhere", Action("other".into()))
                    .with_context_predicate(id!("x")),
                EditableBinding::new("test:act", "Act here", Action("focused".into()))
                    .with_context_predicate(id!("a")),
                EditableBinding::new("test:unrelated", "Unrelated", Action("unrelated".into()))
                    .with_context_predicate(id!("a")),
            ]);
        });
        view.update(&mut app, |_, ctx| ctx.focus_self());

        let dispatched = app.update(|ctx| ctx.dispatch_binding_by_name(window_id, "test:act"))?;

        assert!(dispatched);
        assert_eq!(
            view.read(&app, |view, _| view.actions.clone()),
            vec!["focused".to_string()]
        );
        Ok(())
    })
}

#[test]
fn test_dispatch_binding_by_name_returns_false_for_unknown_name() -> Result<()> {
    App::test((), |mut app| async move {
        let (window_id, view) = app.add_window(WindowStyle::NotStealFocus, |_| View::new("a"));
        app.update(|ctx| {
            ctx.register_editable_bindings(vec![
                EditableBinding::new("test:act", "Act", Action("focused".into()))
                    .with_context_predicate(id!("a")),
            ]);
        });
        view.update(&mut app, |_, ctx| ctx.focus_self());

        let dispatched =
            app.update(|ctx| ctx.dispatch_binding_by_name(window_id, "test:missing"))?;

        assert!(!dispatched);
        assert!(view.read(&app, |view, _| view.actions.is_empty()));
        Ok(())
    })
}

#[test]
fn test_dispatch_binding_by_name_ignores_empty_name_of_fixed_bindings() -> Result<()> {
    App::test((), |mut app| async move {
        let (window_id, view) = app.add_window(WindowStyle::NotStealFocus, |_| View::new("a"));
        app.update(|ctx| {
            ctx.register_fixed_bindings(vec![FixedBinding::new(
                "ctrl-a",
                Action("fixed".into()),
                id!("a"),
            )]);
        });
        view.update(&mut app, |_, ctx| ctx.focus_self());

        let dispatched = app.update(|ctx| ctx.dispatch_binding_by_name(window_id, ""))?;

        assert!(!dispatched);
        assert!(view.read(&app, |view, _| view.actions.is_empty()));
        Ok(())
    })
}

#[test]
fn test_dispatch_binding_by_name_returns_false_for_disabled_binding() -> Result<()> {
    App::test((), |mut app| async move {
        let (window_id, view) = app.add_window(WindowStyle::NotStealFocus, |_| View::new("a"));
        app.update(|ctx| {
            ctx.register_editable_bindings(vec![
                EditableBinding::new("test:act", "Act", Action("focused".into()))
                    .with_context_predicate(id!("a"))
                    .with_enabled(|| false),
            ]);
        });
        view.update(&mut app, |_, ctx| ctx.focus_self());

        let dispatched = app.update(|ctx| ctx.dispatch_binding_by_name(window_id, "test:act"))?;

        assert!(!dispatched);
        assert!(view.read(&app, |view, _| view.actions.is_empty()));
        Ok(())
    })
}
