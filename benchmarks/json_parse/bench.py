import json
import time

source = '{"user":{"id":42,"name":"Aniket","active":true,"balance":1250.75,"email":null,"roles":["developer","maintainer"],"profile":{"age":22,"verified":true,"skills":[{"name":"Rust","level":4},{"name":"Dart","level":5}]}},"projects":[{"name":"Pace","version":0.3,"open_source":true},{"name":"Hadron","version":1.0,"open_source":false}]}'

start = time.time()
for _ in range(10000):
    json.loads(source)
end = time.time()

print(f"Parsed 10000 times in {(end - start) * 1000:.2f} ms")
