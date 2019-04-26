extern crate nix;

use nix::mount::{mount, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{execvp, getuid, getgid};
use std::{fs, env};
use std::ffi::{OsString, CString};
use std::os::unix::ffi::OsStrExt;

/*

- You can start the container with a non-root initial uid and without CAP_SYS_ADMIN.
- You need to disable seccomp or use a profile that permits our unshare and mount calls. 
- Instead of being done container-wide, the mounts are only visible inside processes run
  with this wrapper tool.
- Every uid and gid except our own will display as nobody / nogroup from processes run
  with this wrapper tool.

*/

fn new_mount_ns() -> Result<(), String> {

    let uid = getuid();
    let gid = getgid();

    // A new user namespace is needed because otherwise we do not have permission to create
    // a mount namespace (assuming we are running as an unprivileged user).
    let flags = CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS;
    assert_eq!(flags.bits(), 268566528,
        "Value of {:?} does not match whitelisted value in seccomp profile", flags);

    // Perform the namespace creation and switch
    unshare(flags)
        .map_err(|e| format!("Unshare failed: {}", e))?;

    // According to a comment in the unshare.c source code, newer kernels require locking down
    // setgroups before permitting setting of the uid/gid maps.
    fs::write("/proc/self/setgroups", "deny")
        .map_err(|e| format!("Write /proc/self/setgroups failed: {}", e))?;

    // Map our original user to itself. All other users/groups will appear as nobody:nogroup.
    // The only other option (as an unprivileged process) is to map the root user/group inside
    // the container to our original uid/gid outside.
    fs::write("/proc/self/uid_map", format!("{} {} 1", uid, uid))
        .map_err(|e| format!("Write /proc/self/uid_map failed: {}", e))?;

    fs::write("/proc/self/gid_map", format!("{} {} 1", gid, gid))
        .map_err(|e| format!("Write /pro/self/cgid_map failed: {}", e))?;

    // At this point we are a "normal" user but we still have a full set of capabilities in the
    // the new user namespace, which allows us to perform mount operations.
    // The capabilities should be automatically dropped as soon as we exec a new executable.
    // TODO: Test that the capabilities really are dropped.

    Ok(())
}

// Checks whether we are *probably* in an unprivileged mount namespace.
// Those can have a maximum of 1 uid mapped.
fn is_in_unprivileged_mount_ns() -> Result<bool, String> {
    let uid_map = fs::read_to_string("/proc/self/uid_map")
        .map_err(|e| format!("Read /proc/self/uid_map failed: {}", e))?;

    fn uid_count_from_line(l: &str) -> u32 {
        l.trim().split_whitespace().last().unwrap().parse().unwrap()
    };

    let total_uid_count : u32 = uid_map.lines().map(uid_count_from_line).sum();

    Ok(total_uid_count <= 1)
}

fn bind_mount(src: &str, dest: &str) -> Result<(), String> {

    fs::create_dir_all(dest)
        .map_err(|e| format!("Could not create bind mount destination {}: {}", dest, e))?;

    /*
    The mount(2) manpage says the following, which means we *must* pass MS_REC and we cannot use
    bind mounts to reach directories that have already been obscured by volumes or bind mounts.

    EINVAL
    In an unprivileged mount namespace (i.e., a mount namespace owned by a user namespace that
    was created by an unprivileged user), a bind mount operation (MS_BIND) was attempted without
    specifying (MS_REC), which would have revealed the filesystem tree underneath one of the
    submounts of the directory being bound.
    */
    let flags = MsFlags::MS_BIND | MsFlags::MS_REC;
    assert_eq!(flags.bits(), 20480,
        "Value of {:?} does not match whitelisted value in seccomp profile", flags);

    mount::<str, str, str, str>(Some(src), dest, None, flags, None)
        .map_err(|e| format!("Could not bind-mount {} over {}: {}", src, dest, e))
}

fn apply_bind_mounts_from_bindfstab(fname: &str) -> Result<(), String> {

    let bindfstab = fs::read_to_string(fname)
        .map_err(|e| format!("Read {} failed: {}", fname, e))?;

    // Number lines and then filter out ones starting with '#' or containing only whitespace
    let bindfs_lines_iter = bindfstab.lines().enumerate()
        .filter(|(_i, l)| !l.starts_with('#') && l.trim().len() > 0);

    // Parse the lines into src:dest tuples and perform the bind mount.
    // Give an error if there are not exactly two fields.
    for (i, l) in bindfs_lines_iter {
        let fields : Vec<_> = l.trim().split(":").collect();
        match fields.as_slice() {
            [src, dest] => bind_mount(src, dest)?,
            _ => Err(format!("Invalid syntax on {} line {}: {:?}", fname, i + 1, l))?,
        }
    }

    Ok(())
}

fn execute_main_program() -> Result<(), String> {

    let cmd : Vec<_> = if env::args_os().count() > 1 {

        // We get args in the OS-abstracted OsString type, but execvp takes them in the
        // C ABI CString type. Use the as_bytes method from the platform-specific OsStringExt
        // extension trait to bridge the gap.
        fn osstring_to_cstring(s : OsString) -> CString {
            CString::new(s.as_bytes()).unwrap()
        }

        env::args_os().skip(1).map(osstring_to_cstring).collect()
    } else {
        // TODO: Get the user's default shell from fstab.
        vec![CString::new("/bin/bash").unwrap()]
    };

    execvp(&cmd[0], &cmd)
        .map_err(|e| format!("Executing program {:?} failed: {}", cmd, e))?;

    // We should never get here.
    unreachable!();
}

fn main() -> Result<(), String> {
    // This assumes that if we're already in an unprivileged mount namespace, it was
    // set up by this program so there is no work to do.
    if !is_in_unprivileged_mount_ns()? {
        new_mount_ns()?;

        apply_bind_mounts_from_bindfstab("/etc/bindfstab")?;
    }

    execute_main_program()
}
