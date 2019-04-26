# localbind

Perform local bind mounts inside your Docker container, while diverging as little from proper security practices as possible.

The only requirement is that you use a seccomp profile with two extra rules (provided in this repo, see `dev.sh` for an example). No extra capabilities are needed, and you can have a non-root user as the default user for your container.

Because this tool uses unprivileged mount namespaces, it is not able to affect the container globally. It starts a new shell or program, inside which the requested bind mounts will be available. Another side effect is that all user and group names except for those of the current user will appear as "nouser" and "nogroup" in the output of commands like `ls`.

## Obtaining the binary

Run ./dev.sh to start a Docker development environment. Inside, type `cargo build --release`. The binary is in `./target/release/localbind`, both inside and outside the container (the source directory is mounted in the container).

The release binary can be made significantly smaller by running `strip ./target/release/localbind`.

At some point I would like to make a Docker image available that only contains the binary, for easy use as a source image in multistage Docker builds.

## Usage

### Configuring your system

The system that Docker runs on must have unprivileged user namespaces enabled. Systems where this is known to already be the case include Ubuntu and Docker for Mac. Systems where this is known to be restricted by default include Debian.

A system that has the Debian-style restriction in place looks like this:

```
$ /sbin/sysctl kernel.unprivileged_userns_clone
kernel.unprivileged_userns_clone = 0
```

If the value is 0 this will prevent localbind from working. If it does not exist or has the value 1 it is ok.

Some systems also might have the following setting that restricts unprivileged user namespaces:

```
$ /sbin/sysctl kernel.userns_restrict
kernel.userns_restrict = 1
```

If the value is 1 this will prevent localbind from working. If it does not exist or has the value 0 it is ok. Note that this is the opposite of the previous setting.

To change these settings you have to add a configuration in `/etc/sysctl.d/` that sets it to the desired value at boot. You can use `sysctl -w` to set it as well, but this will be lost after a reboot.

Whether it is a good idea to change this setting depends a lot on what kind of system it is. For a Docker host or private machine I don't think it matters a lot.
- This setting does not affect the security of Docker containers running without extra capabilities and with the default seccomp profile (besides a loss of defense in depth) because the default seccomp profile blocks all syscalls that make use of this feature. (The seccomp profile shipped with localbind only permits the use of user- and mount namespaces, and it restricts the use of the mount syscall to bind mounts only. Notably, it does not allow creating network namespaces or mounting arbitrary filesystems.)
- This setting does not affect the security of privileged Docker containers, because they already have much greater access than changing this setting can provide.

### Configuring your container

You must start your container with the provided seccomp profile (or no seccomp profile at all, but this is not recommended).

For `docker run` or `docker start` you can use:

```
--security-opt seccomp="/path/to/the/file/docker-localbind-seccomp-profile.json"
```

For `docker-compose` you can specify it [in your compose file](https://docs.docker.com/compose/compose-file/#security_opt).

### Using `localbind` in your container

The following will run `npm start`. The `npm` process and all its children will see `/tmp/node_modules` mounted over `/src/node_modules`. This bind mount will not be visible in other processes.

`localbind -v /tmp/node_modules:/src/node_modules npm start`

You may specify the `-v` argument multiple times to apply multiple bind mounts. If you do not specify a command, a shell is spawned instead. This can be confusing, since the command prompt looks the same as the normal shell (which you can return to by typing "exit").

NOTE: Currently invoking `localbind` from inside a process spawned by `localbind` currently runs the requested program but silently does *not* apply your mounts. This is considered a bug and will be changed at some point.

## Alternatives

If you start a container with seccomp disabled, the `CAP_SYS_ADMIN` capability, and running as the "root" user inside the container, you can just use the normal "mount" command to perform bind mounts.

Hopefully one day Docker will have native support for local bind mounts. See this [GitHub issue](https://github.com/moby/moby/issues/39134) for more details.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
