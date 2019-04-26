extern crate nix;
extern crate structopt;

use nix::mount::{mount, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getuid, getgid};
use std::process::Command;
use std::os::unix::process::CommandExt;
use std::fs;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::ffi::{OsString, OsStr};
use std::os::unix::ffi::OsStrExt;
use structopt::StructOpt;

/*

Start an interactive dev container by typing ./dev.sh

In the container, try:

cargo run -- -v /:/tmp/testbindmount1 -v /home/user:/tmp/testbindmount1/tmp/testbindmount1

Some more details about the pros / cons of this tool:
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

fn do_mount(VolumeSpec {src, dest}: &VolumeSpec) -> Result<(), String> {

    fs::create_dir_all(dest)
        .map_err(|e| format!("Could not create bind mount destination {:?}: {}", dest, e))?;

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

    mount::<PathBuf, PathBuf, str, str>(Some(src), dest, None, flags, None)
        .map_err(|e| format!("Could not bind-mount {:?} over {:?}: {}", src, dest, e))
}

fn execute_main_program(cmd: &Vec<OsString>) -> Result<(), String> {

    let e = Command::new(&cmd[0]).args(cmd.iter().skip(1)).exec();

    Err(format!("Executing program {:?} failed: {}", cmd, e))
}

#[derive(Debug)]
struct VolumeSpec {
    src: PathBuf,
    dest: PathBuf,
}

impl TryFrom<&OsStr> for VolumeSpec {
    type Error = String;
    fn try_from(s: &OsStr) -> Result<VolumeSpec, String> {
        let fields: Vec<_> = s.as_bytes()
            .split(|b| *b == b':')
            .map(OsStr::from_bytes)
            .collect();
        match fields.as_slice() {
            [src, dest] => Ok(VolumeSpec {src: src.into(), dest: dest.into()}),
            _ => Err(format!("Volume specification should contain exactly two fields: {:?}", s))?,
        }
    }
}

fn volume_spec_from_os_string(s: &OsStr) -> Result<VolumeSpec, OsString> {
    Ok(VolumeSpec::try_from(s)?)
}

/// Run programs inside an environment with various local bind mounts configured.
#[derive(StructOpt, Debug)]
#[structopt(name = "localbind")]
struct Opt {
    /// Activate debug mode
    #[structopt(short = "d", long = "debug")]
    debug: bool,

    /// Specify a bind mount
    #[structopt(short = "v", long = "volume", parse(try_from_os_str="volume_spec_from_os_string"), number_of_values=1, value_name="mountspec")]
    mounts: Vec<VolumeSpec>,

    #[structopt(parse(from_os_str))]
    cmd: Vec<OsString>,
}

fn main() -> Result<(), String> {
    let opt = Opt::from_args();
    
    // This assumes that if we're already in an unprivileged mount namespace, it was
    // set up by this program so there is no work to do.
    if !is_in_unprivileged_mount_ns()? {
        new_mount_ns()?;

        for spec in opt.mounts {
            do_mount(&spec)?;
        }
    }

    if opt.cmd.len() == 0 {
        execute_main_program(&vec![OsString::from("/bin/bash")])?;
    } else {
        execute_main_program(&opt.cmd)?;
    }

    Ok(())
}
