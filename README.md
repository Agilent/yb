# yb (*Y*octo *B*uddy)

yb is designed to make it easy to setup and (perhaps more importantly) keep Yocto environments **up-to-date and in-sync** with your team. It is early in development, but we are releasing it now as it is already useful in certain workflows.

Motivation
===========

This tool was heavily inspired by [kas](https://github.com/siemens/kas), [myrepos](https://myrepos.branchable.com/), and Google's [repo](https://gerrit.googlesource.com/git-repo) tool. We are also familiar with [whisk](https://github.com/garmin/whisk).

All of these tools are great for doing initial environment setup for CI and/or new developers coming onboard. In our estimation, however, that is the easy part. The harder part is ensuring your environment stays up-to-date as your product(s) evolve through development:
* Layers get added, removed, and updated
* DISTRO and MACHINE configurations are added
* Recommended local.conf settings may drift over time: perhaps new SSTATE_MIRRORS or BB_HASHSERVE servers come on-line.

Historically, it has been painful to keep all of this in-sync. 

License
========
Copyright 2022 Agilent Technologies, Inc.

This software is licensed under the MIT license.

Some portions have been adapted from [git2-rs](https://github.com/rust-lang/git2-rs) which is dual-licensed MIT and Apache 2.0. We have chosen to use it as MIT licensed.

Disclaimer
========
This is not an official Agilent or Synopsys product. No support is implied.
