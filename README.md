# `binderlay`

*Work in progress. Breaking changes will occur without notice.*

Playground for mount namespaces, bind mounts, OverlayFS, etc.

## Nix flake

This repository contains a Nix Flake. To refer to it, use the following Flake URL:

```
github:dramforever/binderlay
```

### Running `binderlay` as a flake

```console
$ nix run github:dramforever/binderlay -- <arguments>
```

## What does it do?

```console
$ binderlay <operations> [--] <program> <argv0> [<args...>]
```

`binderlay` will:

- Call `unshare(2)` to move itself into a new user namespace and mount namespace.
- Map the executing user's own uid and gid to be the same as the original user namespace.
- Perform operations within the mount namespace as listed
- `execv` another program with the listed argument list

Each operation is specified using several command line parameters. A lone `--` terminates the operation list. The following operations are available

- `--bind <src> <dest>`: Bind mount `src` to `dest`
- `--tmpfs <dest>`: Mount a `tmpfs` on `dest`
- `--overlayfs <lower> <upper> <work> <dest>`: Mount an OverlayFS on `dest`, with `lowerdir=<lower>,upperdir=<upper>,workdir=<work>`
- `--fs <type> <src> <options> <dest>`: Mount a filesystem on `dest`. Similar to `mount -t <type> -o <options> <src> <dest>`
- `--mkdir <dest>`: `mkdir` the directory `<dest>`. Currently will not also make parent directories.
- `--pivot-root <dest>`: Perform a `pivot_root(2)` and `chroot` into `dest`

An example that runs `/usr/bin/bash` in a chroot with only `/usr`:

```bash
binderlay \
    --mkdir /tmp/work \
    --tmpfs /tmp/work \
    --mkdir /tmp/work/usr \
    --bind /usr /tmp/work/usr \
    --pivot-root /tmp/work \
    /usr/bin/bash bash
```
