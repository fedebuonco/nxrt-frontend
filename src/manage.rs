//! libnxrt_manage (nxrt/manage/nxrt_manage.h): a game folder's files, without the runtime. A thin safe layer over the
//! C ABI; build.rs compiles the library from the nxrt submodule.
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

const NXM_ABI: c_int = 1;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct NxmGameFiles {
    exefs: bool,
    romfs_dir: bool,
    romfs_package: bool,
    build_id: [u8; 8],
}

#[repr(C)]
struct NxmRomfs {
    _opaque: [u8; 0],
}

extern "C" {
    fn nxm_abi() -> c_int;
    fn nxm_game_files(game_dir: *const c_char, out: *mut NxmGameFiles) -> bool;
    fn nxm_romfs_open(game_dir: *const c_char, why: *mut c_char, why_size: usize) -> *mut NxmRomfs;
    fn nxm_romfs_close(r: *mut NxmRomfs);
    fn nxm_romfs_list(
        r: *mut NxmRomfs,
        f: extern "C" fn(*const c_char, u64, *mut c_void),
        ctx: *mut c_void,
    ) -> c_int;
}

fn cpath(p: &Path) -> Option<CString> {
    CString::new(p.to_string_lossy().as_bytes()).ok()
}

/// The library linked is the one this app was written against.
pub fn abi_ok() -> bool {
    unsafe { nxm_abi() == NXM_ABI }
}

/// What a game folder holds.
#[derive(Clone, Debug, Default)]
pub struct GameFiles {
    pub exefs: bool,
    pub romfs_dir: bool,
    pub romfs_package: bool,
    /// exefs/main's build id (16 hex digits), when exefs is there
    pub build_id: Option<String>,
}

pub fn game_files(dir: &Path) -> Option<GameFiles> {
    let c = cpath(dir)?;
    let mut out = NxmGameFiles::default();
    if !unsafe { nxm_game_files(c.as_ptr(), &mut out) } {
        return None;
    }
    Some(GameFiles {
        exefs: out.exefs,
        romfs_dir: out.romfs_dir,
        romfs_package: out.romfs_package,
        build_id: out.exefs.then(|| out.build_id.iter().map(|b| format!("{b:02x}")).collect()),
    })
}

/// A summary of the game folder's RomFS: its file count and total size.
#[derive(Clone, Debug, Default)]
pub struct RomfsSummary {
    pub files: usize,
    pub bytes: u64,
}

extern "C" fn count_file(_path: *const c_char, size: u64, ctx: *mut c_void) {
    let s = unsafe { &mut *(ctx as *mut RomfsSummary) };
    s.files += 1;
    s.bytes += size;
}

/// The RomFS (romfs/ or the package read in place): Err(why) when there is none or it can't be read.
pub fn romfs_summary(dir: &Path) -> Result<RomfsSummary, String> {
    let c = cpath(dir).ok_or("bad path")?;
    let mut why = [0 as c_char; 256];
    let r = unsafe { nxm_romfs_open(c.as_ptr(), why.as_mut_ptr(), why.len()) };
    if r.is_null() {
        return Err(unsafe { CStr::from_ptr(why.as_ptr()) }.to_string_lossy().into_owned());
    }
    let mut s = RomfsSummary::default();
    let n = unsafe { nxm_romfs_list(r, count_file, &mut s as *mut _ as *mut c_void) };
    unsafe { nxm_romfs_close(r) };
    if n < 0 {
        return Err("the game data couldn't be read".into());
    }
    Ok(s)
}
