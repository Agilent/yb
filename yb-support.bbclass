# Author: Chris Laplante <chris.laplante@agilent.com>
#
# This bbclass maintains the build history data that backs the
# yb (https://github.com/Agilent/yb) "twice-bake" subcommand. One day,
# it might do more, hence the generic name.
#
# Ideally, yb should be able to operate in any Yocto environment without
# advance configuration. This build history support could/should? be folded
# into the 'buildstats' bbclass in poky and upstreamed. But that's a lot of
# work I don't feel like doing right now. So I # took the cheaters way out
# and wrote this small bbclass.

YB_SUPPORT_BASEDIR = "${TMPDIR}/yb-support"
YB_BUILD_HISTORY_DIR = "${YB_SUPPORT_BASEDIR}/history"

addhandler yb_support_eventhandler
yb_support_eventhandler[eventmask] = " \
    bb.build.TaskFailed \
    bb.build.TaskStarted \
    bb.build.TaskSucceeded \
    bb.event.BuildStarted \
"

# BitBake hashing seems to account for the presence or absence of event
# handlers, but not the actual contents of the code. So there is no
# obvious way to force things to rebuild when, e.g., our history format
# changes. The best we can do (without playing crazy games like injecting
# vardeps flags into every task in the task graph) is to just manually
# keep track of our build history format. We write it to every json file.
# The yb tool itself will be resposible for telling the user about version
# mismatches.
YB_BUILD_HISTORY_VERSION = "2"

python yb_support_eventhandler() {
    import json

    support_dir = None
    bn = d.getVar("BUILDNAME")
    if bn is not None:
        support_dir = os.path.join(d.getVar("YB_BUILD_HISTORY_DIR"), bn)
        bb.utils.mkdirhier(support_dir)

    if isinstance(e, bb.event.BuildStarted):
        return

    task_dir = os.path.join(support_dir, d.getVar("PF"))
    bb.utils.mkdirhier(task_dir)
    task_file = os.path.join(task_dir, "{0}.json".format(e.task))

    data = {}
    try:
        with open(task_file, "r") as f:
            data = json.loads(f.read())
    except FileNotFoundError:
        # Seed initial data
        prefuncs = (d.getVarFlag(e.task, 'prefuncs', expand=True) or '').split()
        postfuncs = (d.getVarFlag(e.task, 'postfuncs', expand=True) or '').split()

        runfmt = d.getVar('BB_RUNFMT') or "run.{func}.{pid}"
        pid = os.getpid()

        # Get the name of the run files
        task_runfile = runfmt.format(func=d.getVar("BB_RUNTASK"), pid=pid)
        prefunc_runfiles = [ (f, runfmt.format(func=f, pid=pid)) for f in prefuncs ]
        postfunc_runfiles = [ (f, runfmt.format(func=f, pid=pid)) for f in postfuncs ]

        data = {
            "PN": e.pn,
            "PV": e.pv,
            "T": d.getVar("T"),
            "WORKDIR": d.getVar("WORKDIR"),
            "log_file": e.logfile,
            "mc": e._mc,
            "postfunc_runfiles": postfunc_runfiles,
            "prefunc_runfiles": prefunc_runfiles,
            "task_file": e.taskfile,
            "task_runfile": task_runfile,
            "task": e.task,
            "class_version": d.getVar("YB_BUILD_HISTORY_VERSION")
        }

    if isinstance(e, bb.build.TaskStarted):
        data["start_time"] = e.time

    if isinstance(e, bb.build.TaskFailed):
        data["end_time"] = e.time
        data["outcome"] = "FAIL"

    if isinstance(e, bb.build.TaskSucceeded):
        data["end_time"] = e.time
        data["outcome"] = "SUCCESS"

    with open(task_file, "w") as f:
        f.write(json.dumps(data, indent=4, sort_keys=True))
}

