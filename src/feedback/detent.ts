/** Optional, gesture-initiated feedback. Never unlock audio with a media element:
 * WebKit's ambient Web Audio session intentionally obeys the Ring/Silent switch.
 * https://bugs.webkit.org/show_bug.cgi?id=252746
 */
export class DetentFeedback {
  private context?: AudioContext;

  beginGesture(event: Event) {
    if (!event.isTrusted) return;
    try {
      if (navigator.vibrate?.(0)) return;
      // Transient maps to iOS's ambient session. Older WebKit already defaults
      // Web Audio to ambient; leave that default alone when AudioSession is absent.
      const session = (navigator as Navigator & { audioSession?: { type: string } }).audioSession;
      if (session) session.type = 'transient';
      this.context ??= new AudioContext();
      if (this.context.state === 'suspended') void this.context.resume().catch(() => {});
    } catch { /* Feedback must never prevent a camera input. */ }
  }

  step() {
    try {
      if (navigator.vibrate?.(8)) return;
    } catch { /* Unsupported vibration falls through to an already unlocked context. */ }
    const context = this.context;
    if (!context || context.state !== 'running' || document.hidden) return;
    const start = context.currentTime;
    const oscillator = context.createOscillator(), gain = context.createGain();
    oscillator.type = 'triangle';
    oscillator.frequency.setValueAtTime(1700, start);
    oscillator.frequency.exponentialRampToValueAtTime(500, start + .012);
    gain.gain.setValueAtTime(.045, start);
    gain.gain.exponentialRampToValueAtTime(.0001, start + .016);
    oscillator.connect(gain); gain.connect(context.destination);
    oscillator.onended = () => { oscillator.disconnect(); gain.disconnect(); };
    oscillator.start(start); oscillator.stop(start + .018);
  }
}
