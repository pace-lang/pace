#!/usr/bin/env python3
import os
import sys
import json
import time
import subprocess
import statistics
import platform
from pathlib import Path

WARMUPS = 3
ITERATIONS = 10

def format_ms(ms):
    return f"{ms:.3f} ms"

def get_system_info():
    try:
        with open('/proc/cpuinfo', 'r') as f:
            for line in f:
                if 'model name' in line:
                    cpu = line.split(':')[1].strip()
                    break
    except:
        cpu = platform.processor()
    
    try:
        with open('/proc/meminfo', 'r') as f:
            for line in f:
                if 'MemTotal' in line:
                    ram_kb = int(line.split()[1])
                    ram_gb = round(ram_kb / (1024 * 1024))
                    break
    except:
        ram_gb = "Unknown"
        
    os_name = f"{platform.system()} {platform.machine()}"
    return f"{cpu}, {ram_gb}GiB RAM, {os_name}"

def get_tool_versions():
    versions = {}
    cmds = {
        "rust": "rustc --version",
        "zig": "zig version",
        "go": "go version",
        "java": "javac --version",
        "dart": "dart --version",
        "python": "python3 --version",
        "pace": "../target/release/pace --version"
    }
    
    for lang, cmd in cmds.items():
        try:
            p = subprocess.run(cmd, shell=True, capture_output=True, text=True)
            out = p.stdout.strip() if p.stdout else p.stderr.strip()
            # grab first line
            versions[lang] = out.split('\n')[0]
        except:
            versions[lang] = "Unknown"
            
    return versions

def run_command(cmd, cwd, capture_output=False):
    import shlex
    cmd_args = shlex.split(cmd)
    t0 = time.perf_counter()
    if capture_output:
        p = subprocess.Popen(cmd_args, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    else:
        p = subprocess.Popen(cmd_args, cwd=cwd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    
    _, status, rusage = os.wait4(p.pid, 0)
    t1 = time.perf_counter()
    
    err = p.stderr.read() if p.stderr else ""
    returncode = os.waitstatus_to_exitcode(status) if hasattr(os, "waitstatus_to_exitcode") else os.WEXITSTATUS(status)
    
    ms = (t1 - t0) * 1000
    mem_kb = rusage.ru_maxrss
    
    cpu_time = rusage.ru_utime + rusage.ru_stime
    real_time = t1 - t0
    cpu_percent = (cpu_time / real_time) * 100 if real_time > 0 else 0
    
    return ms, mem_kb, cpu_percent, returncode, err

def color_time(ms, all_ms):
    sorted_ms = sorted(all_ms)
    if len(sorted_ms) < 2:
        return f"$\\color{{#ca8a04}}{{\\text{{{ms:.3f} ms}}}}$"
    if ms <= sorted_ms[1]: # Top 2
        return f"$\\color{{#16a34a}}{{\\text{{{ms:.3f} ms}}}}$"
    elif ms >= sorted_ms[-2]: # Bottom 2
        return f"$\\color{{#dc2626}}{{\\text{{{ms:.3f} ms}}}}$"
    else:
        return f"$\\color{{#ca8a04}}{{\\text{{{ms:.3f} ms}}}}$"
        
def get_color_code(ms, all_ms, is_pace):
    sorted_ms = sorted(all_ms)
    color = "#ca8a04" # default average
    if len(sorted_ms) >= 2:
        if ms <= sorted_ms[1]:
            color = "#16a34a"
        elif ms >= sorted_ms[-2]:
            color = "#dc2626"
        
    val_str = f"{ms:.3f} ms"
    if is_pace:
        return f"$\\color{{{color}}}{{\\mathbf{{{val_str}}}}}$"
    else:
        return f"$\\color{{{color}}}{{\\text{{{val_str}}}}}$"

def main():
    benchmarks_dir = Path("benchmarks")
    if not benchmarks_dir.exists():
        benchmarks_dir = Path(".")
        
    print(f"Pace Benchmarking Suite")
    print(f"Warmup runs: {WARMUPS} | Measurement runs: {ITERATIONS}\n")

    sys_info = get_system_info()
    print(f"System: {sys_info}")
    
    versions = get_tool_versions()
    print("\nTool Versions:")
    for lang, ver in versions.items():
        print(f"  {lang.capitalize()}: {ver}")
    print()

    results = {}

    for d in sorted(benchmarks_dir.iterdir()):
        if not d.is_dir():
            continue
            
        manifest_path = d / "manifest.json"
        if not manifest_path.exists():
            continue
            
        with open(manifest_path, 'r') as f:
            manifest = json.load(f)
            
        name = manifest.get("name", d.name)
        targets = manifest.get("targets", {})
        
        results[name] = {}
        
        for lang, target in targets.items():
            build_cmd = target.get("build")
            run_cmd = target.get("run")
            
            build_dir = d / "build"
            build_dir.mkdir(exist_ok=True)
            
            compile_ms = None
            if build_cmd:
                t0 = time.perf_counter()
                p = subprocess.run(build_cmd, shell=True, cwd=d, capture_output=True, text=True)
                t1 = time.perf_counter()
                if p.returncode != 0:
                    continue
                compile_ms = (t1 - t0) * 1000
            
            for _ in range(WARMUPS):
                run_command(run_cmd, cwd=d)
                
            exec_times = []
            rss_mems = []
            cpu_percents = []
            
            for _ in range(ITERATIONS):
                ms, mem, cpu, code, err = run_command(run_cmd, cwd=d)
                if code == 0:
                    exec_times.append(ms)
                    rss_mems.append(mem)
                    cpu_percents.append(cpu)
                else:
                    break
            
            if not exec_times:
                continue
                
            median_ms = statistics.median(exec_times)
            median_rss = statistics.median(rss_mems)
            median_cpu = statistics.median(cpu_percents)
            
            results[name][lang] = {
                "compile_ms": compile_ms,
                "exec_median": median_ms,
                "mem_kb": median_rss,
                "cpu_percent": median_cpu
            }

    print("\n" + "="*50)
    print("MARKDOWN OUTPUT FOR README.md")
    print("="*50 + "\n")
    
    print(f"*Tested on **{sys_info.split(',')[0]}**, **{sys_info.split(',')[1].strip()}**, **{sys_info.split(',')[2].strip()}**.*\n")
    
    for name, langs in results.items():
        print(f"### {name.upper()}\n")
        print("| Language | Execution Time (Median) | Peak Memory | CPU Usage | Compile Time |")
        print("| :--- | :--- | :--- | :--- | :--- |")
        
        # sort by execution time
        sorted_langs = sorted(langs.items(), key=lambda x: x[1]['exec_median'])
        all_ms = [x[1]['exec_median'] for x in sorted_langs]
        
        for lang, data in sorted_langs:
            lang_name = "**Pace**" if lang == "pace" else lang.capitalize()
            
            exec_str = get_color_code(data['exec_median'], all_ms, lang == "pace")
            mem_str = f"{data['mem_kb'] / 1024:.2f} MB"
            cpu_str = f"{data['cpu_percent']:.0f}%"
            comp_str = f"{data['compile_ms']:.3f} ms" if data['compile_ms'] is not None else "N/A"
            
            print(f"| {lang_name} | {exec_str} | {mem_str} | {cpu_str} | {comp_str} |")
        print("\n")

if __name__ == "__main__":
    main()
