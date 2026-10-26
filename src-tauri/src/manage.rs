//! libnxrt_manage (nxrt/manage/nxrt_manage.h): what a game package is (its title, name, icon), read with the keys. A thin safe layer over the
//! C ABI; build.rs compiles the library from the nxrt submodule.
use std::ffi::{c_char, c_int, CStr, CString};
use std::path::Path;

const NXM_ABI: c_int = 2;

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
    is_update: bool,
}

extern "C" {
    fn nxm_package_info(
        package: *const c_char,
        keys: *const c_char,
        out: *mut NxmPackage,
        why: *mut c_char,
        why_size: usize,
    ) -> bool;
    fn nxm_package_free(p: *mut NxmPackage);
    fn nxm_find_keys(package: *const c_char, out: *mut c_char, out_size: usize) -> bool;
    fn nxm_package_setup_update(
        package: *const c_char,
        update: *const c_char,
        keys: *const c_char,
        out_dir: *const c_char,
        title_id: *mut u64,
        why: *mut c_char,
        why_size: usize,
    ) -> bool;
    fn nxm_abi() -> c_int;
}

fn cpath(p: &Path) -> Option<CString> {
    CString::new(p.to_string_lossy().as_bytes()).ok()
}

/// The library linked is the one this app was written against.
pub fn abi_ok() -> bool {
    unsafe { nxm_abi() == NXM_ABI }
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
    /// an update: played with its base game
    pub is_update: bool,
}

fn cstr(b: &[c_char]) -> String {
    unsafe { CStr::from_ptr(b.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// The keys file for a package: prod.keys beside it, else ~/.switch/prod.keys.
pub fn find_keys(package: &Path) -> Option<std::path::PathBuf> {
    let c = cpath(package)?;
    let mut out = [0 as c_char; 1200];
    unsafe { nxm_find_keys(c.as_ptr(), out.as_mut_ptr(), out.len()) }.then(|| cstr(&out).into())
}

/// A game folder set up from a package (and its update): its ExeFS written to <out_dir>/exefs, its RomFS described in
/// <out_dir>/romfs.src (read from the package, and the update's changes, in place). Seconds.
pub fn package_setup(
    package: &Path,
    update: Option<&Path>,
    keys: &Path,
    out_dir: &Path,
) -> Result<(), String> {
    let (p, k, o) = (
        cpath(package).ok_or("bad path")?,
        cpath(keys).ok_or("bad path")?,
        cpath(out_dir).ok_or("bad path")?,
    );
    let u = update.and_then(cpath);
    let mut why = [0 as c_char; 256];
    let mut tid = 0u64;
    let up = u.as_ref().map_or(std::ptr::null(), |u| u.as_ptr());
    if unsafe {
        nxm_package_setup_update(
            p.as_ptr(),
            up,
            k.as_ptr(),
            o.as_ptr(),
            &mut tid,
            why.as_mut_ptr(),
            why.len(),
        )
    } {
        Ok(())
    } else {
        Err(cstr(&why))
    }
}

pub fn package_info(package: &Path, keys: &Path) -> Result<Package, String> {
    let (p, k) = (
        cpath(package).ok_or("bad path")?,
        cpath(keys).ok_or("bad path")?,
    );
    let mut out: NxmPackage = unsafe { std::mem::zeroed() };
    let mut why = [0 as c_char; 256];
    if !unsafe {
        nxm_package_info(
            p.as_ptr(),
            k.as_ptr(),
            &mut out,
            why.as_mut_ptr(),
            why.len(),
        )
    } {
        return Err(cstr(&why));
    }
    let icon = (!out.icon.is_null() && out.icon_size > 0)
        .then(|| unsafe { std::slice::from_raw_parts(out.icon, out.icon_size) }.to_vec());
    let r = Package {
        title_id: format!("{:016x}", out.title_id),
        build_id: out
            .has_build_id
            .then(|| out.build_id.iter().map(|b| format!("{b:02x}")).collect()),
        name: cstr(&out.name),
        publisher: cstr(&out.publisher),
        version: cstr(&out.version),
        icon,
        is_update: out.is_update,
    };
    unsafe { nxm_package_free(&mut out) };
    Ok(r)
}
