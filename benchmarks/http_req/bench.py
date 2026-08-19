import urllib.request
import time

def main():
    url = "https://jsonplaceholder.typicode.com/todos/1"
    iterations = 10
    success_count = 0
    
    start_time = time.time()
    
    for _ in range(iterations):
        with urllib.request.urlopen(url) as response:
            if response.status == 200:
                success_count += 1
                body = response.read()
                
    end_time = time.time()
    print(f"Python: {success_count}/{iterations} successful requests in {end_time - start_time:.2f}s")

if __name__ == "__main__":
    main()
