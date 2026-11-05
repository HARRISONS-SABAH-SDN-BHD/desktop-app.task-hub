//! Physical input only. UI Automation is used to observe, never to set hidden values.
use crate::model::Result;
use std::{ffi::c_void, mem::size_of};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Mouse {
    x: i32,
    y: i32,
    data: u32,
    flags: u32,
    time: u32,
    extra: usize,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Key {
    vk: u16,
    scan: u16,
    flags: u32,
    time: u32,
    extra: usize,
}
#[repr(C)]
union Payload {
    mouse: Mouse,
    key: Key,
}
#[repr(C)]
struct Input {
    kind: u32,
    payload: Payload,
}
#[link(name = "user32")]
extern "system" {
    fn SendInput(count: u32, input: *const Input, size: i32) -> u32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn GetAsyncKeyState(key: i32) -> i16;
    pub fn GetForegroundWindow() -> *mut c_void;
    pub fn SetForegroundWindow(window: *mut c_void) -> i32;
    pub fn ShowWindow(window: *mut c_void, command: i32) -> i32;
    pub fn GetAncestor(window: *mut c_void, flags: u32) -> *mut c_void;
    fn WindowFromPoint(point: Point) -> *mut c_void;
    fn OpenInputDesktop(flags: u32, inherit: i32, access: u32) -> *mut c_void;
    fn CloseDesktop(desktop: *mut c_void) -> i32;
    fn GetUserObjectInformationW(
        object: *mut c_void,
        index: i32,
        info: *mut c_void,
        bytes: u32,
        needed: *mut u32,
    ) -> i32;
}
pub fn desktop_ready() -> bool {
    // Read-only probe: never switch desktops or interact with a lock screen.
    let desktop = unsafe { OpenInputDesktop(0, 0, 1) };
    if desktop.is_null() {
        return false;
    }
    let mut name = [0u16; 256];
    let mut needed = 0;
    let mut receiving: i32 = 0;
    let named = unsafe {
        GetUserObjectInformationW(
            desktop,
            2,
            name.as_mut_ptr().cast(),
            (name.len() * 2) as u32,
            &mut needed,
        )
    } != 0;
    let active = unsafe {
        GetUserObjectInformationW(
            desktop,
            6,
            (&mut receiving as *mut i32).cast(),
            4,
            &mut needed,
        )
    } != 0;
    unsafe {
        CloseDesktop(desktop);
    }
    let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
    named
        && active
        && receiving != 0
        && String::from_utf16_lossy(&name[..end]).eq_ignore_ascii_case("Default")
}
fn emit(inputs: &[Input]) -> Result<()> {
    if unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<Input>() as i32,
        )
    } != inputs.len() as u32
    {
        return Err("Windows refused input. Keep Task Hub and target apps non-elevated and unlock the desktop.".into());
    }
    Ok(())
}
fn key(vk: u16, scan: u16, flags: u32) -> Input {
    Input {
        kind: 1,
        payload: Payload {
            key: Key {
                vk,
                scan,
                flags,
                time: 0,
                extra: 0,
            },
        },
    }
}
pub fn press(vk: u16) -> Result<()> {
    emit(&[key(vk, 0, 0), key(vk, 0, 2)])
}
pub fn chord(modifier: u16, vk: u16) -> Result<()> {
    emit(&[
        key(modifier, 0, 0),
        key(vk, 0, 0),
        key(vk, 0, 2),
        key(modifier, 0, 2),
    ])
}
pub fn unicode(ch: char) -> Result<()> {
    let mut buffer = [0; 2];
    let mut inputs = Vec::new();
    for unit in ch.encode_utf16(&mut buffer).iter() {
        inputs.extend([key(0, *unit, 4), key(0, *unit, 6)]);
    }
    emit(&inputs)
}
pub fn cursor() -> Result<Point> {
    let mut point = Point { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err("Could not read pointer position".into());
    }
    Ok(point)
}
pub fn interrupted() -> bool {
    unsafe { GetAsyncKeyState(0x1B) < 0 }
}
pub fn modifiers_down() -> bool {
    [0x10, 0x11, 0x12, 0x5B, 0x5C]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(*key) < 0 })
}
pub fn click(hwnd: isize, x: i32, y: i32) -> Result<()> {
    mouse_click(hwnd, x, y, false)
}
pub fn context_click(hwnd: isize, x: i32, y: i32) -> Result<()> {
    mouse_click(hwnd, x, y, true)
}
fn mouse_click(hwnd: isize, x: i32, y: i32, right: bool) -> Result<()> {
    let hit = unsafe { GetAncestor(WindowFromPoint(Point { x, y }), 2) } as isize;
    if hit != hwnd {
        return Err(
            "A different window covers the target control. Automation stopped before clicking."
                .into(),
        );
    }
    if unsafe { SetCursorPos(x, y) } == 0 {
        return Err("Could not move pointer".into());
    }
    if unsafe { GetForegroundWindow() } as isize != hwnd {
        return Err("Window focus changed before the click".into());
    }
    emit(&[
        Input {
            kind: 0,
            payload: Payload {
                mouse: Mouse {
                    x: 0,
                    y: 0,
                    data: 0,
                    flags: if right { 8 } else { 2 },
                    time: 0,
                    extra: 0,
                },
            },
        },
        Input {
            kind: 0,
            payload: Payload {
                mouse: Mouse {
                    x: 0,
                    y: 0,
                    data: 0,
                    flags: if right { 16 } else { 4 },
                    time: 0,
                    extra: 0,
                },
            },
        },
    ])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_layout_matches_win32_abi() {
        assert_eq!(
            size_of::<Input>(),
            if cfg!(target_pointer_width = "64") {
                40
            } else {
                28
            }
        );
    }
}
