use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
use std::cell::RefCell;
use std::rc::Rc;

type WinitCallback = Box<dyn FnMut(&slint::Window, &WindowEvent) -> EventResult>;

/// Multiplexes `window.on_winit_window_event` so multiple subsystems (OS D&D, terminal, hover) coexist.
#[derive(Default, Clone)]
pub struct WinitEventHub {
    handlers: Rc<RefCell<Vec<WinitCallback>>>,
}

impl WinitEventHub {
    pub fn push(&self, handler: impl FnMut(&slint::Window, &WindowEvent) -> EventResult + 'static) {
        self.handlers.borrow_mut().push(Box::new(handler));
    }

    pub fn install(self, window: &slint::Window) {
        let handlers = self.handlers;
        window.on_winit_window_event(move |win, event| {
            let mut result = EventResult::Propagate;
            for handler in handlers.borrow_mut().iter_mut() {
                if matches!(handler(win, event), EventResult::PreventDefault) {
                    result = EventResult::PreventDefault;
                }
            }
            result
        });
    }
}
