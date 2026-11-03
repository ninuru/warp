use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;
use winit::event_loop::EventLoopProxy;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

use crate::windowing::winit::app::CustomEvent;

#[derive(Clone)]
pub(in crate::windowing::winit) struct X11FullscreenTracker {
    inner: Arc<TrackerInner>,
}

struct TrackerInner {
    conn: RustConnection,
    net_wm_state: u32,
    net_wm_state_fullscreen: u32,
    fullscreen_windows: Mutex<HashSet<Window>>,
}

impl X11FullscreenTracker {
    pub(in crate::windowing::winit) fn new(
        event_loop_proxy: EventLoopProxy<CustomEvent>,
    ) -> anyhow::Result<Self> {
        let (conn, _) = RustConnection::connect(None)?;
        let net_wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let net_wm_state_fullscreen = conn
            .intern_atom(false, b"_NET_WM_STATE_FULLSCREEN")?
            .reply()?
            .atom;
        let inner = Arc::new(TrackerInner {
            conn,
            net_wm_state,
            net_wm_state_fullscreen,
            fullscreen_windows: Default::default(),
        });
        let thread_inner = inner.clone();
        std::thread::Builder::new()
            .name("x11-fullscreen-tracker".to_string())
            .spawn(move || thread_inner.listen(event_loop_proxy))?;
        Ok(Self { inner })
    }

    pub(in crate::windowing::winit) fn track(&self, window: Window) -> anyhow::Result<()> {
        self.inner
            .conn
            .change_window_attributes(
                window,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            )?
            .check()?;
        self.inner.refresh(window)?;
        Ok(())
    }

    pub(in crate::windowing::winit) fn is_fullscreen(&self, window: Window) -> bool {
        self.inner.fullscreen_windows.lock().contains(&window)
    }
}

impl TrackerInner {
    fn listen(&self, event_loop_proxy: EventLoopProxy<CustomEvent>) {
        loop {
            match self.conn.wait_for_event() {
                Ok(Event::PropertyNotify(event)) if event.atom == self.net_wm_state => {
                    match self.refresh(event.window) {
                        Ok(false) => {}
                        Ok(true) => {
                            if event_loop_proxy
                                .send_event(CustomEvent::WindowFullscreenStateChanged)
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(err) => log::warn!("Failed to read X11 fullscreen state: {err:#}"),
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    log::warn!("Stopped tracking X11 fullscreen state: {err:#}");
                    return;
                }
            }
        }
    }

    fn refresh(&self, window: Window) -> anyhow::Result<bool> {
        let is_fullscreen = self
            .conn
            .get_property(false, window, self.net_wm_state, AtomEnum::ATOM, 0, 1024)?
            .reply()?
            .value32()
            .is_some_and(|mut atoms| atoms.any(|atom| atom == self.net_wm_state_fullscreen));
        let mut fullscreen_windows = self.fullscreen_windows.lock();
        Ok(if is_fullscreen {
            fullscreen_windows.insert(window)
        } else {
            fullscreen_windows.remove(&window)
        })
    }
}
