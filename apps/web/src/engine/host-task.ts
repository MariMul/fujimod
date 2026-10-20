/** Yield a task turn without accumulating the browser's nested-timer clamp. */
export function scheduleHostTask(operation: () => void, delay = 0): () => void {
  if (delay > 0) {
    const timer = setTimeout(operation, delay);
    return () => clearTimeout(timer);
  }
  const channel = new MessageChannel();
  const cancel = () => {
    channel.port1.close();
    channel.port2.close();
  };
  channel.port1.onmessage = () => {
    cancel();
    operation();
  };
  channel.port2.postMessage(null);
  return cancel;
}
