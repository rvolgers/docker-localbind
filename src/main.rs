extern crate nix;
extern crate structopt;

use std::io;
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
use std::io::ErrorKind::{NotFound, InvalidInput, PermissionDenied};

/*

Start an interactive Docker dev container by typing ./dev.sh

In the container, type:

```
cargo run -- -v /:/tmp/testbindmount1 -v /home/user:/tmp/testbindmount1/tmp/testbindmount1
```

This spawns a new shell in a new mount (and user-) namespace which has the given bind mounts
applied.

Some more details about the pros / cons of this tool:
- You can start the container with a non-root initial uid and without CAP_SYS_ADMIN.
- You need to disable seccomp or use a profile that permits our unshare and mount calls.
  A version of the default Docker seccomp profile with the required two rules added at the
  top is provided in docker-localbind-seccomp-profile.json.
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
        .map_err(|e| format!("Unshare failed (try running 'localbind -t' to diagnose): {}", e))?;

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
        .map_err(|e| format!("Write /pro/self/gid_map failed: {}", e))?;

    // At this point we are a "normal" user but we still have a full set of capabilities in the
    // the new user namespace, which allows us to perform mount operations.
    // The capabilities will be automatically dropped as soon as we exec a new executable.

    // This println shows we have full capabilities but the inheritable capability set is empty.
    // If we do 'cat /proc/self/status' in the spawned shell the permitted and effective sets
    // will also be empty.
    // See also: https://www.kernel.org/doc/html/latest/security/credentials.html
    //println!("{}", fs::read_to_string("/proc/self/status").unwrap());

    Ok(())
}

fn test_config() {

    println!("Checking kernel.unprivileged_userns_clone sysctl:\n{}\n", 
        match fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone") {
            Ok(ref s) if s == "1\n" => format!("Ok (value is '1')"),
            Ok(ref s) => format!("Problem! (value is '{}', localbind requires '1')", s.trim()),
            Err(ref e) if e.kind() == NotFound => format!("Ok (setting appears to be absent)"),
            Err(ref e) => format!("Unknown (error: {})", e),
        });

    println!("Checking kernel.userns_restrict sysctl:\n{}\n",
        match fs::read_to_string("/proc/sys/kernel/userns_restrict") {
            Ok(ref s) if s == "0\n" => format!("Ok (value is '0')"),
            Ok(ref s) => format!("Problem! (value is '{}', localbind requires '0')", s.trim()),
            Err(ref e) if e.kind() == NotFound => format!("Ok (setting appears to be absent)"),
            Err(ref e) => format!("Unknown (error: {})", e),
        });

    // TODO could add check that dir /sys/kernel/security/apparmor exists
    println!("Checking apparmor profile:\n{}\n",
        match fs::read_to_string("/proc/self/attr/current") {
            Ok(ref s) if s == "unconfined\n" =>
                format!("Ok (unconfined, please consider using localbind's provided profile)"),
            Ok(ref s) if s.contains("localbind") =>
                format!("Ok ('{}' contains 'localbind', so assuming it's localbind's provided profile)", s.trim()),
            Ok(ref s) if s.contains("docker-default") =>
                format!("Problem! ('{}' sounds like it might be docker's default profile, which won't work. try localbind's provided profile.)", s.trim()),
            Ok(ref s) =>
                format!("Unknown (unrecognized profile name '{}', is apparmor in use or is this some other security module like selinux?)", s.trim()),
            Err(ref e) if e.kind() == NotFound =>
                format!("Probably ok (apparmor not detected)"),
            Err(ref e) =>
                format!("Unknown (error: {})", e),
        });

    fn nix_error_to_io_error(e: nix::Error) -> io::Error {
        match e {
            nix::Error::Sys(errno) => io::Error::from(errno),
            _ => io::Error::new(io::ErrorKind::Other, e),
        }
    }

    // We're depending on the kernel checking this flag before checking permissions.
    // Hence, if we get EPERM it's probably due to seccomp filtering.
    let bad_flags = MsFlags::MS_NOUSER;
    let bad_flags_mount_err = mount::<str, str, str, str>(None, "/", None, bad_flags, None)
        .map_err(nix_error_to_io_error)
        .expect_err("intentionally invalid mount succeeded");

    // We're depending on the kernel checking the existence of this path before checking permissions.
    // Hence, if we get EPERM it's probably due to seccomp filtering.
    let flags = MsFlags::MS_BIND | MsFlags::MS_REC;
    let bad_path = "/this_is_a_path_that_definitely_doesn't_exist_1c7c166191362314f6e5a06a0c07603c";
    let bad_path_mount_err = mount::<str, str, str, str>(Some(bad_path), bad_path, None, flags, None)
        .map_err(nix_error_to_io_error)
        .expect_err("intentionally invalid mount succeeded");

    println!("Checking seccomp profile:\n{}\n", match (bad_flags_mount_err.kind(), bad_path_mount_err.kind()) {
        (PermissionDenied, PermissionDenied) =>
            format!("Problem? (default seccomp profile might be in use, which won't work. try localbind's provided profile.)"),
        (PermissionDenied, NotFound) =>
            format!("Ok (localbind's provided seccomp profile appears to be in use)"),
        (InvalidInput, NotFound) =>
            format!("Ok (possibly unconfined, please consider using localbind's provided profile)"),
        (ref a, ref b) =>
            format!("Unknown (unexpected error codes ({:?} / {:?}). please use localbind's provided profile for best results.)", a, b),
    });
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
        .map_err(|e| format!("Could not bind-mount {:?} over {:?}: {}. If this was a permission error, try running 'localbind -t' to diagnose.", src, dest, e))
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

// OsStr is a platform-independent abstraction (on Windows it's an UTF-16 string).
// But on Unix we can use the OsStrExt trait to interoperate with a raw byte string easily.
fn split_fields(s: &OsStr) -> impl Iterator<Item=&OsStr> {
    s.as_bytes().split(|b| *b == b':').map(OsStr::from_bytes)
}

impl TryFrom<&OsStr> for VolumeSpec {
    type Error = String;
    fn try_from(s: &OsStr) -> Result<VolumeSpec, String> {
        let fields: Vec<_> = split_fields(s).collect();
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
    /// Specify a bind mount in the format srcpath:destpath.
    /// This option can be specified multiple times.
    #[structopt(short = "v", long = "volume", parse(try_from_os_str="volume_spec_from_os_string"), number_of_values=1, value_name="mountspec")]
    mounts: Vec<VolumeSpec>,

    #[structopt(parse(from_os_str))]
    cmd: Vec<OsString>,

    #[structopt(short = "t", long = "test-config")]
    test_config: bool,
}

fn main() -> Result<(), String> {
    let opt = Opt::from_args();

    if opt.test_config {
        test_config();
        return Ok(());
    }

    // This assumes that if we're already in an unprivileged mount namespace, it was
    // set up by this program so there is no work to do.
    if !is_in_unprivileged_mount_ns()? {
        new_mount_ns()?;

        for spec in opt.mounts {
            do_mount(&spec)?;
        }
    }

    if opt.cmd.len() == 0 {
        // TODO Use the user's default shell instead? Or the value of $SHELL?
        execute_main_program(&vec![OsString::from("/bin/bash")])?;
    } else {
        execute_main_program(&opt.cmd)?;
    }

    Ok(())
}
