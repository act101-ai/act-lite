//! Owner-only access on Windows: the equivalent of unix modes `0700` and
//! `0600`. Each function makes the current user the owner and sets a
//! protected DACL (inheritance from the parent off) whose one entry gives the
//! current user full control, so the store is private wherever it lives, not
//! only under the user profile. Ownership matters: an owner can always
//! rewrite the DACL, so a store left owned by another account is not private.

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW, SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetSecurityDescriptorOwner,
    GetTokenInformation, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const DACL: u32 = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;

/// Restrict `dir` to the current user. Files created in it later inherit the
/// same single entry (`OICI`: object and container inherit).
pub(crate) fn restrict_dir(dir: &Path) -> io::Result<()> {
    restrict_named(dir, "OICI")
}

/// Restrict the existing file at `path` to the current user.
pub(crate) fn restrict_existing_file(path: &Path) -> io::Result<()> {
    restrict_named(path, "")
}

/// Set the DACL first: it grants the current user full control, which
/// includes the `WRITE_OWNER` right that the second call needs. The second
/// call sets owner and DACL together, so a DACL that a previous owner put
/// back between the two calls is replaced, and after it that account is
/// neither owner nor on the DACL. A caller that is not the owner gets this far
/// only if the old DACL gave it `WRITE_DAC`; any failure fails the load or
/// save. As with unix `chmod`, a handle another process opened earlier keeps
/// the access it was granted.
fn restrict_named(path: &Path, inherit: &str) -> io::Result<()> {
    let name = wide(path.as_os_str());
    with_owner_only(inherit, |owner, dacl| unsafe {
        match SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL,
            null_mut(),
            null_mut(),
            dacl,
            null(),
        ) {
            0 => SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL,
                owner,
                null_mut(),
                dacl,
                null(),
            ),
            code => code,
        }
    })
}

/// Restrict the open `file` to the current user. The handle must have
/// `WRITE_DAC` and `WRITE_OWNER` access.
pub(crate) fn restrict_file(file: &std::fs::File) -> io::Result<()> {
    let handle = file.as_raw_handle() as HANDLE;
    with_owner_only("", |owner, dacl| unsafe {
        SetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL,
            owner,
            null_mut(),
            dacl,
            null(),
        )
    })
}

/// Build owner `<user>` and DACL `D:P(A;<inherit>;FA;;;<user>)` and pass them
/// to `apply`, which returns a Win32 error code (0 is success).
fn with_owner_only(inherit: &str, apply: impl FnOnce(PSID, *const ACL) -> u32) -> io::Result<()> {
    let sid = current_user_sid()?;
    let sddl = wide(OsStr::new(&format!("O:{sid}D:P(A;{inherit};FA;;;{sid})")));
    let mut sd: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `sddl` is NUL-terminated; on success `sd` is a LocalAlloc'd
    // descriptor that `Local` frees.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let sd = Local(sd);
    let mut present = 0;
    let mut defaulted = 0;
    let mut dacl: *mut ACL = null_mut();
    let mut owner: PSID = null_mut();
    // SAFETY: `sd.0` is a valid descriptor; `dacl` and `owner` point into it
    // and are used only while `sd` lives.
    if unsafe { GetSecurityDescriptorDacl(sd.0, &mut present, &mut dacl, &mut defaulted) } == 0
        || unsafe { GetSecurityDescriptorOwner(sd.0, &mut owner, &mut defaulted) } == 0
    {
        return Err(io::Error::last_os_error());
    }
    match apply(owner, dacl) {
        0 => Ok(()),
        code => Err(io::Error::from_raw_os_error(code as i32)),
    }
}

/// The string SID (`S-1-5-21-...`) of the user this process runs as.
pub(crate) fn current_user_sid() -> io::Result<String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: the pseudo-handle from GetCurrentProcess needs no close.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Token(token);
    let mut len = 0u32;
    // SAFETY: a size query with a null buffer; it fails and sets `len`.
    unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut len) };
    // u64 elements keep the buffer aligned for TOKEN_USER.
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` bytes.
    if unsafe { GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), len, &mut len) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: on success `buf` starts with a TOKEN_USER whose SID points into `buf`.
    let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text = null_mut();
    // SAFETY: `sid` is valid while `buf` lives; `text` is LocalAlloc'd.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let text = Local(text.cast());
    // SAFETY: `text` is a NUL-terminated wide string.
    Ok(unsafe { from_wide_ptr(text.0.cast()) })
}

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// # Safety
/// `p` must point to a NUL-terminated UTF-16 string.
unsafe fn from_wide_ptr(p: *const u16) -> String {
    let mut len = 0;
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}

/// Memory from LocalAlloc, freed on drop.
struct Local(*mut core::ffi::c_void);

impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a Win32 call that LocalAlloc'd it.
        unsafe { LocalFree(self.0) };
    }
}

struct Token(HANDLE);

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: an open token handle owned by this value.
        unsafe { CloseHandle(self.0) };
    }
}

/// The owner and DACL of `path` as SDDL, for tests.
#[cfg(test)]
pub(crate) fn security_sddl(path: &Path) -> String {
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::OWNER_SECURITY_INFORMATION;
    let info = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
    let name = wide(path.as_os_str());
    let mut sd: PSECURITY_DESCRIPTOR = null_mut();
    let rc = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            info,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut sd,
        )
    };
    assert_eq!(rc, 0, "GetNamedSecurityInfoW failed for {}", path.display());
    let sd = Local(sd);
    let mut text = null_mut();
    let ok = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            sd.0,
            SDDL_REVISION_1,
            info,
            &mut text,
            null_mut(),
        )
    };
    assert_ne!(ok, 0, "{}", io::Error::last_os_error());
    let text = Local(text.cast());
    unsafe { from_wide_ptr(text.0.cast()) }
}

/// Assert that the current user owns `path` and that `path` has a protected
/// DACL whose only entry gives the current user full control with
/// inheritance flags `inherit`.
#[cfg(test)]
pub(crate) fn assert_owner_only(path: &Path, inherit: &str) {
    let sddl = security_sddl(path);
    let sid = current_user_sid().unwrap();
    let (owner, dacl) = sddl
        .strip_prefix("O:")
        .and_then(|rest| rest.split_once("D:"))
        .unwrap_or_else(|| panic!("no owner or DACL on {}: {sddl}", path.display()));
    assert_eq!(
        owner,
        sid,
        "{} is not owned by the current user: {sddl}",
        path.display()
    );
    let (flags, aces) = dacl
        .find('(')
        .map(|i| dacl.split_at(i))
        .unwrap_or_else(|| panic!("no DACL entries on {}: {sddl}", path.display()));
    assert!(
        flags.contains('P'),
        "DACL of {} is not protected: {sddl}",
        path.display()
    );
    assert_eq!(
        aces,
        format!("(A;{inherit};FA;;;{sid})"),
        "DACL of {} is not owner-only",
        path.display()
    );
}
