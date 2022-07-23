use std::{
    env::{self, ArgsOs},
    error::Error,
    ffi::{CString, OsString},
    fmt::Display,
    fs,
    os::unix::prelude::OsStrExt,
};

use nix::{
    errno::Errno,
    mount::{self, mount, umount2, MntFlags, MsFlags},
    sched::{unshare, CloneFlags},
    sys::stat::Mode,
    unistd::{chdir, execv, getgid, getuid, mkdir, pivot_root},
};

#[derive(Debug)]
enum Filesystem {
    Bind {
        src: OsString,
    },
    Tmpfs,
    Overlay {
        lower: OsString,
        upper: OsString,
        work: OsString,
    },
    Generic {
        fstype: OsString,
        src: OsString,
        data: OsString,
    },
}

impl Filesystem {
    fn to_args(
        &self,
    ) -> (
        Option<OsString>,
        Option<OsString>,
        mount::MsFlags,
        Option<OsString>,
    ) {
        use Filesystem::*;

        match self {
            Bind { src } => (
                Some(src.clone()),
                None,
                MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_PRIVATE,
                None,
            ),
            Tmpfs => (None, Some("tmpfs".into()), MsFlags::empty(), None),
            Overlay { lower, upper, work } => {
                let mut data = OsString::new();
                data.push("lowerdir=");
                data.push(lower);
                data.push(",upperdir=");
                data.push(upper);
                data.push(",workdir=");
                data.push(work);
                (
                    Some("overlay".into()),
                    Some("overlay".into()),
                    MsFlags::empty(),
                    Some(data),
                )
            }
            Generic { fstype, src, data } => (
                Some(src.clone()),
                Some(fstype.clone()),
                MsFlags::empty(),
                Some(data.clone()),
            ),
        }
    }

    pub fn to_source(&self) -> Option<OsString> {
        self.to_args().0
    }
    pub fn to_fstype(&self) -> Option<OsString> {
        self.to_args().1
    }
    pub fn to_flags(&self) -> mount::MsFlags {
        self.to_args().2
    }
    pub fn to_data(&self) -> Option<OsString> {
        self.to_args().3
    }
}

#[derive(Debug)]
enum MountAction {
    Mount { fs: Filesystem, dest: OsString },
    Mkdir { dest: OsString },
    PivotRoot { dest: OsString },
}

#[derive(Debug)]
struct Action {
    actions: Vec<MountAction>,
    exec_path: OsString,
    exec_args: Vec<OsString>,
}

#[derive(Debug)]
struct ArgsError(String);

impl Error for ArgsError {}

impl Display for ArgsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgsError(msg) => writeln!(f, "Error: {}", msg),
        }
    }
}

fn parse_args(args: ArgsOs) -> Result<Action, ArgsError> {
    let mut args = args.peekable();
    let mut actions = Vec::new();

    args.next().ok_or(ArgsError("No argv0".to_string()))?;

    loop {
        match args.peek() {
            Some(arg) if arg.to_string_lossy().starts_with("-") => {
                use Filesystem::*;
                use MountAction::*;

                let arg = arg.clone();

                let mut go = || {
                    args.next()
                        .ok_or(ArgsError(format!("Missing parameters for {:?}", arg)))
                };

                if arg == "--" {
                    args.next();
                    break;
                } else if arg == "--bind" {
                    let (_, src, dest) = (go()?, go()?, go()?);
                    actions.push(Mount {
                        fs: Bind { src },
                        dest,
                    })
                } else if arg == "--tmpfs" {
                    let (_, dest) = (go()?, go()?);
                    actions.push(Mount { fs: Tmpfs, dest })
                } else if arg == "--overlayfs" {
                    let (_, lower, upper, work, dest) = (go()?, go()?, go()?, go()?, go()?);
                    actions.push(Mount {
                        fs: Overlay { lower, upper, work },
                        dest,
                    })
                } else if arg == "--fs" {
                    let (_, fstype, src, data, dest) = (go()?, go()?, go()?, go()?, go()?);
                    actions.push(Mount {
                        fs: Generic { fstype, src, data },
                        dest,
                    })
                } else if arg == "--mkdir" {
                    let (_, dest) = (go()?, go()?);
                    actions.push(Mkdir { dest })
                } else if arg == "--pivot-root" {
                    let (_, dest) = (go()?, go()?);
                    actions.push(PivotRoot { dest })
                } else {
                    return Err(ArgsError(format!("Unknown parameter {:?}", arg)));
                }
            }
            Some(_) => break,
            None => break,
        }
    }

    let exec_path = args
        .next()
        .ok_or(ArgsError("Missing path to program".to_string()))?;
    let exec_args = args.collect();

    Ok(Action {
        actions,
        exec_path,
        exec_args,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let actions = parse_args(env::args_os())?;

    let uid = getuid();
    let gid = getgid();

    unshare(CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWUSER)?;

    fs::write("/proc/self/setgroups", "deny")?;
    fs::write("/proc/self/uid_map", format!("{} {} 1", uid, uid))?;
    fs::write("/proc/self/gid_map", format!("{} {} 1", gid, gid))?;

    for action in actions.actions {
        use MountAction::*;

        println!("Running {:?}", action);

        match action {
            Mount { fs, dest } => mount(
                fs.to_source().as_deref(),
                dest.as_os_str(),
                fs.to_fstype().as_deref(),
                fs.to_flags(),
                fs.to_data().as_deref(),
            )?,
            Mkdir { dest } => match mkdir(dest.as_os_str(), Mode::from_bits_truncate(0o755)) {
                Ok(()) => (),
                Err(Errno::EEXIST) => (),
                Err(err) => Err(err)?,
            },
            PivotRoot { dest } => {
                chdir(dest.as_os_str())?;
                pivot_root(".", ".")?;
                umount2(".", MntFlags::MNT_DETACH)?;
            }
        }
    }

    execv(
        &CString::new(actions.exec_path.as_bytes()).unwrap(),
        &actions
            .exec_args
            .iter()
            .map(|x| CString::new(x.as_bytes()).unwrap())
            .collect::<Vec<_>>(),
    )?;

    Ok(())
}
