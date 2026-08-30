#!/usr/bin/env python3
import os
import sys
import json
import time
import subprocess
import statistics
from pathlib import Path

WARMUPS = 3
ITERATIONS = 10

def format_ms(ms):
    return f"{ms:.3f} ms"

def run_command(cmd, cwd, capture_output=False):
    t0 = time.perf_counter()
    if capture_output:
        p = subprocess.Popen(cmd, shell=True, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    else:
        # Actually capture stderr always so we can debug failures
        p = subprocess.Popen(cmd, shell=True, cwd=cwd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    
    _, status, rusage = os.wait4(p.pid, 0)
    t1 = time.perf_counter()
    
    # decode stderr if needed
    err = p.stderr.read() if p.stderr else ""
    returncode = os.waitstatus_to_exitcode(status) if hasattr(os, "waitstatus_to_exitcode") else os.WEXITSTATUS(status)
    
    ms = (t1 - t0) * 1000
    mem_kb = rusage.ru_maxrss
    
    cpu_time = rusage.ru_utime + rusage.ru_stime
    real_time = t1 - t0
    cpu_percent = (cpu_time / real_time) * 100 if real_time > 0 else 0
    
    return ms, mem_kb, cpu_percent, returncode, err

def main():
    benchmarks_dir = Path("benchmarks")
    if not benchmarks_dir.exists():
        print("Error: benchmarks directory not found.")
        sys.exit(1)
        
    print(f"Pace Benchmarking Suite")
    print(f"Warmup runs: {WARMUPS} | Measurement runs: {ITERATIONS}\n")

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
        
        print(f"\n--- {name.upper()} ---")
        results[name] = {}
        
        for lang, target in targets.items():
            print(f"> {lang.capitalize()}")
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
                    print(f"  [ERROR] Build failed:\n{p.stderr}")
                    continue
                compile_ms = (t1 - t0) * 1000
            
            # Warmups
            for _ in range(WARMUPS):
                run_command(run_cmd, cwd=d)
                
            # Measurements
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
                    print(f"  [ERROR] Run failed:\n{err}")
                    break
            
            if not exec_times:
                print(f"  [ERROR] Run failed")
                continue
                
            median_ms = statistics.median(exec_times)
            mean_ms = statistics.mean(exec_times)
            stdev_ms = statistics.stdev(exec_times) if len(exec_times) > 1 else 0
            
            median_rss = statistics.median(rss_mems)
            median_cpu = statistics.median(cpu_percents)
            
            results[name][lang] = {
                "compile_ms": compile_ms,
                "exec_median": median_ms,
                "exec_mean": mean_ms,
                "exec_stdev": stdev_ms,
                "mem_kb": median_rss,
                "cpu_percent": median_cpu
            }
            
            print(f"  Compile: {format_ms(compile_ms) if compile_ms else 'N/A'}")
            print(f"  Exec Median: {format_ms(median_ms)} (±{format_ms(stdev_ms)})")
            print(f"  Mem Peak: {median_rss / 1024:.2f} MB | CPU: {median_cpu:.0f}%")

if __name__ == "__main__":
    main()
