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

#[repr(C)]
struct NxmPackage {
    title_id: u64,
    build_id: [u8; 8],
    has_build_id: bool,
    name: [c_char; 0x201],
    publisher: [c_char; 0x101],
    version: [c_char; 0x11],
    icon: *mut u8,
    icon_size: usize,
}

extern "C" {
    fn nxm_package_info(package: *const c_char, keys: *const c_char, out: *mut NxmPackage, why: *mut c_char, why_size: usize) -> bool;
    fn nxm_package_free(p: *mut NxmPackage);
    fn nxm_find_keys(package: *const c_char, out: *mut c_char, out_size: usize) -> bool;
    fn nxm_package_setup(package: *const c_char, keys: *const c_char, out_dir: *const c_char, title_id: *mut u64, why: *mut c_char, why_size: usize) -> bool;
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

/// What a game package (.nsp / .xci) is: read from its headers and control data, without setting it up.
#[derive(Clone, Debug, Default)]
pub struct Package {
    pub title_id: String,
    /// exefs/main's build id: which version of the game
    pub build_id: Option<String>,
    pub name: String,
    pub publisher: String,
    pub version: String,
    /// the game's icon, a JPEG
    pub icon: Option<Vec<u8>>,
}

fn cstr(b: &[c_char]) -> String {
    unsafe { CStr::from_ptr(b.as_ptr()) }.to_string_lossy().into_owned()
}

/// The keys file for a package: prod.keys beside it, else ~/.switch/prod.keys.
pub fn find_keys(package: &Path) -> Option<std::path::PathBuf> {
    let c = cpath(package)?;
    let mut out = [0 as c_char; 1200];
    unsafe { nxm_find_keys(c.as_ptr(), out.as_mut_ptr(), out.len()) }.then(|| cstr(&out).into())
}

/// A game folder set up from a package: its ExeFS written to <out_dir>/exefs, its RomFS described in
/// <out_dir>/romfs.src (read from the package in place). Seconds.
pub fn package_setup(package: &Path, keys: &Path, out_dir: &Path) -> Result<(), String> {
    let (p, k, o) = (cpath(package).ok_or("bad path")?, cpath(keys).ok_or("bad path")?, cpath(out_dir).ok_or("bad path")?);
    let mut why = [0 as c_char; 256];
    let mut tid = 0u64;
    if unsafe { nxm_package_setup(p.as_ptr(), k.as_ptr(), o.as_ptr(), &mut tid, why.as_mut_ptr(), why.len()) } {
        Ok(())
    } else {
        Err(cstr(&why))
    }
}

pub fn package_info(package: &Path, keys: &Path) -> Result<Package, String> {
    let (p, k) = (cpath(package).ok_or("bad path")?, cpath(keys).ok_or("bad path")?);
    let mut out: NxmPackage = unsafe { std::mem::zeroed() };
    let mut why = [0 as c_char; 256];
    if !unsafe { nxm_package_info(p.as_ptr(), k.as_ptr(), &mut out, why.as_mut_ptr(), why.len()) } {
        return Err(cstr(&why));
    }
    let icon = (!out.icon.is_null() && out.icon_size > 0)
        .then(|| unsafe { std::slice::from_raw_parts(out.icon, out.icon_size) }.to_vec());
    let r = Package {
        title_id: format!("{:016x}", out.title_id),
        build_id: out.has_build_id.then(|| out.build_id.iter().map(|b| format!("{b:02x}")).collect()),
        name: cstr(&out.name),
        publisher: cstr(&out.publisher),
        version: cstr(&out.version),
        icon,
    };
    unsafe { nxm_package_free(&mut out) };
    Ok(r)
}
