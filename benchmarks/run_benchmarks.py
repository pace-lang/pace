import os
import subprocess
import time
import re
import sys

BENCHMARKS_DIR = os.path.dirname(os.path.abspath(__file__))
TMP_DIR = "/tmp/pace_benchmarks"

LANGUAGES = {
    "Pace": {"ext": ".pace", "compile": ["cargo", "run", "--manifest-path", os.path.join(BENCHMARKS_DIR, "../Cargo.toml"), "--release", "--", "build", "{src}", "--release"], "run": ["{dir}/{basename}"], "needs_dir": False},
    "Rust": {"ext": ".rs", "compile": ["rustc", "-O", "{src}", "-o", "{out}"], "run": ["{out}"], "needs_dir": False},
    "Go": {"ext": ".go", "compile": ["go", "build", "-o", "{out}", "{src}"], "run": ["{out}"], "needs_dir": False},
    "Zig": {"ext": ".zig", "compile": ["zig", "build-exe", "-O", "ReleaseFast", "{src}", "-femit-bin={out}"], "run": ["{out}"], "needs_dir": False},
    "Java": {"ext": ".java", "compile": ["javac", "-d", "{dir}", "{src}"], "run": ["java", "-cp", "{dir}", "{basename}"], "needs_dir": False},
    "Python": {"ext": ".py", "compile": None, "run": ["python3", "{src}"], "needs_dir": False},
    "Node.js": {"ext": ".js", "compile": None, "run": ["node", "{src}"], "needs_dir": False},
    "Dart": {"ext": ".dart", "compile": ["dart", "compile", "exe", "{src}", "-o", "{out}"], "run": ["{out}"], "needs_dir": False},
}

def parse_time_output(stderr):
    time_match = re.search(r"User time \(seconds\):\s+([\d\.]+)", stderr)
    mem_match = re.search(r"Maximum resident set size \(kbytes\):\s+(\d+)", stderr)
    user_time = float(time_match.group(1)) if time_match else 0.0
    mem_mb = float(mem_match.group(1)) / 1024.0 if mem_match else 0.0
    return user_time, mem_mb

def run_normal_benchmark(scenario_dir):
    print(f"\nRunning Scenario: {os.path.basename(scenario_dir)}")
    print("-" * 50)
    
    os.makedirs(TMP_DIR, exist_ok=True)
    results = []

    for lang, config in LANGUAGES.items():
        src_file = None
        for f in os.listdir(scenario_dir):
            if f.endswith(config["ext"]):
                src_file = os.path.join(scenario_dir, f)
                break
        if not src_file:
            continue
            
        basename = os.path.splitext(os.path.basename(src_file))[0]
        out_bin = os.path.join(TMP_DIR, basename + "_" + lang.lower())
        
        if config["compile"]:
            compile_cmd = [part.format(src=src_file, out=out_bin, dir=TMP_DIR, basename=basename) for part in config["compile"]]
            if lang == "Pace":
                out_bin = os.path.join(scenario_dir, "target/release", basename)
            
            try:
                subprocess.run(compile_cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            except subprocess.CalledProcessError as e:
                print(f"Failed to compile {lang}: {e}")
                continue

        run_cmd = [part.format(src=src_file, out=out_bin, dir=TMP_DIR, basename=basename) for part in config["run"]]
        if lang == "Pace":
            run_cmd = [out_bin]
            
        time_cmd = ["/usr/bin/time", "-v"] + run_cmd
        
        try:
            subprocess.run(run_cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            res = subprocess.run(time_cmd, capture_output=True, text=True)
            user_time, mem_mb = parse_time_output(res.stderr)
            results.append((lang, user_time, mem_mb))
            print(f"{lang:10} | Time: {user_time:.3f}s | Peak Mem: {mem_mb:.2f} MB")
        except Exception as e:
            print(f"Failed to run {lang}: {e}")
    return results

def run_http_benchmark(scenario_dir):
    print(f"\nRunning HTTP Server Benchmark (Single & Multi-user)")
    print("-" * 50)
    
    os.makedirs(TMP_DIR, exist_ok=True)
    load_test_script = os.path.join(scenario_dir, "load_test.py")

    for lang, config in LANGUAGES.items():
        src_file = None
        for f in os.listdir(scenario_dir):
            if f.startswith("server") and f.endswith(config["ext"]):
                src_file = os.path.join(scenario_dir, f)
                break
        if not src_file:
            continue
            
        basename = os.path.splitext(os.path.basename(src_file))[0]
        out_bin = os.path.join(TMP_DIR, basename + "_" + lang.lower())
        
        if config["compile"]:
            compile_cmd = [part.format(src=src_file, out=out_bin, dir=TMP_DIR, basename=basename) for part in config["compile"]]
            if lang == "Pace":
                out_bin = os.path.join(scenario_dir, "target/release", basename)
            
            try:
                subprocess.run(compile_cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            except subprocess.CalledProcessError as e:
                continue

        run_cmd = [part.format(src=src_file, out=out_bin, dir=TMP_DIR, basename=basename) for part in config["run"]]
        if lang == "Pace":
            run_cmd = [out_bin]
            
        port = 3000
        
        try:
            # Start server
            server_proc = subprocess.Popen(run_cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            time.sleep(2) # Wait for server to bind
            
            # Run single load test
            res_single = subprocess.run(["python3", load_test_script, str(port), "single"], capture_output=True, text=True)
            rps_single_match = re.search(r"RPS\):\s+([\d\.]+)", res_single.stdout)
            rps_single = float(rps_single_match.group(1)) if rps_single_match else 0.0
            
            # Run multi load test
            res_multi = subprocess.run(["python3", load_test_script, str(port), "multi"], capture_output=True, text=True)
            rps_multi_match = re.search(r"RPS\):\s+([\d\.]+)", res_multi.stdout)
            rps_multi = float(rps_multi_match.group(1)) if rps_multi_match else 0.0
            
            server_proc.terminate()
            server_proc.wait()
            
            print(f"{lang:10} | Single-User RPS: {rps_single:.2f} | Multi-User RPS: {rps_multi:.2f}")
        except Exception as e:
            print(f"Failed to run {lang} HTTP server: {e}")

if __name__ == "__main__":
    if not os.path.exists("/usr/bin/time"):
        print("Error: /usr/bin/time not found.")
        sys.exit(1)
        
    scenarios = [d for d in os.listdir(BENCHMARKS_DIR) if os.path.isdir(os.path.join(BENCHMARKS_DIR, d)) and d not in ("target", ".idea")]
    scenarios.sort()
    
    for s in scenarios:
        path = os.path.join(BENCHMARKS_DIR, s)
        if s == "http_server":
            run_http_benchmark(path)
        else:
            run_normal_benchmark(path)
