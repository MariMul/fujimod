"""Host browser RSS and macOS physical charge, sampled every 100 ms.

This measures the headed host browser, not iPhone hardware. WebKit XPC
processes may be launchd children; include new processes in this browser bundle
and exclude those present before the run. Run one instance of the bundle at a
time. RSS, physical charge and Wasm capacity are distinct measurements.
"""
import asyncio
import ctypes
from pathlib import Path
import sys

async def processes():
    process = await asyncio.create_subprocess_exec(
        'ps', '-axo', 'pid=,ppid=,rss=,comm=', stdout=asyncio.subprocess.PIPE)
    stdout, _ = await process.communicate()
    return [(int(pid), int(parent), int(rss)*1024, command)
            for pid,parent,rss,command in (line.split(None,3) for line in stdout.decode().splitlines())]


if sys.platform == 'darwin':
    _proc = ctypes.CDLL('/usr/lib/libproc.dylib')
    _proc.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
    _proc.proc_pid_rusage.restype = ctypes.c_int


def physical_footprint(pid):
    """macOS RUSAGE_INFO_V0 physical charge, including compressed pages.

    Unlike summed RSS, this excludes clean shared framework/zero pages. The
    two measurements stay separate; a footprint pass is not an RSS pass.
    """
    if sys.platform != 'darwin':
        return None
    # sys/resource.h: 16-byte UUID, then ten u64 fields; footprint is field 8.
    usage = ctypes.create_string_buffer(96)
    if _proc.proc_pid_rusage(pid, 0, usage):
        return None
    return int.from_bytes(usage.raw[72:80], 'little')


async def memory_samples(root, result, stop, baseline, webkit_directory):
    while not stop.is_set():
        rows = await processes()
        children = {root}
        while True:
            expanded = children | {pid for pid, parent, _, _ in rows if parent in children}
            if expanded == children:
                break
            children = expanded
        if webkit_directory:
            children.update(pid for pid,_,_,command in rows
                            if pid not in baseline and command.startswith(webkit_directory))
        selected = [(pid,rss,Path(command).name) for pid,_,rss,command in rows if pid in children]
        total = sum(rss for _,rss,_ in selected)
        content = sum(rss for _,rss,name in selected if 'WebContent' in name or 'Renderer' in name)
        renderer = max((rss for _,rss,name in selected if 'WebContent' in name or 'Renderer' in name), default=0)
        result['peak_renderer_rss_bytes'] = max(result.get('peak_renderer_rss_bytes', 0), renderer)
        memory_without_driver = sum(rss for pid,rss,_ in selected if pid != root)
        result['peak_content_rss_bytes'] = max(result.get('peak_content_rss_bytes', 0), content)
        result['peak_browser_rss_bytes'] = max(result.get('peak_browser_rss_bytes', 0), memory_without_driver)
        if total > result.get('peak_rss_bytes',0):
            result.update(peak_rss_bytes=total,peak_processes=selected)
        footprints = [(pid, physical_footprint(pid), name) for pid, _, name in selected if pid != root]
        if footprints and all(value is not None for _,value,_ in footprints):
            charge = sum(value for _,value,_ in footprints)
            if charge > result.get('peak_browser_physical_footprint_bytes', 0):
                result.update(peak_browser_physical_footprint_bytes=charge,
                              footprint_processes=footprints)
        result['memory_samples'] = result.get('memory_samples', 0) + 1
        await asyncio.sleep(.1)
