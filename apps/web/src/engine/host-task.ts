/** One worker task queue; idle ports do not retain message listeners. */
export class HostTasks {
  private channel?: MessageChannel;
  private serial = 0;
  private pending = new Map<number, () => void>();
  private receive = (event: MessageEvent<number>) => {
    const operation = this.pending.get(event.data);
    this.pending.delete(event.data);
    if (!this.pending.size) this.channel!.port1.onmessage = null;
    operation?.();
  };
  schedule(operation: () => void, delay = 0): () => void {
    if (delay > 0) {
      const timer = setTimeout(operation, delay);
      return () => clearTimeout(timer);
    }
    const channel = (this.channel ??= new MessageChannel());
    const serial = ++this.serial;
    this.pending.set(serial, operation);
    channel.port1.onmessage = this.receive;
    channel.port2.postMessage(serial);
    return () => {
      this.pending.delete(serial);
      if (!this.pending.size) channel.port1.onmessage = null;
    };
  }
}
