import time
import urllib.request
import sys
from concurrent.futures import ThreadPoolExecutor

port = int(sys.argv[1]) if len(sys.argv) > 1 else 3000
url = f"http://127.0.0.1:{port}/json"

def fetch(_):
    try:
        response = urllib.request.urlopen(url)
        response.read()
        return True
    except Exception as e:
        return False

def run_benchmark(mode):
    total_requests = 2000 if mode == "multi" else 500
    threads = 20 if mode == "multi" else 1
    
    print(f"Sending {total_requests} requests with {threads} threads ({mode} mode)...")
    
    start_time = time.time()
    
    with ThreadPoolExecutor(max_workers=threads) as executor:
        results = list(executor.map(fetch, range(total_requests)))
        
    end_time = time.time()
    
    successful = sum(1 for r in results if r)
    duration = end_time - start_time
    rps = successful / duration if duration > 0 else 0
    
    print(f"Time taken: {duration:.2f}s")
    print(f"Successful requests: {successful}")
    print(f"Requests per second (RPS): {rps:.2f}")

if __name__ == "__main__":
    mode = sys.argv[2] if len(sys.argv) > 2 else "multi"
    run_benchmark(mode)
