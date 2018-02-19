use std::mem;
use std::ptr;
use std::cell::Cell;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use winapi::shared::windef::*;
use winapi::shared::minwindef::*;
use winapi::um::winuser::*;
use winapi::um::libloaderapi::*;
use winapi::um::winbase::*;
use core::*;
use framework::*;


/// Window message handler callback function
pub extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT
{
    let win_ptr = ffi!(GetWindowLongPtrW(hwnd, 0));
    if win_ptr == 0 {
        return ffi!(DefWindowProcW(hwnd, msg, wparam, lparam));
    }

    return GLWindow::handle_os_message(win_ptr, hwnd, msg, wparam, lparam);
}


/// Engine implementation for Windows.
pub struct GLEngine {
    hinstance: HINSTANCE,
    window_class_name: Vec<u16>,

    // Number of active/non-closed windows
    window_count: Cell<i32>,

}

impl GLEngine {
    pub fn new() -> Result<Box<GLEngine>, Error> {
        let window_class_name = OsStr::new("shine")
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect::<Vec<_>>();

        let hinstance = ffi!(GetModuleHandleW(ptr::null()));

        let class = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as UINT,
            style: CS_HREDRAW | CS_VREDRAW | CS_OWNDC,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: mem::size_of::<*mut GLWindow>() as i32,
            hInstance: hinstance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: window_class_name.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        let res = ffi!(RegisterClassExW(&class));
        if res == 0 {
            return Err(Error::InitializeError(format!("")));
        }

        Ok(Box::new(GLEngine {
            hinstance: hinstance,
            window_class_name: window_class_name,

            // While no window is created it is set an extremal value, not to terminate dispatch event before
            // the window creation has terminated.
            window_count: Cell::new(i32::max_value()),
        }))
    }

    pub fn quit(&self) {
        ffi!(PostQuitMessage(0));
    }

    pub fn dispatch_event(&self, timeout: DispatchTimeout) -> bool {
        let mut new_window_count = self.window_count.get();

        let mut msg: MSG = unsafe { mem::zeroed() };

        match timeout {
            DispatchTimeout::Immediate => {
                if ffi!(PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE)) == 0 {
                    return true;
                }
            }

            DispatchTimeout::Infinite => {
                if ffi!(GetMessageW(&mut msg, ptr::null_mut(), 0, 0)) == 0 {
                    // Only happens if the message is `WM_QUIT`.
                    //debug_assert_eq!(msg.message, WM_QUIT);
                    return false;
                }
            }

            DispatchTimeout::Time(timeout) => {
                if ffi!(PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE)) == 0 {
                    let secs_part = (timeout.as_secs() * 1000) as i64;
                    let nanos_part = (timeout.subsec_nanos() / 1000_000) as i64;
                    let timeout_ms = secs_part + nanos_part;

                    // no pending message, let's wait for some
                    if ffi!(MsgWaitForMultipleObjects(0, ptr::null_mut(), FALSE, timeout_ms as u32, QS_ALLEVENTS)) != WAIT_OBJECT_0 {
                        return true;
                    }

                    if ffi!(PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE)) == 0 {
                        // it shall never happen, but who knows, stay on the safe side :)
                        return true;
                    }
                }
            }
        }

        if msg.message == win_messages::WM_DR_WINDOW_CREATED {
            //println!("dispatching WM_DR_WINDOW_CREATED, new_window_count: {}", new_window_count);
            if new_window_count == i32::max_value() {
                new_window_count = 1;
            } else {
                new_window_count += 1;
            }
        } else if msg.message == win_messages::WM_DR_WINDOW_DESTROYED {
            new_window_count -= 1;
            //println!("dispatching  WM_DR_WINDOW_DESTROYED, new_window_count: {}", new_window_count);
        }

        // messages are delegated to the window in the window proc
        ffi!(TranslateMessage(&msg));
        ffi!(DispatchMessageW(&msg));


        self.window_count.set(new_window_count);
        new_window_count > 0
    }


    pub fn get_window_class_name(&self) -> &Vec<u16> {
        &self.window_class_name
    }

    pub fn get_instance(&self) -> HINSTANCE {
        self.hinstance
    }
}

impl Drop for GLEngine {
    fn drop(&mut self) {
        ffi!(UnregisterClassW(self.window_class_name.as_ptr(), self.hinstance));
    }
}

