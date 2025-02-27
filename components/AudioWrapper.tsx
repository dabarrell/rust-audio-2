'use client';

import dynamic from "next/dynamic";

// Use dynamic import with no SSR for the AudioControls component
// This is necessary because it uses browser-only APIs
const AudioControls = dynamic(() => import("@/components/AudioControls"), {
  ssr: false,
});

export default function AudioWrapper() {
  return <AudioControls />;
}
