// Runs one request of the quantization pipeline off the main thread.
// The page posts a postcard-encoded request; the module posts its replies
// back through `worker_handle`.
import init, { worker_handle } from "./qualetize_gui.js";

const ready = init();
self.onmessage = async (event) => {
  await ready;
  worker_handle(new Uint8Array(event.data));
};
