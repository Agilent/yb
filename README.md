# yb (*Y*octo *B*uddy)

yb is designed to make it easy to setup and (perhaps more importantly) keep Yocto environments **up-to-date and in-sync** with your team. It is early in development, but we are releasing it now as it is already useful.

Motivation
===========

This tool was heavily inspired by [kas](https://github.com/siemens/kas), [myrepos](https://myrepos.branchable.com/), and Google's [repo](https://gerrit.googlesource.com/git-repo) tool. We are also familiar with [whisk](https://github.com/garmin/whisk).

All of these tools are great for doing initial environment setup for CI and/or new developers coming onboard. In our estimation, however, that is the easy part. The harder part is ensuring your environment stays up-to-date as your product(s) evolve through development:
* Layers get added, removed, and updated
* DISTRO and MACHINE configurations are added
* Recommended local.conf settings may drift over time: perhaps new SSTATE_MIRRORS or BB_HASHSERVE servers come on-line.

Historically, it has been painful to keep all of this in-sync, usually manifesting as emails sent team-wide everytime bblayers.conf needs to change.

Goals and non-goals
====================

yb strives to be a tool for helping developers with their everyday development tasks. Unlike kas, it does *not* enforce separation of build environment and host. yb is designed to complement the Yocto workflow you're already used to - for example, there is no `yb shell` command. You'll run the `bitbake` command as usual.

Specs and streams: keeping in-sync
==========================================

Much like kas' configuration files (see https://kas.readthedocs.io/en/latest/userguide.html), yb has **specs** which are also .yaml files. In fact, the format is nearly the same (though interoperability is not guaranteed - if that's a feature you want please open an issue). 

A basic spec looks like this:

<details>
  <summary>Basic spec (click to expand)</summary>
  
```yaml
header:
  version: 1
  name: "nightly"

repos:
  poky:
    url: "git://git.yoctoproject.org/poky"
    refspec: "honister"
    layers:
      meta:
      meta-poky:

  meta-openembedded:
    url: "git://git.openembedded.org/meta-openembedded"
    refspec: "honister"
    layers:
      meta-networking:
      meta-python:
      meta-filesystems:
      meta-webserver:
      meta-oe:
```
</details>

Specs live in **streams**. A stream is just a git repo that you've hosted somewhere accessible by your developers.

If you need to add a layer to your build, just do it in the spec and commit the change to the stream. Developers using that stream with `yb` will automatically have the stream refreshed the next time they run `yb status` or `yb sync` (see below). 

# Installation

The easiest way to install yb is to use the pre-compiled, statically-linked binary available here: https://github.com/Agilent/yb/releases/tag/0.0.11. Simply download and unpack into PATH. It should run on any modern-ish 64-bit Linux system. If you want binaries for other systems (e.g. Windows or 32-bit Linux) please file an issue.

Alternatively, you can build yb yourself. You'll need a nightly Rust compiler. To build and run, use ```cargo run -- --help``` (equivalent to doing `yb --help`).

# Basic usage

yb supports two kinds of environments ("envs" for short): vanilla Yocto and yb. You'll know you have a yb env if you see a hidden .yb/ directory inside your yocto/ directory.

## Vanilla Yocto

A vanilla Yocto env is one in which you haven't (yet) used `yb init` to initialize a yb env. In this case, yb operates with reduced functionality but can still be extremely useful. See for example the `yb status` command below. To use:

1. In a terminal, activate your Yocto env. This is usually a matter of doing `source setupsdk` or `source oe-init-build-env`. 
2. Try running `yb status`

## Converting vanilla Yocto env to yb env

To do the conversion, simply activate your Yocto env as usual and then run `yb init`:

1. `source setupsdk` or `source oe-init-build-env`
2. `yb init` (or `yb init -s PATH_TO_STREAM`)
3. Try running `yb status`

## Creating a new yb env from scratch

You can create a new yb env (and skeleton yocto/ directory) by running `yb init` outside of any existing environments:

1. Ensure you are _not_ in the context of an existing yb or vanilla Yocto env. If you are, launch a new terminal and/or cd somewhere else.
2. `yb init` (or `yb init -s PATH_TO_STREAM`)
3. cd yocto

Note that even if you pass a stream to `yb init`, no layers are cloned yet. You'll need `yb sync` for that (see below).

# Commands

## `yb activate`: set the active spec
| Vanilla Yocto env | yb env |
| ------------- | ------------- |
| :x:  | :heavy_check_mark:  |

Use this command to set the active spec. It doesn't actually make any changes to your layers or confs. You'll need `yb sync` for that (see below).

```bash
yb activate nightly
```

## `yb status`: report env status
| Vanilla Yocto env | yb env |
| ------------- | ------------- |
| :heavy_check_mark:  | :heavy_check_mark:  |

The `yb status` command was designed to provide useful status information across all the repos in your Yocto env.

Better yet, it also works in vanilla Yocto envs. As long as you run `yb status` in a terminal in which you have an activated Yocto env (i.e. `which bitbake` prints a path), yb will find the path to where your layers live and report their statuses.

| ![yb status with vanilla Yocto env](/images/yb.0.0.11.status.vanilla.gif) | 
|:--:| 
| `yb status` is run in the context of a vanilla Yocto env. |

Use the `--skip-unremarkable` / `-s` flag to hide repos for which there is no actionable status information. The result is a more concise summary.

| ![yb status with vanilla Yocto env](/images/yb.0.0.11.status.vanilla.skip.unremarkable.gif) | 
|:--:| 
| `yb status` is run in the context of a vanilla Yocto env with the `--skip-unremarkable` / `-s` flag. |

When run in the context of a yb env, however, yb can help even more. If a yb env is found, yb will fetch the current stream to see if any specs were updated. Then it will report how your current env differs from that of the activated spec.

| ![yb status with yb env](/images/yb.0.0.11.status.missing.repo.gif) | 
|:--:| 
| `yb status` is run in the context of a yb env with an activated spec. |

## `yb sync`: make my env match the active spec
| Vanilla Yocto env | yb env |
| ------------- | ------------- |
| :x:  | :heavy_check_mark:  |

`yb sync` with the `-a/--apply` flag will do what is needed to make your env reflect that of the activated spec. It currently supports these actions:
* Clone repos
* Add/remove layers from bblayers.conf (creating it first if necessary)
* Switch branches
* Do fast-forward git pull
* Create local tracking branch
* Reset working directory (only if given `--force` flag)

As a precaution, `yb sync` does nothing but report what would have been done. To actually make changes you need to pass the `-a`/`--apply` flag.

When used within a yb env, `yb sync` will first pull any stream updates.

| ![yb sync and status](/images/yb.0.0.11.sync.and.status.gif) | 
|:--:| 
| `yb sync` is first run in dry-run only mode (the default) to show what would be done. Then it is run again with `--apply`/`-a` flag. Finally, `yb status` is run to show that the env is up-to-date. |

## `yb run`: run a command for each repo
| Vanilla Yocto env | yb env |
| ------------- | ------------- |
| :heavy_check_mark:  | :heavy_check_mark:  |

This works in either yb or Yocto env. It doesn't matter what directory you run it in as long as yb can find the env.

```bash
yb run -n -- git branch --show-current
```

| ![yb run](/images/yb.0.0.11.run.show.branch.gif) | 
|:--:| 
| `yb run` using the `-no-return-codes`/`-n` flag to display just the current branch of each repo. |

Project status
==============

What's working:
* Everything described above, plus a few other utility commands (e.g. `yb list` to view specs and streams)

TODO:
- [ ] Reconstitute the auto-update feature deployed internally
- [ ] Screenshots in README!
- [ ] Support modifications to local.conf in specs
- [ ] Some kind of matrix build support (multiple MACHINE/DISTRO/?)
- [ ] Usage in CI environment
- [ ] Documentation...
- [ ] Tests
- [ ] User-friendly output for `yb sync`
- [ ] Make updating streams more robust than just a `git pull`
- [ ] Support some kind of local stream? (for now, just use a local git repo - file:// works as a URI)
- [ ] Make `--porcelain` flag more than just dumping internal structures as JSON which are liable to change; and make it work for other commands

Possible TODOs (i.e. features I probably won't use but that other tools support - submit issue reports please)
- [ ] Layer patches

Ideas:
- [ ] Maybe menuconfig like kas? 
- [ ] Multiconfig support
- [ ] Some kind of `yb stash`/`yb stash pop` (like `git stash`/`git stash pop`) across all layers/local.conf at once. Would be useful for doing a quick build in between experiementing.

Why not Python?
===============

Basically because [this](https://xkcd.com/1987/). Rust lets you build statically-linked binaries with no hassle. There is no beating that for distribution. Also, the type system and ecosystem are great.

License
========
Copyright 2022 Agilent Technologies, Inc.

This software is licensed under the MIT license.

Some portions have been adapted from [git2-rs](https://github.com/rust-lang/git2-rs) which is dual-licensed MIT and Apache 2.0. We have chosen to use it as MIT licensed.

Disclaimer
========
This is not an official Agilent product. No support is implied.
