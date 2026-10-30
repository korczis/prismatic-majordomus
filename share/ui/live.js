// A screen that says it follows a live channel (`data-follow`, ADR 0176) is read again when the
// server says what it shows changed, and says how it is fed. The channel is the one socket
// client's (`socket.js`, the same file the Cockpit runs); the rendering stays the server's:
// a change frame asks `ui.js` to read the screen again, and nothing here draws a row.
//
// The line a screen marks `data-follow-state` says one of three things, never more than is
// true: live while the socket is open, reconnecting while it is not, and reading on a timer
// once it has given up.

import { follow, LIVE, RECONNECTING } from './socket.js';

/** What the state line says, by the state of the channel. */
export const SAID = {
  connecting: 'Opening the live channel; until it is open this page is as it was served.',
  live: 'Live: the server says when the blackboard changes and this page is read again.',
  reconnecting: 'Reconnecting to the live channel; this page is as it was last read.',
  offline: 'The live channel is closed: this page is read again every minute instead.',
};

/**
 * Follow `path` for a screen.
 *
 * @param {string} path the channel the server rendered on the screen
 * @param {{ refresh(): void, say(text: string): void }} screen what reads the screen again,
 *   and what writes its state line
 * @returns {{ live(): boolean, said(): string, close(): void }}
 */
export function followScreen(path, screen) {
  let word = 'connecting';
  const tell = (next) => {
    word = next;
    screen.say(SAID[next]);
  };
  tell('connecting');
  const channel = follow(path, {
    message(frame) {
      // Every snapshot reads the screen again, a connection's first one too: the blackboard
      // may have changed between the page being rendered and the channel opening, and the
      // server does not send a snapshot that says nothing new, so a change in that gap would
      // otherwise never reach a page that says it is live. One read per connection is the
      // price; after a reconnection it is also what brings in what changed while it was down.
      if (frame.type !== 'blackboard.snapshot' || !frame.data) return;
      screen.refresh();
    },
    state(name) {
      if (name === LIVE) tell('live');
      else if (name === RECONNECTING) tell('reconnecting');
      else tell('offline');
    },
  });
  return {
    live: () => channel.state() === LIVE,
    said: () => SAID[word],
    close: () => channel.close(),
  };
}
