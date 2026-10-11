//! Keeps the daemon running until `zt down` asks it to stop or the machine goes down.
//!
//! On Windows the daemon links user32 (through the clipboard), so the system tells it about
//! log-off and shutdown with window messages rather than console events: it owns a hidden
//! top-level window that answers `WM_QUERYENDSESSION` and, on `WM_ENDSESSION`, stops the
//! service before the system ends the process. Elsewhere, and whenever a daemon dies without
//! warning, the daemon lock tells observers it is gone.

/// Why the machine ended the daemon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SystemEnd {
    LogOff,
    Shutdown,
}

impl SystemEnd {
    pub(crate) fn describe(self) -> &'static str {
        match self {
            SystemEnd::LogOff => "the user is signing out",
            SystemEnd::Shutdown => "the machine is shutting down",
        }
    }
}

/// Window class of the daemon's hidden window; its title is `zt daemon <pid>`.
#[cfg(windows)]
pub(crate) const WINDOW_CLASS: &str = "ztDaemon";

/// Polls `keep_running` until it says no, or calls `on_end` when the machine ends the user's
/// session and returns what ended it. `on_end` runs while the system waits, so it must finish
/// the daemon's cleanup itself.
#[cfg(not(windows))]
pub(crate) fn watch(
    keep_running: impl Fn() -> bool + 'static,
    _on_end: impl Fn(SystemEnd) + 'static,
) -> Option<SystemEnd> {
    while keep_running() {
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    None
}

#[cfg(windows)]
pub(crate) use windows::watch;

#[cfg(windows)]
mod windows {
    use super::{SystemEnd, WINDOW_CLASS};
    use std::cell::{Cell, RefCell};
    use std::ptr::null;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, ENDSESSION_LOGOFF,
        GetMessageW, KillTimer, MSG, PostQuitMessage, RegisterClassW, SetTimer, TranslateMessage,
        WM_DESTROY, WM_ENDSESSION, WM_QUERYENDSESSION, WM_TIMER, WNDCLASSW, WS_OVERLAPPED,
    };

    const POLL_TIMER: usize = 1;

    struct Handlers {
        keep_running: Box<dyn Fn() -> bool>,
        on_end: Box<dyn Fn(SystemEnd)>,
    }

    thread_local! {
        static HANDLERS: RefCell<Option<Handlers>> = const { RefCell::new(None) };
        static ENDED: Cell<Option<SystemEnd>> = const { Cell::new(None) };
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    pub(crate) fn watch(
        keep_running: impl Fn() -> bool + 'static,
        on_end: impl Fn(SystemEnd) + 'static,
    ) -> Option<SystemEnd> {
        HANDLERS.with(|handlers| {
            *handlers.borrow_mut() = Some(Handlers {
                keep_running: Box::new(keep_running),
                on_end: Box::new(on_end),
            })
        });
        let class = wide(WINDOW_CLASS);
        let title = wide(&format!("zt daemon {}", std::process::id()));
        let window = unsafe {
            let instance = GetModuleHandleW(null());
            let mut class_info: WNDCLASSW = std::mem::zeroed();
            class_info.lpfnWndProc = Some(window_proc);
            class_info.hInstance = instance;
            class_info.lpszClassName = class.as_ptr();
            RegisterClassW(&class_info);
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                null(),
            )
        };
        if window.is_null() {
            // Without a window the daemon still stops on `zt down`; the daemon lock covers the
            // machine going down.
            if let Some(handlers) = HANDLERS.with(|handlers| handlers.borrow_mut().take()) {
                while (handlers.keep_running)() {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            }
            return None;
        }
        unsafe {
            SetTimer(window, POLL_TIMER, 250, None);
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        HANDLERS.with(|handlers| handlers.borrow_mut().take());
        ENDED.with(Cell::get)
    }

    fn keep_running() -> bool {
        HANDLERS.with(|handlers| {
            handlers
                .borrow()
                .as_ref()
                .is_none_or(|handlers| (handlers.keep_running)())
        })
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_TIMER if wparam == POLL_TIMER => {
                if !keep_running() {
                    unsafe {
                        KillTimer(window, POLL_TIMER);
                        DestroyWindow(window);
                    }
                }
                0
            }
            // Never stand in the way of a shutdown.
            WM_QUERYENDSESSION => 1,
            WM_ENDSESSION => {
                if wparam != 0 && ENDED.with(Cell::get).is_none() {
                    let end = if (lparam as u32) & ENDSESSION_LOGOFF != 0 {
                        SystemEnd::LogOff
                    } else {
                        SystemEnd::Shutdown
                    };
                    ENDED.with(|ended| ended.set(Some(end)));
                    HANDLERS.with(|handlers| {
                        if let Some(handlers) = handlers.borrow().as_ref() {
                            (handlers.on_end)(end);
                        }
                    });
                    unsafe {
                        KillTimer(window, POLL_TIMER);
                        DestroyWindow(window);
                    }
                }
                0
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                0
            }
            _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
    }
}
