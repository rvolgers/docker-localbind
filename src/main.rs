extern crate nix;

use nix::mount::{mount, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::execv;
use std::fs::create_dir_all;
use std::ffi::CString;

/*
Problem:
- All uid's / gid's look like "nobody"
- We can install a single mapping, and make ourselves show up as "root" or as the normal user.
- Even this remains TODO.


--- Below this line is before above problem.

Way forward:
- Can't reveal over-mounted dirs, so they must be available some other way
    - Move them out of the way in the Docker build phase
        - Check if this is actually still slow if done in same build step as 'npm ci'
- After that I think there are no more problems?
    - Just need to do the mounts in the right places and exec a new process
    - The exec will / should drop the extra capabilities we have
- We could even make this an LD_PRELOAD lib (or similar) to have it done automatically
    - (Unless a parent process has done it already of course.)
    - But, this is kind of ugly and breaks if there are any statically linked processes.

What is the end result:
- We can start the container as a normal user and without CAP_SYS_ADMIN.
    - But, we now need to permit 'unshare' in the seccomp profile, instead of just mount.
- We now need to move the pre-baked node_modules dirs out of the way, which is a shame.
- Instead of being done container-wide, the mounts are only visible inside processes run
  with this wrapper tool.
- Still seems like an improvement overall.

Style nits:
- Proper rust style wants us to use a more structured error type than just a String.

*/

fn new_mount_ns() -> Result<(), String> {

    // A new user namespace is needed because otherwise we do not have permission to create
    // a mount namespace.
    let flags = CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS;

    unshare(flags).map_err(|e| format!("unshare failed: {}", e))?;

    // TODO install the uid/gid mappings

    Ok(())
}

fn bind_mount(src: &str, dest: &str) -> Result<(), String> {

    create_dir_all(dest)
    .map_err(|e| format!("Could not create bind mount destination {}: {}", dest, e))?;

    /*
    The mount(2) manpage says the following, which implies we can't use unpriviliged user
    namespaces to reveal the original node_modules directories:

    EINVAL
    In an unprivileged mount namespace (i.e., a mount namespace owned by a user namespace that
    was created by an unprivileged user), a bind mount operation (MS_BIND) was attempted without
    specifying (MS_REC), which would have revealed the filesystem tree underneath one of the
    submounts of the directory being bound.
    */
    let flags = MsFlags::MS_BIND | MsFlags::MS_REC;

    mount::<str, str, str, str>(Some(src), dest, None, flags, None)
    .map_err(|e| format!("Could not bind-mount {} over {}: {}", src, dest, e))
}

fn main() -> Result<(), String> {
    new_mount_ns()?;

    bind_mount("/", "/home/user/container")?;

    // TODO make this less ugly.
    execv(&CString::new("/bin/bash").unwrap(), &[CString::new("/bin/bash").unwrap()]).expect("exec");

    Ok(())
}
