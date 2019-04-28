# localbind

Perform local bind mounts inside your Docker container, while diverging as little from proper security practices as possible.

The only requirement is that you use a seccomp profile with two extra rules (provided in this repo, see `dev.sh` for an example). No extra capabilities are needed, and you can have a non-root user as the default user for your container.

Because this tool uses unprivileged mount namespaces, it is not able to affect the container globally. It starts a new shell or program, inside which the requested bind mounts will be available. Another side effect is that all user and group names except for those of the current user will appear as "nouser" and "nogroup" in the output of commands like `ls`.

## Obtaining the binary

Run ./dev.sh to start a Docker development environment. Inside, type `cargo build --release`. The binary is in `./target/release/localbind`, both inside and outside the container (the source directory is mounted in the container).

The release binary can be made significantly smaller by running `strip ./target/release/localbind`.

At some point I would like to make a Docker image available that only contains the binary, for easy use as a source image in multistage Docker builds.

## Usage

### Configuring your Docker host

In the ideal case you won't have to change anything on your Docker host. However, there are some things that can prevent `localbind` from working. If you run `localbind -t` inside a container it will tell you what's wrong.

- If Docker uses AppArmor on your system, you will have to install the provided `./profiles/docker-localbind-apparmor-profile` to `/etc/apparmor.d` (or you can load it temporarily using `sudo apparmor_parser -r -W ./profiles/docker-localbind-apparmor-profile`).
- If unprivileged user namespaces are blocked by a sysctl setting on your system, you will have to install *one* of the two provided `profiles/00docker-localbind-*.conf` files to `/etc/sysctl.d`, depending on which of the two settings is used by your system (or you can set it temporarily using `sudo sysctl -w` or by writing the desired value to the file under `/proc/sys/kernel/`). **Before you do this, please carefully read the "Security" section below.**

### Configuring your container

You must start your container with the provided seccomp profile (or no seccomp profile at all, but this is very much not recommended). If your system uses AppArmor you will additionally have to load the AppArmor profile (see above for how to make it available on your Docker host). Alternatively you can disable apparmor confinement, which is not recommended, but probably less of a terrible idea than disabling seccomp.

For `docker run` or `docker start` you can use:

```
--security-opt seccomp="/path/to/the/file/docker-localbind-seccomp-profile.json"
--security-opt apparmor=docker_localbind
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

## Notes on security

This section tries to shed some light on the security impact of `docker-localbind` and the configuration it requires. This advice is provided in the hope that it will be a useful starting point for evaluating the suitablity of `docker-localbind` for your purposes, but like the rest of `docker-localbind` it is offered with ABSOLUTELY NO WARRANTY of correctness or suitability for any purpose.

### About unprivileged user namespaces

Some systems have unprivileged user namespaces disabled by default, notably Debian. The reason is that the ability for otherwise unprivileged users to create a user namespace allows them to access a lot of extra functionality in the Linux kernel. This functionality is likely to still contain some bugs that enable a user to escalate their privileges, since it was only accessible to super users for most of the Linux kernel's development history.

Docker tries hard to be secure on systems that permit unprivileged user namespaces. It does this mainly by blocking access to all / nearly all of this functionality through its seccomp filter. This means that if you allow unprivileged user namespaces it becomes more important to keep Docker's seccomp filtering enabled for untrusted containers. It is also important not to run untrusted containers with extra capabilities or privileges. This is obviously true in general, but in regards to seccomp it also causes Docker to disable some of the seccomp filters to permit common use cases for those extra capabilities and privileges.

The remaining impact is that users on your system who do not already have root or root-equivalent access (such as `sudo` rights, being able to use `docker` directly, or controlling privileged containers) have an increased chance of being able to use future Linux Kernel 0-day exploits. I suspect on most systems running Docker all the local user accounts already have root-equivalent access, so enabling unprivileged user namespaces changes little in that regard. But this is a tradeoff people have to make for themselves.

### About the seccomp profile

The seccomp profile is derived from [Docker's default seccomp profile](https://github.com/moby/moby/blob/master/profiles/seccomp/default.json). Two additional rules have been added to the policy:
- An `unshare` system call where the flags argument is exactly equal to `CLONE_NEWUSER | CLONE_NEWNS` is permitted. Such a call simultaneously creates a new user namespace and a new mount namespace.
- A `mount` system call where the flags argument is exactly equal to `MS_BIND | MS_REC` is permitted. Such a call performs a private recursive bind mount operation. "Recursive" means all mounts under the source path will be bind-mounted along with the source path. "Private" means future mounts and unmounts in either the source or destination path will not affect the other one.

### About the apparmor profile

The apparmor profile is derived from [the template used to generate Docker's default apparmor profile](https://github.com/moby/moby/blob/master/profiles/apparmor/template.go), with the following changes:
- Remove the need for templating by filling in the proper `#include` statements, the name of the apparmor profile itself (`docker_localbind`) and the name of the apparmor profile used by `dockerd` (the Docker daemon itself, not the containers), which is usually `unconfined`.
- Remove an explicit blanket "deny" rule that prohibits all mount operations. "Deny" is already the default, and since "deny" takes precedence over "allow" it would otherwise not be possible to make an exception to this rule.
- Add a rule that permits private, recursive bind mounts.

### About the bind mount functionality itself

Overall, `docker-localbind` takes pains to only use functionality offered by the Linux kernel to any unprivileged process (at least in some distributions). This greatly simplifies the security analysis, and for this reason I am not aware of any way the core functionality of the tool could have a negative security impact.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
