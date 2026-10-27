export function PersonalNote() {
  return (
    <aside class="personal-note" aria-label="A note from Jake">
      <p>Hey,</p>
      <p>
        FujiMod is a 23-day-old vibe-coded firmware modification made to unlock the full potential
        of every Fujifilm camera as a creative tool. We’re starting with the X70 and have concepts
        of a master plan to bring FujiMod to the rest of the EXR II bodies (X-T1, X-E2/X-E2S, …) and
        beyond.
      </p>
      <p>
        I haven't tested every feature on camera yet (there are so many!), but most tests pass in
        the emulator, and I feel it's safe for folks to start testing FujiMod by dual booting from
        the SD card. <a href="#software">Head on down</a> to try it out. Even if you don't have an
        X70, you can run FujiMod in the emulator on this website, although it's a bit slow. (Enable
        webcam access and use your imagination 😂.)
      </p>
      <p class="signature">
        Yours truly,
        <br />
        <span class="signature-name">Jake</span>
      </p>
    </aside>
  );
}
