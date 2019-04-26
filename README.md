# localbind

Perform local bind mounts inside your Docker container, while diverging as little from proper security practices as possible.

The only requirement is that you use a seccomp profile with two extra rules (provided in this repo, see `dev.sh` for an example). No extra capabilities are needed, and you can have a non-root user as the default user for your container.

Because this tool uses unprivileged mount namespaces, it is not able to affect the container globally. It starts a new shell or program, inside which the requested bind mounts will be available. Another side effect is that all user and group names except for those of the current user will appear as "nouser" and "nogroup" in the output of commands like `ls`.

## Obtaining the binary

Run ./dev.sh to start a Docker development environment. Inside, type `cargo build --release`. The binary is in `./target/release/localbind`, both inside and outside the container (the source directory is mounted in the container).

The release binary can be made significantly smaller by running `strip ./target/release/localbind`.

At some point I would like to make a Docker image available that only contains the binary, for easy use as a source image in multistage Docker builds.

## Usage

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
