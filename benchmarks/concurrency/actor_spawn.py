import threading

def worker(id):
    pass

threads = []
for i in range(10000):
    t = threading.Thread(target=worker, args=(i,))
    threads.append(t)
    t.start()

for t in threads:
    t.join()

print("Spawned 10000 actors.")
