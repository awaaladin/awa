import { useEffect, useRef, useState } from 'react';
import { useTheme } from '../theme/ThemeProvider';

type Props = {
  /** Called once: when the video ends, the user skips, or playback fails. */
  onDone: () => void;
};

const STALL_MS = 1500; // if the video has not started by then, do not make anyone wait
const MAX_MS = 7000; // hard cap, longer than the 6 s video

/**
 * AWA's boot animation: the owner's 6 s video. Skippable with any key or
 * click, and never blocks readiness: a missing, slow or broken video skips
 * itself. Low-memory mode uses the 197 KB 480p encode; reduced motion shows
 * the final frame briefly instead of playing.
 */
export function BootSequence({ onDone }: Props) {
  const { lowMemory, reducedMotion } = useTheme();
  const video = useRef<HTMLVideoElement>(null);
  const [leaving, setLeaving] = useState(false);
  const done = useRef(false);

  const finish = () => {
    if (done.current) return;
    done.current = true;
    setLeaving(true);
    window.setTimeout(onDone, 160);
  };

  useEffect(() => {
    if (reducedMotion) {
      const t = window.setTimeout(finish, 900);
      return () => window.clearTimeout(t);
    }
    const stall = window.setTimeout(() => {
      if (!video.current || video.current.currentTime === 0) finish();
    }, STALL_MS);
    const cap = window.setTimeout(finish, MAX_MS);
    const skip = (e: KeyboardEvent) => {
      if (!e.repeat) finish();
    };
    window.addEventListener('keydown', skip);
    return () => {
      window.clearTimeout(stall);
      window.clearTimeout(cap);
      window.removeEventListener('keydown', skip);
    };
  }, []);

  return (
    <div className={`boot ${leaving ? 'is-leaving' : ''}`} onClick={finish} role="presentation">
      {reducedMotion ? (
        <img src="/boot/awa-boot-last.jpg" alt="AWA" />
      ) : (
        <video
          ref={video}
          src={lowMemory ? '/boot/awa-boot-480p.mp4' : '/boot/awa-boot-720p.mp4'}
          poster="/boot/awa-boot-last.jpg"
          muted
          playsInline
          autoPlay
          preload="auto"
          onEnded={finish}
          onError={finish}
          aria-label="AWA is starting"
        />
      )}
      <span className="boot-skip">Press any key to skip</span>
    </div>
  );
}
