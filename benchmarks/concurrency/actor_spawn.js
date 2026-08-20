import { Worker, isMainThread, parentPort, workerData } from 'worker_threads';

if (isMainThread) {
    let active = 0;
    for (let i = 0; i < 1000; i++) { // Using 1000 because Node limits worker count severely
        active++;
        const worker = new Worker(__filename, { workerData: i });
        worker.on('exit', () => {
            active--;
            if (active === 0) {
                console.log("Spawned 1000 actors.");
            }
        });
    }
} else {
    // do nothing
}
