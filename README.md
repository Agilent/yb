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

# Quickstart

yb supports two kinds of environments: bare Yocto and yb.

## Bare Yocto

A bare Yocto environment is one in which you aren't using any specs or streams. In this case, yb operates with reduced functionality but can still be extremely useful. See for example the `yb status` command below. To use:

1. In a terminal, activate your Yocto environment. This is usually a matter of doing `source setupsdk` or `source oe-init-build-env`. 
2. Run `yb init`
3. Try running `yb status` 

## yb environment

A yb environment is created by providing a stream URI to `yb init`. If you do it within an activated Yocto environment, then the yb environment is constructed within the Yocto environment. Otherwise, a new skeleton yocto/ directory is created and the environment created within that. 

For example:

```bash
# This assumes you're not within a Yocto environment, i.e. typing 'bitbake' gives command not found error
yb init -s git@github.com:my-company/our-streams.git
cd yocto
```

# Commands

## `yb activate`: set the active spec

Use this command to set the active spec. It doesn't actually make any changes to your layers or confs. You'll need `yb sync` for that (see below).

```bash
yb activate nightly
```

## `yb sync`: make my environment match the active spec

`yb sync` with the `-a/--apply` flag will do what is needed to make your environment reflect that of the activated spec. It currently supports these actions:
* Clone repos
* Add/remove layers from bblayers.conf (creating it first if necessary)
* Switch branches
* Do fast-forward git pull
* Create local tracking branch
* Reset working directory (only if given `--force` flag)

As a precaution, `yb sync` does nothing but report what would have been done. To actually make changes you need to pass the `-a`/`--apply` flag.

When used within a yb environment, `yb sync` will first pull any stream updates.

## `yb status`: report environment status

The `yb status` command was designed to provide useful status information across all the repos in your Yocto environment. 

Better yet, it works even with vanilla Yocto environments (i.e. ones in which you haven't used `yb init`). As long as you run `yb status` in a terminal in which you have an activated Yocto environment (i.e. `which bitbake` prints a path), yb will find the path to where your layers live and report their statuses.

When run in the context of a yb environment, however, yb can help even more. If a yb environment is found, yb will fetch the current stream to see if any specs were updated. Then it will report how your current environment differs from that of the activated spec.

## `yb run`: run a command for each repo
 
This works in either yb or Yocto environments. 

```bash
yb run -- git status -s
```

Project status
==============

What's working:
* Everything described above, plus a few other utility commands (e.g. `yb list` to view specs and streams)

TODO:
- [ ] Reconstitute the auto-update feature deployed internally
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

Building
========

This software requires a nightly Rust compiler.  

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
