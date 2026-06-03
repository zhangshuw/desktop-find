use crate::desktop::{show_overlay, DesktopItem};
use tauri::AppHandle;
use windows::core::{w, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Memory::{VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_ACCESS_RIGHTS, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};
use windows::Win32::UI::Controls::{LVITEMW, LVIF_TEXT, LVIR_BOUNDS, LVM_GETITEMCOUNT, LVM_GETITEMRECT, LVM_GETITEMTEXTW};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, FindWindowExW, FindWindowW, GetWindowThreadProcessId, SendMessageW};

pub fn highlight_desktop_item(app: &AppHandle, item: &DesktopItem) -> Result<(), String> {
    show_overlay(app, item)
}

pub fn enrich_desktop_item_positions(items: &mut [DesktopItem]) {
    let Ok(entries) = desktop_icon_entries() else {
        return;
    };

    for item in items.iter_mut() {
        let Some(entry) = entries.iter().find(|entry| entry.name.eq_ignore_ascii_case(&item.name)) else {
            continue;
        };

        item.x = Some(entry.rect.left);
        item.y = Some(entry.rect.top);
        item.width = Some((entry.rect.right - entry.rect.left).max(1) as u32);
        item.height = Some((entry.rect.bottom - entry.rect.top).max(1) as u32);
    }
}

#[derive(Debug)]
struct DesktopIconEntry {
    name: String,
    rect: RECT,
}

fn desktop_icon_entries() -> Result<Vec<DesktopIconEntry>, String> {
    let listview = find_desktop_listview().ok_or_else(|| "未找到桌面图标 ListView".to_string())?;
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(listview, Some(&mut process_id));
    }

    if process_id == 0 {
        return Err("无法读取桌面 ListView 所属进程".to_string());
    }

    let access = PROCESS_ACCESS_RIGHTS(
        PROCESS_QUERY_INFORMATION.0 | PROCESS_VM_OPERATION.0 | PROCESS_VM_READ.0 | PROCESS_VM_WRITE.0,
    );
    let process = ProcessHandle::open(access, process_id)?;
    let item_count = unsafe { SendMessageW(listview, LVM_GETITEMCOUNT, None, None).0 as i32 };
    let item_count = item_count.max(0).min(512);
    let mut entries = Vec::new();

    for index in 0..item_count {
        let Ok(name) = read_listview_item_text(listview, &process, index) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let Ok(rect) = read_listview_item_rect(listview, &process, index) else {
            continue;
        };
        entries.push(DesktopIconEntry { name, rect });
    }

    Ok(entries)
}

fn find_desktop_listview() -> Option<HWND> {
    unsafe {
        if let Ok(progman) = FindWindowW(w!("Progman"), PCWSTR::null()) {
            if let Some(listview) = find_listview_under(progman) {
                return Some(listview);
            }
        }

        let mut result = HWND::default();
        let result_ptr = &mut result as *mut HWND;
        let _ = EnumWindows(Some(enum_windows_find_desktop), LPARAM(result_ptr as isize));

        if result.is_invalid() {
            None
        } else {
            Some(result)
        }
    }
}

unsafe extern "system" fn enum_windows_find_desktop(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if let Some(listview) = find_listview_under(hwnd) {
        let result = lparam.0 as *mut HWND;
        if !result.is_null() {
            unsafe {
                *result = listview;
            }
        }
        return false.into();
    }

    true.into()
}

fn find_listview_under(parent: HWND) -> Option<HWND> {
    unsafe {
        let def_view = FindWindowExW(Some(parent), None, w!("SHELLDLL_DefView"), PCWSTR::null()).ok()?;
        FindWindowExW(Some(def_view), None, w!("SysListView32"), PCWSTR::null()).ok()
    }
}

fn read_listview_item_rect(listview: HWND, process: &ProcessHandle, index: i32) -> Result<RECT, String> {
    let mut rect = RECT {
        left: LVIR_BOUNDS as i32,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let remote = RemoteAllocation::new(process.handle, std::mem::size_of::<RECT>())?;

    unsafe {
        WriteProcessMemory(
            process.handle,
            remote.ptr,
            (&rect as *const RECT).cast(),
            std::mem::size_of::<RECT>(),
            None,
        )
        .map_err(|error| format!("写入远程 RECT 失败：{error}"))?;

        let ok = SendMessageW(
            listview,
            LVM_GETITEMRECT,
            Some(WPARAM(index as usize)),
            Some(LPARAM(remote.ptr as isize)),
        );

        if ok.0 == 0 {
            return Err("读取桌面图标矩形失败".to_string());
        }

        ReadProcessMemory(
            process.handle,
            remote.ptr,
            (&mut rect as *mut RECT).cast(),
            std::mem::size_of::<RECT>(),
            None,
        )
        .map_err(|error| format!("读取远程 RECT 失败：{error}"))?;
    }

    Ok(rect)
}

fn read_listview_item_text(listview: HWND, process: &ProcessHandle, index: i32) -> Result<String, String> {
    const TEXT_CAPACITY: usize = 260;
    let lvitem_size = std::mem::size_of::<LVITEMW>();
    let text_size = TEXT_CAPACITY * std::mem::size_of::<u16>();
    let remote = RemoteAllocation::new(process.handle, lvitem_size + text_size)?;
    let remote_text = unsafe { remote.ptr.add(lvitem_size) } as *mut u16;

    let item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: index,
        iSubItem: 0,
        pszText: PWSTR(remote_text),
        cchTextMax: TEXT_CAPACITY as i32,
        ..Default::default()
    };

    unsafe {
        WriteProcessMemory(
            process.handle,
            remote.ptr,
            (&item as *const LVITEMW).cast(),
            lvitem_size,
            None,
        )
        .map_err(|error| format!("写入远程 LVITEM 失败：{error}"))?;

        SendMessageW(
            listview,
            LVM_GETITEMTEXTW,
            Some(WPARAM(index as usize)),
            Some(LPARAM(remote.ptr as isize)),
        );

        let mut buffer = [0u16; TEXT_CAPACITY];
        ReadProcessMemory(
            process.handle,
            remote_text.cast(),
            buffer.as_mut_ptr().cast(),
            text_size,
            None,
        )
        .map_err(|error| format!("读取远程图标文本失败：{error}"))?;

        let len = buffer.iter().position(|value| *value == 0).unwrap_or(TEXT_CAPACITY);
        Ok(String::from_utf16_lossy(&buffer[..len]))
    }
}

struct ProcessHandle {
    handle: HANDLE,
}

impl ProcessHandle {
    fn open(access: PROCESS_ACCESS_RIGHTS, process_id: u32) -> Result<Self, String> {
        let handle = unsafe { OpenProcess(access, false, process_id) }
            .map_err(|error| format!("打开桌面进程失败：{error}"))?;
        Ok(Self { handle })
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

struct RemoteAllocation {
    process: HANDLE,
    ptr: *mut core::ffi::c_void,
}

impl RemoteAllocation {
    fn new(process: HANDLE, size: usize) -> Result<Self, String> {
        let allocation_type = windows::Win32::System::Memory::VIRTUAL_ALLOCATION_TYPE(MEM_COMMIT.0 | MEM_RESERVE.0);
        let ptr = unsafe { VirtualAllocEx(process, None, size, allocation_type, PAGE_READWRITE) };
        if ptr.is_null() {
            return Err("分配远程内存失败".to_string());
        }

        Ok(Self { process, ptr })
    }
}

impl Drop for RemoteAllocation {
    fn drop(&mut self) {
        unsafe {
            let _ = VirtualFreeEx(self.process, self.ptr, 0, MEM_RELEASE);
        }
    }
}
