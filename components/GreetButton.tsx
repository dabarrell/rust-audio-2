'use client';

import { greet } from "@/crate/pkg";

function GreetButton() {
  return <button onClick={() => greet()}>Greet</button>;
}

export default GreetButton;
